//! Stable class union. Values retain first-occurrence order; an auxiliary
//! budgeted heap index groups equal names without repeated linear searches.
use super::*;
use core::cmp::Ordering;

#[derive(Default)]
pub(super) struct Classes(Vec<String>);

impl Classes {
    pub(super) fn push(&mut self, name: String, b: &mut Budget) -> Result<(), Error> {
        b.charge(Resource::Work, 1)?;
        if self.0.len() == self.0.capacity() {
            let capacity = self
                .0
                .capacity()
                .max(1)
                .checked_mul(2)
                .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
            let bytes = capacity
                .checked_mul(core::mem::size_of::<String>())
                .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
            b.charge(Resource::AllocationUnits, bytes as u64)?;
            b.charge(Resource::Work, self.0.len() as u64)?;
            self.0
                .try_reserve_exact(capacity - self.0.len())
                .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        }
        self.0.push(name);
        Ok(())
    }

    pub(super) fn finish(mut self, b: &mut Budget) -> Result<Vec<String>, Error> {
        b.poll()?;
        let count = self.0.len();
        let bytes = count
            .checked_mul(core::mem::size_of::<usize>() + core::mem::size_of::<bool>())
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        let mut order = Vec::new();
        let mut keep = Vec::new();
        order
            .try_reserve_exact(count)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        keep.try_reserve_exact(count)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::Work, (count as u64).saturating_mul(2))?;
        order.extend(0..count);
        keep.resize(count, true);
        let compare = |left: usize, right: usize, b: &mut Budget| -> Result<Ordering, Error> {
            let a = &self.0[left];
            let z = &self.0[right];
            b.charge(Resource::Work, a.len().min(z.len()) as u64 + 2)?;
            Ok(a.cmp(z).then_with(|| left.cmp(&right)))
        };
        fn sift(
            indices: &mut [usize],
            mut root: usize,
            compare: &impl Fn(usize, usize, &mut Budget) -> Result<Ordering, Error>,
            b: &mut Budget,
        ) -> Result<(), Error> {
            while root < indices.len() / 2 {
                b.charge(Resource::Work, 1)?;
                let mut child = root * 2 + 1;
                if child + 1 < indices.len()
                    && compare(indices[child], indices[child + 1], b)? == Ordering::Less
                {
                    child += 1;
                }
                if compare(indices[root], indices[child], b)? != Ordering::Less {
                    break;
                }
                b.charge(Resource::Work, 1)?;
                indices.swap(root, child);
                root = child;
            }
            Ok(())
        }
        for root in (0..count / 2).rev() {
            sift(&mut order, root, &compare, b)?;
        }
        for end in (1..count).rev() {
            b.charge(Resource::Work, 1)?;
            order.swap(0, end);
            sift(&mut order[..end], 0, &compare, b)?;
        }
        for pair in order.windows(2) {
            let a = &self.0[pair[0]];
            let z = &self.0[pair[1]];
            b.charge(Resource::Work, a.len().min(z.len()) as u64 + 1)?;
            if a == z {
                keep[pair[1]] = false;
            }
        }
        // Charge the bounded compaction before mutating the output sequence.
        b.charge(Resource::Work, count as u64)?;
        let mut index = 0;
        self.0.retain(|_| {
            let yes = keep[index];
            index += 1;
            yes
        });
        Ok(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, vec};
    use nepl3_core::budget::Limits;

    fn budget() -> Budget {
        Budget::new(Limits {
            work: 100_000_000,
            allocation_units: 100_000_000,
            ..Limits::default()
        })
    }
    fn union(input: &[String], b: &mut Budget) -> Result<Vec<String>, Error> {
        let mut classes = Classes::default();
        for name in input {
            classes.push(name.clone(), b)?;
        }
        classes.finish(b)
    }
    #[test]
    fn union_preserves_first_occurrence_and_sticky_boundaries() -> Result<(), Error> {
        let input = vec!["z".into(), "a".into(), "z".into(), "aa".into(), "a".into()];
        let mut measured = budget();
        let expected = union(&input, &mut measured)?;
        assert_eq!(expected, ["z", "a", "aa"]);
        assert!(union(&[], &mut budget())?.is_empty());
        for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = measured.usage().work,
                StopReason::AllocationLimit => {
                    limits.allocation_units = measured.usage().allocation_units
                }
                _ => return Err(Error::InternalShape),
            }
            assert_eq!(union(&input, &mut Budget::new(limits))?, expected);
            match reason {
                StopReason::WorkLimit => limits.work -= 1,
                StopReason::AllocationLimit => limits.allocation_units -= 1,
                _ => return Err(Error::InternalShape),
            }
            let mut limited = Budget::new(limits);
            assert!(
                matches!(union(&input, &mut limited), Err(Error::Stopped(actual)) if actual == reason)
            );
            assert_eq!(limited.poll(), Err(reason));
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            union(&[], &mut cancelled),
            Err(Error::Stopped(StopReason::Cancelled))
        ));
        Ok(())
    }
    #[test]
    fn union_work_scales_with_names_occurrences_and_name_length() -> Result<(), Error> {
        let mut prior = 0;
        for count in [128, 256, 512] {
            let input: Vec<_> = (0..count).rev().map(|n| format!("class-{n:04}")).collect();
            let mut b = budget();
            assert_eq!(union(&input, &mut b)?, input);
            let work = b.usage().work;
            if prior != 0 {
                assert!(work < prior * 3, "quadratic union growth");
            }
            prior = work;
        }
        let mut short = budget();
        let mut long = budget();
        let mut prior_duplicates = 0;
        for count in [128, 256, 512] {
            let input: Vec<_> = (0..count).map(|n| format!("repeat-{:04}", n % 8)).collect();
            let mut b = budget();
            assert_eq!(union(&input, &mut b)?.len(), 8);
            if prior_duplicates != 0 {
                assert!(
                    b.usage().work < prior_duplicates * 3,
                    "repeated class growth"
                );
            }
            prior_duplicates = b.usage().work;
        }
        for (b, prefix) in [
            (&mut short, "x"),
            (&mut long, "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
        ] {
            let input: Vec<_> = (0..256).map(|n| format!("{prefix}-{:04}", n % 8)).collect();
            assert_eq!(union(&input, b)?.len(), 8);
        }
        assert!(long.usage().work > short.usage().work);
        Ok(())
    }
}
