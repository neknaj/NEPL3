use super::*;
use crate::{
    profile::ProviderRequirement,
    recovery::{ForeignStep, ParseTree},
};
use nepl3_core::syntax::canonical::{BundleMappings, CanonicalError};

pub(super) fn resolution(
    value: &ReferenceResolution,
    budget: &mut Budget,
) -> Result<ReferenceResolution, BindingError> {
    budget.charge(Resource::Work, 1)?;
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<ReferenceResolution>() as u64,
    )?;
    Ok(match value {
        ReferenceResolution::Resolved(id) => ReferenceResolution::Resolved(*id),
        ReferenceResolution::Unresolved(name) => {
            ReferenceResolution::Unresolved(text(name, budget)?)
        }
        ReferenceResolution::Ambiguous(ids) => {
            let mut values = Vec::new();
            for id in ids {
                push(&mut values, *id, budget)?;
            }
            ReferenceResolution::Ambiguous(values)
        }
        ReferenceResolution::Deferred(values) => {
            let mut copied = Vec::new();
            for value in values {
                push(&mut copied, value.clone_with_budget(budget)?, budget)?;
            }
            ReferenceResolution::Deferred(copied)
        }
    })
}
fn canonical(error: CanonicalError) -> BindingError {
    match error {
        CanonicalError::Stopped(reason) => BindingError::Stopped(reason),
        _ => BindingError::Target,
    }
}
pub(super) struct Invocation<'a> {
    pub tree: &'a ParseTree,
    pub path: &'a [ForeignStep],
    pub node: NodeRef,
    pub provider: &'a ProviderRequirement,
    pub authority: FactAuthority,
}
impl Machine<'_, '_> {
    pub(super) fn history(
        &self,
        delta: &FactDelta,
        call: Invocation<'_>,
        budget: &mut Budget,
    ) -> Result<Option<BindingResolutionBatch>, BindingError> {
        if delta.resolutions.is_empty() {
            return Ok(None);
        }
        let mappings = BundleMappings::new(&call.tree.bundle, budget).map_err(canonical)?;
        let mapped = |owner: &SyntaxBundle, node, budget: &mut Budget| {
            for entry in mappings.entries() {
                budget.charge(Resource::Work, 1)?;
                if core::ptr::eq(entry.bundle(), owner) {
                    return entry.mapped(node).map_err(canonical);
                }
            }
            Err(BindingError::Target)
        };
        let mut owner = &call.tree.bundle;
        let mut path = Vec::new();
        for step in call.path {
            push(
                &mut path,
                ForeignStep {
                    node: mapped(owner, step.node, budget)?,
                    field: text(&step.field, budget)?,
                },
                budget,
            )?;
            owner = crate::tree::path(owner, core::slice::from_ref(step), self.registry, budget)?;
        }
        let target = CanonicalBindingTarget {
            path,
            node: mapped(owner, call.node, budget)?,
        };
        let mut updates = Vec::new();
        for update in &delta.resolutions {
            budget.charge(Resource::Work, self.facts()?.occurrences.len() as u64)?;
            let old = self
                .facts()?
                .occurrences
                .iter()
                .find(|v| v.id == update.occurrence)
                .ok_or(BindingError::Fact(FactError::MissingOccurrence))?;
            push(
                &mut updates,
                ResolutionTransition {
                    occurrence: update.occurrence,
                    before: resolution(&old.resolution, budget)?,
                    after: resolution(&update.resolution, budget)?,
                },
                budget,
            )?;
        }
        let p = call.provider;
        budget.charge(
            Resource::Work,
            (p.provider.len() + p.operation.name.len() + p.operation.schema.package.len()) as u64
                + 66,
        )?;
        budget.charge(
            Resource::AllocationUnits,
            (p.provider.len() + p.operation.name.len() + p.operation.schema.package.len()) as u64
                + core::mem::size_of::<ProviderRequirement>() as u64,
        )?;
        Ok(Some(BindingResolutionBatch {
            provider: p.clone(),
            authority: call.authority,
            target,
            updates,
        }))
    }
}
