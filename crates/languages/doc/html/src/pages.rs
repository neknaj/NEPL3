//! Whole-page-set HTML rendering. Every referenced anchor must survive output
//! selection; a semantic label alone does not prove the emitted destination.
use crate::build::{copy, hex_id, push};
use crate::*;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    model::{DocEmbed, EmbedRef, LinkTarget},
    pages::{self, PageDestination, PageLinkPlan, PageSet},
    prepare,
};
use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode};

mod anchors;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagesHtmlRequest {
    pub set: PageSet,
    pub options: RenderOptions,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedPages {
    pub identity: Digest,
    pub fragments: Vec<RenderedFragment>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum PagesRenderError<'a, E> {
    Stopped(StopReason),
    Input(pages::PageError<'a, E>),
    NeedsResolution(PageLinkPlan),
    Preparation(LocalPreparationError<'a, E>),
    Render(RenderError),
    MissingOutputAnchor { page: u64, node: u64, target: u64 },
    InvalidExternalUri { page: u64, node: u64 },
}
impl<E> From<StopReason> for PagesRenderError<'_, E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}

/// Native Code-aware rendering retains each imported occurrence separately.
/// This does not add a portable operation or accept a serialized render proof.
#[derive(Debug)]
pub struct RenderedCodePages {
    pub pages: RenderedPages,
    pub foreign: Vec<Vec<ForeignPlacement>>,
}
#[derive(Debug)]
pub enum PagesCodeRenderError<'a, E, F> {
    Pages(PagesRenderError<'a, E>),
    Foreign {
        page: u64,
        error: ForeignRenderError<F>,
    },
    /// Highlight decoration cannot define semantic Doc anchors. Qualified
    /// presentation identities belong in DataId rather than DOM Id.
    CodeId {
        page: u64,
        node: u64,
    },
}
impl<E, F> From<StopReason> for PagesCodeRenderError<'_, E, F> {
    fn from(reason: StopReason) -> Self {
        Self::Pages(PagesRenderError::Stopped(reason))
    }
}
impl<'a, E, F> From<PagesRenderError<'a, E>> for PagesCodeRenderError<'a, E, F> {
    fn from(error: PagesRenderError<'a, E>) -> Self {
        Self::Pages(error)
    }
}

pub fn render_pages<'a, C: FoundationValueCodec>(
    request: &'a PagesHtmlRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RenderedPages, PagesRenderError<'a, C::Error>> {
    type NoAdapter<C> =
        fn(u64, &DocEmbed, EmbedRef, &mut C, &mut Budget) -> Result<HtmlRequest, core::convert::Infallible>;
    match render_pages_impl::<C, core::convert::Infallible, NoAdapter<C>>(request, r, c, b, None, false) {
        Ok(result) => Ok(result.pages),
        Err(PagesCodeRenderError::Pages(error)) => Err(error),
        Err(PagesCodeRenderError::CodeId { .. }) => {
            Err(PagesRenderError::Render(RenderError::InternalShape))
        }
        Err(PagesCodeRenderError::Foreign { error, .. }) => match error {
            ForeignRenderError::Render(error) => Err(PagesRenderError::Render(error)),
            ForeignRenderError::Foreign(never) => match never {},
        },
    }
}

/// Explicit native host composition. The adapter receives the exact immutable
/// page's Code embed and must display its retained bytes without evaluation.
/// All page links are resolved before rendering; other guest kinds stay pending.
/// Code markup must not carry DOM Id attributes: they cannot satisfy Doc links
/// or local References when the real target is hidden by language selection.
pub fn render_pages_with_code<'a, C: FoundationValueCodec, F>(
    request: &'a PagesHtmlRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
    adapter: &mut impl FnMut(u64, &DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, F>,
) -> Result<RenderedCodePages, PagesCodeRenderError<'a, C::Error, F>> {
    render_pages_impl(request, r, c, b, Some(&mut |page, embed, index, _: &mut C, b: &mut Budget| adapter(page, embed, index, b)), false)
}

