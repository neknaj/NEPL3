//! Display preparation for explicitly adopted namespaces in a whole PageSet.
//! The caller owns Sentence selection; this module adds no language dependency.
use super::*;
use crate::namespace::display::PreparedDisplayNamespace;
use nepl3_doc_core::pages::{PageDestination, PageSet, namespace as routing};
use nepl3_doc_core::prepare::DocRequirement;

pub struct PreparedPages<'a> {
    set: &'a PageSet,
    pages: Vec<PreparedDisplayNamespace<'a>>,
}
impl<'a> PreparedPages<'a> {
    pub fn set(&self) -> &'a PageSet {
        self.set
    }
    pub fn page(&self, index: u64) -> Option<&PreparedDisplayNamespace<'a>> {
        self.pages.get(usize::try_from(index).ok()?)
    }
}
#[derive(Debug)]
pub enum PreparationError<'a, E> {
    Stopped(StopReason),
    Routing(routing::Error<'a, E>),
    Member {
        page: u64,
        member: MemberId,
        error: LocalPreparationError<'a, E>,
    },
    Unresolved {
        page: u64,
        member: MemberId,
        requirement: usize,
    },
    InternalShape,
}
impl<E> From<StopReason> for PreparationError<'_, E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Revalidate every adopted owner and resolve page links before exposing any
/// prepared page. Assets remain unresolved; safe external links use the shared
/// URI policy. Rendering still has to verify final cross-page output anchors.
pub fn prepare<'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    namespaces: &[&CheckedNamespace<'_, 'a>],
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedPages<'a>, PreparationError<'a, C::Error>> {
    budget.poll()?;
    let checked =
        routing::resolve(set, namespaces, registry, codec, budget).map_err(
            |error| match error {
                routing::Error::Stopped(reason) => PreparationError::Stopped(reason),
                error => PreparationError::Routing(error),
            },
        )?;
    let mut pages = Vec::new();
    let mut pending_links = checked.links();
    for (page, namespace) in namespaces.iter().enumerate() {
        let mut members = Vec::new();
        for (member, document) in namespace.documents().enumerate() {
            budget.charge(Resource::Work, 1)?;
            let member = MemberId(member as u64);
            let plan = checked
                .member_plan(page as u64, member)
                .ok_or(PreparationError::InternalShape)?;
            let mut links = Vec::new();
            while let Some((link, rest)) = pending_links.split_first() {
                budget.charge(Resource::Work, 1)?;
                if link.site.page != page as u64 || link.site.member != member {
                    break;
                }
                let target = match link.target {
                    PageDestination::Page { index } => {
                        &set.pages[index as usize].registration.route
                    }
                    PageDestination::File { index } => {
                        &set.files[index as usize].registration.route
                    }
                };
                build::push(
                    &mut links,
                    (
                        link.site.node,
                        HtmlHref::BetweenArtifacts {
                            source: build::copy(&set.pages[page].registration.route, budget)?,
                            target: build::copy(target, budget)?,
                            fragment: link
                                .fragment
                                .as_ref()
                                .map(|name| build::hex_id(name, budget))
                                .transpose()?,
                        },
                    ),
                    budget,
                )?;
                pending_links = rest;
            }
            for (index, requirement) in plan.requirements.iter().enumerate() {
                budget.charge(Resource::Work, 1)?;
                match requirement {
                    DocRequirement::Link {
                        target:
                            nepl3_doc_core::model::LinkTarget::Page { .. }
                            | nepl3_doc_core::model::LinkTarget::Relative { .. },
                        ..
                    } => continue,
                    DocRequirement::Foreign { kind, .. }
                        if crate::display::selected(*kind, true)
                            && (*kind != nepl3_doc_core::model::EmbedKind::Code
                                || member == MemberId(0)) =>
                    {
                        continue;
                    }
                    _ => {}
                }
                match crate::display::admit_external(requirement, &mut links, budget) {
                    Ok(true) => {}
                    Ok(false) => {
                        return Err(PreparationError::Unresolved {
                            page: page as u64,
                            member,
                            requirement: index,
                        });
                    }
                    Err(LocalPreparationError::Stopped(reason)) => {
                        return Err(PreparationError::Stopped(reason));
                    }
                    Err(error) => {
                        return Err(PreparationError::Member {
                            page: page as u64,
                            member,
                            error,
                        });
                    }
                }
            }
            let mut prepared =
                crate::prepare::prepare_rendering(document, options, plan.document_digest, budget)
                    .map_err(|error| match error {
                        LocalPreparationError::Stopped(reason) => PreparationError::Stopped(reason),
                        error => PreparationError::Member {
                            page: page as u64,
                            member,
                            error,
                        },
                    })?;
            prepared.external_links = links;
            build::push(&mut members, prepared, budget)?;
        }
        build::push(
            &mut pages,
            PreparedDisplayNamespace(PreparedNamespace { members }),
            budget,
        )?;
    }
    if !pending_links.is_empty() {
        return Err(PreparationError::InternalShape);
    }
    budget.poll()?;
    Ok(PreparedPages { set, pages })
}

