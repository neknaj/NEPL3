//! Budgeted UTF-8 comparison shared by schema and language admission.
use crate::budget::{Budget, Resource, StopReason};

/// Lexicographic UTF-8 byte order equals Rust str order. Admit each byte pair
/// before reading it; mismatching prefixes do not scan the untouched suffix.
pub fn compare_with_budget(
    left: &str,
    right: &str,
    budget: &mut Budget,
) -> Result<core::cmp::Ordering, StopReason> {
    budget.charge(Resource::Work, 1)?;
    compare_bytes(left, right, budget)
}

/// Exact byte equality with a constant-cost length rejection. No normalization
/// or allocation; same-length comparisons inspect only the matching prefix.
pub fn equal_with_budget(left: &str, right: &str, budget: &mut Budget) -> Result<bool, StopReason> {
    budget.charge(Resource::Work, 1)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    Ok(compare_bytes(left, right, budget)?.is_eq())
}

fn compare_bytes(
    left: &str,
    right: &str,
    budget: &mut Budget,
) -> Result<core::cmp::Ordering, StopReason> {
    for index in 0..left.len().min(right.len()) {
        budget.charge(Resource::Work, 3)?;
        let order = left.as_bytes()[index].cmp(&right.as_bytes()[index]);
        if order != core::cmp::Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(left.len().cmp(&right.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn budget(work: u64) -> Budget {
        Budget::new(crate::budget::Limits {
            work,
            ..Default::default()
        })
    }
    #[test]
    fn equality_rejects_lengths_before_bytes_and_preserves_stops() -> Result<(), StopReason> {
        let long = "界".repeat(4096);
        let tail = alloc::format!("{long}x");
        for (left, right, work) in [
            ("", "", 1),
            (long.as_str(), tail.as_str(), 1),
            (long.as_str(), long.as_str(), 1 + 3 * long.len() as u64),
            ("abc", "xbc", 4),
        ] {
            let mut exact = budget(work);
            assert_eq!(equal_with_budget(left, right, &mut exact)?, left == right);
            assert_eq!(exact.usage().work, work);
            let mut short = budget(work - 1);
            assert_eq!(
                equal_with_budget(left, right, &mut short),
                Err(StopReason::WorkLimit)
            );
            let used = short.usage();
            assert_eq!(
                equal_with_budget("", "", &mut short),
                Err(StopReason::WorkLimit)
            );
            assert_eq!(short.usage(), used);
        }
        let mut cancelled = budget(100);
        cancelled.stop(StopReason::Cancelled);
        let used = cancelled.usage();
        assert_eq!(
            equal_with_budget("x", "", &mut cancelled),
            Err(StopReason::Cancelled)
        );
        assert_eq!(cancelled.usage(), used);
        Ok(())
    }
    #[test]
    fn comparison_admits_only_visited_pairs_with_exact_sticky_boundaries() -> Result<(), StopReason>
    {
        let huge = "界".repeat(1024);
        let tail = alloc::format!("{huge}x");
        let mismatch = alloc::format!("{huge}y");
        for (a, b, pairs) in [
            ("", "", 0),
            ("", huge.as_str(), 0),
            ("a", "ab", 1),
            ("x", huge.as_str(), 1),
            (huge.as_str(), huge.as_str(), huge.len()),
            (tail.as_str(), mismatch.as_str(), huge.len() + 1),
        ] {
            let work = 1 + 3 * pairs as u64;
            let mut exact = budget(work);
            assert_eq!(compare_with_budget(a, b, &mut exact)?, a.cmp(b));
            assert_eq!(exact.usage().work, work);
            for limit in [0, work - 1] {
                let mut short = budget(limit);
                assert_eq!(
                    compare_with_budget(a, b, &mut short),
                    Err(StopReason::WorkLimit)
                );
                assert_eq!(
                    short.usage().work,
                    if limit == 0 {
                        0
                    } else {
                        1 + 3 * ((limit - 1) / 3)
                    }
                );
                let used = short.usage();
                assert_eq!(
                    compare_with_budget("", "", &mut short),
                    Err(StopReason::WorkLimit)
                );
                assert_eq!(short.usage(), used);
            }
        }
        for reason in [
            StopReason::Cancelled,
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::DepthLimit,
            StopReason::NodeLimit,
            StopReason::AllocationLimit,
            StopReason::OutputLimit,
            StopReason::DiagnosticLimit,
            StopReason::EventLimit,
        ] {
            let mut b = budget(100);
            b.charge(Resource::Work, 7)?;
            b.stop(reason);
            let before = b.usage();
            assert_eq!(compare_with_budget("a", "b", &mut b), Err(reason));
            assert_eq!(b.usage(), before);
        }
        Ok(())
    }
}
