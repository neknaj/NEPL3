//! Typed Sentence insertion for the all-variant, asset-free Article display
//! subset. This host does not infer visibility for Single mode or image alt.
use super::{
    Selection,
    namespace::{self, MemberOrigin, SentenceOccurrenceId},
};
use crate::doc::annotations::{self, ForeignRecord, SentenceAnnotationRenderer, document, foreign};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    labels::namespace::{CheckedNamespace, MemberId},
    model::{DocEmbedView, EmbedKind, EmbedRef},
};
use nepl3_doc_html::{ParallelMode, RenderOptions, namespace::display};
use nepl3_markup::html::HtmlRequest;
use nepl3_suite::adapters::sentence::{document_guests::OwnerId, html};

#[derive(Debug)]
pub enum Error<E, F> {
    Stopped(StopReason),
    SingleMode,
    Selection,
    Resolve(Box<namespace::Error<E>>),
    Document {
        member: Option<MemberOrigin>,
        error: Box<document::Error<E>>,
    },
    Sentence(Box<annotations::Error<E>>),
    Adapter(F),
    Render(nepl3_doc_html::RenderError),
    Projection(crate::doc::math::ProjectionError),
}
impl<E, F> From<StopReason> for Error<E, F> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct SentenceRecord {
    pub occurrence: SentenceOccurrenceId,
    pub owner: OwnerId,
    /// Index into the completed Article's foreign placement sequence.
    pub placement: usize,
    /// Element indices in the final Article; node indices in the retained owner.
    pub origins: Vec<html::ElementOrigin>,
    pub foreign: Vec<ForeignRecord>,
}
pub struct Rendered<'s, 'a> {
    pub resolution: namespace::Resolution<'s, 'a>,
    pub article: nepl3_doc_html::RenderedInlineWithForeign,
    pub sentences: Vec<SentenceRecord>,
}
fn boxed<T>(value: T, b: &mut Budget) -> Result<Box<T>, StopReason> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    Ok(Box::new(value))
}
fn push<T>(items: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        (core::mem::size_of::<T>() as u64).saturating_mul(2),
    )?;
    items
        .try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    items.push(value);
    Ok(())
}

/// Render every structural Sentence occurrence in Rows/Columns mode. The shared
/// display preparation rejects assets before any callback, so no image-alt
/// occurrence or hidden variant can be confused with a visible placement.
/// Other supported foreign kinds require the supplied explicit host adapter.
pub fn render<'s, 'a, C: FoundationValueCodec, F>(
    input: &'s Selection<'a>,
    options: &RenderOptions,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    other: &mut impl FnMut(&DocEmbedView<'_>, EmbedRef, &mut C, &mut Budget) -> Result<HtmlRequest, F>,
    b: &mut Budget,
) -> Result<Rendered<'s, 'a>, Error<C::Error, F>> {
    b.poll()?;
    if matches!(options.parallel, ParallelMode::Single { .. }) {
        return Err(Error::SingleMode);
    }
    let mut adopted = Vec::new();
    for index in 0..input.sentences().len() {
        push(&mut adopted, SentenceOccurrenceId(index), b)?;
    }
    let result = namespace::resolve_with(input, &adopted, host, b, |checked, members, host, b| {
        render_resolved(input, options, checked, members, host, other, b)
    });
    b.poll()?;
    let (resolution, (article, sentences)) = match result {
        Ok(result) => result,
        Err(namespace::OperationError::Resolve(error)) => {
            return Err(Error::Resolve(boxed(error, b)?));
        }
        Err(namespace::OperationError::Operation(error)) => return Err(error),
    };
    Ok(Rendered {
        resolution,
        article,
        sentences,
    })
}
fn render_resolved<C: FoundationValueCodec, F>(
    input: &Selection<'_>,
    options: &RenderOptions,
    checked: &CheckedNamespace<'_, '_>,
    members: namespace::Members<'_>,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    other: &mut impl FnMut(&DocEmbedView<'_>, EmbedRef, &mut C, &mut Budget) -> Result<HtmlRequest, F>,
    b: &mut Budget,
) -> Result<
    (
        nepl3_doc_html::RenderedInlineWithForeign,
        Vec<SentenceRecord>,
    ),
    Error<C::Error, F>,
