//! Session-local reuse of source conflict checks; never an admission proof.
#[cfg(test)]
mod tests;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    source::{SourceError, SourceSnapshot, SourceStore, SourceStoreScope},
};

#[derive(Default)]
pub(super) struct SourceChecks {
    environment: Vec<SourceSnapshot>,
    environment_scope: Option<SourceStoreScope>,
    checked: Vec<SourceSnapshot>,
}
impl SourceChecks {
    pub(super) fn check(
        &mut self,
        incoming: &[SourceSnapshot],
        store: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(), SourceError> {
        budget.poll()?;
        let owned = self
            .environment_scope
            .as_ref()
            .is_some_and(|scope| store.matches_scope(scope));
        let mut same = owned || self.environment.len() == store.snapshots().len();
        if same && !owned {
            for (old, new) in self.environment.iter().zip(store.snapshots()) {
                if !old.eq_with_budget(new, budget)? {
                    same = false;
                    break;
                }
            }
        }
        let mut prefix = 0;
        if same {
            for (old, new) in self.checked.iter().zip(incoming) {
                if !old.eq_with_budget(new, budget)? {
                    break;
                }
                prefix += 1;
            }
        }
        for added in &incoming[prefix..] {
            budget.charge(Resource::Work, 1)?;
            if let Some(prior) = store.get_revision_with_budget(
                &added.identity().source,
                added.identity().revision,
                budget,
            )? {
                budget.charge(Resource::Work, prior.uri().len() as u64 + 33)?;
                if prior.identity() != added.identity() || prior.uri() != added.uri() {
                    return Err(SourceError::IdentityConflict);
                }
            }
        }
        // Build replacement ownership before publishing a new environment.
        // Retained snapshots prevent allocator address reuse from proving equality.
        if !same {
            let mut environment = Vec::new();
            for source in store.snapshots() {
                environment.push(source.clone_with_budget(budget)?);
            }
            self.environment = environment;
            self.checked.clear();
        } else {
            self.checked.truncate(prefix);
        }
        self.environment_scope = store.scope();
        for source in &incoming[prefix..] {
            self.checked.push(source.clone_with_budget(budget)?);
        }
        Ok(())
    }
}
