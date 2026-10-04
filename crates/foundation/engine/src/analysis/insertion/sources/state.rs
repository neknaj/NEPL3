//! Read-only comparison with an explicit store, without external-currentness claims.
use super::{SourceEvidence, SourceInventory};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    source::{SourceError, SourceSnapshot, SourceStore},
};

#[derive(Clone, Copy, Debug)]
pub enum StoredState<'s> {
    /// The complete snapshot equals the greatest retained revision.
    Latest { stored: &'s SourceSnapshot },
    /// Equal complete snapshot, with a greater revision retained in this store.
    Historical {
        stored: &'s SourceSnapshot,
        latest: &'s SourceSnapshot,
    },
    /// The same source/revision is retained with different bytes or metadata.
    Conflict { stored: &'s SourceSnapshot },
    /// No matching source/revision; another revision may be retained.
    Missing { latest: Option<&'s SourceSnapshot> },
}
#[derive(Clone, Copy, Debug)]
pub struct Observation<'i, 's> {
    pub evidence: &'i SourceEvidence<'i>,
    pub state: StoredState<'s>,
}
/// Complete ordered comparison, borrowing both inputs. Store mutation requires
/// ending this borrow. A later external host mutation is outside this guarantee.
pub struct SourceComparison<'i, 's> {
    inventory: &'i SourceInventory<'i>,
    store: &'s SourceStore,
    observations: Vec<Observation<'i, 's>>,
    report: Report,
}
impl<'i, 's> SourceComparison<'i, 's> {
    pub fn inventory(&self) -> &'i SourceInventory<'i> {
        self.inventory
    }
    pub fn store(&self) -> &'s SourceStore {
        self.store
    }
    pub fn observations(&self) -> &[Observation<'i, 's>] {
        &self.observations
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
#[derive(Debug)]
pub enum ComparisonError {
    LimitsMismatch,
    Source(SourceError),
    Stopped(StopReason),
}
impl From<StopReason> for ComparisonError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<SourceError> for ComparisonError {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}
impl ComparisonError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(reason) | Self::Source(SourceError::Stopped(reason)) => Some(*reason),
            _ => None,
        }
    }
}
/// Classify every retained occurrence without selecting a freshness policy.
/// Candidate-only generated revisions may legitimately be Missing; historical
/// retained evidence may legitimately be Historical. This operation performs no
/// admission, edit, authority check, dependency-completeness or host-context check.
/// Continue the cumulative Budget; matching Limits do not prove continuity.
pub fn compare<'i, 's>(
    inventory: &'i SourceInventory<'i>,
    store: &'s SourceStore,
    b: &mut Budget,
) -> Result<SourceComparison<'i, 's>, ComparisonError> {
    if b.limits() != inventory.limits {
        return Err(ComparisonError::LimitsMismatch);
    }
    b.poll()?;
    let mut observations = Vec::new();
    for evidence in inventory.entries() {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        let id = evidence.source.identity();
        let stored = store.get_revision_with_budget(&id.source, id.revision, b)?;
        let state = match stored {
            Some(stored) if !evidence.source.eq_with_budget(stored, b)? => {
                StoredState::Conflict { stored }
            }
            Some(stored) => {
                let latest = store
                    .latest_with_budget(&id.source, b)?
                    .ok_or(ComparisonError::Source(SourceError::MissingSnapshot))?;
                if core::ptr::eq(stored, latest) {
                    StoredState::Latest { stored }
                } else {
                    StoredState::Historical { stored, latest }
                }
            }
            None => StoredState::Missing {
                latest: store.latest_with_budget(&id.source, b)?,
            },
        };
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Observation<'i, 's>>() as u64,
        )?;
        observations.push(Observation { evidence, state });
    }
    Ok(SourceComparison {
        inventory,
        store,
        observations,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
