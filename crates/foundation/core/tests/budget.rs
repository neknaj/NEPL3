use nepl3_core::budget::{Budget, Limits, Resource, StopReason, Usage};

fn limits(cap: u64) -> Limits {
    Limits {
        source_bytes: cap,
        work: cap,
        depth: cap,
        nodes: cap,
        allocation_units: cap,
        output_bytes: cap,
        diagnostics: cap,
        events: cap,
    }
}
fn usage(amount: u64) -> Usage {
    Usage {
        source_bytes: amount,
        work: amount,
        depth: amount,
        nodes: amount,
        allocation_units: amount,
        output_bytes: amount,
        diagnostics: amount,
        events: amount,
    }
}

#[test]
fn depth_scope_restores_the_caller_even_after_callback_replaces_budget() {
    let mut b = Budget::new(limits(100));
    let result: Result<(), StopReason> = b.with_depth_at_least(7, |b| {
        let inner: Result<(), StopReason> = b.with_depth(|b| {
            assert_eq!(b.current_depth(), 8);
            *b = Budget::new(limits(100));
            b.cancel();
            Err(StopReason::Cancelled)
        });
        assert_eq!(b.current_depth(), 7);
        inner
    });
    assert_eq!(result, Err(StopReason::Cancelled));
    assert_eq!(b.current_depth(), 0);
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
}

#[test]
fn observed_delegated_cost_is_atomic_across_all_resources_and_preserves_stops() {
    for resource in 0..8 {
        let mut b = Budget::new(limits(10));
        assert_eq!(b.record_observed_usage(usage(3)), Ok(()));
        let before = b.usage();
        let mut observed = usage(1);
        let reason = match resource {
            0 => {
                observed.source_bytes = 8;
                StopReason::SourceLimit
            }
            1 => {
                observed.work = 8;
                StopReason::WorkLimit
            }
            2 => {
                observed.depth = 11;
                StopReason::DepthLimit
            }
            3 => {
                observed.nodes = 8;
                StopReason::NodeLimit
            }
            4 => {
                observed.allocation_units = 8;
                StopReason::AllocationLimit
            }
            5 => {
                observed.output_bytes = 8;
                StopReason::OutputLimit
            }
            6 => {
                observed.diagnostics = 8;
                StopReason::DiagnosticLimit
            }
            _ => {
                observed.events = 8;
                StopReason::EventLimit
            }
        };
        assert_eq!(b.record_observed_usage(observed), Err(reason));
        assert_eq!(b.usage(), before); // No earlier resource was partially applied.
        assert_eq!(b.record_observed_usage(usage(2)), Err(reason));
        assert_eq!(
            b.usage(),
            Usage {
                depth: 3,
                ..usage(5)
            }
        );
        assert_eq!(b.charge(Resource::Work, 0), Err(reason));
    }
    let mut b = Budget::new(limits(10));
    b.cancel();
    assert_eq!(
        b.record_observed_usage(usage(4)),
        Err(StopReason::Cancelled)
    );
    assert_eq!(b.usage(), usage(4));
    assert_eq!(
        b.record_observed_usage(usage(7)),
        Err(StopReason::Cancelled)
    );
    assert_eq!(b.usage(), usage(4));
    let mut overflow = Budget::new(limits(u64::MAX));
    assert_eq!(overflow.record_observed_usage(usage(u64::MAX)), Ok(()));
    assert_eq!(
        overflow.record_observed_usage(usage(1)),
        Err(StopReason::SourceLimit)
    );
    assert_eq!(overflow.usage(), usage(u64::MAX));
}

#[test]
fn temporary_ceilings_restore_on_result_paths_and_cannot_regrant_reserved_capacity() {
    let outer = limits(100);
    let mut b = Budget::new(outer);
    let result: Result<(), StopReason> = b.with_ceiling(limits(10), |b| {
        assert_eq!(b.limits(), limits(10));
        b.with_ceiling(outer, |b| {
            assert_eq!(b.limits(), limits(10));
            b.charge(Resource::Work, 10)
        })?;
        // An ordinary error restores ceilings without inventing a sticky stop.
        Err(StopReason::Cancelled)
    });
    assert_eq!(result, Err(StopReason::Cancelled));
    assert_eq!(b.limits(), outer);
    assert_eq!(b.poll(), Ok(()));
    assert_eq!(b.usage().work, 10);
    let mut entered = false;
    let result: Result<(), StopReason> = b.with_ceiling(limits(9), |_| {
        entered = true;
        Ok(())
    });
    assert_eq!(result, Err(StopReason::WorkLimit));
    assert!(!entered);
    assert_eq!(b.limits(), outer);
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    let mut b = Budget::new(outer);
    let result: Result<(), StopReason> =
        b.with_ceiling(limits(10), |b| b.charge(Resource::Events, 11));
    assert_eq!(result, Err(StopReason::EventLimit));
    assert_eq!(b.limits(), outer);
    assert_eq!(b.usage(), Usage::default());
    assert_eq!(b.poll(), Err(StopReason::EventLimit));
}

