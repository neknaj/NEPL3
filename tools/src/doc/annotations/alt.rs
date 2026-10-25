//! Explicit native Sentence alt-text selection, separate from HTML guests.
//! The document and each lowered Sentence retain their own source ownership.
use super::SentenceAnnotationRenderer;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{model::*, text};
use nepl3_sentence_core::{syntax::SentenceSyntax, text as sentence_text};

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Document(nepl3_doc_core::portable::PortableError<E>),
    Selection(super::Error<E>),
    Sentence(nepl3_sentence_core::syntax::Error),
    Text(sentence_text::Error),
    Shape(nepl3_doc_core::check::ShapeError),
    Identity,
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Owned source evidence and identity-bound text, not reusable validation proof.
pub struct Projection {
    pub sentences: Vec<(EmbedRef, SentenceSyntax)>,
    pub resolved: Vec<text::ResolvedInlineText>,
}

/// Select only independent Sentence owners in Image/InlineImage alt slots.
/// Ruby readings and annotation notes are omitted by BaseOnly. Selected foreign
/// meaning is deliberately unresolved: no Math/Doc HTML fallback supplies text.
/// All validation, lowering, identity and projection use the caller's operation.
pub fn prepare<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    budget: &mut Budget,
) -> Result<Projection, Error<C::Error>> {
    let prepared =
        text::prepare(document, host.registry, host.codec, budget).map_err(Error::Document)?;
    let mut projection = Projection {
        sentences: Vec::new(),
        resolved: Vec::new(),
    };
    let shape = document
        .value
        .validate_shape(budget)
        .map_err(Error::Shape)?;
    let depths = nepl3_doc_core::print::guest_depths(&shape, budget)?;
    let mut owners: Vec<(EmbedRef, u64)> = Vec::new();
    for node in &document.value.nodes {
        budget.charge(Resource::Work, 1)?;
        let alt = match node.kind {
            DocKind::Image { alt, .. } | DocKind::InlineImage { alt, .. } => alt,
            _ => continue,
        };
        let DocKind::SentenceGuest { syntax } = document.value.nodes[alt.0 as usize].kind else {
            continue;
        };
        let depth = *depths.get(syntax.0 as usize).ok_or(Error::Identity)?;
        let mut found = false;
        for (prior, prior_depth) in &mut owners {
            budget.charge(Resource::Work, 1)?;
            if *prior == syntax {
                *prior_depth = (*prior_depth).max(depth);
                found = true;
                break;
            }
        }
        if !found {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<(EmbedRef, u64)>() as u64,
            )?;
            owners
                .try_reserve_exact(1)
                .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
            owners.push((syntax, depth));
        }
    }
    let base = budget.current_depth();
    for (syntax, depth) in owners {
        budget.charge(Resource::Work, 1)?;
        let guest = document
            .value
            .embeds
            .get(syntax.0 as usize)
            .ok_or(Error::Identity)?;
        let depth = base
            .checked_add(depth)
            .ok_or_else(|| budget.stop(StopReason::DepthLimit))?;
        let (sentence, value) = project_guest(&guest.closure, depth, host, budget)?;
        let mut target = None;
        for candidate in &prepared.identity().embeds {
            budget.charge(Resource::Work, 1)?;
            if candidate.embed == syntax {
                target = Some(candidate);
                break;
            }
        }
        let target = target.ok_or(Error::Identity)?;
        budget.charge(
            Resource::AllocationUnits,
            (core::mem::size_of::<(EmbedRef, SentenceSyntax)>()
                + core::mem::size_of::<text::ResolvedInlineText>()) as u64,
        )?;
        projection
            .sentences
            .try_reserve_exact(1)
            .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
        projection
            .resolved
            .try_reserve_exact(1)
            .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
        projection.resolved.push(text::ResolvedInlineText {
            document_digest: prepared.identity().document_digest,
            embed: syntax,
            guest_digest: target.guest_digest,
            text: value,
        });
        projection.sentences.push((syntax, sentence));
    }
    Ok(projection)
}

fn project_guest<C: FoundationValueCodec>(
    guest: &nepl3_core::syntax::ForeignClosureView<'_>,
    depth: u64,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    budget: &mut Budget,
) -> Result<(SentenceSyntax, String), Error<C::Error>> {
    budget.with_depth_at_least(depth, |budget| {
        let sentence = host.lower_view(guest, budget).map_err(Error::Selection)?;
        sentence
            .validate(host.registry, budget, host.codec.source_admission())
            .map_err(Error::Sentence)?;
        let selected = sentence_text::prepare(
            &sentence.value,
            host.registry,
            budget,
            host.codec.source_admission(),
        )
        .map_err(Error::Text)?;
        let value = selected
            .render(sentence_text::AnnotationPolicy::BaseOnly, &[], budget)
            .map_err(Error::Text)?;
        Ok((sentence, value))
    })
}
#[cfg(test)]
mod tests;
