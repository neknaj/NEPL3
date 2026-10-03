//! Built-in Entity publication, distinct from visibility and occurrence completion.
use super::*;
use nepl3_core::source::Digest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BirthKind {
    Bind,
    Export,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Phase group IDs belong only to this run; they are not declaration identities.
pub enum BirthPhase {
    Ordinary,
    Header { group: ScopeId },
    Body { group: ScopeId },
}
#[derive(Debug)]
pub struct EntityBirth {
    pub owner: CanonicalBindingTarget,
    pub name_target: CanonicalBindingTarget,
    pub package: package::PackageIdentity,
    pub execution_digest: Digest,
    pub binding: BindingId,
    /// Run-local dispatch ordinal, never a cross-revision identity.
    pub execution_step: u64,
    pub entity: EntityId,
    pub scope: ScopeId,
    pub namespace: NamespaceRef,
    /// The namespace stage before introduction or export-list insertion.
    pub namespace_stage: StageId,
    pub kind: BirthKind,
    pub phase: BirthPhase,
}
#[derive(Debug, Eq, PartialEq)]
pub enum BirthAccessError {
    LimitsMismatch,
    Incomplete,
    MissingEntity,
    Facts,
    Stopped(StopReason),
}
impl From<StopReason> for BirthAccessError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
pub enum BirthLookup<'a> {
    /// Entity publication only, without visibility or occurrence-completion authority.
    Born {
        entity: &'a Entity,
        birth: &'a EntityBirth,
    },
    /// The entity exists, but this built-in-only ledger does not issue its provenance.
    Untraced(&'a Entity),
}
pub(crate) fn lookup<'a>(
    facts: &'a FactSet,
    rows: &'a [EntityBirth],
    id: EntityId,
    b: &mut Budget,
) -> Result<BirthLookup<'a>, BirthAccessError> {
    b.with_depth(|b| {
        let mut entity = None;
        for value in &facts.entities {
            b.charge(Resource::Work, 1)?;
            b.charge(Resource::Nodes, 1)?;
            if value.id == id {
                if entity.is_some() {
                    return Err(BirthAccessError::Facts);
                }
                entity = Some(value);
            }
        }
        let entity = entity.ok_or(BirthAccessError::MissingEntity)?;
        let mut birth = None;
        for row in rows {
            b.charge(Resource::Work, 1)?;
            b.charge(Resource::Nodes, 1)?;
            if row.entity == id {
                if birth.is_some() {
                    return Err(BirthAccessError::Facts);
                }
                birth = Some(row);
            }
        }
        Ok(match birth {
            Some(birth) => BirthLookup::Born { entity, birth },
            None => BirthLookup::Untraced(entity),
        })
    })
}
