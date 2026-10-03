use super::*;
use crate::{
    binding::probe::MissingReferenceSite,
    recovery::{ParseTree, RecoveryKind},
};

impl Machine<'_, '_> {
    fn probe_recovery<'t>(
        &self,
        target: Target,
        tree: &'t ParseTree,
        budget: &mut Budget,
    ) -> Result<Option<&'t RecoveryKind>, BindingError> {
        let owner = self
            .bundles
            .get(target.bundle)
            .ok_or(BindingError::Target)?
            .bundle;
        for recovery in &tree.recovery {
            budget.charge(Resource::Work, 1)?;
            if core::ptr::eq(
                crate::tree::path(&tree.bundle, &recovery.path, self.registry, budget)?,
                owner,
            ) {
                for entry in &recovery.entries {
                    budget.charge(Resource::Work, 1)?;
                    if entry.node == target.node {
                        return Ok(Some(&entry.kind));
                    }
                }
            }
        }
        Ok(None)
    }
    fn probe_target(
        &self,
        target: Target,
        tree: &ParseTree,
        budget: &mut Budget,
    ) -> Result<CanonicalBindingTarget, BindingError> {
        let owner = self
            .bundles
            .get(target.bundle)
            .ok_or(BindingError::Target)?
            .bundle;
        for context in &tree.contexts {
            budget.charge(Resource::Work, 1)?;
            if core::ptr::eq(
                crate::tree::path(&tree.bundle, &context.path, self.registry, budget)?,
                owner,
            ) {
                return crate::facts::phase::target(
                    tree,
                    &context.path,
                    target.node,
                    self.registry,
                    budget,
                )
                .map_err(BindingError::from);
            }
        }
        Err(BindingError::Target)
    }
    pub(super) fn probe_action(
        &self,
        frame: &Frame,
        layout: &Layout<'_>,
        binding: &Binding,
        event: (BindingId, u64),
        tree: &ParseTree,
        budget: &mut Budget,
    ) -> Result<Option<MissingReferenceSite>, BindingError> {
        let (id, execution_step) = event;
        let (namespace, selector, reference) = match binding {
            Binding::Custom(_) => return Err(BindingError::MissingProvider),
            Binding::Bind { .. } | Binding::Reference { .. } if frame.phase.header => {
                return Ok(None);
            }
            Binding::Bind { namespace, name } | Binding::Export { namespace, name } => {
                (namespace, name, false)
            }
            Binding::Reference { namespace, name } => (namespace, name, true),
            _ => return Ok(None),
        };
        let target = match selector {
            NameSelector::SelfValue => frame.target,
            NameSelector::Field(name) => {
                self.child(frame.target, Self::field(layout, name, budget)?, budget)?
            }
        };
        let Some(recovery) = self.probe_recovery(target, tree, budget)? else {
            return Ok(None);
        };
        let RecoveryKind::Missing { anchor, .. } = recovery else {
            return Err(BindingError::RecoveredTree);
        };
        if !reference {
            return Err(BindingError::RecoveredTree);
        }
        if anchor.start() != anchor.end() {
            return Err(BindingError::Target);
        }
        let namespace = self.namespace(frame.target.bundle, layout.package, namespace, budget)?;
        let namespace_stage = self.namespace_stage(frame.target.bundle, frame.stage, namespace)?;
        Ok(Some(MissingReferenceSite {
            owner: self.probe_target(frame.target, tree, budget)?,
            name_target: self.probe_target(target, tree, budget)?,
            binding: id,
            execution_step,
            stage: frame.stage,
            namespace_stage,
            namespace,
            anchor: span(anchor, budget)?,
        }))
    }
}
