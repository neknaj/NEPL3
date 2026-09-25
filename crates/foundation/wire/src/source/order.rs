//! Canonical source ordering with interruptible, allocation-free heapsort.
use super::{SourceError, SourceRef, SourceSnapshot, WireError};
use core::cmp::Ordering;
use nepl3_core::budget::{Budget, Resource, StopReason};

fn identity(a: &SourceRef, z: &SourceRef, b: &mut Budget) -> Result<Ordering, StopReason> {
    // Bound the bytes compared, including equal prefixes and empty identifiers.
    b.charge(
        Resource::Work,
        a.source_id.0.len().min(z.source_id.0.len()) as u64 + 1,
    )?;
    let order = a.source_id.cmp(&z.source_id);
    if order != Ordering::Equal {
        return Ok(order);
    }
    b.charge(Resource::Work, 1)?;
    Ok(a.revision.cmp(&z.revision))
}

fn compare(a: &SourceRef, z: &SourceRef, b: &mut Budget) -> Result<Ordering, StopReason> {
    let order = identity(a, z, b)?;
    if order != Ordering::Equal {
        return Ok(order);
    }
    b.charge(Resource::Work, 32)?;
    Ok(a.digest.cmp(&z.digest))
}

pub(super) fn sort(
    entries: &mut [(SourceRef, &SourceSnapshot)],
    b: &mut Budget,
) -> Result<(), WireError> {
    fn sift(
        entries: &mut [(SourceRef, &SourceSnapshot)],
        mut root: usize,
        b: &mut Budget,
    ) -> Result<(), StopReason> {
        while root < entries.len() / 2 {
            let mut child = root * 2 + 1;
            if child + 1 < entries.len()
                && compare(&entries[child].0, &entries[child + 1].0, b)? == Ordering::Less
            {
                child += 1;
            }
            if compare(&entries[root].0, &entries[child].0, b)? != Ordering::Less {
                break;
            }
            b.charge(Resource::Work, 1)?;
            entries.swap(root, child);
            root = child;
        }
        Ok(())
    }
    b.poll()?;
    // Source tables produced by prior canonical boundaries are already ordered.
    // A linear checked pass also establishes uniqueness for that case.
    let mut ordered = true;
    for pair in entries.windows(2) {
        match identity(&pair[0].0, &pair[1].0, b)? {
            Ordering::Less => {}
            Ordering::Equal => return Err(SourceError::IdentityConflict.into()),
            Ordering::Greater => {
                ordered = false;
                break;
            }
        }
    }
    if ordered {
        return Ok(());
    }
    for root in (0..entries.len() / 2).rev() {
        sift(entries, root, b)?;
    }
    for end in (1..entries.len()).rev() {
        b.charge(Resource::Work, 1)?;
        entries.swap(0, end);
        sift(&mut entries[..end], 0, b)?;
    }
    for pair in entries.windows(2) {
        if identity(&pair[0].0, &pair[1].0, b)? == Ordering::Equal {
            return Err(SourceError::IdentityConflict.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
