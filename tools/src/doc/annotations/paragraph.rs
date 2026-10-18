//! Compose explicitly placed Sentence owners in one paragraph namespace.
//! This host operation does not infer an Article traversal or import definitions
//! from Math annotation scopes into the paragraph.
use super::*;
use document::namespace::{self as namespace, Selected};
use nepl3_sentence_core::model::Root;
use nepl3_suite::adapters::sentence::document_guests::{self as guests, Placement};
use std::sync::Arc;

pub struct RenderedParagraph<'a> {
    pub paragraph: html::paragraph::Paragraph<'a>,
    /// One ordered foreign record list per Sentence placement. Element indices
    /// point into the completed paragraph arena, not the temporary Sentence.
    pub foreign: Vec<Vec<ForeignRecord>>,
}
fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        (core::mem::size_of::<T>() as u64).saturating_mul(2),
    )?;
    items
        .try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    items.push(item);
    Ok(())
}

/// Render a complete paragraph using a single explicitly selected Doc namespace.
/// Doc fragments may refer forward across Sentence placements. All metadata and
/// source owners survive composition. Any failed member rejects the whole result.
pub fn render<'a, C: FoundationValueCodec>(
    inputs: &'a [SentenceSyntax],
    placements: &'a [Placement],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Result<RenderedParagraph<'a>, Error<C::Error>> {
    b.poll()?;
    let result = render_inner(inputs, placements, host, b);
    b.poll()?;
    result
}

