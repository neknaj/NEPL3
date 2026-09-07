//! Atomic source edits. Every fallible validation and logical charge precedes
//! the final store mutation; admission and consumed work are never rolled back.
use super::*;

struct Plan<'a> {
    source: &'a SourceSnapshot,
    first: usize,
    last: usize,
    revision: u64,
    length: usize,
    digest: Digest,
    admitted: bool,
}
fn comparison(a: &SourceId, b: &SourceId, budget: &mut Budget) -> Result<(), SourceError> {
    budget.charge(
        Resource::Work,
        (a.0.len() as u64)
            .saturating_add(b.0.len() as u64)
            .saturating_add(34),
    )?;
    Ok(())
}
fn allocation<T>(count: usize, budget: &mut Budget) -> Result<(), SourceError> {
    budget.charge(
        Resource::AllocationUnits,
        (count as u64).saturating_mul(core::mem::size_of::<T>() as u64),
    )?;
    Ok(())
}
impl SourceStore {
    /// Atomically creates one next revision for each edited source. The admission
    /// ledger is shared with the caller's operation, including both original and
    /// generated snapshots. A failed transaction keeps resource charges but makes
    /// no source-store changes. Returned IDs are ordered by SourceId.
    pub fn apply(
        &mut self,
        edits: &[TextEdit],
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Vec<SnapshotId>, SourceError> {
        budget.charge(Resource::Work, 1)?;
        allocation::<&TextEdit>(edits.len(), budget)?;
        let mut sorted: Vec<&TextEdit> = Vec::with_capacity(edits.len());
        // Insertion sort permits cancellation/limits before every comparison and
        // only moves borrowed pointers; no owned sort keys or hidden scratch heap.
        for edit in edits {
            sorted.push(edit);
            let mut position = sorted.len() - 1;
            while position > 0 {
                let prior = sorted[position - 1];
                comparison(
                    &prior.span.snapshot.source,
                    &edit.span.snapshot.source,
                    budget,
                )?;
                if (
                    &prior.span.snapshot.source,
                    prior.span.start,
                    prior.span.end,
                ) <= (&edit.span.snapshot.source, edit.span.start, edit.span.end)
                {
                    break;
                }
                sorted.swap(position - 1, position);
                position -= 1;
            }
        }
        let mut plans = Vec::new();
        let mut cursor = 0;
        while cursor < sorted.len() {
            let id = &sorted[cursor].span.snapshot;
            let mut latest: Option<&SourceSnapshot> = None;
            for source in &self.snapshots {
                comparison(&id.source, &source.storage.id.source, budget)?;
                if source.storage.id.source == id.source
                    && latest.is_none_or(|v| v.storage.id.revision < source.storage.id.revision)
                {
                    latest = Some(source);
                }
            }
            let source = latest.ok_or(SourceError::MissingSnapshot)?;
            comparison(&id.source, &source.storage.id.source, budget)?;
            if source.storage.id != *id {
                return Err(SourceError::SnapshotMismatch);
            }
            admission.admit_existing(source, budget)?;
            let revision = id.revision.checked_add(1).ok_or(SourceError::Revision)?;
            let mut next = cursor + 1;
            while next < sorted.len() {
                comparison(&sorted[next].span.snapshot.source, &id.source, budget)?;
                if sorted[next].span.snapshot.source != id.source {
                    break;
                }
                next += 1;
            }
            let mut end = 0;
            let mut previous_start = None;
            let mut length = source.storage.text.len() as u64;
            for edit in &sorted[cursor..next] {
                comparison(
                    &edit.span.snapshot.source,
                    &source.storage.id.source,
                    budget,
                )?;
                let expected = source.slice(&edit.span)?;
                budget.charge(Resource::Work, expected.len() as u64)?;
                if Digest::of(expected.as_bytes()) != edit.expected_digest {
                    return Err(SourceError::ExpectedDigest);
                }
                if edit.span.start < end || previous_start == Some(edit.span.start) {
                    return Err(SourceError::OverlappingEdits);
                }
                length = length
                    .checked_sub(edit.span.end - edit.span.start)
                    .and_then(|n| n.checked_add(edit.replacement.len() as u64))
                    .ok_or_else(|| SourceError::Stopped(budget.stop(StopReason::SourceLimit)))?;
                end = edit.span.end;
                previous_start = Some(edit.span.start);
            }
            // Determine the exact generated identity before allocating its text.
            budget.charge(Resource::Work, length)?;
            let mut hash = Sha256::new();
            end = 0;
            for edit in &sorted[cursor..next] {
                hash.update(
                    source
                        .storage
                        .text
                        .get(end as usize..edit.span.start as usize)
                        .ok_or(SourceError::Bounds)?
                        .as_bytes(),
                );
                hash.update(edit.replacement.as_bytes());
                end = edit.span.end;
            }
            hash.update(
                source
                    .storage
                    .text
                    .get(end as usize..)
                    .ok_or(SourceError::Bounds)?
                    .as_bytes(),
            );
            let digest = Digest(hash.finalize().into());
            let length = usize::try_from(length)
                .map_err(|_| SourceError::Stopped(budget.stop(StopReason::AllocationLimit)))?;
            allocation::<Plan<'_>>(1, budget)?;
            plans.push(Plan {
                source,
                first: cursor,
                last: next,
                revision,
                length,
                digest,
                admitted: false,
            });
            cursor = next;
        }
        // All semantic failures, including a conflict with an independently
        // reserved output, precede any publication of generated identities.
        for plan in &mut plans {
            plan.admitted = admission.check_parts(
                &plan.source.storage.id.source,
                plan.revision,
                plan.digest,
                &plan.source.storage.uri,
                budget,
            )?;
        }
        let mut admitted_outputs = Vec::new();
        for plan in &plans {
            if !plan.admitted {
                budget.charge(Resource::SourceBytes, plan.length as u64)?;
                budget.charge(
                    Resource::Work,
                    (plan.source.storage.id.source.0.len() as u64)
                        .saturating_add(plan.source.storage.uri.len() as u64)
                        .saturating_add(1),
                )?;
                // Temporary ownership and the final admission-ledger slot.
                allocation::<(SnapshotId, String)>(2, budget)?;
                budget.charge(
                    Resource::AllocationUnits,
                    (plan.source.storage.id.source.0.len() as u64)
                        .saturating_add(plan.source.storage.uri.len() as u64),
                )?;
                admitted_outputs.push((
                    SnapshotId {
                        source: plan.source.storage.id.source.clone(),
                        revision: plan.revision,
                        digest: plan.digest,
                    },
                    plan.source.storage.uri.clone(),
                ));
            }
        }
        allocation::<SourceSnapshot>(plans.len(), budget)?;
        allocation::<SnapshotId>(plans.len(), budget)?;
        // Reserve logical ownership for the additional final store entries too.
        allocation::<SourceSnapshot>(plans.len(), budget)?;
        let mut prepared = Vec::with_capacity(plans.len());
        let mut ids = Vec::with_capacity(plans.len());
        for plan in &plans {
            let source = plan.source;
            budget.charge(
                Resource::Work,
                (plan.length as u64)
                    .saturating_add((source.storage.id.source.0.len() as u64).saturating_mul(2))
                    .saturating_add(source.storage.uri.len() as u64),
            )?;
            budget.charge(
                Resource::AllocationUnits,
                (plan.length as u64)
                    .saturating_add((source.storage.id.source.0.len() as u64).saturating_mul(2))
                    .saturating_add(source.storage.uri.len() as u64),
            )?;
            let mut output = String::with_capacity(plan.length);
            let mut end = 0;
            for edit in &sorted[plan.first..plan.last] {
                output.push_str(
                    source
                        .storage
                        .text
                        .get(end..edit.span.start as usize)
                        .ok_or(SourceError::Bounds)?,
                );
                output.push_str(&edit.replacement);
                end = edit.span.end as usize;
            }
            output.push_str(source.storage.text.get(end..).ok_or(SourceError::Bounds)?);
            let id = SnapshotId {
                source: source.storage.id.source.clone(),
                revision: plan.revision,
                digest: plan.digest,
            };
            ids.push(id.clone());
            prepared.push(SourceSnapshot::from_parts(
                id,
                source.storage.uri.clone(),
                output,
                budget,
            )?);
        }
        allocation::<usize>(self.index.len().saturating_add(prepared.len()), budget)?;
        budget.charge(Resource::Work, self.index.len() as u64)?;
        let mut index = Vec::with_capacity(self.index.len().saturating_add(prepared.len()));
        index.extend_from_slice(&self.index);
        for (offset, snapshot) in prepared.iter().enumerate() {
            let at = source_index_position(
                &index,
                |i| {
                    if i < self.snapshots.len() {
                        &self.snapshots[i]
                    } else {
                        &prepared[i - self.snapshots.len()]
                    }
                },
                snapshot,
                budget,
            )?
            .map_or_else(Ok, |_| Err(SourceError::IdentityConflict))?;
            budget.charge(Resource::Work, (index.len() - at) as u64)?;
            index.insert(at, self.snapshots.len() + offset);
        }
        allocation::<usize>(
            admission.index.len().saturating_add(admitted_outputs.len()),
            budget,
        )?;
        budget.charge(Resource::Work, admission.index.len() as u64)?;
        let mut admission_index =
            Vec::with_capacity(admission.index.len().saturating_add(admitted_outputs.len()));
        admission_index.extend_from_slice(&admission.index);
        for (offset, (id, _)) in admitted_outputs.iter().enumerate() {
            let at = admission_index_position(
                &admission_index,
                |i| {
                    if i < admission.admitted.len() {
                        &admission.admitted[i].0
                    } else {
                        &admitted_outputs[i - admission.admitted.len()].0
                    }
                },
                &id.source,
                id.revision,
                budget,
            )?
            .map_or_else(Ok, |_| Err(SourceError::IdentityConflict))?;
            budget.charge(Resource::Work, (admission_index.len() - at) as u64)?;
            admission_index.insert(at, admission.admitted.len() + offset);
        }
        // No typed failure or budget charge can occur after this commit point.
        admission.admitted.extend(admitted_outputs);
        admission.index = admission_index;
        self.snapshots.extend(prepared);
        self.index = index;
        Ok(ids)
    }
}
