//! Pure observation of one traversal depth and its two high-water marks.
pub(super) mod entry;
use super::StopReason;

/// Successful high-water marks, or a stop retaining both previous marks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Observation {
    Observed { usage: u64, measured: u64 },
    Stopped { reason: StopReason },
}

/// Observe `current + relative` without altering inputs or external state.
///
/// A prior stop has precedence. Otherwise the sum must fit u64 and the depth
/// limit. Success takes each old high-water mark's maximum with that sum.
/// Failure does not update either mark. Existing marks may exceed a lowered
/// limit: they are historical observations, not silently reset or rejected here.
/// Active depth is an input only. This constant-time transition allocates nothing.
pub(super) fn observe(
    current: u64,
    relative: u64,
    limit: u64,
    usage: u64,
    measured: u64,
    stopped: Option<StopReason>,
) -> Observation {
    match stopped {
        Some(reason) => Observation::Stopped { reason },
        None => match current.checked_add(relative) {
            Some(depth) if depth <= limit => Observation::Observed {
                usage: usage.max(depth),
                measured: measured.max(depth),
            },
            _ => Observation::Stopped {
                reason: StopReason::DepthLimit,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zero/exact/overflow inputs and earlier high-water marks remain distinct.
    #[test]
    fn exact_limits_overflow_and_historical_marks() {
        for (current, relative, limit, usage, measured, expected) in [
            (
                0,
                0,
                0,
                0,
                0,
                Observation::Observed {
                    usage: 0,
                    measured: 0,
                },
            ),
            (
                3,
                4,
                7,
                5,
                6,
                Observation::Observed {
                    usage: 7,
                    measured: 7,
                },
            ),
            (
                3,
                4,
                6,
                5,
                6,
                Observation::Stopped {
                    reason: StopReason::DepthLimit,
                },
            ),
            (
                3,
                0,
                7,
                90,
                80,
                Observation::Observed {
                    usage: 90,
                    measured: 80,
                },
            ),
            (
                u64::MAX,
                0,
                u64::MAX,
                0,
                0,
                Observation::Observed {
                    usage: u64::MAX,
                    measured: u64::MAX,
                },
            ),
            (
                u64::MAX,
                1,
                u64::MAX,
                0,
                0,
                Observation::Stopped {
                    reason: StopReason::DepthLimit,
                },
            ),
        ] {
            assert_eq!(
                observe(current, relative, limit, usage, measured, None),
                expected
            );
            assert_eq!(
                observe(
                    current,
                    relative,
                    limit,
                    usage,
                    measured,
                    Some(StopReason::WorkLimit)
                ),
                Observation::Stopped {
                    reason: StopReason::WorkLimit
                }
            );
        }
    }
}
