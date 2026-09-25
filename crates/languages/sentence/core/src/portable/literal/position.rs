//! Literal-local positions inherit the one explicitly supplied owner snapshot.
use super::super::{
    Error,
    value::{fields, record},
};
use crate::syntax::NodeLocation;
use alloc::boxed::Box;
use nepl3_core::{
    budget::{Budget, Resource},
    origin::OriginId,
    source::{SourceSnapshot, Span},
    value::{NdfValue, SchemaRef},
};

pub(super) fn encode<E>(
    span: &Span,
    schema: &SchemaRef,
    b: &mut Budget,
) -> Result<NdfValue, Error<E>> {
    record(
        schema,
        "SentenceLiteralSpan",
        [NdfValue::U64(span.start()), NdfValue::U64(span.end())],
        b,
    )
}

pub(super) fn decode<E>(
    value: &NdfValue,
    schema: &SchemaRef,
    owner: &SourceSnapshot,
    b: &mut Budget,
) -> Result<Span, Error<E>> {
    b.charge(Resource::Work, 1)?;
    let [NdfValue::U64(start), NdfValue::U64(end)] = fields(value, schema, "SentenceLiteralSpan")?
    else {
        return Err(Error::Shape);
    };
    owner
        .span_with_budget(*start, *end, b)
        .map_err(crate::syntax::Error::from)
        .map_err(Error::from)
}

pub(super) fn encode_location<E>(
    location: &NodeLocation,
    schema: &SchemaRef,
    b: &mut Budget,
) -> Result<NdfValue, Error<E>> {
    let head = match &location.head {
        Some(span) => {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            NdfValue::Some(Box::new(encode(span, schema, b)?))
        }
        None => NdfValue::None,
    };
    let cover = encode(location.cover.as_ref().ok_or(Error::Shape)?, schema, b)?;
    record(
        schema,
        "SentenceLiteralLocation",
        [NdfValue::U64(location.origin.0), head, cover],
        b,
    )
}

pub(super) fn decode_location<E>(
    value: &NdfValue,
    schema: &SchemaRef,
    owner: &SourceSnapshot,
    b: &mut Budget,
) -> Result<NodeLocation, Error<E>> {
    let [NdfValue::U64(origin), head, cover] = fields(value, schema, "SentenceLiteralLocation")?
    else {
        return Err(Error::Shape);
    };
    let head = match head {
        NdfValue::None => None,
        NdfValue::Some(span) => Some(decode(span, schema, owner, b)?),
        _ => return Err(Error::Shape),
    };
    Ok(NodeLocation {
        origin: OriginId(*origin),
        head,
        cover: Some(decode(cover, schema, owner, b)?),
    })
}
