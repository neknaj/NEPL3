//! Dependent request envelopes share the declared-source ReadRequest boundary.
//! The dispatch layer separately validates first/state against its selected
//! ProviderSignature before invoking the provider.
use super::*;
use crate::model::DependentRequest;

/// Encode a preceding result and the request starting at its ending cursor.
pub fn to_value<C: FoundationValueCodec>(
    request: &DependentRequest,
    schema: &SchemaRef,
    codec: &mut C,
    sources: &SourceStore,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    budget.poll()?;
    if request.end != request.request.start {
        return Err(PortableError::Shape);
    }
    let nested = request_to_value(&request.request, schema, codec, sources, registry, budget)?;
    let value = record(
        schema,
        "DependentRequest",
        [
            request.first.clone_with_budget(budget)?,
            NdfValue::U64(request.end),
            nested,
        ],
        budget,
    )?;
    validate_named(&value, schema, "DependentRequest", registry, budget)?;
    Ok(value)
}

/// Admit the nested request's explicit source table before decoding its context.
pub fn sources<C: FoundationValueCodec>(
    value: &NdfValue,
    schema: &SchemaRef,
    codec: &mut C,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Vec<SourceSnapshot>, PortableError<C::Error>> {
    validate_named(value, schema, "DependentRequest", registry, budget)?;
    request_sources(
        &fields(value, schema, "DependentRequest", 3)?[2],
        schema,
        codec,
        registry,
        budget,
    )
}

/// Decode using a store constructed from the explicit source table.
/// The end cursor must equal the nested request start, whose UTF-8 range is
/// checked by the common ReadRequest decoder.
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    schema: &SchemaRef,
    codec: &mut C,
    sources: &SourceStore,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<DependentRequest, PortableError<C::Error>> {
    validate_named(value, schema, "DependentRequest", registry, budget)?;
    let fields = fields(value, schema, "DependentRequest", 3)?;
    let end = number(&fields[1])?;
    let request = request_from_value(&fields[2], schema, codec, sources, registry, budget)?;
    if end != request.start {
        return Err(PortableError::Shape);
    }
    Ok(DependentRequest {
        first: fields[0].clone_with_budget(budget)?,
        end,
        request,
    })
}
