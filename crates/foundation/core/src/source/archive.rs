//! Operation-local source table for an explicit archive decoder.
//!
//! This native helper is not a wire format or a syntax-validation proof. A
//! decoder must validate its own version/schema, decode checked snapshots once,
//! resolve each bundle's explicit memberships, validate the resulting bundles,
//! and call `finish` before publishing the archive result. No ambient store is
//! consulted and no source is implicitly added to a bundle.
use super::{SourceAdmission, SourceError, SourceRef, SourceSnapshot};
use crate::budget::{Budget, Resource, StopReason};
use alloc::vec::Vec;
use core::cmp::Ordering;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveSourceError {
    Source(SourceError),
    NonCanonical,
    MissingMembership,
    UnusedSource,
}
impl From<SourceError> for ArchiveSourceError {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}
impl From<StopReason> for ArchiveSourceError {
    fn from(error: StopReason) -> Self {
        Self::Source(SourceError::Stopped(error))
    }
}

/// Borrowed immutable snapshots and operation-local membership accounting.
/// Construction accepts only an already canonical, duplicate-free table.
#[derive(Debug)]
pub struct ArchiveSources<'a> {
    sources: &'a [SourceSnapshot],
    used: Vec<bool>,
}
impl<'a> ArchiveSources<'a> {
    pub fn new(
        sources: &'a [SourceSnapshot],
        admission: &mut SourceAdmission,
        budget: &mut Budget,
    ) -> Result<Self, ArchiveSourceError> {
        budget.poll()?;
        for (index, source) in sources.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            if index > 0 {
                let previous = sources[index - 1].identity();
                let current = source.identity();
                let order = previous.compare_with_budget(current, budget)?;
                budget.charge(
                    Resource::Work,
                    previous.source.0.len().min(current.source.0.len()) as u64 + 8,
                )?;
                if order != Ordering::Less
                    || (previous.source == current.source && previous.revision == current.revision)
                {
                    return Err(ArchiveSourceError::NonCanonical);
                }
            }
            admission.admit_existing(source, budget)?;
        }
        budget.charge(Resource::AllocationUnits, sources.len() as u64)?;
        let used = alloc::vec![false; sources.len()];
        budget.poll()?;
        Ok(Self { sources, used })
    }

    /// Materialize precisely this membership list using checked snapshot clones.
    /// Each list must itself be ordered and unique. Empty means no local sources,
    /// even when the archive table contains sources used by another bundle.
    pub fn resolve(
        &mut self,
        members: &[SourceRef],
        budget: &mut Budget,
    ) -> Result<Vec<SourceSnapshot>, ArchiveSourceError> {
        budget.poll()?;
        let mut indices = Vec::new();
        let mut result = Vec::new();
        let mut previous = None;
        for member in members {
            budget.charge(Resource::Work, 1)?;
            let index = self
                .find(member, budget)?
                .ok_or(ArchiveSourceError::MissingMembership)?;
            if previous.is_some_and(|prior| prior >= index) {
                return Err(ArchiveSourceError::NonCanonical);
            }
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<usize>() as u64,
            )?;
            indices.push(index);
            result.push(self.sources[index].clone_with_budget(budget)?);
            previous = Some(index);
        }
        // Charge before marking: a failed call does not make unused entries
        // appear referenced, even if the caller retries with a fresh budget.
        budget.charge(Resource::Work, indices.len() as u64)?;
        for index in indices {
            self.used[index] = true;
        }
        Ok(result)
    }

    fn find(&self, member: &SourceRef, budget: &mut Budget) -> Result<Option<usize>, StopReason> {
        let (mut low, mut high) = (0, self.sources.len());
        while low < high {
            let middle = low + (high - low) / 2;
            let id = self.sources[middle].identity();
            budget.charge(
                Resource::Work,
                id.source.0.len().min(member.source_id.0.len()) as u64 + 41,
            )?;
            let order = id
                .source
                .cmp(&member.source_id)
                .then_with(|| id.revision.cmp(&member.revision))
                .then_with(|| id.digest.cmp(&member.digest));
            match order {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Some(middle)),
            }
        }
        Ok(None)
    }

    /// Reject non-minimal archives after every bundle has been decoded.
    /// This says nothing about the validity of those bundles' syntax or origins.
    pub fn finish(self, budget: &mut Budget) -> Result<(), ArchiveSourceError> {
        budget.charge(Resource::Work, self.used.len() as u64)?;
        if self.used.iter().any(|used| !used) {
            return Err(ArchiveSourceError::UnusedSource);
        }
        budget.poll()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
