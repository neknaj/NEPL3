use super::*;
use crate::binding::trace::ReferenceIssuance;
use nepl3_core::source::Digest;

pub(super) struct PendingReference {
    owner: CanonicalBindingTarget,
    name_target: CanonicalBindingTarget,
    package: package::PackageIdentity,
    execution_digest: Digest,
    binding: BindingId,
    execution_step: u64,
}
impl PendingReference {
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
    pub(super) fn prepare_reference_trace(
        &self,
        frame: &Frame,
        layout: &Layout<'_>,
        selector: &NameSelector,
        event: (BindingId, u64),
        tree: &crate::recovery::ParseTree,
        budget: &mut Budget,
    ) -> Result<PendingReference, BindingError> {
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
        Ok(PendingReference {
            owner: self.probe_target(frame.target, tree, budget)?,
            name_target: self.probe_target(target, tree, budget)?,
            package: package.clone(),
            execution_digest: layout.selection.execution_digest,
            binding: event.0,
            execution_step: event.1,
        })
    }
}
