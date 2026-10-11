use super::*;
use nepl3_core::budget::{Resource, Usage};

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
fn reentry_rejects_spent_ceiling_before_observing_saved_depth() -> Result<(), StopReason> {
    let outer = limits(100);
    let mut budget = Budget::new(outer);
    let root = ExecutionScope::root(&mut budget, Limits { work: 1, ..outer })?;
    // Capture does not execute the frame. Other work can consume the shared
    // budget before this saved frame is entered or resumed.
    budget.charge(Resource::Work, 2)?;
    let before = budget.usage();
    assert_eq!(before.depth, 0);
    assert_eq!(root.depth, 1);
    let mut calls = 0;
    let result: Result<(), StopReason> = root.run(&mut budget, |_| {
        calls += 1;
        Ok(())
    });
    assert_eq!(result, Err(StopReason::WorkLimit));
    assert_eq!(calls, 0);
    // A reversed depth/ceiling wrapper order would incorrectly raise this mark.
    assert_eq!(budget.usage(), before);
    assert_eq!(budget.current_depth(), 0);
    assert_eq!(budget.limits(), outer);
    assert_eq!(budget.poll(), Err(StopReason::WorkLimit));
    Ok(())
}

#[test]
fn reentry_rejects_saved_depth_under_a_later_zero_ceiling() -> Result<(), StopReason> {
    let outer = limits(100);
    let mut budget = Budget::new(outer);
    let root = ExecutionScope::root(&mut budget, outer)?;
    let before = budget.usage();
    let temporary = Limits { depth: 0, ..outer };
    let mut calls = 0;
    let result: Result<(), StopReason> = budget.with_ceiling(temporary, |budget| {
        let result = root.run(budget, |_| {
            calls += 1;
            Ok::<_, StopReason>(())
        });
        assert_eq!(budget.limits(), temporary);
        assert_eq!(budget.current_depth(), 0);
        assert_eq!(budget.usage(), before);
        result
    });
    assert_eq!(result, Err(StopReason::DepthLimit));
    assert_eq!(calls, 0);
    assert_eq!(budget.usage(), before);
    assert_eq!(budget.limits(), outer);
    assert_eq!(budget.current_depth(), 0);
    assert_eq!(budget.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

#[test]
fn root_retains_temporary_capture_ceiling_after_outer_restoration() -> Result<(), StopReason> {
    let outer = limits(100);
    let temporary = Limits {
        depth: 4,
        ..limits(7)
    };
    let mut budget = Budget::new(outer);
    let root = budget.with_ceiling(temporary, |budget| ExecutionScope::root(budget, outer))?;
    assert_eq!(budget.limits(), outer);
    assert_eq!(budget.usage(), Usage::default());
    root.run(&mut budget, |budget| {
        assert_eq!(budget.limits(), temporary);
        assert_eq!(budget.current_depth(), 1);
        budget.charge(Resource::Work, 7)
    })?;
    assert_eq!(budget.limits(), outer);
    assert_eq!(budget.current_depth(), 0);
    assert_eq!(budget.usage().work, 7);
    assert_eq!(budget.usage().depth, 1);
    Ok(())
}

#[test]
fn callback_error_keeps_charges_and_restores_host_frame() -> Result<(), StopReason> {
    #[derive(Debug, PartialEq)]
    enum Failure {
        Domain,
        Resource(StopReason),
    }
    impl From<StopReason> for Failure {
        fn from(reason: StopReason) -> Self {
            Self::Resource(reason)
        }
    }
    let outer = limits(100);
    let saved = limits(10);
    for host_depth in [3, 7] {
        let mut budget = Budget::new(outer);
        // Capture under host depth four, producing a saved depth of five.
        let root = budget.with_depth_at_least(4, |budget| ExecutionScope::root(budget, saved))?;
        assert_eq!(root.depth, 5);
        budget.with_depth_at_least(host_depth, |budget| {
            let result: Result<(), Failure> = root.run(budget, |budget| {
                assert_eq!(budget.limits(), saved);
                // Exercise both a deeper saved frame and a deeper active host.
                assert_eq!(budget.current_depth(), host_depth.max(5));
                budget.charge(Resource::Work, 2)?;
                Err(Failure::Domain)
            });
            assert_eq!(result, Err(Failure::Domain));
            assert_eq!(budget.limits(), outer);
            assert_eq!(budget.current_depth(), host_depth);
            assert_eq!(budget.usage().work, 2);
            assert_eq!(budget.usage().depth, host_depth.max(5));
            budget.poll()
        })?;
        assert_eq!(budget.current_depth(), 0);
        assert_eq!(budget.limits(), outer);
        assert_eq!(budget.usage().work, 2);
        assert_eq!(budget.poll(), Ok(()));
    }
    Ok(())
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
