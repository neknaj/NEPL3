//! Operation-local native indexing. Equality is checked before digest reuse.
use super::*;
use core::cmp::Ordering;
use nepl3_core::{
    origin::{Mapping, MappingKind},
    source::{SnapshotId, SourceSnapshot},
};

fn identity(a: &SnapshotId, z: &SnapshotId, b: &mut Budget) -> Result<Ordering, WireError> {
    // Both identities are immutably borrowed for this operation. Identical
    // storage establishes equality without comparing source-name bytes again.
    // Addresses select work reuse only and never determine portable ordering.
    b.charge(Resource::Work, 1)?;
    if core::ptr::eq(a, z) {
        return Ok(Ordering::Equal);
    }
    b.charge(
        Resource::Work,
        a.source.0.len().min(z.source.0.len()) as u64 + 34,
    )?;
    Ok(a.cmp(z))
}

mod positions;
pub(super) use positions::SourcePositions;
#[cfg(test)]
mod tests;

/// Collapse repeated immutable storage before comparing full source identities.
/// Address order is local scratch only; Pool::new subsequently establishes the
/// content order and checks independently stored declarations for conflicts.
pub(super) fn unique_source_storage<'a>(
    mut inputs: Vec<&'a SourceSnapshot>,
    b: &mut Budget,
) -> Result<Vec<&'a SourceSnapshot>, WireError> {
    b.poll()?;
    let count = inputs.len();
    if count < 2 {
        return Ok(inputs);
    }
    let bytes = count
        .checked_mul(2 * core::mem::size_of::<usize>() + core::mem::size_of::<bool>())
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut order = Vec::new();
    let mut scratch = Vec::new();
    let mut selected = Vec::new();
    order
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    scratch
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    selected
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::Work, count as u64)?;
    order.extend(0..count);
    selected.resize(count, false);
    let mut width = 1_usize;
    while width < count {
        scratch.clear();
        let mut start = 0_usize;
        while start < count {
            let middle = start.saturating_add(width).min(count);
            let end = middle.saturating_add(width).min(count);
            let (mut left, mut right) = (start, middle);
            while left < middle || right < end {
                // Stable merging and fixed work per output slot ensure that
                // addresses affect neither representatives nor charged work.
                b.charge(Resource::Work, 3)?;
                let take_left = left < middle
                    && (right == end
                        || core::ptr::from_ref(inputs[order[left]].identity()).addr()
                            <= core::ptr::from_ref(inputs[order[right]].identity()).addr());
                let at = if take_left {
                    let at = left;
                    left += 1;
                    at
                } else {
                    let at = right;
                    right += 1;
                    at
                };
                scratch.push(order[at]);
            }
            start = end;
        }
        core::mem::swap(&mut order, &mut scratch);
        width = width.saturating_mul(2);
    }
    for at in 0..count {
        b.charge(Resource::Work, 1)?;
        if at == 0
            || !core::ptr::eq(
                inputs[order[at - 1]].identity(),
                inputs[order[at]].identity(),
            )
        {
            selected[order[at]] = true;
        }
    }
    // Restore first occurrence order before content sorting; otherwise address
    // order could change the later content comparison cost.
    let mut retained = 0;
    for at in 0..count {
        b.charge(Resource::Work, 1)?;
        if selected[at] {
            inputs[retained] = inputs[at];
            retained += 1;
        }
    }
    inputs.truncate(retained);
    Ok(inputs)
}

pub(super) trait Content {
    fn compare(&self, other: &Self, b: &mut Budget) -> Result<Ordering, WireError>;
    fn same(&self, other: &Self, b: &mut Budget) -> Result<bool, WireError>;
    fn value(
        &self,
        schema: &SchemaRef,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError>;
}
impl Content for SourceSnapshot {
    fn compare(&self, other: &Self, b: &mut Budget) -> Result<Ordering, WireError> {
        identity(self.identity(), other.identity(), b)
    }
    fn same(&self, other: &Self, b: &mut Budget) -> Result<bool, WireError> {
        b.charge(Resource::Work, 1)?;
        if core::ptr::eq(self.identity(), other.identity()) {
            return Ok(true);
        }
        b.charge(
            Resource::Work,
            self.uri().len().min(other.uri().len()) as u64
                + self.text().len().min(other.text().len()) as u64
                + 2,
        )?;
        Ok(self.uri() == other.uri() && self.text() == other.text())
    }
    fn value(
        &self,
        schema: &SchemaRef,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        let mut value = sources_value(core::slice::from_ref(self), schema, admission, b)?;
        let NdfValue::List(values) = &mut value else {
            return Err(WireError::InvalidType);
        };
        values.pop().ok_or(WireError::InvalidType)
    }
}
/// Comparison keys belong to one immutable source pool. Positions are local
/// search metadata; encoding always uses the complete original Mapping.
pub(super) struct IndexedMapping<'a> {
    mapping: &'a Mapping,
    key: (usize, u64, u64, usize, u64, u64, u8),
}
impl Content for IndexedMapping<'_> {
    fn compare(&self, other: &Self, b: &mut Budget) -> Result<Ordering, WireError> {
        b.charge(Resource::Work, 7)?;
        Ok(self.key.cmp(&other.key))
    }
    fn same(&self, _: &Self, b: &mut Budget) -> Result<bool, WireError> {
        b.charge(Resource::Work, 1)?;
        Ok(true) // Source positions uniquely identify complete snapshot identities.
    }
    fn value(
        &self,
        schema: &SchemaRef,
        _: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        mapping_value(self.mapping, schema, b)
    }
}

