//! Borrowed indexes over expanded HTML occurrences. Sorting meters its own
//! comparisons and swaps; it does not assume a collection's internal algorithm.
use super::HtmlError;
use alloc::vec::Vec;
use core::cmp::Ordering;
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Clone, Copy)]
struct Entry<'a> {
    value: &'a str,
    node: u64,
    order: usize,
}

#[derive(Default)]
pub(crate) struct Identity<'a> {
    ids: Vec<Entry<'a>>,
    links: Vec<Entry<'a>>,
}

fn push<'a>(
    entries: &mut Vec<Entry<'a>>,
    node: u64,
    value: &'a str,
    b: &mut Budget,
) -> Result<(), HtmlError> {
    b.charge(Resource::Work, 1)?;
    if entries.len() == entries.capacity() {
        let capacity = entries
            .capacity()
            .max(4)
            .checked_mul(2)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        let bytes = (capacity - entries.capacity())
            .checked_mul(core::mem::size_of::<Entry<'a>>())
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        b.charge(Resource::Work, entries.len() as u64)?;
        entries
            .try_reserve_exact(capacity - entries.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    entries.push(Entry {
        value,
        node,
        order: entries.len(),
    });
    Ok(())
}

fn compare(a: &str, c: &str, b: &mut Budget) -> Result<Ordering, HtmlError> {
    b.charge(Resource::Work, a.len().min(c.len()) as u64 + 1)?;
    Ok(a.cmp(c))
}

fn entry_compare(a: Entry<'_>, c: Entry<'_>, b: &mut Budget) -> Result<Ordering, HtmlError> {
    let order = compare(a.value, c.value, b)?;
    b.charge(Resource::Work, 1)?;
    Ok(order.then_with(|| a.order.cmp(&c.order)))
}

fn sift(values: &mut [Entry<'_>], mut root: usize, b: &mut Budget) -> Result<(), HtmlError> {
    while root < values.len() / 2 {
        b.charge(Resource::Work, 1)?;
        let mut child = root * 2 + 1;
        if child + 1 < values.len()
            && entry_compare(values[child], values[child + 1], b)? == Ordering::Less
        {
            child += 1;
        }
        if entry_compare(values[root], values[child], b)? != Ordering::Less {
            break;
        }
        b.charge(Resource::Work, 1)?;
        values.swap(root, child);
        root = child;
    }
    Ok(())
}

impl<'a> Identity<'a> {
    pub(crate) fn id(
        &mut self,
        node: u64,
        value: &'a str,
        b: &mut Budget,
    ) -> Result<(), HtmlError> {
        push(&mut self.ids, node, value, b)
    }

    pub(crate) fn link(
        &mut self,
        node: u64,
        value: &'a str,
        b: &mut Budget,
    ) -> Result<(), HtmlError> {
        push(&mut self.links, node, value, b)
    }

    pub(crate) fn finish(mut self, b: &mut Budget) -> Result<(), HtmlError> {
        b.poll()?;
        for root in (0..self.ids.len() / 2).rev() {
            sift(&mut self.ids, root, b)?;
        }
        for end in (1..self.ids.len()).rev() {
            b.charge(Resource::Work, 1)?;
            self.ids.swap(0, end);
            sift(&mut self.ids[..end], 0, b)?;
        }
        // Within each equal-ID group, occurrence order is increasing. Report
        // the earliest repeated occurrence across all groups, including DAGs.
        let mut duplicate: Option<Entry<'_>> = None;
        for pair in self.ids.windows(2) {
            if compare(pair[0].value, pair[1].value, b)? == Ordering::Equal
                && duplicate.is_none_or(|previous| pair[1].order < previous.order)
            {
                duplicate = Some(pair[1]);
            }
        }
        if let Some(entry) = duplicate {
            return Err(HtmlError::DuplicateId(entry.node));
        }
        for link in self.links {
            let (mut lo, mut hi) = (0, self.ids.len());
            let mut found = false;
            while lo < hi {
                b.charge(Resource::Work, 1)?;
                let mid = lo + (hi - lo) / 2;
                match compare(self.ids[mid].value, link.value, b)? {
                    Ordering::Less => lo = mid + 1,
                    Ordering::Greater => hi = mid,
                    Ordering::Equal => {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                return Err(HtmlError::MissingFragment(link.node));
            }
        }
        Ok(())
    }
}
