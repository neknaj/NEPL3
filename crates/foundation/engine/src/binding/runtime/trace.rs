use super::*;
use crate::binding::trace::ReferenceIssuance;
use nepl3_core::source::Digest;

pub(super) struct PendingNamedSite {
    owner: CanonicalBindingTarget,
    name_target: CanonicalBindingTarget,
    package: package::PackageIdentity,
    execution_digest: Digest,
    binding: BindingId,
    execution_step: u64,
    phase: Phase,
}
impl PendingNamedSite {
    pub(super) fn born(
        self,
        entity: EntityId,
        scope: ScopeId,
        namespace_stage: StageId,
        namespace: NamespaceRef,
        kind: crate::binding::trace::birth::BirthKind,
    ) -> Result<crate::binding::trace::birth::EntityBirth, BindingError> {
        use crate::binding::trace::birth::{BirthPhase, EntityBirth};
        let phase = match (self.phase.group, self.phase.header) {
            (None, false) => BirthPhase::Ordinary,
            (Some(group), false) => BirthPhase::Body { group },
            (Some(group), true) => BirthPhase::Header { group },
            (None, true) => return Err(BindingError::Target),
        };
        Ok(EntityBirth {
            owner: self.owner,
            name_target: self.name_target,
            package: self.package,
            execution_digest: self.execution_digest,
            binding: self.binding,
            execution_step: self.execution_step,
            entity,
            scope,
            namespace_stage,
            namespace,
            kind,
            phase,
        })
    }
    pub(super) fn issued(
        self,
        occurrence: OccurrenceId,
        stage: StageId,
        namespace_stage: StageId,
        namespace: NamespaceRef,
    ) -> ReferenceIssuance {
        ReferenceIssuance {
            owner: self.owner,
            name_target: self.name_target,
            package: self.package,
            execution_digest: self.execution_digest,
            binding: self.binding,
            execution_step: self.execution_step,
            occurrence,
            stage,
            namespace_stage,
            namespace,
        }
    }
}
impl Machine<'_, '_> {
    pub(super) fn prepare_named_trace(
        &self,
        frame: &Frame,
        layout: &Layout<'_>,
        selector: &NameSelector,
        event: (BindingId, u64),
        tree: &crate::recovery::ParseTree,
        budget: &mut Budget,
    ) -> Result<PendingNamedSite, BindingError> {
        let target = match selector {
            NameSelector::SelfValue => frame.target,
            NameSelector::Field(name) => {
                self.child(frame.target, Self::field(layout, name, budget)?, budget)?
            }
        };
        let package = &layout.selection.entry.package;
        budget.charge(Resource::Work, package.schema.package.len() as u64 + 75)?;
        budget.charge(
            Resource::AllocationUnits,
            package.schema.package.len() as u64,
        )?;
        Ok(PendingNamedSite {
            owner: self.probe_target(frame.target, tree, budget)?,
            name_target: self.probe_target(target, tree, budget)?,
            package: package.clone(),
            execution_digest: layout.selection.execution_digest,
            binding: event.0,
            execution_step: event.1,
            phase: frame.phase,
        })
    }
}
