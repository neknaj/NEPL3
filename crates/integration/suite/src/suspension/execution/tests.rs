use super::*;
use nepl3_core::budget::Usage;

fn limits(value: u64) -> Limits {
    Limits {
        source_bytes: value,
        work: value,
        depth: value,
        nodes: value,
        allocation_units: value,
        output_bytes: value,
        diagnostics: value,
        events: value,
    }
}

#[test]
fn saved_depth_boundaries_use_checked_increment() {
    for (depth, cap, accepted) in [
        (0, 0, false),
        (0, 1, true),
        (3, 3, false),
        (3, 4, true),
        (u64::MAX - 1, u64::MAX, true),
        (u64::MAX, u64::MAX, false),
    ] {
        let parent = ExecutionScope {
            limits: limits(u64::MAX),
            depth,
        };
        let result = admission::transition(
            parent,
            Limits {
                depth: cap,
                ..limits(u64::MAX)
            },
        );
        if accepted {
            let child = result.ok();
            assert!(child.is_some_and(|child| child.depth as u128 == depth as u128 + 1));
        } else {
            assert!(matches!(result, Err(StopReason::DepthLimit)));
        }
        assert_eq!(parent.depth, depth);
    }
}

#[test]
fn child_preserves_accounting_and_prior_stop_at_active_host_depth() -> Result<(), StopReason> {
    for prior in [
        None,
        Some(StopReason::Cancelled),
        Some(StopReason::SourceLimit),
        Some(StopReason::WorkLimit),
        Some(StopReason::DepthLimit),
        Some(StopReason::NodeLimit),
        Some(StopReason::AllocationLimit),
        Some(StopReason::OutputLimit),
        Some(StopReason::DiagnosticLimit),
        Some(StopReason::EventLimit),
    ] {
        for parent_depth in [2, u64::MAX] {
            let mut budget = Budget::new(limits(u64::MAX));
            budget.record_observed_usage(Usage {
                source_bytes: 2,
                work: 3,
                depth: 5,
                nodes: 7,
                allocation_units: 11,
                output_bytes: 13,
                diagnostics: 17,
                events: 19,
            })?;
            let parent = ExecutionScope {
                limits: limits(u64::MAX),
                depth: parent_depth,
            };
            let result: Result<(), StopReason> = budget.with_depth_at_least(4, |budget| {
                if let Some(reason) = prior {
                    budget.stop(reason);
                }
                let before = budget.usage();
                let before_limits = budget.limits();
                let result = parent.child(limits(u64::MAX), budget);
                assert_eq!(budget.usage(), before);
                assert_eq!(budget.limits(), before_limits);
                assert_eq!(budget.current_depth(), 4);
                let expected =
                    prior.or((parent_depth == u64::MAX).then_some(StopReason::DepthLimit));
                assert_eq!(result.as_ref().err().copied(), expected);
                assert_eq!(budget.poll().err(), expected);
                result.map(|_| ())
            });
            assert_eq!(budget.current_depth(), 0);
            assert_eq!(
                result.err(),
                prior.or((parent_depth == u64::MAX).then_some(StopReason::DepthLimit))
            );
        }
    }
    Ok(())
}
