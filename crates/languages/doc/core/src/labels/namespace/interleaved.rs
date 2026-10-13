//! Stable cross-member site order. This does not prove a host's Article traversal.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Index into the selected occurrence's `Member::definitions()`, not a Doc node.
pub struct DefinitionIndex {
    pub member: MemberId,
    pub index: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Index into the selected occurrence's `Member::references()`, not a global label.
pub struct ReferenceIndex {
    pub member: MemberId,
    pub index: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SiteKind {
    Definition,
    Reference,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderError {
    Member {
        kind: SiteKind,
        member: MemberId,
    },
    Index {
        kind: SiteKind,
        member: MemberId,
        index: u64,
    },
    Next {
        kind: SiteKind,
        member: MemberId,
        expected: u64,
        actual: u64,
    },
    Missing {
        kind: SiteKind,
        member: MemberId,
        next: u64,
        total: u64,
    },
}
fn sites<'s, 'a>(member: &'s Member<'a>, kind: SiteKind) -> &'s [LabelSite<'a>] {
    match kind {
        SiteKind::Definition => &member.definitions,
        SiteKind::Reference => &member.references,
    }
}
fn lookup<'a>(
    members: &[&Member<'a>],
    kind: SiteKind,
    member: MemberId,
    index: u64,
) -> Result<NamespaceSite<'a>, Error<'a>> {
    let owner = usize::try_from(member.0)
        .ok()
        .and_then(|n| members.get(n))
        .ok_or(Error::Order(OrderError::Member { kind, member }))?;
    let site = usize::try_from(index)
        .ok()
        .and_then(|n| sites(owner, kind).get(n))
        .ok_or(Error::Order(OrderError::Index {
            kind,
            member,
            index,
        }))?;
    Ok(NamespaceSite {
        member,
        site: *site,
    })
}
fn validate<'a>(
    members: &[&Member<'a>],
    kind: SiteKind,
    order: impl Iterator<Item = (MemberId, u64)>,
    b: &mut Budget,
) -> Result<(), Error<'a>> {
    b.poll()?;
    let mut next = Vec::new();
    for _ in members {
        b.charge(Resource::Work, 1)?;
        push(&mut next, 0_u64, b)?;
    }
    for (member, index) in order {
        b.charge(Resource::Work, 1)?;
        lookup(members, kind, member, index)?;
        let cursor = usize::try_from(member.0)
            .ok()
            .and_then(|n| next.get_mut(n))
            .ok_or(Error::Order(OrderError::Member { kind, member }))?;
        if index != *cursor {
            return Err(Error::Order(OrderError::Next {
                kind,
                member,
                expected: *cursor,
                actual: index,
            }));
        }
        // Lookup proved index < a real slice length, so its successor fits u64.
        *cursor = index + 1;
    }
    for (number, (member, next)) in members.iter().zip(next).enumerate() {
        b.charge(Resource::Work, 1)?;
        let total = sites(member, kind).len() as u64;
        if next != total {
            return Err(Error::Order(OrderError::Missing {
                kind,
                member: MemberId(number as u64),
                next,
                total,
            }));
        }
    }
    Ok(())
}
/// Resolve every collected site exactly once, preserving each member's local
/// order. Member IDs remain display-occurrence IDs, even for repeated borrows.
/// The caller selects cross-member interleaving. This proof does not establish
/// Article traversal or change a renderer's contiguous whole-member output.
pub fn resolve_interleaved<'m, 'a>(
    members: &'m [&'m Member<'a>],
    definitions: &[DefinitionIndex],
    references: &[ReferenceIndex],
    b: &mut Budget,
) -> Result<CheckedNamespace<'m, 'a>, Error<'a>> {
    validate(
        members,
        SiteKind::Definition,
        definitions.iter().map(|s| (s.member, s.index)),
        b,
    )?;
    validate(
        members,
        SiteKind::Reference,
        references.iter().map(|s| (s.member, s.index)),
        b,
    )?;
    resolve_sites(
        members,
        definitions
            .iter()
            .map(|s| lookup(members, SiteKind::Definition, s.member, s.index)),
        references
            .iter()
            .map(|s| lookup(members, SiteKind::Reference, s.member, s.index)),
        b,
    )
}
