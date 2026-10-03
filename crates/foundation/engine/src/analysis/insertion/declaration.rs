//! Bounded native declaration correspondence across one checked name insertion.
//! Positive evidence does not accept the edit or establish program equivalence.
use super::*;
use crate::{
    analysis::{
        probe::{named::BoundNamedProbe, read},
        trace::named::{BoundNamedTrace, EntityBirthAccessError},
    },
    binding::trace::birth::{BirthLookup, BirthPhase, EntityBirth},
};
use nepl3_core::{
    budget::Resource,
    facts::{Entity, ReferenceResolution},
};
mod context;
mod location;
mod source;

#[derive(Debug)]
pub enum DeclarationError {
    Access(BindingAccessError),
    Birth(EntityBirthAccessError),
    Read(read::ReadError),
    Tree(crate::tree::TreeError),
    Canonical(nepl3_core::syntax::canonical::CanonicalError),
    Source(SourceError),
    Stopped(StopReason),
    ProofMismatch,
    Structure,
}
impl From<StopReason> for DeclarationError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnprovenReason {
    OriginalAmbiguous,
    OriginalUnresolved,
    OriginalDeferred,
    FinalAmbiguous,
    FinalUnresolved,
    FinalDeferred,
    UntracedEntity,
    RepeatedBirth,
    SharedSyntax,
    Context,
    Source,
    Action,
}
/// Private immutable joins retain both runs, the checked insertion, and actual entities.
pub struct SameDeclaration<'a, 'tree, 'p> {
    original: &'a BoundNamedProbe<'tree, 'p>,
    candidate: &'a BoundNamedTrace<'a, 'p>,
    reference: &'a reference::MatchedReference<'a, 'tree, 'p>,
    original_entity: &'a Entity,
    candidate_entity: &'a Entity,
    original_birth: &'a EntityBirth,
    candidate_birth: &'a EntityBirth,
}
impl<'a, 'tree, 'p> SameDeclaration<'a, 'tree, 'p> {
    pub fn original(&self) -> &'a BoundNamedProbe<'tree, 'p> {
        self.original
    }
    pub fn candidate(&self) -> &'a BoundNamedTrace<'a, 'p> {
        self.candidate
    }
    pub fn reference(&self) -> &'a reference::MatchedReference<'a, 'tree, 'p> {
        self.reference
    }
    pub fn original_entity(&self) -> &'a Entity {
        self.original_entity
    }
    pub fn candidate_entity(&self) -> &'a Entity {
        self.candidate_entity
    }
    pub fn original_birth(&self) -> &'a EntityBirth {
        self.original_birth
    }
    pub fn candidate_birth(&self) -> &'a EntityBirth {
        self.candidate_birth
    }
}
pub enum DeclarationOutcome<'a, 'tree, 'p> {
    Same(SameDeclaration<'a, 'tree, 'p>),
    /// A conservative failure is not evidence that the declarations differ.
    Unproven(UnprovenReason),
}
fn phase(value: BirthPhase) -> u8 {
    match value {
        BirthPhase::Ordinary => 0,
        BirthPhase::Header { .. } => 1,
        BirthPhase::Body { .. } => 2,
    }
}
// Private named envelopes authenticate BindingId/name-selector linkage at issuance.
// Raw rows cannot be reassembled into these inputs; this compares only the action
// portion, while structural site coordinates are checked in each tree separately.
fn action(a: &EntityBirth, c: &EntityBirth, b: &mut Budget) -> Result<bool, DeclarationError> {
    b.charge(
        Resource::Work,
        a.package.schema.package.len() as u64 + c.package.schema.package.len() as u64 + 120,
    )?;
    Ok(a.binding == c.binding
        && a.kind == c.kind
        && phase(a.phase) == phase(c.phase)
        && a.package == c.package
        && a.execution_digest == c.execution_digest)
}
fn unique(
    rows: &[EntityBirth],
    wanted: &EntityBirth,
    b: &mut Budget,
) -> Result<bool, DeclarationError> {
    let mut found = false;
    for row in rows {
        b.charge(Resource::Nodes, 1)?;
        if action(row, wanted, b)?
            && read::target_equal(&row.owner, &wanted.owner, b).map_err(DeclarationError::Read)?
            && read::target_equal(&row.name_target, &wanted.name_target, b)
                .map_err(DeclarationError::Read)?
        {
            if found {
                return Ok(false);
            }
            found = true;
        }
    }
    if !found {
        return Err(DeclarationError::Structure);
    }
    Ok(true)
}
/// Cross-revision identity is limited to a unique stable structural/action signature.
pub fn correlate<'a, 'tree, 'p>(
    matched: &'a reference::MatchedReference<'a, 'tree, 'p>,
    original: &'a BoundNamedProbe<'tree, 'p>,
    candidate: &'a BoundNamedTrace<'a, 'p>,
    old_prepared: &PreparedBindingRequest<'_, 'p>,
    new_prepared: &PreparedBindingRequest<'_, 'p>,
    b: &mut Budget,
) -> Result<DeclarationOutcome<'a, 'tree, 'p>, DeclarationError> {
    let checked = matched.insertion().checked();
    if b.limits() != checked.limits()
        || b.limits() != old_prepared.limits
        || b.limits() != new_prepared.limits
    {
        return Err(DeclarationError::Access(BindingAccessError::LimitsMismatch));
    }
    b.charge(Resource::Work, 257)?;
    if old_prepared.key() != checked.keys().0 || new_prepared.key() != checked.keys().1 {
        return Err(DeclarationError::Access(BindingAccessError::StaleAnalysis));
    }
    let read = matched.insertion().choice().read();
    if !core::ptr::eq(original.probe(), read.reply())
        || !core::ptr::eq(original.profile(), checked.original().seed().profile())
        || !core::ptr::eq(candidate.references(), matched.trace())
        || !core::ptr::eq(
            old_prepared.tree.tree(),
            checked.original().execution().tree(),
        )
        || !core::ptr::eq(old_prepared.profile, checked.original().seed().profile())
        || !core::ptr::eq(
            new_prepared.tree.tree(),
            checked.candidate().execution().tree(),
        )
        || !core::ptr::eq(new_prepared.profile, checked.candidate().seed().profile())
    {
        return Err(DeclarationError::ProofMismatch);
    }
    let crate::binding::probe::ProbeOutcome::Hit(hit) = &original.probe().reply().outcome else {
        return Err(DeclarationError::ProofMismatch);
    };
    if !core::ptr::eq(hit.as_ref(), read.hit()) {
        return Err(DeclarationError::ProofMismatch);
    }
    // The read proof retains this exact successful Hit from the contained envelope.
    let choice = matched.insertion().choice().candidate();
    let old_id = match &choice.resolution {
        ReferenceResolution::Resolved(id) => *id,
        ReferenceResolution::Ambiguous(_) => {
            return Ok(DeclarationOutcome::Unproven(
                UnprovenReason::OriginalAmbiguous,
            ));
        }
        ReferenceResolution::Unresolved(_) => {
            return Ok(DeclarationOutcome::Unproven(
                UnprovenReason::OriginalUnresolved,
            ));
        }
        ReferenceResolution::Deferred(_) => {
            return Ok(DeclarationOutcome::Unproven(
                UnprovenReason::OriginalDeferred,
            ));
        }
    };
    let new_id = match &matched.final_reference().occurrence.resolution {
        ReferenceResolution::Resolved(id) => *id,
        ReferenceResolution::Ambiguous(_) => {
            return Ok(DeclarationOutcome::Unproven(UnprovenReason::FinalAmbiguous));
        }
        ReferenceResolution::Unresolved(_) => {
            return Ok(DeclarationOutcome::Unproven(
                UnprovenReason::FinalUnresolved,
            ));
        }
        ReferenceResolution::Deferred(_) => {
            return Ok(DeclarationOutcome::Unproven(UnprovenReason::FinalDeferred));
        }
    };
    let BirthLookup::Born {
        entity: ae,
        birth: ab,
    } = original
        .entity_birth(&checked.keys().0, old_id, b)
        .map_err(DeclarationError::Birth)?
    else {
        return Ok(DeclarationOutcome::Unproven(UnprovenReason::UntracedEntity));
    };
    let BirthLookup::Born {
        entity: ce,
        birth: cb,
    } = candidate
        .entity_birth(&checked.keys().1, new_id, b)
        .map_err(DeclarationError::Birth)?
    else {
        return Ok(DeclarationOutcome::Unproven(UnprovenReason::UntracedEntity));
    };
    if !action(ab, cb, b)? {
        return Ok(DeclarationOutcome::Unproven(UnprovenReason::Action));
    }
    if !unique(original.births(), ab, b)? || !unique(candidate.births(), cb, b)? {
        return Ok(DeclarationOutcome::Unproven(UnprovenReason::RepeatedBirth));
    }
    b.with_depth(|b| {
        let Some(ao) = location::locate(old_prepared, &ab.owner, b)? else {
            return Ok(DeclarationOutcome::Unproven(UnprovenReason::SharedSyntax));
        };
        let Some(an) = location::locate(old_prepared, &ab.name_target, b)? else {
            return Ok(DeclarationOutcome::Unproven(UnprovenReason::SharedSyntax));
        };
        let Some(co) = location::locate(new_prepared, &cb.owner, b)? else {
            return Ok(DeclarationOutcome::Unproven(UnprovenReason::SharedSyntax));
        };
        let Some(cn) = location::locate(new_prepared, &cb.name_target, b)? else {
            return Ok(DeclarationOutcome::Unproven(UnprovenReason::SharedSyntax));
        };
        if !context::same(old_prepared, &ao, new_prepared, &co, b)?
            || !context::same(old_prepared, &an, new_prepared, &cn, b)?
        {
            return Ok(DeclarationOutcome::Unproven(UnprovenReason::Context));
        }
        if !source::same(
            checked,
            source::Evidence {
                owner: &ao,
                name: &an,
                entity: ae,
            },
            source::Evidence {
                owner: &co,
                name: &cn,
                entity: ce,
            },
            b,
        )? {
            return Ok(DeclarationOutcome::Unproven(UnprovenReason::Source));
        }
        Ok(DeclarationOutcome::Same(SameDeclaration {
            original,
            candidate,
            reference: matched,
            original_entity: ae,
            candidate_entity: ce,
            original_birth: ab,
            candidate_birth: cb,
        }))
    })
}

