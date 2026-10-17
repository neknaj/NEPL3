//! Operation-local ordering reuse; never a source admission or schema receipt.
use super::*;
use alloc::vec::Vec;

#[derive(Default)]
pub(crate) struct OrderCache {
    #[cfg(target_has_atomic = "ptr")]
    previous: Vec<SourceSnapshot>,
    #[cfg(target_has_atomic = "ptr")]
    indices: Vec<usize>,
}

impl OrderCache {
    #[cfg(not(target_has_atomic = "ptr"))]
    pub(crate) fn ordered<'a>(
        &mut self,
        sources: &'a [SourceSnapshot],
        b: &mut Budget,
    ) -> Result<Vec<(&'a SnapshotId, &'a SourceSnapshot)>, WireError> {
        super::super::ordered_sources(sources, b)
    }

    #[cfg(target_has_atomic = "ptr")]
    pub(crate) fn ordered<'a>(
        &mut self,
        sources: &'a [SourceSnapshot],
        b: &mut Budget,
    ) -> Result<Vec<(&'a SnapshotId, &'a SourceSnapshot)>, WireError> {
        b.poll()?;
        let mut prefix = self.previous.len() <= sources.len();
        if prefix {
            for (old, current) in self.previous.iter().zip(sources) {
                b.charge(Resource::Work, 1)?;
                // Owning immutable snapshots prevents stale-address reuse.
                // A mismatch falls back without comparing independently decoded text.
                if !core::ptr::eq(old.identity(), current.identity()) {
                    prefix = false;
                    break;
                }
            }
        }
        let start = if prefix { self.previous.len() } else { 0 };
        let mut tail = Vec::new();
        for index in start..sources.len() {
            push_index(&mut tail, index, b)?;
        }
        super::sort_by_key(&mut tail, |index| sources[*index].identity(), b)?;
        let indices = if prefix {
            let mut merged = Vec::new();
            let (mut left, mut right) = (0, 0);
            while left < self.indices.len() && right < tail.len() {
                let a = self.indices[left];
                let z = tail[right];
                match super::identity(sources[a].identity(), sources[z].identity(), b)? {
                    Ordering::Less => {
                        push_index(&mut merged, a, b)?;
                        left += 1;
                    }
                    Ordering::Greater => {
                        push_index(&mut merged, z, b)?;
                        right += 1;
                    }
                    Ordering::Equal => return Err(SourceError::IdentityConflict.into()),
                }
            }
            for &index in &self.indices[left..] {
                push_index(&mut merged, index, b)?;
            }
            for &index in &tail[right..] {
                push_index(&mut merged, index, b)?;
            }
            merged
        } else {
            tail
        };
        let mut retained = Vec::new();
        for source in sources {
            retained.push(source.clone_with_budget(b)?);
        }
        let mut ordered = Vec::new();
        for &index in &indices {
            b.charge(Resource::Work, 1)?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<(&SnapshotId, &SourceSnapshot)>() as u64,
            )?;
            let current = &sources[index];
            ordered.push((current.identity(), current));
        }
        // A failed ordering, duplicate check or allocation leaves prior metadata intact.
        self.previous = retained;
        self.indices = indices;
        Ok(ordered)
    }
}

#[cfg(target_has_atomic = "ptr")]
fn push_index(values: &mut Vec<usize>, index: usize, b: &mut Budget) -> Result<(), WireError> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<usize>() as u64,
    )?;
    values.push(index);
    Ok(())
}

#[cfg(all(test, target_has_atomic = "ptr"))]
mod tests;
