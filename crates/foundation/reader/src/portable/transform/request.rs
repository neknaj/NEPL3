//! Transform inputs resolve positions against an explicit dispatch closure.
use super::*;
use nepl3_core::origin::Mapping;

/// Encode with the host-selected sources and mappings. The operation dispatcher
/// additionally checks the value against the selected ProviderSignature.
pub fn to_value<C: FoundationValueCodec>(
    request: &TransformRequest,
    schema: &SchemaRef,
    codec: &mut C,
    sources: &SourceStore,
    mappings: &[Mapping],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut scoped = codec.scoped_with_mappings(sources, mappings);
    request
        .context
        .check(&mut scoped, sources, registry, budget)
        .map_err(PortableError::Context)?;
    let value = record(
        schema,
        "TransformRequest",
        [
            request.value.clone_with_budget(budget)?,
            scoped
                .encode_span(&request.span, budget)
                .map_err(boundary)?,
            scoped
                .encode_views(&request.view, budget)
                .map_err(boundary)?,
            super::super::context::to_value(&request.context, schema, &mut scoped, budget)?,
        ],
        budget,
    )?;
    validate_named(&value, schema, "TransformRequest", registry, budget)?;
    Ok(value)
}

/// Decode without granting access to the codec's ambient source store.
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    schema: &SchemaRef,
    codec: &mut C,
    sources: &SourceStore,
    mappings: &[Mapping],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<TransformRequest, PortableError<C::Error>> {
    validate_named(value, schema, "TransformRequest", registry, budget)?;
    let f = fields(value, schema, "TransformRequest", 4)?;
    let mut scoped = codec.scoped_with_mappings(sources, mappings);
    let context = super::super::context::from_value(&f[3], schema, &mut scoped, budget)?;
    context
        .check(&mut scoped, sources, registry, budget)
        .map_err(PortableError::Context)?;
    Ok(TransformRequest {
        value: f[0].clone_with_budget(budget)?,
        span: scoped.decode_span(&f[1], budget).map_err(boundary)?,
        view: scoped.decode_views(&f[2], budget).map_err(boundary)?,
        context,
    })
}
