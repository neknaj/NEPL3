//! Explicit store-local preconditions for an atomic edit transaction.
use super::*;

/// Indices refer to the caller's unchanged dependency or edit slices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuardedEditError {
    MissingDependency { dependency: usize },
    ChangedDependency { dependency: usize },
    NotWritable { edit: usize },
    Source(SourceError),
}
impl From<SourceError> for GuardedEditError {
    fn from(value: SourceError) -> Self {
        Self::Source(value)
    }
}
impl From<StopReason> for GuardedEditError {
    fn from(value: StopReason) -> Self {
        Self::Source(SourceError::Stopped(value))
    }
}
impl GuardedEditError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Source(SourceError::Stopped(reason)) => Some(*reason),
            _ => None,
        }
    }
}
impl SourceStore {
    /// Recheck explicitly selected current dependencies and exact host-granted
    /// writable snapshot identities, then apply the complete edit transaction.
    ///
    /// Each dependency must equal the greatest retained revision, including URI
    /// and complete bytes. Historical evidence must not be passed as a current
    /// dependency. Every edit must match an identity in `writable`; source names
    /// and URIs alone never grant permission. Duplicate guards are harmless.
    ///
    /// The caller supplies both the dependency policy and host authority. This
    /// function does not authenticate those inputs, discover dependencies, prove
    /// semantic validity, or establish external filesystem/editor freshness.
    /// A host must keep its authoritative state and permissions stable through
    /// this call and separately arrange external publication. The exclusive
    /// borrow prevents mutation of this store between checks and application.
    ///
    /// Failure leaves the store unchanged. Budget usage and admission follow
    /// `apply` and are not rolled back. Guard checks allocate no owned data;
    /// their work includes store searches and every writable-ID comparison.
    pub fn apply_guarded(
        &mut self,
        edits: &[TextEdit],
        dependencies: &[&SourceSnapshot],
        writable: &[SnapshotId],
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Vec<SnapshotId>, GuardedEditError> {
        budget.poll()?;
        for (dependency, expected) in dependencies.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            let stored = self
                .latest_with_budget(&expected.identity().source, budget)?
                .ok_or(GuardedEditError::MissingDependency { dependency })?;
            if !expected.eq_with_budget(stored, budget)? {
                return Err(GuardedEditError::ChangedDependency { dependency });
            }
        }
        for (edit, requested) in edits.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            let mut allowed = false;
            for grant in writable {
                if requested
                    .span
                    .snapshot_ref()
                    .compare_with_budget(grant, budget)?
                    == core::cmp::Ordering::Equal
                {
                    allowed = true;
                    break;
                }
            }
            if !allowed {
                return Err(GuardedEditError::NotWritable { edit });
            }
        }
        self.apply(edits, budget, admission).map_err(Into::into)
    }
}