pub(super) struct Pool<'a, T> {
    entries: Vec<(&'a T, Digest)>,
    pub values: Vec<Entry>,
}
impl<T> Pool<'_, T> {
    fn find(
        &self,
        mut compare: impl FnMut(&T, &mut Budget) -> Result<Ordering, WireError>,
        b: &mut Budget,
    ) -> Result<usize, WireError> {
        let (mut low, mut high) = (0, self.entries.len());
        while low < high {
            let mid = low + (high - low) / 2;
            match compare(self.entries[mid].0, b)? {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => return Ok(mid),
            }
        }
        Err(WireError::InvalidType)
    }
}
impl SourcePositions<'_, '_> {
    pub fn mapping<'m>(
        &self,
        mapping: &'m Mapping,
        b: &mut Budget,
    ) -> Result<IndexedMapping<'m>, WireError> {
        let source = self.position(mapping.source.snapshot_ref(), b)?;
        let target = self.position(mapping.target.snapshot_ref(), b)?;
        b.charge(Resource::Work, 5)?;
        Ok(IndexedMapping {
            mapping,
            key: (
                source,
                mapping.source.start(),
                mapping.source.end(),
                target,
                mapping.target.start(),
                mapping.target.end(),
                match mapping.kind {
                    MappingKind::Exact => 0,
                    MappingKind::Transformed => 1,
                },
            ),
        })
    }
}
impl<'a, T: Content> Pool<'a, T> {
    pub fn new(
        mut inputs: Vec<&'a T>,
        domain: &[u8],
        schema: &SchemaRef,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        fn sift<T: Content>(
            items: &mut [&T],
            mut root: usize,
            b: &mut Budget,
        ) -> Result<(), WireError> {
            while root < items.len() / 2 {
                let mut child = root * 2 + 1;
                if child + 1 < items.len()
                    && items[child].compare(items[child + 1], b)? == Ordering::Less
                {
                    child += 1;
                }
                if items[root].compare(items[child], b)? != Ordering::Less {
                    break;
                }
                b.charge(Resource::Work, 1)?;
                items.swap(root, child);
                root = child;
            }
            Ok(())
        }
        for root in (0..inputs.len() / 2).rev() {
            sift(&mut inputs, root, b)?;
        }
        for end in (1..inputs.len()).rev() {
            b.charge(Resource::Work, 1)?;
            inputs.swap(0, end);
            sift(&mut inputs[..end], 0, b)?;
        }
        let mut pool = Self {
            entries: Vec::new(),
            values: Vec::new(),
        };
        for input in inputs {
            b.charge(Resource::Work, 1)?;
            if let Some((prior, _)) = pool.entries.last()
                && prior.compare(input, b)? == Ordering::Equal
            {
                if !prior.same(input, b)? {
                    return Err(WireError::InvalidType);
                }
                continue;
            }
            let value = input.value(schema, admission, b)?;
            let digest = crate::encode::digest(domain, &value, b)?;
            push(&mut pool.entries, (input, digest), b)?;
            push(
                &mut pool.values,
                Entry {
                    digest,
                    value,
                    used: false,
                },
                b,
            )?;
        }
        Ok(pool)
    }
    pub fn references(
        &self,
        inputs: &[T],
        canonical: bool,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        let mut positions = Vec::new();
        for input in inputs {
            let position = self.find(|found, b| found.compare(input, b), b)?;
            push(&mut positions, position, b)?;
        }
        self.references_at(positions, canonical, b)
    }
    fn references_at(
        &self,
        mut positions: Vec<usize>,
        canonical: bool,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        // Source lists are already validated unique. Normalize their positions
        // with an explicitly metered O(n log n) heap.
        if canonical {
            fn sift(items: &mut [usize], mut root: usize, b: &mut Budget) -> Result<(), WireError> {
                while root < items.len() / 2 {
                    b.charge(Resource::Work, 3)?;
                    let mut child = root * 2 + 1;
                    if child + 1 < items.len() && items[child] < items[child + 1] {
                        child += 1;
                    }
                    if items[root] >= items[child] {
                        break;
                    }
                    items.swap(root, child);
                    root = child;
                }
                Ok(())
            }
            for root in (0..positions.len() / 2).rev() {
                sift(&mut positions, root, b)?;
            }
            for end in (1..positions.len()).rev() {
                b.charge(Resource::Work, 1)?;
                positions.swap(0, end);
                sift(&mut positions[..end], 0, b)?;
            }
        }
        let mut refs = Vec::new();
        for at in positions {
            push(&mut refs, bytes(&self.entries[at].1.0, b)?, b)?;
        }
        Ok(NdfValue::List(refs))
    }
}
