//! Page-set link resolution. This proof covers semantic page/label existence,
//! not asset/guest preparation or the availability of a selected HTML anchor.
use crate::{
    labels,
    model::*,
    portable,
    prepare::{self, DocRequirement, PreparationError},
};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::Digest,
    value_codec::{FoundationCodecError, FoundationValueCodec},
};

pub const SET_DOMAIN: &[u8] = b"NEPL3.Doc.Pages.v1\0";
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageRegistration {
    pub id: String,
    pub source: String,
    pub route: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageDocument {
    pub registration: PageRegistration,
    pub document: DocumentSyntax,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageSet {
    pub pages: Vec<PageDocument>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageLink {
    pub page: u64,
    pub node: u64,
    pub target: u64,
    pub fragment: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageRequirement {
    pub page: u64,
    pub requirement: DocRequirement,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageLinkPlan {
    pub identity: Digest,
    pub links: Vec<PageLink>,
    pub remaining: Vec<PageRequirement>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageField {
    Id,
    Source,
    Route,
}
#[derive(Debug, Eq, PartialEq)]
pub enum PageError<'a, E> {
    Stopped(StopReason),
    Boundary(portable::PortableError<E>),
    Input {
        page: u64,
        error: PreparationError<'a, E>,
    },
    Empty,
    InvalidRegistration {
        page: u64,
        field: PageField,
    },
    Collision {
        page: u64,
        previous: u64,
        field: PageField,
    },
    InvalidRelative {
        page: u64,
        node: u64,
    },
    MissingPage {
        page: u64,
        node: u64,
    },
    MissingFragment {
        page: u64,
        node: u64,
        target: u64,
    },
}
impl<E> From<StopReason> for PageError<'_, E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl<E> From<portable::PortableError<E>> for PageError<'_, E> {
    fn from(e: portable::PortableError<E>) -> Self {
        match e {
            portable::PortableError::Stopped(s) => Self::Stopped(s),
            e => Self::Boundary(e),
        }
    }
}
pub struct CheckedPages<'a> {
    set: &'a PageSet,
    plan: PageLinkPlan,
    document_digests: Vec<Digest>,
}
impl<'a> CheckedPages<'a> {
    pub fn set(&self) -> &'a PageSet {
        self.set
    }
    pub fn plan(&self) -> &PageLinkPlan {
        &self.plan
    }
    pub fn into_plan(self) -> PageLinkPlan {
        self.plan
    }
    /// Digest of this exact borrowed set's document, in page order. This native
    /// proof is constructed by resolve, never by decoding a supplied link plan.
    pub fn document_digest(&self, page: u64) -> Option<Digest> {
        usize::try_from(page)
            .ok()
            .and_then(|p| self.document_digests.get(p))
            .copied()
    }
}
pub(crate) fn push<T>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    if v.len() == v.capacity() {
        let capacity = v.capacity().saturating_mul(2).max(1);
        let bytes = capacity
            .checked_mul(core::mem::size_of::<T>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        v.reserve_exact(capacity - v.len());
    }
    v.push(item);
    Ok(())
}
fn copy(s: &str, b: &mut Budget) -> Result<String, StopReason> {
    b.charge(Resource::Work, s.len() as u64)?;
    b.charge(Resource::AllocationUnits, s.len() as u64)?;
    Ok(s.into())
}
fn segment(s: &str, ascii: bool) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.chars().all(|c| {
            if ascii {
                c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')
            } else {
                !c.is_control() && !matches!(c, '/' | '\\' | ':' | '?' | '#' | '%')
            }
        })
}
fn path(s: &str, ascii: bool) -> bool {
    s.split('/').all(|s| segment(s, ascii))
}
fn overlaps(a: &str, b: &str) -> bool {
    a == b
        || a.strip_prefix(b).is_some_and(|s| s.starts_with('/'))
        || b.strip_prefix(a).is_some_and(|s| s.starts_with('/'))
}
pub(crate) fn registrations<E>(set: &PageSet, b: &mut Budget) -> Result<(), PageError<'static, E>> {
    b.poll()?;
    if set.pages.is_empty() {
        return Err(PageError::Empty);
    }
    for (index, page) in set.pages.iter().enumerate() {
        let r = &page.registration;
        for (field, text) in [
            (PageField::Id, &r.id),
            (PageField::Source, &r.source),
            (PageField::Route, &r.route),
        ] {
            b.charge(Resource::Work, text.len() as u64 + 1)?;
            let valid = match field {
                PageField::Id => segment(text, true),
                PageField::Source => path(text, false),
                PageField::Route => path(text, true),
            };
            if !valid {
                return Err(PageError::InvalidRegistration {
                    page: index as u64,
                    field,
                });
            }
            for (previous, other) in set.pages[..index].iter().enumerate() {
                let prior = match field {
                    PageField::Id => &other.registration.id,
                    PageField::Source => &other.registration.source,
                    PageField::Route => &other.registration.route,
                };
                b.charge(Resource::Work, (text.len() + prior.len()) as u64 + 1)?;
                if match field {
                    PageField::Id => text == prior,
                    _ => overlaps(text, prior),
                } {
                    return Err(PageError::Collision {
                        page: index as u64,
                        previous: previous as u64,
                        field,
                    });
                }
            }
        }
    }
    Ok(())
}
fn relative(source: &str, target: &str, b: &mut Budget) -> Result<Option<String>, StopReason> {
    b.charge(Resource::Work, (source.len() + target.len()) as u64 + 1)?;
    if target.is_empty() || target.starts_with('/') {
        return Ok(None);
    }
    let mut parts = Vec::new();
    if let Some((parent, _)) = source.rsplit_once('/') {
        for part in parent.split('/') {
            push(&mut parts, part, b)?;
        }
    }
    for part in target.split('/') {
        b.charge(Resource::Work, 1)?;
        match part {
            "." => (),
            ".." => {
                if parts.pop().is_none() {
                    return Ok(None);
                }
            }
            _ if segment(part, false) => push(&mut parts, part, b)?,
            _ => return Ok(None),
        }
    }
    if parts.is_empty() || matches!(target.rsplit('/').next(), Some("." | "..")) {
        return Ok(None);
    }
    let mut len = parts.len() - 1;
    for part in &parts {
        len = len
            .checked_add(part.len())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    }
    b.charge(Resource::Work, len as u64)?;
    b.charge(Resource::AllocationUnits, len as u64)?;
    let mut out = String::with_capacity(len);
    for (i, part) in parts.iter().enumerate() {
        if i != 0 {
            out.push('/');
        }
        out.push_str(part);
    }
    Ok(Some(out))
}
/// Resolve against the complete explicit set, never an ambient host filesystem.
/// External links, assets and guests remain named requirements; no renderer or
/// evaluator is executed. All source identities share one admission ledger.
pub fn resolve<'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<CheckedPages<'a>, PageError<'a, C::Error>> {
    registrations(set, b)?;
    let value = portable::pages::set_to_value(set, registry, c, b)?;
    let identity = c
        .canonical_value_digest(SET_DOMAIN, &value, b)
        .map_err(|e| match e.stop_reason() {
            Some(s) => PageError::Stopped(s),
            None => PageError::Boundary(portable::PortableError::Foundation(e)),
        })?;
    let mut definitions = Vec::new();
    let mut plans = Vec::new();
    let mut document_digests = Vec::new();
    for (page, input) in set.pages.iter().enumerate() {
        let labels = labels::check(&input.document, registry, b, c.source_admission()).map_err(
            |e| match e {
                labels::LabelError::Stopped(s) => PageError::Stopped(s),
                e => PageError::Input {
                    page: page as u64,
                    error: PreparationError::Label(e),
                },
            },
        )?;
        // set_to_value has already structurally validated and encoded every
        // document. Hash the exact child value instead of constructing it again.
        let document_value = portable::pages::document_value(&value, page, b)?;
        let document_digest = c
            .canonical_value_digest(prepare::DOCUMENT_DOMAIN, document_value, b)
            .map_err(|e| match e.stop_reason() {
                Some(s) => PageError::Stopped(s),
                None => PageError::Input {
                    page: page as u64,
                    error: PreparationError::Boundary(portable::PortableError::Foundation(e)),
                },
            })?;
        let requirements = prepare::requirements(&labels, c, b).map_err(|error| match error {
            PreparationError::Stopped(s) => PageError::Stopped(s),
            error => PageError::Input {
                page: page as u64,
                error,
            },
        })?;
        let plan = prepare::DocPreparationPlan {
            document_digest,
            requirements,
        };
        push(&mut document_digests, document_digest, b)?;
        push(&mut definitions, labels, b)?;
        push(&mut plans, plan, b)?;
    }
    let mut links = Vec::new();
    let mut remaining = Vec::new();
    for (page_index, plan) in plans.into_iter().enumerate() {
        let page = page_index as u64;
        for requirement in plan.requirements {
            b.charge(Resource::Work, 1)?;
            let DocRequirement::Link {
                node,
                target: ref link_target,
            } = requirement
            else {
                push(&mut remaining, PageRequirement { page, requirement }, b)?;
                continue;
            };
            let (key, fragment, by_source) = match link_target {
                LinkTarget::Page { page: id, fragment } => (copy(id, b)?, fragment, false),
                LinkTarget::Relative { path, fragment } => (
                    relative(&set.pages[page_index].registration.source, path, b)?
                        .ok_or(PageError::InvalidRelative { page, node })?,
                    fragment,
                    true,
                ),
                LinkTarget::External { .. } => {
                    push(&mut remaining, PageRequirement { page, requirement }, b)?;
                    continue;
                }
            };
            let mut found = None;
            for (index, candidate) in set.pages.iter().enumerate() {
                let value = if by_source {
                    &candidate.registration.source
                } else {
                    &candidate.registration.id
                };
                b.charge(Resource::Work, (key.len() + value.len()) as u64 + 1)?;
                if value == &key {
                    found = Some(index);
                    break;
                }
            }
            let target = found.ok_or(PageError::MissingPage { page, node })?;
            if let Some(fragment) = fragment {
                let mut found = false;
                for definition in definitions[target].definitions() {
                    b.charge(
                        Resource::Work,
                        (fragment.len() + definition.name.len()) as u64 + 1,
                    )?;
                    if definition.name == fragment {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(PageError::MissingFragment {
                        page,
                        node,
                        target: target as u64,
                    });
                }
            }
            push(
                &mut links,
                PageLink {
                    page,
                    node,
                    target: target as u64,
                    fragment: fragment.as_ref().map(|s| copy(s, b)).transpose()?,
                },
                b,
            )?;
        }
    }
    Ok(CheckedPages {
        set,
        document_digests,
        plan: PageLinkPlan {
            identity,
            links,
            remaining,
        },
    })
}
