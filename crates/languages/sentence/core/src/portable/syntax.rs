//! Portable presentation boundary scoped to its declared local source closure.
use super::{
    Error, boundary, schema, validate_named,
    value::{self, fields, record, reserve},
};
use crate::syntax::{NodeLocation, SentenceSyntax, SentenceView};
use alloc::boxed::Box;
use nepl3_core::{
    budget::{Budget, Resource},
    origin::OriginId,
    schema::SchemaRegistry,
    source::Span,
    value::{NdfValue, SchemaRef},
    value_codec::FoundationValueCodec,
};

fn optional<C: FoundationValueCodec>(
    v: &Option<Span>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, Error<C::Error>> {
    match v {
        None => Ok(NdfValue::None),
        Some(v) => {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            Ok(NdfValue::Some(Box::new(
                c.encode_span(v, b).map_err(boundary)?,
            )))
        }
    }
}
fn optional_from<C: FoundationValueCodec>(
    v: &NdfValue,
    c: &mut C,
    b: &mut Budget,
) -> Result<Option<Span>, Error<C::Error>> {
    match v {
        NdfValue::None => Ok(None),
        NdfValue::Some(v) => Ok(Some(c.decode_span(v, b).map_err(boundary)?)),
        _ => Err(Error::Shape),
    }
}
pub(super) fn location<C: FoundationValueCodec>(
    v: &NodeLocation,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, Error<C::Error>> {
    let origin = record(
        c.foundation_schema(),
        "OriginRef",
        [NdfValue::U64(v.origin.0)],
        b,
    )?;
    let head = optional(&v.head, c, b)?;
    let cover = optional(&v.cover, c, b)?;
    record(s, "SentenceNodeLocation", [origin, head, cover], b)
}
pub(super) fn location_from<C: FoundationValueCodec>(
    v: &NdfValue,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<NodeLocation, Error<C::Error>> {
    let [origin, head, cover] = fields(v, s, "SentenceNodeLocation")? else {
        return Err(Error::Shape);
    };
    let [NdfValue::U64(origin)] = fields(origin, c.foundation_schema(), "OriginRef")? else {
        return Err(Error::Shape);
    };
    let origin = OriginId(*origin);
    Ok(NodeLocation {
        origin,
        head: optional_from(head, c, b)?,
        cover: optional_from(cover, c, b)?,
    })
}

pub fn to_value<C: FoundationValueCodec>(
    input: &SentenceSyntax,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, Error<C::Error>> {
    let s = schema(registry, b)?;
    input.validate(registry, b, c.source_admission())?;
    let source_values = c.encode_sources(&input.sources, b).map_err(boundary)?;
    let store = crate::syntax::sources(&input.sources, c.source_admission(), b)?;
    let mut scoped = c.scoped_with_mappings(&store, &input.source_maps);
    let value = value::encode(&input.value, s, &mut scoped, b)?;
    let mut locations = reserve(input.locations.len(), b)?;
    for v in &input.locations {
        locations.push(location(v, s, &mut scoped, b)?);
    }
    let origins = scoped.encode_origins(&input.origins, b).map_err(boundary)?;
    let mut views = reserve(input.views.len(), b)?;
    for v in &input.views {
        let head = scoped.encode_span(&v.head, b).map_err(boundary)?;
        let view = scoped.encode_views(&v.view, b).map_err(boundary)?;
        views.push(record(
            s,
            "SentenceView",
            [NdfValue::U64(v.owner), head, view],
            b,
        )?);
    }
    let maps = scoped
        .encode_mappings(&input.source_maps, b)
        .map_err(boundary)?;
    let out = record(
        s,
        "SentenceSyntax",
        [
            value,
            NdfValue::List(locations),
            source_values,
            origins,
            NdfValue::List(views),
            maps,
        ],
        b,
    )?;
    validate_named(registry, &out, "SentenceSyntax", b)?;
    b.poll()?;
    Ok(out)
}

pub fn from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<SentenceSyntax, Error<C::Error>> {
    let s = schema(registry, b)?;
    validate_named(registry, input, "SentenceSyntax", b)?;
    let [
        value,
        NdfValue::List(locations),
        sources,
        origins,
        NdfValue::List(views),
        maps,
    ] = fields(input, s, "SentenceSyntax")?
    else {
        return Err(Error::Shape);
    };
    let sources = c.decode_sources(sources, b).map_err(boundary)?;
    let store = crate::syntax::sources(&sources, c.source_admission(), b)?;
    let mut scoped = c.scoped(&store);
    let value = value::decode(value, s, &mut scoped, b)?;
    let mut decoded_locations = reserve(locations.len(), b)?;
    for v in locations {
        decoded_locations.push(location_from(v, s, &mut scoped, b)?);
    }
    let origins = scoped.decode_origins(origins, b).map_err(boundary)?;
    let source_maps = scoped.decode_mappings(maps, b).map_err(boundary)?;
    let decoded_views = {
        let mut mapped = scoped.scoped_with_mappings(&store, &source_maps);
        let mut decoded_views = reserve(views.len(), b)?;
        for v in views {
            let [NdfValue::U64(owner), head, view] = fields(v, s, "SentenceView")? else {
                return Err(Error::Shape);
            };
            decoded_views.push(SentenceView {
                owner: *owner,
                head: mapped.decode_span(head, b).map_err(boundary)?,
                view: mapped.decode_views(view, b).map_err(boundary)?,
            });
        }
        decoded_views
    };
    let out = SentenceSyntax {
        value,
        locations: decoded_locations,
        sources,
        origins,
        views: decoded_views,
        source_maps,
    };
    out.validate(registry, b, scoped.source_admission())?;
    b.poll()?;
    Ok(out)
}