// Native host recovery only. panic=abort/Wasm execution cannot promise that
// destructors run; these tests deliberately unwind without a panic hook.
#[cfg(all(panic = "unwind", not(target_family = "wasm")))]
mod unwind_scopes {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    fn unwind() -> ! {
        resume_unwind(Box::new("budget scope test"))
    }

    #[test]
    fn caught_nested_unwind_restores_enclosing_depth_and_ceiling() -> Result<(), StopReason> {
        let mut b = Budget::new(limits(100));
        b.with_depth_at_least(4, |b| {
            b.with_ceiling(limits(80), |b| {
                let caught = catch_unwind(AssertUnwindSafe(|| -> Result<(), StopReason> {
                    b.with_depth(|b| {
                        b.with_ceiling(limits(20), |b| {
                            b.with_depth_at_least(9, |b| {
                                b.charge(Resource::Work, 7)?;
                                unwind()
                            })
                        })
                    })
                }));
                assert!(caught.is_err());
                assert_eq!(b.current_depth(), 4);
                assert_eq!(b.limits(), limits(80));
                assert_eq!(b.usage().work, 7);
                assert_eq!(b.usage().depth, 9);
                // Restored quota, but not refunded work: total30 is above inner20.
                b.charge(Resource::Work, 23)
            })
        })?;
        assert_eq!(b.current_depth(), 0);
        assert_eq!(b.limits(), limits(100));
        assert_eq!(b.usage().work, 30);
        assert_eq!(b.usage().depth, 9);
        assert_eq!(b.poll(), Ok(()));
        Ok(())
    }

    #[test]
    fn unwinding_restores_scope_without_clearing_charges_or_stops() {
        for reason in [StopReason::Cancelled, StopReason::WorkLimit] {
            let mut b = Budget::new(limits(100));
            let caught = catch_unwind(AssertUnwindSafe(|| -> Result<(), StopReason> {
                b.with_depth_at_least(5, |b| {
                    b.with_ceiling(limits(10), |b| {
                        b.charge(Resource::Work, 7)?;
                        if reason == StopReason::Cancelled {
                            b.cancel();
                        } else {
                            assert_eq!(b.charge(Resource::Work, 100), Err(reason));
                        }
                        unwind()
                    })
                })
            }));
            assert!(caught.is_err());
            assert_eq!(b.current_depth(), 0);
            assert_eq!(b.limits(), limits(100));
            assert_eq!(b.usage().work, 7);
            assert_eq!(b.usage().depth, 5);
            assert_eq!(b.poll(), Err(reason));
        }
    }

    #[test]
    fn caught_measurement_unwind_merges_both_enclosing_and_inner_peaks() -> Result<(), StopReason> {
        for (outer, inner) in [(20, 3), (3, 20)] {
            let mut b = Budget::new(limits(100));
            b.observe_depth(90)?;
            let (_, measured) = b.measure_depth(|b| {
                b.observe_depth(outer)?;
                let caught = catch_unwind(AssertUnwindSafe(|| -> Result<((), u64), StopReason> {
                    b.measure_depth(|b| {
                        b.observe_depth(inner)?;
                        unwind()
                    })
                }));
                assert!(caught.is_err());
                Ok::<(), StopReason>(())
            })?;
            assert_eq!(measured, 20);
            assert_eq!(b.usage().depth, 90);
            assert_eq!(b.current_depth(), 0);
            let (_, next) = b.measure_depth(|b| b.observe_depth(2))?;
            assert_eq!(next, 2);
        }
        Ok(())
    }

    struct PanickingConversion;
    impl From<StopReason> for PanickingConversion {
        fn from(_: StopReason) -> Self {
            unwind()
        }
    }
    #[test]
    fn ceiling_preflight_restores_limits_when_error_conversion_unwinds() -> Result<(), StopReason> {
        let mut b = Budget::new(limits(100));
        b.charge(Resource::Work, 30)?;
        let mut called = false;
        let caught = catch_unwind(AssertUnwindSafe(|| {
            b.with_ceiling::<(), PanickingConversion>(limits(20), |_| {
                called = true;
                Ok(())
            })
        }));
        assert!(caught.is_err());
        assert!(!called);
        assert_eq!(b.limits(), limits(100));
        assert_eq!(b.usage().work, 30);
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        Ok(())
    }
}
