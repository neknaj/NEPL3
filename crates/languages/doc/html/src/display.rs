//! Native composition of retained Code and Math display guests. Selected host
//! adapters receive exact immutable closures; no guest is evaluated implicitly.
use crate::*;
use nepl3_core::{budget::Budget, schema::SchemaRegistry, value_codec::FoundationValueCodec};
use nepl3_doc_core::{model::*, prepare};
pub struct PreparedDisplayArticle<'a>(pub(crate) crate::prepare::PreparedRendering<'a>);
pub fn prepare_display<'a, C: FoundationValueCodec>(
 document: &'a DocumentSyntax, options: &'a RenderOptions, registry: &SchemaRegistry,
 codec: &mut C, budget: &mut Budget,
) -> Result<PreparedDisplayArticle<'a>, LocalPreparationError<'a, C::Error>> {
 prepare_selected(document, options, registry, codec, budget, true).map(PreparedDisplayArticle)
}
pub(crate) fn prepare_selected<'a, C: FoundationValueCodec>(
 document: &'a DocumentSyntax, options: &'a RenderOptions, registry: &SchemaRegistry,
 codec: &mut C, budget: &mut Budget, math: bool,
) -> Result<crate::prepare::PreparedRendering<'a>, LocalPreparationError<'a, C::Error>> {
    let plan =
        prepare::inspect(document, registry, codec, budget).map_err(|error| match error {
            prepare::PreparationError::Stopped(reason) => LocalPreparationError::Stopped(reason),
            other => LocalPreparationError::Input(other),
        })?;
    for requirement in &plan.requirements {
        budget.charge(nepl3_core::budget::Resource::Work, 1)?;
        if !matches!(requirement, prepare::DocRequirement::Foreign { kind, .. } if selected(*kind, math)) {
            return Err(LocalPreparationError::NeedsResolution(plan));
        }
    }
    crate::prepare::prepare_rendering(document, options, plan.document_digest, budget)
}
pub fn render_display<E>(prepared: &PreparedDisplayArticle<'_>,
 adapter: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>, budget: &mut Budget,
) -> Result<RenderedInlineWithForeign, ForeignRenderError<E>> {
 crate::build::render_prepared_with_foreign(&prepared.0, &[], adapter, budget, true)
}
pub(crate) fn selected(kind: EmbedKind, math: bool) -> bool {
 kind == EmbedKind::Code || math && matches!(kind, EmbedKind::InlineMath | EmbedKind::DisplayMath)
}
