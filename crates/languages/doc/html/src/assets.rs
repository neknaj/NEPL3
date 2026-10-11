//! Native prototype: explicit static SVG inputs, validated under this operation.
//! These raw inputs are not portable proofs; local-only prepare remains unchanged.
use crate::*;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{model::*, prepare, text};
use nepl3_markup::html::{HtmlAttribute, svg};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SvgMode {
    External,
    Embedded,
}
pub struct SvgInput<'a> {
    pub id: &'a str,
    pub svg: &'a str,
}
pub struct PreparedSvgArticle<'a>(pub(crate) crate::prepare::PreparedRendering<'a>);
#[derive(Debug)]
pub enum AssetError<'a, E> {
    Preparation(LocalPreparationError<'a, E>),
    Text(nepl3_doc_core::portable::PortableError<E>),
    Stopped(StopReason),
    Duplicate,
    Unused,
    Missing,
    InvalidSvg,
    Digest,
    Alt,
    Unsupported,
}
impl<E> From<StopReason> for AssetError<'_, E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
pub fn prepare_svg<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    inputs: &[SvgInput<'_>],
    mode: SvgMode,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedSvgArticle<'a>, AssetError<'a, C::Error>> {
    prepare_svg_impl(
        document, options, inputs, mode, registry, codec, budget, None,
    )
    .map(PreparedSvgArticle)
}

/// Local composition with retained Code guests. Code output remains an explicit
/// host callback at render time; other foreign requirements stay unresolved.
pub struct PreparedSvgCodeArticle<'a>(pub(crate) crate::prepare::PreparedRendering<'a>);

pub fn prepare_svg_code<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    inputs: &[SvgInput<'_>],
    mode: SvgMode,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedSvgCodeArticle<'a>, AssetError<'a, C::Error>> {
    prepare_svg_impl(
        document, options, inputs, mode, registry, codec, budget, Some(false),
    )
    .map(PreparedSvgCodeArticle)
}

/// Static SVG assets composed with selected Code and Math display adapters.
pub struct PreparedSvgDisplayArticle<'a>(pub(crate) crate::prepare::PreparedRendering<'a>);
pub fn prepare_svg_display<'a, C: FoundationValueCodec>(
 document: &'a DocumentSyntax, options: &'a RenderOptions, inputs: &[SvgInput<'_>],
 mode: SvgMode, registry: &SchemaRegistry, codec: &mut C, budget: &mut Budget,
) -> Result<PreparedSvgDisplayArticle<'a>, AssetError<'a, C::Error>> {
 prepare_svg_impl(document, options, inputs, mode, registry, codec, budget, Some(true)).map(PreparedSvgDisplayArticle)
}
pub fn render_svg_display<E>(prepared: &PreparedSvgDisplayArticle<'_>,
 adapter: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>, budget: &mut Budget,
) -> Result<RenderedInlineWithForeign, ForeignRenderError<E>> {
 crate::build::render_prepared_with_foreign(&prepared.0, &[], adapter, budget, true)
}

#[allow(clippy::too_many_arguments)]
fn prepare_svg_impl<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    inputs: &[SvgInput<'_>],
    mode: SvgMode,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    display: Option<bool>,
) -> Result<crate::prepare::PreparedRendering<'a>, AssetError<'a, C::Error>> {
    let plan = prepare::inspect(document, registry, codec, budget)
        .map_err(|e| AssetError::Preparation(LocalPreparationError::Input(e)))?;
    for (i, input) in inputs.iter().enumerate() {
        budget.charge(Resource::SourceBytes, input.svg.len() as u64)?;
        for prior in &inputs[..i] {
            budget.charge(Resource::Work, (input.id.len() + prior.id.len()) as u64)?;
            if input.id == prior.id {
                return Err(AssetError::Duplicate);
            }
        }
        if !svg::validate(input.svg, budget)? {
            return Err(AssetError::InvalidSvg);
        }
        let mut used = false;
        for req in &plan.requirements {
            budget.charge(Resource::Work, input.id.len() as u64 + 1)?;
            if let prepare::DocRequirement::Asset { asset, .. } = req {
                used |= asset.id == input.id;
            }
        }
        if !used {
            return Err(AssetError::Unused);
        }
    }
    let mut images = Vec::new();
    for req in &plan.requirements {
        budget.charge(Resource::Work, 1)?;
        if matches!(req, prepare::DocRequirement::Foreign { kind, .. } if display.is_some_and(|math| crate::display::selected(*kind, math)))
        {
            continue;
        }
        let prepare::DocRequirement::Asset { node, asset } = req else {
            return Err(AssetError::Unsupported);
        };
        let mut found = None;
        for input in inputs {
            budget.charge(Resource::Work, (asset.id.len() + input.id.len()) as u64)?;
            if asset.id == input.id {
                found = Some(input);
                break;
            }
        }
        let input = found.ok_or(AssetError::Missing)?;
        budget.charge(Resource::Work, input.svg.len() as u64)?;
        let digest = Digest::of(input.svg.as_bytes());
        if asset.digest.is_some_and(|d| d != digest) {
            return Err(AssetError::Digest);
        }
        let alt = match &document.value.nodes[*node as usize].kind {
            DocKind::Image { alt, .. } | DocKind::InlineImage { alt, .. } => *alt,
            _ => return Err(AssetError::Unsupported),
        };
        let alt = match text::plain_text_borrowed(
            document,
            alt,
            text::AnnotationPolicy::BaseOnly,
            &[],
            registry,
            codec,
            budget,
        )
        .map_err(AssetError::Text)?
        .outcome
        {
            text::PlainTextOutcome::Complete { text } => text,
            text::PlainTextOutcome::Stopped { reason } => return Err(AssetError::Stopped(reason)),
            _ => return Err(AssetError::Alt),
        };
        let src = match mode {
            SvgMode::Embedded => HtmlAttribute::EmbeddedSvg {
                svg: crate::build::copy(input.svg, budget)?,
            },
            SvgMode::External => {
                budget.charge(Resource::AllocationUnits, 75)?;
                let mut path = String::from("assets/");
                for c in digest.0 {
                    path.push(b"0123456789abcdef"[(c >> 4) as usize] as char);
                    path.push(b"0123456789abcdef"[(c & 15) as usize] as char);
                }
                path.push_str(".svg");
                HtmlAttribute::Src { path }
            }
        };
        crate::build::push(&mut images, (*node, src, alt), budget)?;
    }
    let mut prepared =
        crate::prepare::prepare_rendering(document, options, plan.document_digest, budget)
            .map_err(AssetError::Preparation)?;
    prepared.images = images;
    Ok(prepared)
}
pub fn render_svg(
    prepared: &PreparedSvgArticle<'_>,
    budget: &mut Budget,
) -> Result<RenderedFragment, RenderError> {
    crate::build::render_prepared(&prepared.0, &[], budget)
}

pub fn render_svg_code<E>(
    prepared: &PreparedSvgCodeArticle<'_>,
    adapter: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<RenderedInlineWithForeign, ForeignRenderError<E>> {
    crate::build::render_prepared_with_foreign(&prepared.0, &[], adapter, budget, true)
}
