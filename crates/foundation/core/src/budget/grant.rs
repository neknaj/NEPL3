//! Pure conversion of relative additive capacity to cumulative ceilings.
use super::{Limits, StopReason, Usage};

impl Limits {
    /// Convert this relative grant against actual cumulative usage and outer caps.
    /// Seven counters add with checked arithmetic; depth remains absolute and
    /// must cover the historical peak. Depth is checked first, then additive
    /// resources in field order. This pure calculation admits no work, grants
    /// no source authority and changes no Budget or sticky stop.
    pub fn cumulative_ceiling(self, base: Usage, outer: Limits) -> Result<Limits, StopReason> {
        let grant = self;
        fn add(a: u64, b: u64, max: u64, reason: StopReason) -> Result<u64, StopReason> {
            a.checked_add(b).filter(|v| *v <= max).ok_or(reason)
        }
        if base.depth > grant.depth || grant.depth > outer.depth {
            return Err(StopReason::DepthLimit);
        }
        Ok(Limits {
            source_bytes: add(
                base.source_bytes,
                grant.source_bytes,
                outer.source_bytes,
                StopReason::SourceLimit,
            )?,
            work: add(base.work, grant.work, outer.work, StopReason::WorkLimit)?,
            depth: grant.depth,
            nodes: add(base.nodes, grant.nodes, outer.nodes, StopReason::NodeLimit)?,
            allocation_units: add(
                base.allocation_units,
                grant.allocation_units,
                outer.allocation_units,
                StopReason::AllocationLimit,
            )?,
            output_bytes: add(
                base.output_bytes,
                grant.output_bytes,
                outer.output_bytes,
                StopReason::OutputLimit,
            )?,
            diagnostics: add(
                base.diagnostics,
                grant.diagnostics,
                outer.diagnostics,
                StopReason::DiagnosticLimit,
            )?,
            events: add(
                base.events,
                grant.events,
                outer.events,
                StopReason::EventLimit,
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(field: usize, value: u64) -> Limits {
        let mut limits = Limits::default();
        match field {
            0 => limits.source_bytes = value,
            1 => limits.work = value,
            2 => limits.nodes = value,
            3 => limits.allocation_units = value,
            4 => limits.output_bytes = value,
            5 => limits.diagnostics = value,
            _ => limits.events = value,
        }
        limits
    }
    fn usage(limits: Limits) -> Usage {
        Usage {
            source_bytes: limits.source_bytes,
            work: limits.work,
            depth: limits.depth,
            nodes: limits.nodes,
            allocation_units: limits.allocation_units,
            output_bytes: limits.output_bytes,
            diagnostics: limits.diagnostics,
            events: limits.events,
        }
    }
    #[test]
    fn additive_conversion_matches_wider_integer_oracle() {
        let values = [0, 1, 3, u64::MAX - 1, u64::MAX];
        for (field, reason) in [
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::NodeLimit,
            StopReason::AllocationLimit,
            StopReason::OutputLimit,
            StopReason::DiagnosticLimit,
            StopReason::EventLimit,
        ]
        .into_iter()
        .enumerate()
        {
            for base in values {
                for capacity in values {
                    for maximum in values {
                        // Wider arithmetic is independent of production checked_add.
                        let sum = u128::from(base) + u128::from(capacity);
                        let expected = if sum <= u128::from(maximum) {
                            Ok(one(field, sum as u64))
                        } else {
                            Err(reason)
                        };
                        assert_eq!(
                            one(field, capacity)
                                .cumulative_ceiling(usage(one(field, base)), one(field, maximum)),
                            expected
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn absolute_depth_and_failure_priority_are_explicit() {
        let values = [0, 1, 3, u64::MAX];
        for historical in values {
            for granted in values {
                for maximum in values {
                    let grant = Limits {
                        depth: granted,
                        ..Limits::default()
                    };
                    let outer = Limits {
                        depth: maximum,
                        ..Limits::default()
                    };
                    let base = Usage {
                        depth: historical,
                        ..Usage::default()
                    };
                    let expected = if historical <= granted && granted <= maximum {
                        Ok(grant)
                    } else {
                        Err(StopReason::DepthLimit)
                    };
                    assert_eq!(grant.cumulative_ceiling(base, outer), expected);
                }
            }
        }
        // Preserve the existing delegated admission's depth-first priority.
        let grant = Limits {
            source_bytes: u64::MAX,
            work: u64::MAX,
            depth: 0,
            ..Limits::default()
        };
        let base = Usage {
            source_bytes: 1,
            work: 1,
            depth: 1,
            ..Usage::default()
        };
        let outer = Limits {
            source_bytes: u64::MAX,
            work: u64::MAX,
            depth: 1,
            ..Limits::default()
        };
        assert_eq!(
            grant.cumulative_ceiling(base, outer),
            Err(StopReason::DepthLimit)
        );
        assert_eq!(
            Limits { depth: 1, ..grant }.cumulative_ceiling(base, outer),
            Err(StopReason::SourceLimit)
        );
    }
}
