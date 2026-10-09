//! Explicit native Article composition for Code and inline/display Math guests.
//! The host selects actual renderers. This neither evaluates guests nor admits
//! arbitrary markup without the existing structural and final HTML checks.
use crate::*;
use nepl3_core::{budget::Budget, schema::SchemaRegistry, value_codec::FoundationValueCodec};
use nepl3_doc_core::{model::*, prepare};

pub struct PreparedArticle<'a>(pub(crate) crate::prepare::PreparedRendering<'a>);

/// Admit only the selected Code/Math guest requirements. External links/assets
/// and all other unresolved requirements remain explicit failures.
pub fn prepare<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedArticle<'a>, LocalPreparationError<'a, C::Error>> {
    let plan =
        prepare::inspect(document, registry, codec, budget).map_err(|error| match error {
            prepare::PreparationError::Stopped(reason) => LocalPreparationError::Stopped(reason),
            other => LocalPreparationError::Input(other),
        })?;
    for requirement in &plan.requirements {
        budget.charge(nepl3_core::budget::Resource::Work, 1)?;
        if !matches!(
            requirement,
            prepare::DocRequirement::Foreign {
                kind: EmbedKind::Code | EmbedKind::InlineMath | EmbedKind::DisplayMath,
                ..
            }
        ) {
            return Err(LocalPreparationError::NeedsResolution(plan));
        }
    }
    crate::prepare::prepare_rendering(document, options, plan.document_digest, budget)
        .map(PreparedArticle)
}

/// The immutable embed kind specifies inline versus display mode. Display Math
/// is checked in a block slot; inline Math and Code require phrasing content.
/// No existing local-only or Code-only preparation route is widened.
pub fn render<E>(
    prepared: &PreparedArticle<'_>,
    adapter: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<RenderedInlineWithForeign, ForeignRenderError<E>> {
    crate::build::render_prepared_with_foreign(&prepared.0, &[], adapter, budget, true)
}

mod context;
pub use context::Context;

/// Render with the exact prepared document/node/occurrence context. This is a
/// native host boundary, not a portable identity or artifact admission receipt.
/// Existing render uses the same traversal and retains its original interface.
pub fn render_with_context<'a, E>(
    prepared: &PreparedArticle<'a>,
    adapter: &mut impl FnMut(Context<'a>, &mut Budget) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<RenderedInlineWithForeign, ForeignRenderError<E>> {
    crate::build::render_prepared_with_context(&prepared.0, &[], adapter, budget, true)
}
