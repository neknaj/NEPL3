//! Native composition of retained Code and Math display guests. Selected host
//! adapters receive exact immutable closures; no guest is evaluated implicitly.
use crate::*;
use nepl3_core::{budget::Budget, schema::SchemaRegistry, value_codec::FoundationValueCodec};
use nepl3_doc_core::{model::*, prepare};
pub struct PreparedDisplayArticle<'a>(pub(crate) crate::prepare::PreparedRendering<'a>);
pub fn prepare_display<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedDisplayArticle<'a>, LocalPreparationError<'a, C::Error>> {
    prepare_selected(document, options, registry, codec, budget, true).map(PreparedDisplayArticle)
}
pub(crate) fn prepare_selected<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    math: bool,
) -> Result<crate::prepare::PreparedRendering<'a>, LocalPreparationError<'a, C::Error>> {
    let plan =
        prepare::inspect(document, registry, codec, budget).map_err(|error| match error {
            prepare::PreparationError::Stopped(reason) => LocalPreparationError::Stopped(reason),
            other => LocalPreparationError::Input(other),
        })?;
    let mut links = Vec::new();
    for requirement in &plan.requirements {
        budget.charge(nepl3_core::budget::Resource::Work, 1)?;
        if math && admit_external(requirement, &mut links, budget)? {
            continue;
        }
        if !matches!(requirement, prepare::DocRequirement::Foreign { kind, .. } if selected(*kind, math))
        {
            return Err(LocalPreparationError::NeedsResolution(plan));
        }
    }
    let mut prepared =
        crate::prepare::prepare_rendering(document, options, plan.document_digest, budget)?;
    prepared.external_links = links;
    Ok(prepared)
}
pub fn render_display<E>(
    prepared: &PreparedDisplayArticle<'_>,
    adapter: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<RenderedInlineWithForeign, ForeignRenderError<E>> {
    crate::build::render_prepared_with_foreign(
        &prepared.0,
        &prepared.0.external_links,
        adapter,
        budget,
        true,
    )
}
pub(crate) fn selected(kind: EmbedKind, math: bool) -> bool {
    kind == EmbedKind::Code
        || math && matches!(kind, EmbedKind::InlineMath | EmbedKind::DisplayMath)
}

/// External URLs select no external data: admit only the shared safe URI policy.
/// The node binding belongs to the immutable document inspected by this call.
pub(crate) fn admit_external<'a, E>(
    requirement: &prepare::DocRequirement,
    links: &mut Vec<(u64, nepl3_markup::html::HtmlHref)>,
    budget: &mut Budget,
) -> Result<bool, LocalPreparationError<'a, E>> {
    let prepare::DocRequirement::Link {
        node,
        target: LinkTarget::External { uri },
    } = requirement
    else {
        return Ok(false);
    };
    if !nepl3_markup::html::external_uri(uri, budget)? {
        return Err(LocalPreparationError::InvalidExternalUri { node: *node });
    }
    let href = nepl3_markup::html::HtmlHref::External {
        uri: crate::build::copy(uri, budget)?,
    };
    crate::build::push(links, (*node, href), budget)?;
    Ok(true)
}