/// Selected Code/Math composition shares the operation's source-admission codec
/// with the host adapter. Math-local IDs are scoped by output occurrence.
pub fn render_pages_with_display<'a, C: FoundationValueCodec, F>(
 request: &'a PagesHtmlRequest, r: &SchemaRegistry, c: &mut C, b: &mut Budget,
 adapter: &mut impl FnMut(u64, &DocEmbed, EmbedRef, &mut C, &mut Budget) -> Result<HtmlRequest, F>,
) -> Result<RenderedCodePages, PagesCodeRenderError<'a, C::Error, F>> {
 render_pages_impl(request, r, c, b, Some(adapter), true)
}

enum CodeAdapterError<E> {
    Host(E),
    Id(u64),
    Stopped(StopReason),
}

fn render_pages_impl<'a, C: FoundationValueCodec, F, A>(
    request: &'a PagesHtmlRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
    mut adapter: Option<&mut A>,
    math: bool,
) -> Result<RenderedCodePages, PagesCodeRenderError<'a, C::Error, F>>
where
    A: FnMut(u64, &DocEmbed, EmbedRef, &mut C, &mut Budget) -> Result<HtmlRequest, F>,
{
    let checked = pages::resolve(&request.set, r, c, b).map_err(|e| match e {
        pages::PageError::Stopped(s) => PagesRenderError::Stopped(s),
        e => PagesRenderError::Input(e),
    })?;
    let plan = checked.plan();
    let mut unresolved = false;
    for pending in &plan.remaining {
        b.charge(Resource::Work, 1)?;
        if let prepare::DocRequirement::Link {
            node,
            target: LinkTarget::External { uri },
        } = &pending.requirement
        {
            if !nepl3_markup::html::external_uri(uri, b)? {
                return Err(PagesRenderError::InvalidExternalUri {
                    page: pending.page,
                    node: *node,
                }
                .into());
            }
        } else if !(adapter.is_some()
            && matches!(
                pending.requirement,
                prepare::DocRequirement::Foreign {
                    kind,
                    ..
                } if crate::display::selected(kind, math)
            ))
        {
            unresolved = true;
        }
    }
    if unresolved {
        return Err(PagesRenderError::NeedsResolution(checked.into_plan()).into());
    }
    let mut fragments = Vec::new();
    let mut foreign = Vec::new();
    let mut page_links = plan.links.as_slice();
    let mut page_pending = plan.remaining.as_slice();
    for (page, input) in request.set.pages.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let document_digest = checked
            .document_digest(page as u64)
            .ok_or(PagesRenderError::Render(RenderError::InternalShape))?;
        let prepared = crate::prepare::prepare_rendering(
            &input.document,
            &request.options,
            document_digest,
            b,
        )
        .map_err(|e| match e {
            LocalPreparationError::Stopped(s) => PagesRenderError::Stopped(s),
            e => PagesRenderError::Preparation(e),
        })?;
        let mut links = Vec::new();
        while let Some((link, rest)) = page_links.split_first() {
            b.charge(Resource::Work, 1)?;
            if link.page != page as u64 {
                break;
            }
            let href = HtmlHref::BetweenArtifacts {
                source: copy(&input.registration.route, b)?,
                target: copy(
                    match link.target {
                        PageDestination::Page { index } => {
                            &request.set.pages[index as usize].registration.route
                        }
                        PageDestination::File { index } => {
                            &request.set.files[index as usize].registration.route
                        }
                    },
                    b,
                )?,
                fragment: link.fragment.as_ref().map(|s| hex_id(s, b)).transpose()?,
            };
            push(&mut links, (link.node, href), b)?;
            page_links = rest;
        }
        while let Some((pending, rest)) = page_pending.split_first() {
            b.charge(Resource::Work, 1)?;
            if pending.page != page as u64 {
                break;
            }
            if let prepare::DocRequirement::Link {
                node,
                target: LinkTarget::External { uri },
            } = &pending.requirement
            {
                let href = HtmlHref::External { uri: copy(uri, b)? };
                push(&mut links, (*node, href), b)?;
            }
            page_pending = rest;
        }
        let fragment = if let Some(adapter) = adapter.as_mut() {
            let rendered = crate::build::render_prepared_with_foreign(
                &prepared,
                &links,
                &mut |embed, index, b| {
                    let markup =
                        adapter(page as u64, embed, index, c, b).map_err(CodeAdapterError::Host)?;
                    for (node, value) in markup.fragment.nodes.iter().enumerate() {
                        b.charge(Resource::Work, 1)
                            .map_err(CodeAdapterError::Stopped)?;
                        if let HtmlNode::Element { attributes, .. } = value {
                            for attribute in attributes {
                                b.charge(Resource::Work, 1)
                                    .map_err(CodeAdapterError::Stopped)?;
                                if embed.kind == nepl3_doc_core::model::EmbedKind::Code && matches!(attribute, HtmlAttribute::Id { .. }) {
                                    return Err(CodeAdapterError::Id(node as u64));
                                }
                            }
                        }
                    }
                    Ok(markup)
                },
                b,
                true,
            );
            // An adapter may report its own error after stopping the shared
            // budget. The formal stop takes precedence over that host error.
            b.poll()?;
            let rendered = rendered.map_err(|error| match error {
                ForeignRenderError::Foreign(CodeAdapterError::Id(node)) => {
                    PagesCodeRenderError::CodeId {
                        page: page as u64,
                        node,
                    }
                }
                ForeignRenderError::Foreign(CodeAdapterError::Stopped(reason)) => {
                    PagesCodeRenderError::from(reason)
                }
                ForeignRenderError::Foreign(CodeAdapterError::Host(error)) => {
                    PagesCodeRenderError::Foreign {
                        page: page as u64,
                        error: ForeignRenderError::Foreign(error),
                    }
                }
                ForeignRenderError::Render(error) => PagesCodeRenderError::Foreign {
                    page: page as u64,
                    error: ForeignRenderError::Render(error),
                },
            })?;
            push(&mut foreign, rendered.foreign, b)?;
            rendered.fragment
        } else {
            crate::build::render_prepared(&prepared, &links, b).map_err(|e| match e {
                RenderError::Stopped(s) => PagesRenderError::Stopped(s),
                e => PagesRenderError::Render(e),
            })?
        };
        push(&mut fragments, fragment, b)?;
    }
    if !page_links.is_empty() || !page_pending.is_empty() {
        return Err(PagesRenderError::Render(RenderError::InternalShape).into());
    }
    // Borrow emitted IDs on the first incoming fragment link. Pages without
    // such links need no collection; semantic labels remain insufficient.
    let mut anchors = Vec::new();
    for _ in &fragments {
        push(&mut anchors, None, b)?;
    }
    // Reject a link whose semantic destination was hidden by language selection.
    // Check emitted hrefs only: a hidden source occurrence is not a broken link.
    for (page, fragment) in fragments.iter().enumerate() {
        for (element, node) in fragment.markup.fragment.nodes.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let HtmlNode::Element { attributes, .. } = node else {
                continue;
            };
            for attr in attributes {
                b.charge(Resource::Work, 1)?;
                let HtmlAttribute::Href {
                    value:
                        HtmlHref::BetweenArtifacts {
                            target,
                            fragment: Some(id),
                            ..
                        },
                } = attr
                else {
                    continue;
                };
                let mut target_index = None;
                for (i, input) in request.set.pages.iter().enumerate() {
                    b.charge(
                        Resource::Work,
                        (target.len() + input.registration.route.len()) as u64 + 1,
                    )?;
                    if &input.registration.route == target {
                        target_index = Some(i);
                        break;
                    }
                }
                let index =
                    target_index.ok_or(PagesRenderError::Render(RenderError::InternalShape))?;
                let ids = match &mut anchors[index] {
                    Some(ids) => ids,
                    slot @ None => slot.insert(anchors::OutputAnchors::collect(
                        &fragments[index].markup.fragment.nodes,
                        b,
                    )?),
                };
                if !ids.contains(id, b)? {
                    // The renderer records origins in element order.
                    let cause = fragment
                        .origins
                        .get(element)
                        .ok_or(PagesRenderError::Render(RenderError::InternalShape))?;
                    return Err(PagesRenderError::MissingOutputAnchor {
                        page: page as u64,
                        node: cause.node,
                        target: index as u64,
                    }
                    .into());
                }
            }
        }
    }
    Ok(RenderedCodePages {
        pages: RenderedPages {
            identity: plan.identity,
            fragments,
        },
        foreign,
    })
}