> {
    let prepared = display::prepare_located(checked, options, host.registry, host.codec, b)
        .map_err(|failure| {
            let origin = match failure.member {
                Some(MemberId(0)) => Some(MemberOrigin::Article),
                Some(member) => members.guest(member).map(|(origin, _)| origin),
                None => None,
            };
            let error = match failure.member.and_then(|member| checked.document(member)) {
                Some(document) => document::preparation_error(
                    failure.error,
                    document,
                    host.registry,
                    host.codec,
                    b,
                ),
                None => document::unlocated_preparation_error(failure.error),
            };
            match boxed(error, b) {
                Ok(error) => Error::Document {
                    member: origin,
                    error,
                },
                Err(reason) => Error::Stopped(reason),
            }
        })?;
    let mut records = Vec::new();
    let mut next_sentence = 0;
    let mut next_member = 1;
    let mut placement = 0;
    let mut adapter = |view: &DocEmbedView<'_>, embed: EmbedRef, b: &mut Budget| {
        let current_placement = placement;
        placement += 1;
        if view.kind != EmbedKind::Sentence {
            return other(view, embed, host.codec, b).map_err(Error::Adapter);
        }
        let occurrence = SentenceOccurrenceId(next_sentence);
        let selected = input
            .sentences()
            .get(next_sentence)
            .ok_or(Error::Selection)?;
        let owner = input
            .owners()
            .get(selected.owner.0)
            .ok_or(Error::Selection)?;
        if owner.embed() != embed {
            return Err(Error::Selection);
        }
        let registry = host.registry;
        let surface = host.surface;
        let math_surface = host.math_surface;
        let doc_surface = host.doc_surface;
        let mut foreign_records = Vec::new();
        let part = html::render_part_with_foreign_codec(
            owner.syntax(),
            registry,
            host.codec,
            &mut |closure, embed, codec, b| {
                let candidate = members.guest(MemberId(next_member));
                let (markup, record) = foreign::render(
                    closure,
                    embed,
                    candidate.map(|(_, selected)| (selected, MemberId(next_member))),
                    document::Preparation::Display(&prepared),
                    &mut SentenceAnnotationRenderer {
                        registry,
                        surface,
                        math_surface,
                        doc_surface,
                        codec,
                    },
                    b,
                )?;
                if matches!(&record, ForeignRecord::Document(_)) {
                    let (origin, _) = candidate.ok_or(annotations::Error::Selection)?;
                    if !matches!(origin,MemberOrigin::SentenceInline{sentence,owner,..}
                        if sentence==occurrence && owner==selected.owner)
                    {
                        return Err(annotations::Error::Selection);
                    }
                    next_member += 1;
                }
                push(&mut foreign_records, record, b)?;
                Ok::<_, annotations::Error<C::Error>>(markup)
            },
            b,
        )
        .map_err(|error| {
            let error = match error {
                html::RenderFailure::Sentence(error) => annotations::Error::Render(error),
                html::RenderFailure::Foreign(error) => error,
            };
            match boxed(error, b) {
                Ok(error) => Error::Sentence(error),
                Err(reason) => Error::Stopped(reason),
            }
        })?;
        if matches!(members.guest(MemberId(next_member)),Some((MemberOrigin::SentenceInline{sentence,..},_)) if sentence==occurrence)
        {
            return Err(Error::Selection);
        }
        let (_, markup, origins, foreign_placements) = part.into_parts();
        if foreign_records.len() != foreign_placements.len() {
            return Err(Error::Selection);
        }
        for (record, placement) in foreign_records.iter_mut().zip(&foreign_placements) {
            foreign::place(record, placement, b).map_err(|error| match boxed(error, b) {
                Ok(error) => Error::Sentence(error),
                Err(reason) => Error::Stopped(reason),
            })?;
        }
        push(
            &mut records,
            SentenceRecord {
                occurrence,
                owner: selected.owner,
                placement: current_placement,
                origins,
                foreign: foreign_records,
            },
            b,
        )?;
        next_sentence += 1;
        Ok(markup)
    };
    let rendered =
        display::render_article(&prepared, MemberId(0), &mut adapter, b).map_err(|error| {
            match error {
                display::ArticleError::Member(_) | display::ArticleError::ExpectedArticle(_) => {
                    Error::Selection
                }
                display::ArticleError::Render(nepl3_doc_html::ForeignRenderError::Render(
                    error,
                )) => Error::Render(error),
                display::ArticleError::Render(nepl3_doc_html::ForeignRenderError::Foreign(
                    error,
                )) => error,
            }
        })?;
    if next_sentence != input.sentences().len()
        || members.guest(MemberId(next_member)).is_some()
        || placement != rendered.foreign.len()
    {
        return Err(Error::Selection);
    }
    for record in &mut records {
        b.charge(Resource::Work, 1)?;
        let placement = rendered
            .foreign
            .get(record.placement)
            .ok_or(Error::Selection)?;
        let owner = input.owners().get(record.owner.0).ok_or(Error::Selection)?;
        if placement.embed != owner.embed() {
            return Err(Error::Selection);
        }
        let mut map = |element| {
            if element >= placement.elements {
                return Err(crate::doc::math::ProjectionError::Mapping(element));
            }
            placement
                .first_element
                .checked_add(element)
                .ok_or(crate::doc::math::ProjectionError::Mapping(element))
        };
        for origin in &mut record.origins {
            b.charge(Resource::Work, 1)?;
            if origin.node >= owner.syntax().value.nodes.len() as u64 {
                return Err(Error::Selection);
            }
            origin.element = map(origin.element).map_err(Error::Projection)?;
        }
        for foreign in &mut record.foreign {
            foreign.remap(&mut map, b).map_err(Error::Projection)?;
        }
    }
    Ok((rendered, records))
}