impl DeclarationError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(s)
            | Self::Access(BindingAccessError::Stopped(s))
            | Self::Canonical(nepl3_core::syntax::canonical::CanonicalError::Stopped(s))
            | Self::Source(SourceError::Stopped(s))
            | Self::Birth(EntityBirthAccessError::Access(BindingAccessError::Stopped(s)))
            | Self::Birth(EntityBirthAccessError::Birth(
                crate::binding::trace::birth::BirthAccessError::Stopped(s),
            )) => Some(*s),
            Self::Read(e) => e.stop_reason(),
            Self::Tree(crate::tree::TreeError::Stopped(s)) => Some(*s),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        binding::trace::birth::BirthKind,
        binding::{CanonicalBindingTarget, StageId},
        package::{BindingId, PackageIdentity},
    };
    use nepl3_core::{
        budget::Limits,
        facts::{EntityId, NamespaceRef, ScopeId},
        source::Digest,
        syntax::NodeRef,
        value::SchemaRef,
    };
    fn birth(id: u64, phase: BirthPhase) -> EntityBirth {
        EntityBirth {
            owner: CanonicalBindingTarget {
                path: alloc::vec![],
                node: NodeRef(0),
            },
            name_target: CanonicalBindingTarget {
                path: alloc::vec![],
                node: NodeRef(1),
            },
            package: PackageIdentity {
                schema: SchemaRef {
                    package: "test.signature".into(),
                    revision: 1,
                    digest: Digest::of(b"schema"),
                },
                semantic_digest: Digest::of(b"semantic"),
            },
            execution_digest: Digest::of(b"execution"),
            binding: BindingId(2),
            execution_step: id + 10,
            entity: EntityId(id),
            scope: ScopeId(id),
            namespace: NamespaceRef(id),
            namespace_stage: StageId(id),
            kind: BirthKind::Bind,
            phase,
        }
    }
    #[test]
    fn actual_birth_signature_ignores_run_local_ids_but_preserves_phase_category()
    -> Result<(), DeclarationError> {
        let limits = Limits {
            work: 100_000,
            nodes: 100_000,
            allocation_units: 100_000,
            ..Limits::default()
        };
        let old = birth(0, BirthPhase::Header { group: ScopeId(1) });
        let other_group = birth(50, BirthPhase::Header { group: ScopeId(99) });
        assert!(action(&old, &other_group, &mut Budget::new(limits))?);
        assert!(!unique(
            &[old, other_group],
            &birth(80, BirthPhase::Header { group: ScopeId(8) }),
            &mut Budget::new(limits)
        )?);
        let rows = [
            birth(0, BirthPhase::Ordinary),
            birth(1, BirthPhase::Header { group: ScopeId(1) }),
            birth(2, BirthPhase::Body { group: ScopeId(1) }),
        ];
        for row in &rows {
            assert!(unique(&rows, row, &mut Budget::new(limits))?);
        }
        let mut other_action = birth(99, BirthPhase::Ordinary);
        other_action.binding = BindingId(3);
        assert!(unique(
            &[birth(0, BirthPhase::Ordinary), other_action],
            &birth(9, BirthPhase::Ordinary),
            &mut Budget::new(limits)
        )?);
        Ok(())
    }
}
