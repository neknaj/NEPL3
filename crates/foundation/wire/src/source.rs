//! Typed foundation source adapters. Source digests and span geometry are checked
//! after structural decoding; a record labelled SourceContent is never trusted.
use crate::{WireError, decode_checked, encode_checked};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{SchemaRegistry, TypeDescriptor, TypeRef},
    source::{
        Digest, SourceAdmission, SourceError, SourceId, SourceRef, SourceSnapshot, SourceStore,
        Span,
    },
    value::{NdfValue, Record, SchemaRef},
};

pub(crate) fn expected(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.foundation".into(),
        revision: 1,
        name: name.into(),
    })
}
pub(crate) fn text(value: &str, budget: &mut Budget) -> Result<NdfValue, WireError> {
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(NdfValue::Text(value.into()))
}
pub(crate) fn record<const N: usize>(
    schema: &SchemaRef,
    kind: &str,
    fields: [NdfValue; N],
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    budget.charge(
        Resource::AllocationUnits,
        (schema.package.len() + kind.len()) as u64
            + (fields.len() * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    Ok(NdfValue::Record(Record {
        schema: schema.clone(),
        kind: kind.into(),
        fields: Vec::from(fields),
    }))
}
fn source_ref(
    reference: &SourceRef,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    budget.charge(Resource::AllocationUnits, 32)?;
    record(
        schema,
        "SourceRef",
        [
            text(&reference.source_id.0, budget)?,
            NdfValue::U64(reference.revision),
            NdfValue::Bytes(reference.digest.0.to_vec()),
        ],
        budget,
    )
}
pub(crate) fn fields<'a>(
    value: &'a NdfValue,
    schema: &SchemaRef,
    name: &str,
    length: usize,
) -> Result<&'a [NdfValue], WireError> {
    match value {
        NdfValue::Record(record)
            if &record.schema == schema && record.kind == name && record.fields.len() == length =>
        {
            Ok(&record.fields)
        }
        _ => Err(WireError::InvalidType),
    }
}
pub(crate) fn as_text(value: &NdfValue) -> Result<&str, WireError> {
    match value {
        NdfValue::Text(value) => Ok(value),
        _ => Err(WireError::InvalidType),
    }
}
pub(crate) fn as_u64(value: &NdfValue) -> Result<u64, WireError> {
    match value {
        NdfValue::U64(value) => Ok(*value),
        _ => Err(WireError::InvalidType),
    }
}
pub(crate) fn as_digest(value: &NdfValue) -> Result<Digest, WireError> {
    match value {
        NdfValue::Bytes(value) => Ok(Digest(
            value
                .as_slice()
                .try_into()
                .map_err(|_| WireError::InvalidLength)?,
        )),
        _ => Err(WireError::InvalidType),
    }
}
fn reference(
    value: &NdfValue,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<SourceRef, WireError> {
    let fields = fields(value, schema, "SourceRef", 3)?;
    let id = as_text(&fields[0])?;
    budget.charge(Resource::AllocationUnits, id.len() as u64)?;
    Ok(SourceRef {
        source_id: SourceId(id.into()),
        revision: as_u64(&fields[1])?,
        digest: as_digest(&fields[2])?,
    })
}

/// Sources are ordered canonically by stable ID/revision/digest. The shared
/// admission set prevents repeated references from resetting or double charging inputs.
pub fn encode_sources(
    sources: &[SourceSnapshot],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let entries = sources_value(sources, schema, admission, budget)?;
    let value = record(schema, "SourceBundle", [entries], budget)?;
    encode_checked(&value, &expected("SourceBundle"), registry, budget)
}

pub(crate) fn sources_value(
    sources: &[SourceSnapshot],
    schema: &SchemaRef,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    let mut sorted = Vec::new();
    for source in sources {
        admission.admit_existing(source, budget)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(SourceRef, &SourceSnapshot)>() as u64
                + source.identity().source.0.len() as u64,
        )?;
        sorted.push((source.reference(), source));
    }
    budget.charge(
        Resource::Work,
        (sorted.len() as u64).saturating_mul(sorted.len() as u64),
    )?;
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    if sorted.windows(2).any(|pair| {
        pair[0].0.source_id == pair[1].0.source_id && pair[0].0.revision == pair[1].0.revision
    }) {
        return Err(SourceError::IdentityConflict.into());
    }
    let mut entries = Vec::new();
    for (reference, source) in sorted {
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<NdfValue>() as u64,
        )?;
        entries.push(record(
            schema,
            "SourceContent",
            [
                source_ref(&reference, schema, budget)?,
                text(source.uri(), budget)?,
                text(source.text(), budget)?,
            ],
            budget,
        )?);
    }
    Ok(NdfValue::List(entries))
}

