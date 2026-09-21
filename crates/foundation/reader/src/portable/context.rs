//! Shared structural representation; enclosing requests validate the context.
use super::*;

pub(super) fn to_value<C: FoundationValueCodec>(
    context: &ReaderContext,
    schema: &SchemaRef,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let identity = schema_value(&context.schema, codec.foundation_schema(), budget)?;
    record(
        schema,
        "ReaderContext",
        [
            identity,
            text(&context.category, budget)?,
            text(&context.mode, budget)?,
            codec
                .encode_environment(&context.environment, budget)
                .map_err(PortableError::Boundary)?,
            codec
                .encode_origins(&context.origins, budget)
                .map_err(PortableError::Boundary)?,
        ],
        budget,
    )
}

pub(super) fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    schema: &SchemaRef,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<ReaderContext, PortableError<C::Error>> {
    let f = fields(value, schema, "ReaderContext", 5)?;
    Ok(ReaderContext {
        schema: schema_from(&f[0], codec.foundation_schema(), budget)?,
        category: string(&f[1], budget)?,
        mode: string(&f[2], budget)?,
        environment: codec
            .decode_environment(&f[3], budget)
            .map_err(PortableError::Boundary)?,
        origins: codec
            .decode_origins(&f[4], budget)
            .map_err(PortableError::Boundary)?,
    })
}
