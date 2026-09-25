//! Local immutable tables. Sorting and lookup compare complete identities.
use super::*;
use core::cmp::Ordering;

pub(super) struct Index<'a, T> {
    pub values: Vec<&'a T>,
    compare: fn(&T, &T, &mut Budget) -> Result<Ordering, WireError>,
}
impl<'a, T> Index<'a, T> {
    pub fn new(
        mut values: Vec<&'a T>,
        compare: fn(&T, &T, &mut Budget) -> Result<Ordering, WireError>,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        fn sift<T>(
            values: &mut [&T],
            mut root: usize,
            compare: fn(&T, &T, &mut Budget) -> Result<Ordering, WireError>,
            b: &mut Budget,
        ) -> Result<(), WireError> {
            while root < values.len() / 2 {
                let mut child = root * 2 + 1;
                if child + 1 < values.len()
                    && compare(values[child], values[child + 1], b)? == Ordering::Less
                {
                    child += 1;
                }
                if compare(values[root], values[child], b)? != Ordering::Less {
                    break;
                }
                b.charge(Resource::Work, 1)?;
                values.swap(root, child);
                root = child;
            }
            Ok(())
        }
        b.poll()?;
        for root in (0..values.len() / 2).rev() {
            sift(&mut values, root, compare, b)?;
        }
        for end in (1..values.len()).rev() {
            b.charge(Resource::Work, 1)?;
            values.swap(0, end);
            sift(&mut values[..end], 0, compare, b)?;
        }
        let mut retained = 0;
        for at in 0..values.len() {
            if retained == 0 || compare(values[retained - 1], values[at], b)? != Ordering::Equal {
                b.charge(Resource::Work, 1)?;
                values[retained] = values[at];
                retained += 1;
            }
        }
        values.truncate(retained);
        Ok(Self { values, compare })
    }
    pub fn find(&self, value: &T, b: &mut Budget) -> Result<u64, WireError> {
        let (mut low, mut high) = (0, self.values.len());
        while low < high {
            let mid = low + (high - low) / 2;
            match (self.compare)(self.values[mid], value, b)? {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => return Ok(mid as u64),
            }
        }
        Err(WireError::InvalidType)
    }
}
pub(super) fn schema_order(
    a: &SchemaRef,
    z: &SchemaRef,
    b: &mut Budget,
) -> Result<Ordering, WireError> {
    b.charge(
        Resource::Work,
        a.package.len().min(z.package.len()) as u64 + 34,
    )?;
    Ok(a.package
        .cmp(&z.package)
        .then(a.revision.cmp(&z.revision))
        .then(a.digest.cmp(&z.digest)))
}
pub(super) fn source_order(
    a: &nepl3_core::source::SnapshotId,
    z: &nepl3_core::source::SnapshotId,
    b: &mut Budget,
) -> Result<Ordering, WireError> {
    Ok(a.compare_with_budget(z, b)?)
}
