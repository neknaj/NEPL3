//! Pure transition for one cumulative-resource charge.
//!
//! The caller selects the resource counter, ceiling and failure reason. This
//! module neither reads nor mutates a Budget. The mutable application boundary
//! in Budget::charge preserves all other counters and operational state.
use super::StopReason;

/// A successful next counter or a stop that leaves the prior counter unchanged.
/// No representation combines successful charging with a stopped outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Charge {
    Charged { used: u64 },
    Stopped { reason: StopReason },
}

/// Return the next charge outcome without changing any input or external state.
///
/// An existing stop takes precedence over every counter/ceiling condition.
/// Otherwise, success requires both `used <= limit` and `amount <= limit-used`;
/// the returned counter is then exactly `used+amount`. Every failure retains
/// the old counter at the application boundary. In particular, `used > limit`
/// is a valid input and even a zero charge stops: temporarily lowered ceilings
/// rely on this case. The operation takes constant time and allocates nothing.
///
/// This is a pure scalar transition, not a claim that all Budget operations or
/// callers are pure, nor a proof of schema traversal or physical memory usage.
pub(super) fn transition(
    used: u64,
    limit: u64,
    amount: u64,
    stopped: Option<StopReason>,
    reason: StopReason,
) -> Charge {
    match stopped {
        Some(reason) => Charge::Stopped { reason },
        None => match used.checked_add(amount) {
            Some(next) if next <= limit => Charge::Charged { used: next },
            _ => Charge::Stopped { reason },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lower ceilings, exact bounds, overflow and an earlier stop have distinct
    /// contracts. No input is excluded merely because its counter exceeds limit.
    #[test]
    fn boundaries_and_first_stop() {
        for (used, limit, amount, expected) in [
            (0, 0, 0, Charge::Charged { used: 0 }),
            (4, 5, 1, Charge::Charged { used: 5 }),
            (
                4,
                5,
                2,
                Charge::Stopped {
                    reason: StopReason::WorkLimit,
                },
            ),
            (
                6,
                5,
                0,
                Charge::Stopped {
                    reason: StopReason::WorkLimit,
                },
            ),
            (u64::MAX, u64::MAX, 0, Charge::Charged { used: u64::MAX }),
            (
                u64::MAX,
                u64::MAX,
                1,
                Charge::Stopped {
                    reason: StopReason::WorkLimit,
                },
            ),
        ] {
            assert_eq!(
                transition(used, limit, amount, None, StopReason::WorkLimit),
                expected
            );
            assert_eq!(
                transition(
                    used,
                    limit,
                    amount,
                    Some(StopReason::Cancelled),
                    StopReason::WorkLimit
                ),
                Charge::Stopped {
                    reason: StopReason::Cancelled
                }
            );
        }
    }
}
