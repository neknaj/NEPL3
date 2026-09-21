use super::*;

impl RequestLifetimes {
    /// Atomically register admitted sibling calls and suspend their Running parent.
    /// Each digest is computed by the host from that child's authorized context.
    /// Schema, dependency allowlist and result ordering are checked by the caller.
    /// No callback runs here. Failure preserves all IDs and request phases; budget
    /// charges and any reserved capacity remain consumed.
    /// With N existing requests and K children, sorting and publication cost
    /// O(K log K + N + K) work and O(N + K) temporary entry storage. Ancestry,
    /// call validation and input cloning have additional, separately metered costs.
    pub fn suspend_calls(
        &mut self,
        parent: u64,
        continuation: Continuation,
        calls: &[(&Invoke, Digest)],
        b: &mut Budget,
    ) -> Result<(), LifetimeError> {
        let parent_index = self.active(parent, b)?;
        let entry = &self.entries[parent_index];
        if !matches!(entry.state, State::Running) {
            return Err(LifetimeError::Phase);
        }
        continuation.check_binding(&entry.provider, parent, entry.snapshot, b)?;
        let total = self
            .entries
            .len()
            .checked_add(calls.len())
            .ok_or(LifetimeError::Capacity)?;
        let bytes = calls
            .len()
            .checked_mul(core::mem::size_of::<Entry>())
            .ok_or(LifetimeError::Capacity)?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        let mut prepared: Vec<Entry> = Vec::new();
        prepared
            .try_reserve_exact(calls.len())
            .map_err(|_| LifetimeError::Capacity)?;
        for &(call, snapshot) in calls {
            let entry = self.prepare_call(call, snapshot, Some(parent), b)?;
            b.charge(Resource::Work, 1)?;
            prepared.push(entry);
        }
        sort(&mut prepared, b)?;
        for pair in prepared.windows(2) {
            b.charge(Resource::Work, 1)?;
            if pair[0].id == pair[1].id {
                return Err(LifetimeError::DuplicateRequest);
            }
        }
        let mut shifted_parent = parent_index;
        for entry in &prepared {
            b.charge(Resource::Work, 1)?;
            if entry.id < parent {
                shifted_parent += 1;
            }
        }
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Continuation>() as u64,
        )?;
        let bytes = total
            .checked_mul(core::mem::size_of::<Entry>())
            .ok_or(LifetimeError::Capacity)?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        let mut merged = Vec::new();
        merged
            .try_reserve_exact(total)
            .map_err(|_| LifetimeError::Capacity)?;
        // Each entry is moved once; at most one ID comparison per output entry.
        b.charge(
            Resource::Work,
            (total as u64).saturating_mul(2).saturating_add(1),
        )?;
        let continuation = Box::new(continuation);
        // All fallible checks and accounting precede publication.
        let mut existing = core::mem::take(&mut self.entries).into_iter().peekable();
        let mut children = prepared.into_iter().peekable();
        while let (Some(old), Some(new)) = (existing.peek(), children.peek()) {
            let next = if old.id < new.id {
                existing.next()
            } else {
                children.next()
            };
            if let Some(entry) = next {
                merged.push(entry);
            }
        }
        merged.extend(existing);
        merged.extend(children);
        merged[shifted_parent].state = State::Awaiting {
            continuation,
            dependencies: calls.len(),
        };
        self.entries = merged;
        Ok(())
    }
}

// The same budgeted heapsort strategy used by core's source/fact indexes.
// Sorting mutates only staged entries, so a stop cannot affect live requests.
fn sort(entries: &mut [Entry], b: &mut Budget) -> Result<(), LifetimeError> {
    for root in (0..entries.len() / 2).rev() {
        sift(entries, root, b)?;
    }
    for end in (1..entries.len()).rev() {
        b.charge(Resource::Work, 1)?;
        entries.swap(0, end);
        sift(&mut entries[..end], 0, b)?;
    }
    Ok(())
}
fn sift(entries: &mut [Entry], mut root: usize, b: &mut Budget) -> Result<(), LifetimeError> {
    while root < entries.len() / 2 {
        b.charge(Resource::Work, 1)?;
        let mut child = root * 2 + 1;
        if child + 1 < entries.len() {
            b.charge(Resource::Work, 1)?;
            if entries[child].id < entries[child + 1].id {
                child += 1;
            }
        }
        b.charge(Resource::Work, 1)?;
        if entries[root].id >= entries[child].id {
            break;
        }
        b.charge(Resource::Work, 1)?;
        entries.swap(root, child);
        root = child;
    }
    Ok(())
}
