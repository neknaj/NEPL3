//! Duplicate lookup uses scratch indices; observable field order is unchanged.
use super::*;
use core::cmp::Ordering;

fn compare(
    fields: &[ViewField],
    a: usize,
    z: usize,
    b: &mut Budget,
) -> Result<Ordering, StopReason> {
    let (a, z) = (&fields[a].name, &fields[z].name);
    b.charge(Resource::Work, a.len().min(z.len()) as u64 + 1)?;
    #[cfg(test)]
    tests::COMPARISONS.with(|count| count.set(count.get() + 1));
    Ok(a.cmp(z))
}

pub(super) fn first_duplicate(
    fields: &[ViewField],
    b: &mut Budget,
) -> Result<Option<usize>, StopReason> {
    b.poll()?;
    if fields.len() < 2 {
        return Ok(fields.first().filter(|f| f.name.is_empty()).map(|_| 0));
    }
    let bytes = fields
        .len()
        .checked_mul(core::mem::size_of::<usize>())
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut order = Vec::new();
    order
        .try_reserve_exact(fields.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    let mut first = None;
    for (index, field) in fields.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        order.push(index);
        if field.name.is_empty() && first.is_none() {
            first = Some(index);
        }
    }
    fn sift(
        order: &mut [usize],
        fields: &[ViewField],
        mut root: usize,
        b: &mut Budget,
    ) -> Result<(), StopReason> {
        while root < order.len() / 2 {
            let mut child = root * 2 + 1;
            if child + 1 < order.len()
                && compare(fields, order[child], order[child + 1], b)? == Ordering::Less
            {
                child += 1;
            }
            if compare(fields, order[root], order[child], b)? != Ordering::Less {
                break;
            }
            b.charge(Resource::Work, 1)?;
            order.swap(root, child);
            root = child;
        }
        Ok(())
    }
    for root in (0..order.len() / 2).rev() {
        sift(&mut order, fields, root, b)?;
    }
    for end in (1..order.len()).rev() {
        b.charge(Resource::Work, 1)?;
        order.swap(0, end);
        sift(&mut order[..end], fields, 0, b)?;
    }
    let mut earliest = order[0];
    for pair in order.windows(2) {
        if compare(fields, pair[0], pair[1], b)? == Ordering::Equal {
            let duplicate = earliest.max(pair[1]);
            first = Some(first.map_or(duplicate, |first| first.min(duplicate)));
            earliest = earliest.min(pair[1]);
        } else {
            earliest = pair[1];
        }
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::budget::Limits;
    use alloc::{format, vec};
    use core::cell::Cell;

    std::thread_local! {
        pub(super) static COMPARISONS: Cell<u64> = const { Cell::new(0) };
    }

    fn budget() -> Budget {
        Budget::new(Limits {
            work: 100_000_000,
            allocation_units: 1_000_000,
            ..Limits::default()
        })
    }

    fn fields(names: &[&str]) -> Vec<ViewField> {
        names
            .iter()
            .map(|name| ViewField {
                name: (*name).into(),
                children: vec![],
            })
            .collect()
    }

    #[test]
    fn earliest_invalid_field_preserves_declaration_order() -> Result<(), StopReason> {
        // The first invalid declaration is the second occurrence in input order,
        // irrespective of lexical order or the heapsort order of equal keys.
        for (names, expected) in [
            (vec!["z", "a", "z", "a"], Some(2)),
            (vec!["z", "a", "a", "z", "a"], Some(2)),
            (vec!["z", "", "z"], Some(1)),
            (vec!["z", "z", ""], Some(1)),
            (vec![""], Some(0)),
            (vec![], None),
            (vec!["a", "A", "á"], None),
        ] {
            let input = fields(&names);
            let original = input.clone();
            assert_eq!(first_duplicate(&input, &mut budget())?, expected);
            assert_eq!(input, original);
        }
        Ok(())
    }

    #[test]
    fn comparisons_and_byte_charges_scale_with_fields_and_names() -> Result<(), StopReason> {
        for length in [8, 128] {
            let mut previous = 0;
            for count in [128, 256, 512] {
                let input: Vec<_> = (0..count)
                    .rev()
                    .map(|index| ViewField {
                        name: format!("{}{:04}", "x".repeat(length), index),
                        children: vec![],
                    })
                    .collect();
                COMPARISONS.with(|value| value.set(0));
                let mut b = budget();
                assert_eq!(first_duplicate(&input, &mut b)?, None);
                let comparisons = COMPARISONS.with(Cell::get);
                // Independently count actual comparisons; low reported Work alone
                // cannot make an uncharged quadratic algorithm pass this test.
                assert!(comparisons > count as u64);
                assert!(comparisons < 32 * count as u64);
                if previous != 0 {
                    assert!(comparisons < previous * 3);
                }
                assert!(b.usage().work >= comparisons * (length as u64 + 5));
                previous = comparisons;
            }
        }
        Ok(())
    }

    #[test]
    fn exact_limits_and_cancelled_small_inputs() -> Result<(), StopReason> {
        let input = fields(&["z", "a", "m"]);
        let mut measured = budget();
        assert_eq!(first_duplicate(&input, &mut measured)?, None);
        let usage = measured.usage();
        let mut limits = budget().limits();
        limits.work = usage.work;
        limits.allocation_units = usage.allocation_units;
        assert_eq!(first_duplicate(&input, &mut Budget::new(limits))?, None);
        let mut low = limits;
        low.work -= 1;
        assert_eq!(
            first_duplicate(&input, &mut Budget::new(low)),
            Err(StopReason::WorkLimit)
        );
        low = limits;
        low.allocation_units -= 1;
        assert_eq!(
            first_duplicate(&input, &mut Budget::new(low)),
            Err(StopReason::AllocationLimit)
        );
        for input in [fields(&[]), fields(&["a"]), fields(&[""])] {
            let mut b = budget();
            b.stop(StopReason::Cancelled);
            assert_eq!(first_duplicate(&input, &mut b), Err(StopReason::Cancelled));
            assert_eq!(b.usage().allocation_units, 0);
        }
        Ok(())
    }

    #[test]
    fn limits_stop_before_comparison_or_allocation() {
        let names = [
            format!("{}a", "x".repeat(4096)),
            format!("{}b", "x".repeat(4096)),
        ];
        let input = fields(&[&names[0], &names[1]]);
        let mut limits = budget().limits();
        limits.work = 100;
        let mut b = Budget::new(limits);
        COMPARISONS.with(|value| value.set(0));
        assert_eq!(first_duplicate(&input, &mut b), Err(StopReason::WorkLimit));
        assert_eq!(COMPARISONS.with(Cell::get), 0);
        assert_eq!(first_duplicate(&input, &mut b), Err(StopReason::WorkLimit));
        limits = budget().limits();
        limits.allocation_units = 0;
        let mut b = Budget::new(limits);
        assert_eq!(
            first_duplicate(&input, &mut b),
            Err(StopReason::AllocationLimit)
        );
        assert_eq!(b.usage().allocation_units, 0);
        assert_eq!(COMPARISONS.with(Cell::get), 0);
    }
}