fn render_inner<'a, C: FoundationValueCodec>(
    inputs: &'a [SentenceSyntax],
    placements: &'a [Placement],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Result<RenderedParagraph<'a>, Error<C::Error>> {
    for input in inputs {
        b.charge(Resource::Work, 1)?;
        if !matches!(input.value.root, Root::Sentence(_)) {
            return Err(Error::Selection);
        }
    }
    let surface = host.doc_surface.ok_or(Error::Selection)?;
    let selected = b
        .with_depth(|b| {
            guests::collect_placed(inputs, placements, surface, host.registry, host.codec, b)
        })
        .map_err(|error| match error {
            guests::PlacedError::Stopped(reason) => Error::Stopped(reason),
            error => match b.charge(
                Resource::AllocationUnits,
                core::mem::size_of_val(&error) as u64,
            ) {
                Ok(()) => Error::Placed(Box::new(error)),
                Err(reason) => Error::Stopped(reason),
            },
        })?;
    let (_, _, selections, occurrences) = selected.into_parts();
    let mut owner_documents = Vec::new();
    let mut local_selections = Vec::new();
    for selection in selections {
        let (documents, local) = selection.into_parts();
        let mut owned = Vec::new();
        for document in documents {
            b.charge(
                Resource::AllocationUnits,
                (core::mem::size_of_val(&document) + 2 * core::mem::size_of::<usize>()) as u64,
            )?;
            push(&mut owned, Arc::new(document), b)?;
        }
        let mut local_selected = Vec::new();
        for occurrence in local {
            let document = owned
                .get(occurrence.document.index())
                .ok_or(Error::Selection)?;
            push(
                &mut local_selected,
                Selected {
                    embed: occurrence.embed,
                    document: Arc::clone(document),
                },
                b,
            )?;
        }
        push(&mut owner_documents, owned, b)?;
        push(&mut local_selections, local_selected, b)?;
    }
    let mut selected = Vec::new();
    for occurrence in &occurrences {
        let document = owner_documents
            .get(occurrence.document.owner.0)
            .and_then(|docs| docs.get(occurrence.document.document.index()))
            .ok_or(Error::Selection)?;
        push(
            &mut selected,
            Selected {
                embed: occurrence.embed,
                document: Arc::clone(document),
            },
            b,
        )?;
    }
    let base = b
        .current_depth()
        .checked_add(1)
        .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
    b.charge(
        Resource::AllocationUnits,
        (inputs.len() as u64).saturating_mul(8),
    )?;
    b.charge(Resource::Work, inputs.len() as u64)?;
    let mut depths = Vec::new();
    depths
        .try_reserve_exact(inputs.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    depths.resize(inputs.len(), base);
    for placement in placements {
        b.charge(Resource::Work, 1)?;
        let depth = base
            .checked_add(placement.depth)
            .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
        let entry = depths.get_mut(placement.owner.0).ok_or(Error::Selection)?;
        *entry = (*entry).max(depth);
    }
    for ((input, local), depth) in inputs.iter().zip(&local_selections).zip(depths) {
        b.with_depth_at_least(depth, |b| {
            namespace::check_content(
                input,
                local,
                host.math_surface,
                host.registry,
                host.codec,
                b,
            )
            .map_err(|error| document_error(error, b))
        })?;
    }
    let members = namespace::inspect(&selected, host.registry, host.codec, b)
        .map_err(|error| document_error(error, b))?;
    let member_refs = namespace::member_refs(&members, b)?;
    let checked = nepl3_doc_core::labels::namespace::resolve(&member_refs, b).map_err(|error| {
        let error = namespace::resolution_error(
            error,
            &selected,
            &member_refs,
            host.registry,
            host.codec,
            b,
        );
        document_error(error, b)
    })?;
    let options = nepl3_doc_html::RenderOptions {
        parallel: nepl3_doc_html::ParallelMode::Rows,
    };
    let prepared = namespace::prepare(&checked, &options, host.registry, host.codec, b)
        .map_err(|error| document_error(error, b))?;
    let mut parts = Vec::new();
    let mut records = Vec::new();
    let mut document_position = 0;
    for (placement_index, placement) in placements.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let input = inputs.get(placement.owner.0).ok_or(Error::Selection)?;
        let depth = base
            .checked_add(placement.depth)
            .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
        let mut foreign = Vec::new();
        let registry = host.registry;
        let sentence_surface = host.surface;
        let math_surface = host.math_surface;
        let doc_surface = host.doc_surface;
        let part = b
            .with_depth_at_least(depth, |b| {
                html::render_part_with_foreign_codec(
                    input,
                    registry,
                    host.codec,
                    &mut |closure, embed, codec, b| {
                        let candidate = selected.get(document_position).map(|selected| {
                            (
                                selected,
                                nepl3_doc_core::labels::namespace::MemberId(
                                    document_position as u64,
                                ),
                            )
                        });
                        let (markup, record) = super::foreign::render(
                            closure,
                            embed,
                            candidate,
                            &prepared,
                            &mut SentenceAnnotationRenderer {
                                registry,
                                surface: sentence_surface,
                                math_surface,
                                doc_surface,
                                codec,
                            },
                            b,
                        )?;
                        if matches!(&record, ForeignRecord::Document(_)) {
                            let occurrence =
                                occurrences.get(document_position).ok_or(Error::Selection)?;
                            if occurrence.placement != placement_index
                                || occurrence.document.owner != placement.owner
                                || occurrence.embed != embed
                            {
                                return Err(Error::Selection);
                            }
                            document_position += 1;
                        }
                        push(&mut foreign, record, b)?;
                        Ok::<_, Error<C::Error>>(markup)
                    },
                    b,
                )
            })
            .map_err(|error| match error {
                html::RenderFailure::Sentence(e) => Error::Render(e),
                html::RenderFailure::Foreign(e) => e,
            })?;
        push(&mut parts, part, b)?;
        push(&mut records, foreign, b)?;
    }
    if document_position != selected.len() {
        return Err(Error::Selection);
    }
    let paragraph = html::paragraph::compose(parts, b).map_err(Error::Render)?;
    for (foreign, placement) in records.iter_mut().zip(paragraph.placements()) {
        if foreign.len() != placement.foreign().len() {
            return Err(Error::Selection);
        }
        for (record, foreign_placement) in foreign.iter_mut().zip(placement.foreign()) {
            super::foreign::place(record, foreign_placement, b)?;
        }
    }
    Ok(RenderedParagraph {
        paragraph,
        foreign: records,
    })
}
