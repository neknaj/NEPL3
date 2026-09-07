//! Typed reader request boundary. Source-table admission precedes context decoding.
pub mod transform;
use crate::{
    context::ContextError,
    model::{OwnedReadRequest, ReaderContext},
};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    source::{Digest, SourceError, SourceId, SourceRef, SourceSnapshot, SourceStore},
    value::{NdfValue, Record, SchemaRef},
    value_codec::FoundationValueCodec,
};

#[derive(Debug)]
pub enum PortableError<E> {
    Stopped(StopReason),
    Boundary(E),
    Schema(SchemaError),
    Source(SourceError),
    Context(ContextError<E>),
    Shape,
    UndeclaredSource,
    Reader(crate::runtime::ReaderError),
}
impl<E> From<StopReason> for PortableError<E> {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
fn fields<'a, E>(
    value: &'a NdfValue,
    schema: &SchemaRef,
    name: &str,
    n: usize,
) -> Result<&'a [NdfValue], PortableError<E>> {
    match value {
        NdfValue::Record(v) if &v.schema == schema && v.kind == name && v.fields.len() == n => {
            Ok(&v.fields)
        }
        _ => Err(PortableError::Shape),
    }
}
fn string<E>(value: &NdfValue, budget: &mut Budget) -> Result<String, PortableError<E>> {
    match value {
        NdfValue::Text(v) => {
            budget.charge(Resource::AllocationUnits, v.len() as u64)?;
            Ok(v.clone())
        }
        _ => Err(PortableError::Shape),
    }
}
fn number<E>(value: &NdfValue) -> Result<u64, PortableError<E>> {
    match value {
        NdfValue::U64(v) => Ok(*v),
        _ => Err(PortableError::Shape),
    }
}
fn digest<E>(value: &NdfValue) -> Result<Digest, PortableError<E>> {
    match value {
        NdfValue::Bytes(v) => Ok(Digest(
            v.as_slice().try_into().map_err(|_| PortableError::Shape)?,
        )),
        _ => Err(PortableError::Shape),
    }
}
fn record<E, const N: usize>(
    schema: &SchemaRef,
    name: &str,
    fields: [NdfValue; N],
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    budget.charge(
        Resource::AllocationUnits,
        (schema.package.len() + name.len() + N * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    Ok(NdfValue::Record(Record {
        schema: schema.clone(),
        kind: name.into(),
        fields: Vec::from(fields),
    }))
}
fn text<E>(value: &str, budget: &mut Budget) -> Result<NdfValue, PortableError<E>> {
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(NdfValue::Text(value.into()))
}
fn schema_value<E>(
    value: &SchemaRef,
    foundation: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    budget.charge(Resource::AllocationUnits, 32)?;
    record(
        foundation,
        "SchemaRef",
        [
            text(&value.package, budget)?,
            NdfValue::U64(value.revision),
            NdfValue::Bytes(value.digest.0.to_vec()),
        ],
        budget,
    )
}
fn schema_from<E>(
    value: &NdfValue,
    foundation: &SchemaRef,
    budget: &mut Budget,
) -> Result<SchemaRef, PortableError<E>> {
    let f = fields(value, foundation, "SchemaRef", 3)?;
    Ok(SchemaRef {
        package: string(&f[0], budget)?,
        revision: number(&f[1])?,
        digest: digest(&f[2])?,
    })
}
fn validate<E>(
    value: &NdfValue,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), PortableError<E>> {
    if schema.package != crate::schema::PACKAGE
        || schema.revision != crate::schema::REVISION
        || registry.descriptor(schema).is_none()
    {
        return Err(PortableError::Shape);
    }
    budget.charge(
        Resource::AllocationUnits,
        (schema.package.len() + "ReadRequest".len()) as u64,
    )?;
    registry
        .validate(
            &TypeDescriptor::Named(TypeRef {
                package: schema.package.clone(),
                revision: schema.revision,
                name: "ReadRequest".into(),
            }),
            value,
            budget,
        )
        .map_err(PortableError::Schema)?;
    Ok(())
}
fn closed<C: FoundationValueCodec>(
    request: &OwnedReadRequest,
    codec: &mut C,
    sources: &SourceStore,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    budget.charge(Resource::Work, request.sources.len() as u64)?;
    let snapshot = request
        .sources
        .iter()
        .find(|s| {
            s.identity().source == request.snapshot.source_id
                && s.identity().revision == request.snapshot.revision
                && s.identity().digest == request.snapshot.digest
        })
        .ok_or(PortableError::UndeclaredSource)?;
    snapshot
        .check_range(request.start, request.limit)
        .map_err(PortableError::Source)?;
    if sources.resolve(&request.snapshot) != Some(snapshot) {
        return Err(PortableError::UndeclaredSource);
    }
    let checked = request
        .context
        .check(codec, sources, registry, budget)
        .map_err(PortableError::Context)?;
    for source in checked.sources() {
        budget.charge(Resource::Work, request.sources.len() as u64)?;
        if !request.sources.iter().any(|declared| declared == *source) {
            return Err(PortableError::UndeclaredSource);
        }
    }
    Ok(())
}

pub fn request_to_value<C: FoundationValueCodec>(
    request: &OwnedReadRequest,
    schema: &SchemaRef,
    codec: &mut C,
    sources: &SourceStore,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    closed(request, codec, sources, registry, budget)?;
    budget.charge(
        Resource::AllocationUnits,
        codec.foundation_schema().package.len() as u64,
    )?;
    let foundation = codec.foundation_schema().clone();
    budget.charge(Resource::AllocationUnits, 32)?;
    let snapshot = record(
        &foundation,
        "SourceRef",
        [
            text(&request.snapshot.source_id.0, budget)?,
            NdfValue::U64(request.snapshot.revision),
            NdfValue::Bytes(request.snapshot.digest.0.to_vec()),
        ],
        budget,
    )?;
    let context = record(
        schema,
        "ReaderContext",
        [
            schema_value(&request.context.schema, &foundation, budget)?,
            text(&request.context.category, budget)?,
            text(&request.context.mode, budget)?,
            codec
                .encode_environment(&request.context.environment, budget)
                .map_err(PortableError::Boundary)?,
            codec
                .encode_origins(&request.context.origins, budget)
                .map_err(PortableError::Boundary)?,
        ],
        budget,
    )?;
    let value = record(
        schema,
        "ReadRequest",
        [
            snapshot,
            codec
                .encode_sources(&request.sources, budget)
                .map_err(PortableError::Boundary)?,
            NdfValue::U64(request.start),
            NdfValue::U64(request.limit),
            NdfValue::Bool(request.final_input),
            context,
            request.state.clone_with_budget(budget)?,
        ],
        budget,
    )?;
    validate(&value, schema, registry, budget)?;
    Ok(value)
}

/// Phase one: accept only the explicit source declarations. The host creates a
/// request-local SourceStore from this result before decoding context positions.
pub fn request_sources<C: FoundationValueCodec>(
    value: &NdfValue,
    schema: &SchemaRef,
    codec: &mut C,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Vec<SourceSnapshot>, PortableError<C::Error>> {
    validate(value, schema, registry, budget)?;
    codec
        .decode_sources(&fields(value, schema, "ReadRequest", 7)?[1], budget)
        .map_err(PortableError::Boundary)
}

/// Phase two: supplied context sources must all occur in the request table, even
/// if the caller accidentally supplies a larger host store to the codec.
pub fn request_from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    schema: &SchemaRef,
    codec: &mut C,
    sources: &SourceStore,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<OwnedReadRequest, PortableError<C::Error>> {
    validate(value, schema, registry, budget)?;
    budget.charge(
        Resource::AllocationUnits,
        codec.foundation_schema().package.len() as u64,
    )?;
    let foundation = codec.foundation_schema().clone();
    let f = fields(value, schema, "ReadRequest", 7)?;
    let reference = fields(&f[0], &foundation, "SourceRef", 3)?;
    let snapshot = SourceRef {
        source_id: SourceId(string(&reference[0], budget)?),
        revision: number(&reference[1])?,
        digest: digest(&reference[2])?,
    };
    let context = fields(&f[5], schema, "ReaderContext", 5)?;
    let context = ReaderContext {
        schema: schema_from(&context[0], &foundation, budget)?,
        category: string(&context[1], budget)?,
        mode: string(&context[2], budget)?,
        environment: codec
            .decode_environment(&context[3], budget)
            .map_err(PortableError::Boundary)?,
        origins: codec
            .decode_origins(&context[4], budget)
            .map_err(PortableError::Boundary)?,
    };
    let NdfValue::Bool(final_input) = &f[4] else {
        return Err(PortableError::Shape);
    };
    let request = OwnedReadRequest {
        snapshot,
        sources: codec
            .decode_sources(&f[1], budget)
            .map_err(PortableError::Boundary)?,
        start: number(&f[2])?,
        limit: number(&f[3])?,
        final_input: *final_input,
        context,
        state: f[6].clone_with_budget(budget)?,
    };
    closed(&request, codec, sources, registry, budget)?;
    Ok(request)
}
