//! Native routing across explicitly adopted per-page Doc namespaces.
//! This is neither a portable proof nor evidence that a target anchor is emitted.
use super::*;
use crate::labels::namespace::{CheckedNamespace, MemberId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Site {
    pub page: u64,
    pub member: MemberId,
    pub node: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Link {
    pub site: Site,
    pub target: PageDestination,
    pub fragment: Option<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequirementId {
    pub page: u64,
    pub member: MemberId,
    pub index: usize,
}
#[derive(Debug)]
pub enum Error<'a, E> {
    Stopped(StopReason),
    Registration(PageError<'static, E>),
    PageCount,
    ScopeMismatch,
    RootMismatch {
        page: u64,
    },
    Preparation {
        page: u64,
        member: Option<MemberId>,
        error: PreparationError<'a, E>,
    },
    Destination {
        member: MemberId,
        error: PageError<'static, E>,
    },
    MissingFragment {
        site: Site,
        target: u64,
    },
}
impl<E> From<StopReason> for Error<'_, E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Operation-local routing for exactly the borrowed set and namespaces.
/// Selection, URI policy and complete output IDs still require validation.
/// No serialized plan can construct this type; legacy identities are unchanged.
pub struct CheckedNamespacePages<'p, 'm, 'a> {
    set: &'a PageSet,
    namespaces: &'p [&'p CheckedNamespace<'m, 'a>],
    plans: Vec<Vec<prepare::DocPreparationPlan>>,
    links: Vec<Link>,
    remaining: Vec<RequirementId>,
}
impl<'p, 'm, 'a> CheckedNamespacePages<'p, 'm, 'a> {
    pub fn set(&self) -> &'a PageSet {
        self.set
    }
    pub fn namespaces(&self) -> &'p [&'p CheckedNamespace<'m, 'a>] {
        self.namespaces
    }
    pub fn links(&self) -> &[Link] {
        &self.links
    }
    pub fn remaining(&self) -> &[RequirementId] {
        &self.remaining
    }
    pub fn member_plan(&self, page: u64, member: MemberId) -> Option<&prepare::DocPreparationPlan> {
        self.plans
            .get(usize::try_from(page).ok()?)?
            .get(usize::try_from(member.0).ok()?)
    }
    pub fn requirement(&self, id: RequirementId) -> Option<&DocRequirement> {
        self.member_plan(id.page, id.member)?
            .requirements
            .get(id.index)
    }
}
/// Member zero must be the exact PageSet Article, not an equal replacement.
/// Other members are explicit host adoption; foreign closures are not traversed.
/// Local references have already been resolved inside each page's namespace.
/// Relative links use the owning page's registered logical source, not route.
pub fn resolve<'p, 'm, 'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    namespaces: &'p [&'p CheckedNamespace<'m, 'a>],
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<CheckedNamespacePages<'p, 'm, 'a>, Error<'a, C::Error>> {
    budget.poll()?;
    let limits = budget.limits();
    let depth = budget.current_depth();
    registrations(set, budget).map_err(|error| match error {
        PageError::Stopped(reason) => Error::Stopped(reason),
        error => Error::Registration(error),
    })?;
    if namespaces.len() != set.pages.len() {
        return Err(Error::PageCount);
    }
    let mut plans = Vec::new();
    for (page, (input, namespace)) in set.pages.iter().zip(namespaces).enumerate() {
        budget.charge(Resource::Work, 1)?;
        if !matches!(input.document.value.root, DocRoot::Article(_))
            || !namespace
                .document(MemberId(0))
                .is_some_and(|root| core::ptr::eq(root, &input.document))
        {
            return Err(Error::RootMismatch { page: page as u64 });
        }
        let member_plans = prepare::inspect_namespace_located(namespace, registry, codec, budget)
            .map_err(|failure| match failure.error {
            PreparationError::Stopped(reason) => Error::Stopped(reason),
            error => Error::Preparation {
                page: page as u64,
                member: failure.member,
                error,
            },
        })?;
        push(&mut plans, member_plans, budget)?;
    }
    let mut links = Vec::new();
    let mut remaining = Vec::new();
    for (page, member_plans) in plans.iter().enumerate() {
        for (member, plan) in member_plans.iter().enumerate() {
            let member = MemberId(member as u64);
            for (index, requirement) in plan.requirements.iter().enumerate() {
                budget.charge(Resource::Work, 1)?;
                if let DocRequirement::Link { node, target } = requirement {
                    let selected = destination(set, page, *node, target, budget).map_err(
                        |error| match error {
                            PageError::Stopped(reason) => Error::Stopped(reason),
                            error => Error::Destination { member, error },
                        },
                    )?;
                    if let Some((target, fragment)) = selected {
                        let site = Site {
                            page: page as u64,
                            member,
                            node: *node,
                        };
                        if let (PageDestination::Page { index }, Some(name)) = (target, fragment) {
                            let mut found = false;
                            for definition in namespaces[index as usize].definitions() {
                                budget.charge(
                                    Resource::Work,
                                    (name.len() + definition.site.name.len()) as u64 + 1,
                                )?;
                                if name == definition.site.name {
                                    found = true;
                                    break;
                                }
                            }
                            if !found {
                                return Err(Error::MissingFragment {
                                    site,
                                    target: index,
                                });
                            }
                        }
                        push(
                            &mut links,
                            Link {
                                site,
                                target,
                                fragment: fragment.as_ref().map(|s| copy(s, budget)).transpose()?,
                            },
                            budget,
                        )?;
                        continue;
                    }
                }
                // Retain the typed plan; do not deep-clone unchecked guest data.
                push(
                    &mut remaining,
                    RequirementId {
                        page: page as u64,
                        member,
                        index,
                    },
                    budget,
                )?;
            }
        }
    }
    budget.poll()?;
    if budget.limits() != limits || budget.current_depth() != depth {
        return Err(Error::ScopeMismatch);
    }
    Ok(CheckedNamespacePages {
        set,
        namespaces,
        plans,
        links,
        remaining,
    })
}
