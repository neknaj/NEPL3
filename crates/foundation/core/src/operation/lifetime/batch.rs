use super::*;

impl RequestLifetimes {
    /// Atomically register admitted sibling calls and suspend their Running parent.
    /// Each digest is computed by the host from that child's authorized context.
    /// Schema, dependency allowlist and result ordering are checked by the caller.
    /// No callback runs here. Failure preserves all IDs and request phases; budget
    /// charges and any reserved capacity remain consumed.
    /// With N existing requests and K children, sorted-vector insertion can move
    /// O(K*K + N*K) entries; these moves are charged before publication.
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
            .checked_mul(core::mem::size_of::<(usize, Entry)>())
            .ok_or(LifetimeError::Capacity)?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        let mut prepared: Vec<(usize, Entry)> = Vec::new();
        prepared
            .try_reserve_exact(calls.len())
            .map_err(|_| LifetimeError::Capacity)?;
        for &(call, snapshot) in calls {
            let entry = self.prepare_call(call, snapshot, Some(parent), b)?;
            let mut lo = 0;
            let mut hi = prepared.len();
            while lo < hi {
                b.charge(Resource::Work, 1)?;
                let mid = lo + (hi - lo) / 2;
                match prepared[mid].1.id.cmp(&entry.id) {
                    core::cmp::Ordering::Less => lo = mid + 1,
                    core::cmp::Ordering::Greater => hi = mid,
                    core::cmp::Ordering::Equal => return Err(LifetimeError::DuplicateRequest),
                }
            }
            let index = self
                .locate(entry.id, b)?
                .err()
                .ok_or(LifetimeError::DuplicateRequest)?;
            b.charge(Resource::Work, (prepared.len() - lo) as u64 + 1)?;
            prepared.insert(lo, (index, entry));
        }
        // Sorted insertion moves only the original suffix: prior siblings have
        // smaller IDs and remain before the next insertion point.
        let mut shifted_parent = parent_index;
        for (index, entry) in &prepared {
            b.charge(Resource::Work, (self.entries.len() - index) as u64 + 1)?;
            if entry.id < parent {
                shifted_parent += 1;
            }
        }
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Continuation>() as u64,
        )?;
        if total > self.entries.capacity() {
            let capacity = self
                .entries
                .capacity()
                .checked_mul(2)
                .ok_or(LifetimeError::Capacity)?
                .max(total);
            let bytes = (capacity - self.entries.capacity())
                .checked_mul(core::mem::size_of::<Entry>())
                .ok_or(LifetimeError::Capacity)?;
            b.charge(Resource::AllocationUnits, bytes as u64)?;
            b.charge(Resource::Work, self.entries.len() as u64)?;
            self.entries
                .try_reserve_exact(capacity - self.entries.len())
                .map_err(|_| LifetimeError::Capacity)?;
        }
        b.charge(Resource::Work, 1)?;
        let continuation = Box::new(continuation);
        // All fallible checks and accounting precede publication.
        for (offset, (index, entry)) in prepared.into_iter().enumerate() {
            self.entries.insert(index + offset, entry);
        }
        self.entries[shifted_parent].state = State::Awaiting {
            continuation,
            dependencies: calls.len(),
        };
        Ok(())
    }
}