#[derive(Debug)]
pub enum RenderError<E> {
    ScopeMismatch,
    Stopped(StopReason),
    Page {
        page: u64,
        error: E,
    },
    Document {
        page: u64,
        error: crate::RenderError,
    },
    Binding {
        page: u64,
    },
    CodeId {
        page: u64,
        element: u64,
    },
    Destination {
        page: u64,
        element: u64,
    },
    MissingAnchor {
        page: u64,
        element: u64,
        target: u64,
    },
}
impl<E> From<StopReason> for RenderError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Complete selected fragments, with per-page foreign placements retained.
/// This native result has no legacy aggregate identity or portable encoding.
pub struct RenderedPages {
    pub fragments: Vec<crate::RenderedInlineWithForeign>,
}
/// The host inserts selected Sentence parts using each page's prepared namespace.
/// Nothing is returned until all callbacks, complete markup checks, Code-ID
/// restrictions and cross-page emitted-anchor checks succeed.
enum AdapterError<E> {
    ScopeMismatch,
    Host(E),
    CodeId(u64),
    Stopped(StopReason),
}
pub fn render<E>(
    prepared: &PreparedPages<'_>,
    adapter: &mut impl FnMut(
        u64,
        &PreparedDisplayNamespace<'_>,
        &nepl3_doc_core::model::DocEmbedView<'_>,
        nepl3_doc_core::model::EmbedRef,
        &mut Budget,
    ) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<RenderedPages, RenderError<E>> {
    budget.poll()?;
    let limits = budget.limits();
    let depth = budget.current_depth();
    let mut fragments = Vec::new();
    for (page, namespace) in prepared.pages.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        let output = display::render_article(
            namespace,
            MemberId(0),
            &mut |view, index, b| {
                let adapter_limits = b.limits();
                let adapter_depth = b.current_depth();
                let output =
                    adapter(page as u64, namespace, view, index, b).map_err(AdapterError::Host);
                b.poll().map_err(AdapterError::Stopped)?;
                if b.limits() != adapter_limits || b.current_depth() != adapter_depth {
                    return Err(AdapterError::ScopeMismatch);
                }
                let output = output?;
                if view.kind == nepl3_doc_core::model::EmbedKind::Code {
                    for (element, node) in output.fragment.nodes.iter().enumerate() {
                        b.charge(Resource::Work, 1).map_err(AdapterError::Stopped)?;
                        if let HtmlNode::Element { attributes, .. } = node {
                            for attribute in attributes {
                                b.charge(Resource::Work, 1).map_err(AdapterError::Stopped)?;
                                if matches!(attribute, HtmlAttribute::Id { .. }) {
                                    return Err(AdapterError::CodeId(element as u64));
                                }
                            }
                        }
                    }
                }
                Ok(output)
            },
            budget,
        );
        budget.poll()?;
        let output = output.map_err(|error| match error {
            display::ArticleError::Member(_) | display::ArticleError::ExpectedArticle(_) => {
                RenderError::Binding { page: page as u64 }
            }
            display::ArticleError::Render(crate::ForeignRenderError::Render(
                crate::RenderError::Stopped(reason),
            )) => RenderError::Stopped(reason),
            display::ArticleError::Render(crate::ForeignRenderError::Render(error)) => {
                RenderError::Document {
                    page: page as u64,
                    error,
                }
            }
            display::ArticleError::Render(crate::ForeignRenderError::Foreign(
                AdapterError::ScopeMismatch,
            )) => RenderError::ScopeMismatch,
            display::ArticleError::Render(crate::ForeignRenderError::Foreign(
                AdapterError::Host(error),
            )) => RenderError::Page {
                page: page as u64,
                error,
            },
            display::ArticleError::Render(crate::ForeignRenderError::Foreign(
                AdapterError::CodeId(element),
            )) => RenderError::CodeId {
                page: page as u64,
                element,
            },
            display::ArticleError::Render(crate::ForeignRenderError::Foreign(
                AdapterError::Stopped(reason),
            )) => RenderError::Stopped(reason),
        })?;
        build::push(&mut fragments, output, budget)?;
    }
    let mut anchors = Vec::new();
    for _ in &fragments {
        build::push(&mut anchors, None, budget)?;
    }
    // Validate actual emitted hrefs, not merely the semantic link plan. IDs in
    // another page cannot satisfy a local reference and hidden sources add no
    // output obligations. Report output coordinates rather than fake Doc nodes.
    for (page, output) in fragments.iter().enumerate() {
        for (element, node) in output.fragment.markup.fragment.nodes.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            let HtmlNode::Element { attributes, .. } = node else {
                continue;
            };
            for attribute in attributes {
                budget.charge(Resource::Work, 1)?;
                if matches!(
                    attribute,
                    HtmlAttribute::Href {
                        value: HtmlHref::Artifact { .. }
                    }
                ) {
                    return Err(RenderError::Destination {
                        page: page as u64,
                        element: element as u64,
                    });
                }
                let HtmlAttribute::Href {
                    value:
                        HtmlHref::BetweenArtifacts {
                            source,
                            target,
                            fragment,
                        },
                } = attribute
                else {
                    continue;
                };
                budget.charge(
                    Resource::Work,
                    (source.len() + prepared.set.pages[page].registration.route.len()) as u64,
                )?;
                if source != &prepared.set.pages[page].registration.route {
                    return Err(RenderError::Destination {
                        page: page as u64,
                        element: element as u64,
                    });
                }
                let mut found = None;
                for (index, entry) in prepared.set.pages.iter().enumerate() {
                    budget.charge(
                        Resource::Work,
                        (target.len() + entry.registration.route.len()) as u64 + 1,
                    )?;
                    if target == &entry.registration.route {
                        found = Some(index);
                        break;
                    }
                }
                let Some(index) = found else {
                    let mut file = false;
                    for entry in &prepared.set.files {
                        budget.charge(
                            Resource::Work,
                            (target.len() + entry.registration.route.len()) as u64 + 1,
                        )?;
                        if target == &entry.registration.route {
                            file = true;
                            break;
                        }
                    }
                    if !file || fragment.is_some() {
                        return Err(RenderError::Destination {
                            page: page as u64,
                            element: element as u64,
                        });
                    }
                    continue;
                };
                let Some(id) = fragment else {
                    continue;
                };
                let ids = match &mut anchors[index] {
                    Some(ids) => ids,
                    slot @ None => slot.insert(crate::pages::anchors::OutputAnchors::collect(
                        &fragments[index].fragment.markup.fragment.nodes,
                        budget,
                    )?),
                };
                let exists = ids.contains(id, budget)?;
                if !exists {
                    return Err(RenderError::MissingAnchor {
                        page: page as u64,
                        element: element as u64,
                        target: index as u64,
                    });
                }
            }
        }
    }
    budget.poll()?;
    if budget.limits() != limits || budget.current_depth() != depth {
        return Err(RenderError::ScopeMismatch);
    }
    Ok(RenderedPages { fragments })
}
