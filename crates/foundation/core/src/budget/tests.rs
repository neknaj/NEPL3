use super::*;

fn budget() -> Budget {
    Budget::new(Limits {
        depth: 100,
        work: 100,
        ..Limits::default()
    })
}

#[test]
fn measured_depth_excludes_prior_usage_and_restores_nested_observation() -> Result<(), StopReason> {
    let mut b = budget();
    b.observe_depth(90)?;
    let (_, depth) = b.measure_depth(|b| {
        b.with_depth_at_least(7, |b| {
            let (_, inner) = b.measure_depth(|b| b.with_depth(|b| b.observe_depth(2)))?;
            assert_eq!(inner, 3);
            // A host observation contributes only the newly supplied depth,
            // while Usage still includes the earlier high-water mark of 90.
            let (_, observed) = b.measure_depth(|b| {
                b.record_observed_usage(Usage {
                    depth: 11,
                    ..Usage::default()
                })
            })?;
            assert_eq!(observed, 4);
            Ok::<_, StopReason>(())
        })
    })?;
    assert_eq!(depth, 11);
    assert_eq!(b.current_depth(), 0);
    assert_eq!(b.usage().depth, 90);
    let (_, empty) = b.measure_depth(|_| Ok::<_, StopReason>(()))?;
    assert_eq!(empty, 0);
    Ok(())
}

#[test]
fn failed_nested_measurement_preserves_charges_and_sticky_stops() -> Result<(), StopReason> {
    let mut b = budget();
    let (_, depth) = b.measure_depth(|b| {
        let rejected: Result<((), u64), StopReason> = b.measure_depth(|b| {
            b.observe_depth(8)?;
            b.charge(Resource::Work, 3)?;
            Err(StopReason::Cancelled)
        });
        assert_eq!(rejected, Err(StopReason::Cancelled));
        Ok::<_, StopReason>(())
    })?;
    assert_eq!(depth, 8);
    assert_eq!(b.usage().work, 3);
    let stopped: Result<((), u64), StopReason> = b.measure_depth(|b| {
        assert_eq!(b.observe_depth(101), Err(StopReason::DepthLimit));
        Ok(())
    });
    assert_eq!(stopped, Err(StopReason::DepthLimit));
    assert_eq!(b.usage().depth, 8);
    let mut called = false;
    let stopped: Result<((), u64), StopReason> = b.measure_depth(|_| {
        called = true;
        Ok(())
    });
    assert_eq!(stopped, Err(StopReason::DepthLimit));
    assert!(!called);
    Ok(())
}

#[test]
fn measurement_cannot_hide_prior_usage_from_a_lower_ceiling() -> Result<(), StopReason> {
    let mut b = budget();
    b.observe_depth(90)?;
    let outer = b.limits();
    let mut called = false;
    let result: Result<((), u64), StopReason> = b.measure_depth(|b| {
        b.with_depth_at_least(7, |b| {
            b.with_ceiling(Limits { depth: 16, ..outer }, |_| {
                called = true;
                Ok(())
            })
        })
    });
    assert_eq!(result, Err(StopReason::DepthLimit));
    assert!(!called);
    assert_eq!(b.usage().depth, 90);
    assert_eq!(b.current_depth(), 0);
    assert_eq!(b.limits(), outer);
    assert_eq!(b.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

/// A real stop/cancel before depth observation retains prior resource charges.
/// Returning Err from a callback is a separate contract from setting this stop.
#[test]
fn prior_stop_and_cancel_preserve_observation_prefix() -> Result<(), StopReason> {
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
        let mut b = budget();
        b.charge(Resource::Work, 3)?;
        b.observe_depth(5)?;
        let before = b.usage();
        assert_eq!(b.stop(reason), reason);
        b.cancel();
        assert_eq!(b.observe_depth(u64::MAX), Err(reason));
        assert_eq!(b.usage(), before);
        assert_eq!(b.current_depth(), 0);
        assert_eq!(b.observed_depth, 5);
    }
    let mut b = budget();
    b.observe_depth(7)?;
    b.cancel();
    assert_eq!(b.observe_depth(0), Err(StopReason::Cancelled));
    assert_eq!(b.usage().depth, 7);
    Ok(())
}

/// Depth scopes restore the saved caller depth, not a decremented callback
/// depth. Callback Result and sticky stop intentionally remain distinct.
#[test]
fn depth_scopes_preserve_callback_result_and_replacement() -> Result<(), StopReason> {
    fn scope(
        b: &mut Budget,
        restored: bool,
        callback: impl FnOnce(&mut Budget) -> Result<(), StopReason>,
    ) -> Result<(), StopReason> {
        if restored {
            b.with_depth_at_least(9, callback)
        } else {
            b.with_depth(callback)
        }
    }
    for restored in [false, true] {
        let mut b = budget();
        b.with_depth_at_least(4, |b| {
            assert_eq!(
                scope(b, restored, |b| {
                    b.charge(Resource::Work, 3)?;
                    Err(StopReason::Cancelled)
                }),
                Err(StopReason::Cancelled)
            );
            assert_eq!(b.current_depth(), 4);
            assert_eq!(b.usage().work, 3);
            assert_eq!(b.poll(), Ok(()));
            assert_eq!(
                scope(b, restored, |b| {
                    b.cancel();
                    Ok(())
                }),
                Ok(())
            );
            assert_eq!(b.current_depth(), 4);
            assert_eq!(b.poll(), Err(StopReason::Cancelled));
            Ok::<_, StopReason>(())
        })?;
        assert_eq!(b.current_depth(), 0);
        let mut b = budget();
        b.with_depth_at_least(4, |b| {
            scope(b, restored, |b| {
                *b = budget();
                b.charge(Resource::Work, 6)?;
                Ok(())
            })?;
            assert_eq!(b.current_depth(), 4);
            assert_eq!(b.usage().work, 6);
            Ok::<_, StopReason>(())
        })?;
        assert_eq!(b.current_depth(), 0);
        assert_eq!(b.usage().work, 6);
    }
    Ok(())
}

#[test]
fn ceiling_scopes_restore_limits_but_preserve_callback_result_and_state() -> Result<(), StopReason>
{
    let mut b = budget();
    let outer = b.limits();
    let narrow = Limits { work: 10, ..outer };
    b.with_ceiling(narrow, |b| {
        assert_eq!(b.limits(), narrow);
        let failed: Result<(), StopReason> = b.with_ceiling(outer, |b| {
            assert_eq!(b.limits(), narrow);
            b.charge(Resource::Work, 3)?;
            Err(StopReason::OutputLimit)
        });
        assert_eq!(failed, Err(StopReason::OutputLimit));
        assert_eq!(b.limits(), narrow);
        assert_eq!(b.poll(), Ok(()));
        b.with_ceiling(outer, |b| {
            b.cancel();
            Ok::<_, StopReason>(())
        })?;
        assert_eq!(b.poll(), Err(StopReason::Cancelled));
        Ok::<_, StopReason>(())
    })?;
    assert_eq!(b.limits(), outer);
    assert_eq!(b.usage().work, 3);
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    let mut b = budget();
    b.with_ceiling(narrow, |b| {
        *b = Budget::new(Limits {
            work: 200,
            depth: 200,
            ..outer
        });
        b.charge(Resource::Work, 6)?;
        b.observe_depth(11)?;
        Ok::<_, StopReason>(())
    })?;
    assert_eq!(b.limits(), outer);
    assert_eq!(b.usage().work, 6);
    assert_eq!(b.usage().depth, 11);
    assert_eq!(b.observed_depth, 11);
    Ok(())
}
