//! Source-referenced sentence payloads. The receiving owner supplies the exact
//! source explicitly; the codec's ambient source store cannot complete it.
use super::*;
use crate::model::{DocKind, DocRoot, DocView};
use alloc::vec;
use nepl3_core::{origin::Origin, source::SourceSnapshot};

fn checked<C: FoundationValueCodec>(
    doc: &DocumentSyntax,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    doc.validate_structure(registry, b, codec.source_admission())?;
    let ([_], [view]) = (doc.sources.as_slice(), doc.views.as_slice()) else {
        return Err(PortableError::Shape);
    };
    if !matches!(doc.value.root, DocRoot::Sentence(_))
        || !doc.value.embeds.is_empty()
        || !doc.source_maps.is_empty()
    {
        return Err(PortableError::Shape);
    }
    b.charge(
        Resource::Work,
        (doc.value.nodes.len() + doc.origins.len()) as u64,
    )?;
    let mut contains = |span: &nepl3_core::source::Span| {
        b.charge(
            Resource::Work,
            (span.snapshot_ref().source.0.len() + view.head.snapshot_ref().source.0.len()) as u64
                + 40,
        )?;
        if span.snapshot_ref() == view.head.snapshot_ref()
            && span.start() >= view.head.start()
            && span.end() <= view.head.end()
        {
            Ok(())
        } else {
            Err(PortableError::Shape)
        }
    };
    for node in &doc.value.nodes {
        if !matches!(
            node.kind,
            DocKind::Sentence { .. }
                | DocKind::Text { .. }
                | DocKind::Concat { .. }
                | DocKind::Ruby { .. }
                | DocKind::Anno { .. }
        ) || node.origin.is_none()
        {
            return Err(PortableError::Shape);
        }
        contains(node.span.as_ref().ok_or(PortableError::Shape)?)?;
    }
    for origin in &doc.origins {
        match origin {
            Origin::Direct(span) => contains(span)?,
            Origin::Composite(inputs) if !inputs.is_empty() => {}
            Origin::Generated {
                callsite, inputs, ..
            } if !inputs.is_empty() => {
                if let Some(span) = callsite {
                    contains(span)?;
                }
            }
            Origin::Synthetic {
                anchor: Some(span), ..
            } => contains(span)?,
            _ => return Err(PortableError::Shape),
        }
    }
    Ok(())
}

fn check_payload<E>(
    input: &NdfValue,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    b.charge(Resource::AllocationUnits, 24)?;
    registry.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.doc".into(),
            revision: 1,
            name: "SentencePayload".into(),
        }),
        input,
        b,
    )?;
    Ok(())
}

/// Encode a single-source literal without duplicating its source bytes in NDF.
/// This payload must travel with its owner's explicit source declaration.
pub fn to_value<C: FoundationValueCodec>(
    doc: &DocumentSyntax,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    checked(doc, registry, c, b)?;
    let s = schema(registry)?;
    let mut store = SourceStore::default();
    store
        .insert_ref_with_budget(&doc.sources[0], b)
        .map_err(StructureError::from)?;
    let mut scoped = c.scoped(&store);
    let value = doc.value.put(s, &mut scoped, b)?;
    let origins = scoped.encode_origins(&doc.origins, b).map_err(boundary)?;
    let view = doc.views[0].put(s, &mut scoped, b)?;
    let out = record(s, "SentencePayload", [value, origins, view], b)?;
    check_payload(&out, registry, b)?;
    Ok(out)
}

/// Decode against only `owner`. An unrelated source already known to `c` is
/// never authority for this payload. Matching the enclosing token's head/view
/// remains the lower operation's responsibility.
pub fn from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    owner: &SourceSnapshot,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocumentSyntax, PortableError<C::Error>> {
    check_payload(input, registry, b)?;
    let s = schema(registry)?;
    let f = fields(input, s, "SentencePayload", 3)?;
    let mut store = SourceStore::default();
    store
        .insert_ref_with_budget(owner, b)
        .map_err(StructureError::from)?;
    let mut scoped = c.scoped(&store);
    scoped.admit_source(owner, b).map_err(boundary)?;
    let value = Value::read(&f[0], s, &mut scoped, b)?;
    let origins = scoped.decode_origins(&f[1], b).map_err(boundary)?;
    let view: DocView = Value::read(&f[2], s, &mut scoped, b)?;
    b.charge(
        Resource::AllocationUnits,
        (core::mem::size_of::<SourceSnapshot>() + core::mem::size_of::<DocView>()) as u64,
    )?;
    let doc = DocumentSyntax {
        value,
        sources: vec![owner.clone_with_budget(b)?],
        origins,
        views: vec![view],
        source_maps: vec![],
    };
    checked(&doc, registry, &mut scoped, b)?;
    Ok(doc)
}
