//! Native host composition for retained Code guests. The adapter receives the
//! exact immutable closure; it must render its retained bytes, never evaluate it.
//! This staged route does not change local-only or portable prepare semantics.
use crate::*;
use nepl3_core::{budget::Budget, schema::SchemaRegistry, value_codec::FoundationValueCodec};
use nepl3_doc_core::model::*;

pub struct PreparedCodeArticle<'a>(pub(crate) crate::prepare::PreparedRendering<'a>);

pub fn prepare_code<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedCodeArticle<'a>, LocalPreparationError<'a, C::Error>> {
    crate::display::prepare_selected(document, options, registry, codec, budget, false)
        .map(PreparedCodeArticle)
}

/// Only phrasing markup is imported inside a local pre/code wrapper. Adapter
/// errors, including unavailable source or analysis stops, remain explicit.
pub fn render_code<E>(
    prepared: &PreparedCodeArticle<'_>,
    adapter: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<RenderedInlineWithForeign, ForeignRenderError<E>> {
    crate::build::render_prepared_with_foreign(&prepared.0, &[], adapter, budget, true)
}
