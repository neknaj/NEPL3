//! A single literal's payload references its owner's source without embedding
//! the whole document in every token. This is not the general syntax envelope.
use super::{
    Error, boundary, schema, syntax, validate_named,
    value::{self, fields, record, reserve},
};
use crate::{
    model::{Kind, Root},
    syntax::{SentenceSyntax, SentenceView},
};
use alloc::vec;
use nepl3_core::{
    budget::{Budget, Resource},
    origin::Origin,
    schema::SchemaRegistry,
    source::{SourceSnapshot, SourceStore, Span},
    value::NdfValue,
    value_codec::FoundationValueCodec,
};

fn checked<C: FoundationValueCodec>(
    input: &SentenceSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<(), Error<C::Error>> {
    input.validate(r, b, c.source_admission())?;
    let ([_], [view], Root::Sentence(root)) = (
        input.sources.as_slice(),
        input.views.as_slice(),
        &input.value.root,
    ) else {
        return Err(Error::Shape);
    };
    if view.owner != root.0 || !input.value.embeds.is_empty() || !input.source_maps.is_empty() {
        return Err(Error::Shape);
    }
    let mut contains = |span: &Span| -> Result<(), Error<C::Error>> {
        b.charge(
            Resource::Work,
            (span.snapshot_ref().source.0.len() + view.head.snapshot_ref().source.0.len()) as u64
                + 40,
        )?;
        if span.snapshot_ref() != view.head.snapshot_ref()
            || span.start() < view.head.start()
            || span.end() > view.head.end()
        {
            return Err(Error::Shape);
        }
        Ok(())
    };
    for (node, location) in input.value.nodes.iter().zip(&input.locations) {
        if !matches!(
            node,
            Kind::Sentence { .. }
                | Kind::Text { .. }
                | Kind::Concat { .. }
                | Kind::Ruby { .. }
                | Kind::InlineAnno { .. }
        ) {
            return Err(Error::Shape);
        }
        contains(location.cover.as_ref().ok_or(Error::Shape)?)?;
        if let Some(head) = &location.head {
            contains(head)?;
        }
    }
    // A reader payload describes original literal syntax. Generated and mapped
    // syntax uses SentenceSyntax, including its explicit source/Origin closure.
    for origin in &input.origins {
        match origin {
            Origin::Direct(span) => contains(span)?,
            _ => return Err(Error::Shape),
        }
    }
    Ok(())
}

/// The receiving token owner must supply the exact snapshot explicitly.
/// Successful validation establishes structure, not source/meaning equality.
pub fn to_value<C: FoundationValueCodec>(
    input: &SentenceSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, Error<C::Error>> {
    let s = schema(r, b)?;
    checked(input, r, c, b)?;
    let ([source], [view]) = (input.sources.as_slice(), input.views.as_slice()) else {
        return Err(Error::Shape);
    };
    let mut store = SourceStore::default();
    store
        .insert_ref_with_budget(source, b)
        .map_err(crate::syntax::Error::from)?;
    let mut scoped = c.scoped(&store);
    let value = value::encode(&input.value, s, &mut scoped, b)?;
    let mut locations = reserve(input.locations.len(), b)?;
    for location in &input.locations {
        locations.push(syntax::location(location, s, &mut scoped, b)?);
    }
    let origins = scoped.encode_origins(&input.origins, b).map_err(boundary)?;
    let head = scoped.encode_span(&view.head, b).map_err(boundary)?;
    let presentation = scoped.encode_views(&view.view, b).map_err(boundary)?;
    let view = record(
        s,
        "SentenceView",
        [NdfValue::U64(view.owner), head, presentation],
        b,
    )?;
    let out = record(
        s,
        "SentenceLiteralPayload",
        [value, NdfValue::List(locations), origins, view],
        b,
    )?;
    validate_named(r, &out, "SentenceLiteralPayload", b)?;
    b.poll()?;
    Ok(out)
}

/// Ambient sources cannot complete this payload. Matching the returned head
/// and View to the enclosing token is the consuming lower operation's job.
pub fn from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    owner: &SourceSnapshot,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<SentenceSyntax, Error<C::Error>> {
    let s = schema(r, b)?;
    validate_named(r, input, "SentenceLiteralPayload", b)?;
    let [value, NdfValue::List(locations), origins, view] =
        fields(input, s, "SentenceLiteralPayload")?
    else {
        return Err(Error::Shape);
    };
    let mut store = SourceStore::default();
    store
        .insert_ref_with_budget(owner, b)
        .map_err(crate::syntax::Error::from)?;
    let mut scoped = c.scoped(&store);
    scoped.admit_source(owner, b).map_err(boundary)?;
    let value = value::decode(value, s, &mut scoped, b)?;
    let mut decoded_locations = reserve(locations.len(), b)?;
    for location in locations {
        decoded_locations.push(syntax::location_from(location, s, &mut scoped, b)?);
    }
    let origins = scoped.decode_origins(origins, b).map_err(boundary)?;
    let [NdfValue::U64(owner_index), head, view] = fields(view, s, "SentenceView")? else {
        return Err(Error::Shape);
    };
    let view = SentenceView {
        owner: *owner_index,
        head: scoped.decode_span(head, b).map_err(boundary)?,
        view: scoped.decode_views(view, b).map_err(boundary)?,
    };
    b.charge(
        Resource::AllocationUnits,
        (core::mem::size_of::<SourceSnapshot>() + core::mem::size_of::<SentenceView>()) as u64,
    )?;
    let out = SentenceSyntax {
        value,
        locations: decoded_locations,
        sources: vec![owner.clone_with_budget(b)?],
        origins,
        views: vec![view],
        source_maps: vec![],
    };
    checked(&out, r, &mut scoped, b)?;
    b.poll()?;
    Ok(out)
}