/// Reconstruct checked snapshots, rejecting forged digests, duplicate/reordered
/// declarations and locator conflicts. Callers share admission through nested calls.
pub fn decode_sources(
    bytes: &[u8],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<SourceSnapshot>, WireError> {
    let value = decode_checked(bytes, &expected("SourceBundle"), registry, budget)?;
    let root = fields(value.value(), schema, "SourceBundle", 1)?;
    sources_from(&root[0], schema, admission, budget)
}

pub(crate) fn sources_from(
    value: &NdfValue,
    schema: &SchemaRef,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<SourceSnapshot>, WireError> {
    let NdfValue::List(entries) = value else {
        return Err(WireError::InvalidType);
    };
    let mut sources = Vec::new();
    let mut previous: Option<SourceRef> = None;
    for entry in entries {
        let fields = fields(entry, schema, "SourceContent", 3)?;
        let reference = reference(&fields[0], schema, budget)?;
        if previous.as_ref().is_some_and(|prior| {
            prior >= &reference
                || (prior.source_id == reference.source_id && prior.revision == reference.revision)
        }) {
            return Err(SourceError::IdentityConflict.into());
        }
        let uri = as_text(&fields[1])?;
        let content = as_text(&fields[2])?;
        budget.charge(Resource::Work, content.len() as u64)?;
        if Digest::of(content.as_bytes()) != reference.digest {
            return Err(SourceError::ExpectedDigest.into());
        }
        budget.charge(
            Resource::AllocationUnits,
            (uri.len() + content.len() + reference.source_id.0.len()) as u64
                + core::mem::size_of::<SourceSnapshot>() as u64,
        )?;
        let source = admission.import(
            reference.source_id.clone(),
            reference.revision,
            String::from(uri),
            content.as_bytes().to_vec(),
            budget,
        )?;
        previous = Some(reference);
        sources.push(source);
    }
    Ok(sources)
}

pub fn encode_span(
    span: &Span,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let value = span_value(span, schema, budget)?;
    encode_checked(&value, &expected("Span"), registry, budget)
}

pub(crate) fn span_value(
    span: &Span,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    let id = span.snapshot_ref();
    budget.charge(Resource::AllocationUnits, id.source.0.len() as u64)?;
    let reference = SourceRef {
        source_id: id.source.clone(),
        revision: id.revision,
        digest: id.digest,
    };
    record(
        schema,
        "Span",
        [
            source_ref(&reference, schema, budget)?,
            NdfValue::U64(span.start()),
            NdfValue::U64(span.end()),
        ],
        budget,
    )
}

pub fn decode_span(
    bytes: &[u8],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    budget: &mut Budget,
) -> Result<Span, WireError> {
    let value = decode_checked(bytes, &expected("Span"), registry, budget)?;
    span_from_value(value.value(), schema, sources, budget)
}

pub(crate) fn span_from_value(
    value: &NdfValue,
    schema: &SchemaRef,
    sources: &SourceStore,
    budget: &mut Budget,
) -> Result<Span, WireError> {
    let fields = fields(value, schema, "Span", 3)?;
    let reference = reference(&fields[0], schema, budget)?;
    let source = sources
        .resolve(&reference)
        .ok_or(SourceError::MissingSnapshot)?;
    budget.charge(
        Resource::AllocationUnits,
        reference.source_id.0.len() as u64,
    )?;
    Ok(source.span(as_u64(&fields[1])?, as_u64(&fields[2])?)?)
}
