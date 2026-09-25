use super::*;

/// Operation-local secondary index. Source-pool positions and portable ordering
/// remain unchanged. Full identities are compared, starting with fixed fields.
pub(in crate::syntax::shared) struct SourcePositions<'p, 's> {
    pool: &'p Pool<'s, SourceSnapshot>,
    index: Vec<usize>,
}

fn compare(a: &SnapshotId, z: &SnapshotId, b: &mut Budget) -> Result<Ordering, WireError> {
    b.charge(Resource::Work, 33)?;
    let fixed = (a.digest, a.revision).cmp(&(z.digest, z.revision));
    if fixed != Ordering::Equal {
        return Ok(fixed);
    }
    b.charge(
        Resource::Work,
        a.source.0.len().min(z.source.0.len()) as u64 + 1,
    )?;
    Ok(a.source.cmp(&z.source))
}

impl<'p, 's> SourcePositions<'p, 's> {
    pub(in crate::syntax::shared) fn references(
        &self,
        sources: &[SourceSnapshot],
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        let mut positions = Vec::new();
        for source in sources {
            let position = self.position(source.identity(), b)?;
            push(&mut positions, position, b)?;
        }
        self.pool.references_at(positions, true, b)
    }

    pub(in crate::syntax::shared) fn new(
        pool: &'p Pool<'s, SourceSnapshot>,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let count = pool.entries.len();
        let bytes = count
            .checked_mul(core::mem::size_of::<usize>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        let mut index = Vec::new();
        index
            .try_reserve_exact(count)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::Work, count as u64)?;
        index.extend(0..count);
        fn sift(
            index: &mut [usize],
            pool: &Pool<'_, SourceSnapshot>,
            mut root: usize,
            b: &mut Budget,
        ) -> Result<(), WireError> {
            while root < index.len() / 2 {
                let mut child = root * 2 + 1;
                if child + 1 < index.len()
                    && compare(
                        pool.entries[index[child]].0.identity(),
                        pool.entries[index[child + 1]].0.identity(),
                        b,
                    )? == Ordering::Less
                {
                    child += 1;
                }
                if compare(
                    pool.entries[index[root]].0.identity(),
                    pool.entries[index[child]].0.identity(),
                    b,
                )? != Ordering::Less
                {
                    break;
                }
                b.charge(Resource::Work, 1)?;
                index.swap(root, child);
                root = child;
            }
            Ok(())
        }
        for root in (0..count / 2).rev() {
            sift(&mut index, pool, root, b)?;
        }
        for end in (1..count).rev() {
            b.charge(Resource::Work, 1)?;
            index.swap(0, end);
            sift(&mut index[..end], pool, 0, b)?;
        }
        Ok(Self { pool, index })
    }

    pub(super) fn position(&self, wanted: &SnapshotId, b: &mut Budget) -> Result<usize, WireError> {
        b.poll()?;
        let (mut low, mut high) = (0, self.index.len());
        while low < high {
            let mid = low + (high - low) / 2;
            let position = self.index[mid];
            match compare(self.pool.entries[position].0.identity(), wanted, b)? {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => return Ok(position),
            }
        }
        Err(WireError::InvalidType)
    }
}
