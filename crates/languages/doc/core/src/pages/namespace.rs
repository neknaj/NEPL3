//! Page links over host-selected Doc namespace occurrences. Selection/lowering
//! of Sentence or other guests belongs to the integration layer.
use super::*;
use crate::labels::namespace::{CheckedNamespace, MemberId};
use nepl3_core::value::NdfValue;

pub const DOMAIN: &[u8] = b"NEPL3.Doc.PageNamespaces.v1\0";

/// Member numbers are local to a page; node numbers are local to that member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Owner {
    pub page: u64,
    pub member: MemberId,
}
pub struct MemberPlan {
    owner: Owner,
    document_digest: Digest,
    links: Vec<PageLink>,
    remaining: Vec<DocRequirement>,
}
impl MemberPlan {
    pub fn owner(&self) -> Owner {
        self.owner
    }
    pub fn document_digest(&self) -> Digest {
        self.document_digest
    }
    pub fn links(&self) -> &[PageLink] {
        &self.links
    }
    pub fn remaining(&self) -> &[DocRequirement] {
        &self.remaining
    }
}
/// Native proof bound to the exact immutable page set and ordered namespaces.
/// It establishes semantic destinations. Rendering must still check which
/// anchors survive language selection and complete HTML composition.
pub struct CheckedPageNamespaces<'n, 'm, 'a> {
    set: &'a PageSet,
    namespaces: &'n [&'n CheckedNamespace<'m, 'a>],
    identity: Digest,
    members: Vec<MemberPlan>,
}
impl<'n, 'm, 'a> CheckedPageNamespaces<'n, 'm, 'a> {
    pub fn set(&self) -> &'a PageSet {
        self.set
    }
    pub fn namespaces(&self) -> &'n [&'n CheckedNamespace<'m, 'a>] {
        self.namespaces
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn members(&self) -> &[MemberPlan] {
        &self.members
    }
    pub fn document(&self, owner: Owner) -> Option<&'a DocumentSyntax> {
        usize::try_from(owner.page)
            .ok()
            .and_then(|page| self.namespaces.get(page))
            .and_then(|namespace| namespace.document(owner.member))
    }
}
#[derive(Debug)]
pub enum Error<'a, E> {
    Stopped(StopReason),
    Page(PageError<'a, E>),
    /// Exactly one namespace per page, with that page's document as member 0.
    Selection,
    Input {
        page: u64,
        error: PreparationError<'a, E>,
    },
    Link {
        owner: Owner,
        error: PageError<'a, E>,
    },
}
impl<E> From<StopReason> for Error<'_, E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Resolve all explicit occurrences, including hidden variants. Member 0 must
/// borrow the exact PageDocument; further members are explicitly selected by
/// the host in occurrence order. No guest is discovered, loaded or evaluated.
/// Identity is DOMAIN + canonical NDF `[PageSet, [[member digest bytes...],...]]`.
/// Each digest is the checked DocumentSyntax digest with DOCUMENT_DOMAIN; page
/// and member ordering, repeated occurrences, source and provenance are bound.
/// This native proof has no wire decoder. Portable callers must reconstruct
/// the selected namespaces and resolve them before trusting a supplied plan.
pub fn resolve<'n, 'm, 'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    namespaces: &'n [&'n CheckedNamespace<'m, 'a>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<CheckedPageNamespaces<'n, 'm, 'a>, Error<'a, C::Error>> {
    let result = resolve_inner(set, namespaces, registry, codec, b);
    b.poll()?;
    result
}
fn resolve_inner<'n, 'm, 'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    namespaces: &'n [&'n CheckedNamespace<'m, 'a>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<CheckedPageNamespaces<'n, 'm, 'a>, Error<'a, C::Error>> {
    registrations(set, b).map_err(Error::Page)?;
    if namespaces.len() != set.pages.len() {
        return Err(Error::Selection);
    }
    for (page, namespace) in set.pages.iter().zip(namespaces) {
        b.charge(Resource::Work, 1)?;
        if !matches!(page.document.value.root, DocRoot::Article(_))
            || !namespace
                .document(MemberId(0))
                .is_some_and(|document| core::ptr::eq(document, &page.document))
        {
            return Err(Error::Selection);
        }
    }
    let value = portable::pages::set_to_value(set, registry, codec, b)
        .map_err(|error| Error::Page(PageError::from(error)))?;
    let mut plans = Vec::new();
    let mut page_values = Vec::new();
    // Complete boundary inspection for every member before resolving links.
    for (page, namespace) in namespaces.iter().enumerate() {
        let inspected =
            prepare::inspect_namespace(namespace, registry, codec, b).map_err(|error| {
                Error::Input {
                    page: page as u64,
                    error,
                }
            })?;
        let mut member_values = Vec::new();
        for plan in &inspected {
            b.charge(Resource::Work, 32)?;
            b.charge(Resource::AllocationUnits, 32)?;
            push(
                &mut member_values,
                NdfValue::Bytes(plan.document_digest.0.to_vec()),
                b,
            )?;
        }
        push(&mut page_values, NdfValue::List(member_values), b)?;
        push(&mut plans, inspected, b)?;
    }
    let mut fields = Vec::new();
    push(&mut fields, value, b)?;
    push(&mut fields, NdfValue::List(page_values), b)?;
    let identity = codec
        .canonical_value_digest(DOMAIN, &NdfValue::List(fields), b)
        .map_err(|error| match error.stop_reason() {
            Some(reason) => Error::Stopped(reason),
            None => Error::Page(PageError::Boundary(portable::PortableError::Foundation(
                error,
            ))),
        })?;
    let mut members = Vec::new();
    for (page, plans) in plans.into_iter().enumerate() {
        for (member, plan) in plans.into_iter().enumerate() {
            let owner = Owner {
                page: page as u64,
                member: MemberId(member as u64),
            };
            let mut output = MemberPlan {
                owner,
                document_digest: plan.document_digest,
                links: Vec::new(),
                remaining: Vec::new(),
            };
            for requirement in plan.requirements {
                b.charge(Resource::Work, 1)?;
                if let DocRequirement::Link { node, target } = &requirement {
                    let link = resolve_link(set, page, *node, target, b, |target, fragment, b| {
                        for definition in namespaces[target].definitions() {
                            b.charge(
                                Resource::Work,
                                (fragment.len() + definition.site.name.len()) as u64 + 1,
                            )?;
                            if definition.site.name == fragment {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    })
                    .map_err(|error| Error::Link { owner, error })?;
                    push(&mut output.links, link, b)?;
                } else {
                    push(&mut output.remaining, requirement, b)?;
                }
            }
            push(&mut members, output, b)?;
        }
    }
    Ok(CheckedPageNamespaces {
        set,
        namespaces,
        identity,
        members,
    })
}
