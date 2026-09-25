//! Operation-local native indexing. Equality is checked before digest reuse.
use super::*;
use core::cmp::Ordering;
use nepl3_core::{
    origin::{Mapping, MappingKind},
    source::{SnapshotId, SourceSnapshot},
};

fn identity(a: &SnapshotId, z: &SnapshotId, b: &mut Budget) -> Result<Ordering, WireError> {
    b.charge(
        Resource::Work,
        a.source.0.len().min(z.source.0.len()) as u64 + 34,
    )?;
    Ok(a.cmp(z))
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
impl Content for Mapping {
    fn compare(&self, other: &Self, b: &mut Budget) -> Result<Ordering, WireError> {
        for (a, z) in [(&self.source, &other.source), (&self.target, &other.target)] {
            let order = identity(a.snapshot_ref(), z.snapshot_ref(), b)?;
            if order != Ordering::Equal {
                return Ok(order);
            }
            b.charge(Resource::Work, 2)?;
            let order = (a.start(), a.end()).cmp(&(z.start(), z.end()));
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        b.charge(Resource::Work, 1)?;
        let rank = |kind| match kind {
            MappingKind::Exact => 0,
            MappingKind::Transformed => 1,
        };
        Ok(rank(self.kind).cmp(&rank(other.kind)))
    }
    fn same(&self, _: &Self, b: &mut Budget) -> Result<bool, WireError> {
        b.charge(Resource::Work, 1)?;
        Ok(true) // compare includes both complete spans and mapping kind.
    }
    fn value(
        &self,
        schema: &SchemaRef,
        _: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        mapping_value(self, schema, b)
    }
}

pub(super) struct Pool<'a, T> {
    entries: Vec<(&'a T, Digest)>,
    pub values: Vec<Entry>,
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
            let (mut low, mut high) = (0, self.entries.len());
            while low < high {
                let mid = low + (high - low) / 2;
                match self.entries[mid].0.compare(input, b)? {
                    Ordering::Less => low = mid + 1,
                    Ordering::Greater => high = mid,
                    Ordering::Equal => {
                        low = mid;
                        break;
                    }
                }
            }
            let (found, _) = self.entries.get(low).ok_or(WireError::InvalidType)?;
            if found.compare(input, b)? != Ordering::Equal {
                return Err(WireError::InvalidType);
            }
            push(&mut positions, low, b)?;
        }
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
