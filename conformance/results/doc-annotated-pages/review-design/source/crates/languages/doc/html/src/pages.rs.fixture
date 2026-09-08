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
    model::LinkTarget,
    pages::{self, PageDestination, PageLinkPlan, PageSet},
    prepare,
};
use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode};

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

pub fn render_pages<'a, C: FoundationValueCodec>(
    request: &'a PagesHtmlRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RenderedPages, PagesRenderError<'a, C::Error>> {
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
                });
            }
        } else {
            unresolved = true;
        }
    }
    if unresolved {
        return Err(PagesRenderError::NeedsResolution(checked.into_plan()));
    }
    let mut fragments = Vec::new();
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
        for link in &plan.links {
            b.charge(Resource::Work, 1)?;
            if link.page != page as u64 {
                continue;
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
        }
        for pending in &plan.remaining {
            b.charge(Resource::Work, 1)?;
            if pending.page != page as u64 {
                continue;
            }
            if let prepare::DocRequirement::Link {
                node,
                target: LinkTarget::External { uri },
            } = &pending.requirement
            {
                let href = HtmlHref::External { uri: copy(uri, b)? };
                push(&mut links, (*node, href), b)?;
            }
        }
        let fragment =
            crate::build::render_prepared(&prepared, &links, b).map_err(|e| match e {
                RenderError::Stopped(s) => PagesRenderError::Stopped(s),
                e => PagesRenderError::Render(e),
            })?;
        push(&mut fragments, fragment, b)?;
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
                let mut found = false;
                for node in &fragments[index].markup.fragment.nodes {
                    b.charge(Resource::Work, 1)?;
                    if let HtmlNode::Element { attributes, .. } = node {
                        for a in attributes {
                            b.charge(Resource::Work, 1)?;
                            if let HtmlAttribute::Id { value } = a {
                                b.charge(Resource::Work, (id.len() + value.len()) as u64)?;
                                if id == value {
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }
                    if found {
                        break;
                    }
                }
                if !found {
                    // The renderer records origins in element order.
                    let cause = fragment
                        .origins
                        .get(element)
                        .ok_or(PagesRenderError::Render(RenderError::InternalShape))?;
                    return Err(PagesRenderError::MissingOutputAnchor {
                        page: page as u64,
                        node: cause.node,
                        target: index as u64,
                    });
                }
            }
        }
    }
    Ok(RenderedPages {
        identity: plan.identity,
        fragments,
    })
}
