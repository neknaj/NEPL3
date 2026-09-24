//! Validate completed HTML against the registered output routes. This proof
//! covers HTML and registered BetweenArtifacts routes and anchors. Other asset
//! destinations and derivation from language inputs require separate host
//! checks, including guest semantics and portable result equivalence.
use super::PreparedPages;
use crate::build::push;
use crate::pages::anchors::OutputAnchors;
use alloc::vec::Vec;
use core::borrow::Borrow;
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_core::source::Digest;
use nepl3_markup::html::{
    HtmlAttribute, HtmlError, HtmlHref, HtmlNode, HtmlRequest, ValidatedHtml, validate,
};

pub struct CheckedOutput<'a> {
    namespace_identity: Digest,
    pages: Vec<ValidatedHtml<'a>>,
}
impl<'a> CheckedOutput<'a> {
    pub fn namespace_identity(&self) -> Digest {
        self.namespace_identity
    }
    pub fn pages(&self) -> &[ValidatedHtml<'a>] {
        &self.pages
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    PageCount,
    Html {
        page: u64,
        error: HtmlError,
    },
    SourceRoute {
        page: u64,
        element: u64,
    },
    TargetRoute {
        page: u64,
        element: u64,
    },
    FileFragment {
        page: u64,
        element: u64,
    },
    MissingAnchor {
        page: u64,
        element: u64,
        target: u64,
    },
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Requests must follow the registered page order. Error coordinates belong to
/// the completed HTML arena; the composing host owns their source mapping.
/// No request is modified, and failure exposes no partially checked page set.
pub fn check<'a>(
    prepared: &PreparedPages<'_, '_, '_, '_>,
    requests: &'a [impl Borrow<HtmlRequest>],
    b: &mut Budget,
) -> Result<CheckedOutput<'a>, Error> {
    b.poll()?;
    let set = prepared.checked().set();
    if requests.len() != set.pages.len() {
        return Err(Error::PageCount);
    }
    let mut pages = Vec::new();
    let mut anchors = Vec::new();
    for (page, request) in requests.iter().enumerate() {
        let request = request.borrow();
        let checked =
            validate(&request.fragment, request.slot, &request.policy, b).map_err(|error| {
                match error {
                    HtmlError::Stopped(reason) => Error::Stopped(reason),
                    error => Error::Html {
                        page: page as u64,
                        error,
                    },
                }
            })?;
        push(&mut pages, checked, b)?;
        push(&mut anchors, None, b)?;
    }
    // Borrow each request exactly once above. A custom Borrow implementation
    // may select a different arena on another call; the retained validation
    // proof is the sole input to route and anchor checks.
    for (page, request) in pages.iter().enumerate() {
        for (element, node) in request.fragment().nodes.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let HtmlNode::Element { attributes, .. } = node else {
                continue;
            };
            for attribute in attributes {
                b.charge(Resource::Work, 1)?;
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
                let page_id = page as u64;
                let element = element as u64;
                if !equal(source, &set.pages[page].registration.route, b)? {
                    return Err(Error::SourceRoute {
                        page: page_id,
                        element,
                    });
                }
                let mut destination = None;
                for (index, input) in set.pages.iter().enumerate() {
                    if equal(target, &input.registration.route, b)? {
                        destination = Some(index);
                        break;
                    }
                }
                if let Some(index) = destination {
                    if let Some(id) = fragment {
                        let ids = match &mut anchors[index] {
                            Some(ids) => ids,
                            slot @ None => slot
                                .insert(OutputAnchors::collect(&pages[index].fragment().nodes, b)?),
                        };
                        if !ids.contains(id, b)? {
                            return Err(Error::MissingAnchor {
                                page: page_id,
                                element,
                                target: index as u64,
                            });
                        }
                    }
                    continue;
                }
                let mut file = false;
                for input in &set.files {
                    if equal(target, &input.registration.route, b)? {
                        file = true;
                        break;
                    }
                }
                if !file {
                    return Err(Error::TargetRoute {
                        page: page_id,
                        element,
                    });
                }
                if fragment.is_some() {
                    return Err(Error::FileFragment {
                        page: page_id,
                        element,
                    });
                }
            }
        }
    }
    b.poll()?;
    Ok(CheckedOutput {
        namespace_identity: prepared.checked().identity(),
        pages,
    })
}

fn equal(left: &str, right: &str, b: &mut Budget) -> Result<bool, StopReason> {
    b.charge(Resource::Work, left.len() as u64)?;
    b.charge(Resource::Work, right.len() as u64)?;
    b.charge(Resource::Work, 1)?;
    Ok(left == right)
}
