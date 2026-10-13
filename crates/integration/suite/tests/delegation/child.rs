use super::*;

#[test]
fn child_settles_differences_after_local_work_and_keeps_absolute_depth() -> Result<(), String> {
    let err = |e| format!("{e:?}");
    let mut parent = Budget::new(limits(100));
    parent.record_observed_usage(usage(7)).map_err(err)?;
    parent
        .with_depth_at_least(12, |parent| {
            let mut issued = IssuedBudget::issue(parent, limits(30))?;
            issued
                .run_local(|b| {
                    for (r, _) in resources() {
                        b.charge(r, 2)?;
                    }
                    Ok(())
                })
                .map_err(|e| e.reason)?;
            let execution = issued
                .execute_local_child(4, |b, _| {
                    assert_eq!(b.current_depth(), 12);
                    assert_eq!(
                        b.usage(),
                        Usage {
                            depth: 12,
                            ..usage(9)
                        }
                    );
                    for (r, _) in resources() {
                        b.charge(r, 3)?;
                    }
                    b.with_depth(|_| Ok::<_, StopReason>("output"))
                })
                .map_err(|_| StopReason::Cancelled)?;
            assert_eq!(execution.output(), &Ok("output"));
            assert_eq!(execution.stopped(), None);
            assert_eq!(
                execution.basis(),
                Usage {
                    depth: 12,
                    ..usage(9)
                }
            );
            assert_eq!(
                execution.cumulative_usage(),
                Usage {
                    depth: 13,
                    ..usage(12)
                }
            );
            assert_eq!(
                execution.settled_usage(),
                Usage {
                    depth: 13,
                    ..usage(3)
                }
            );
            Ok::<_, StopReason>(())
        })
        .map_err(err)?;
    assert_eq!(
        parent.usage(),
        Usage {
            depth: 13,
            ..usage(12)
        }
    );
    assert_eq!(parent.current_depth(), 0);
    assert_eq!(parent.poll(), Ok(()));
    Ok(())
}

#[test]
fn stopped_child_retains_output_and_real_consumption_for_every_resource() -> Result<(), StopReason>
{
    for (r, reason) in resources() {
        let mut parent = Budget::new(limits(100));
        parent.record_observed_usage(usage(2))?;
        let execution = IssuedBudget::issue(&mut parent, limits(5))?
            .execute_local_child(0, |b, _| {
                assert_eq!(b.charge(r, 5), Ok(()));
                assert_eq!(b.charge(r, 1), Err(reason));
                91
            })
            .map_err(|_| StopReason::Cancelled)?;
        assert_eq!(execution.output(), &91);
        assert_eq!(execution.stopped(), Some(reason));
        assert_eq!(parent.usage(), execution.cumulative_usage());
        assert_eq!(parent.poll(), Ok(()));
    }
    Ok(())
}

#[test]
fn child_setup_rejects_historical_depth_and_preserves_parent_stop() -> Result<(), StopReason> {
    use nepl3_suite::suspension::delegation::child::Error;
    for saved_depth in [0, 9] {
        let mut parent = Budget::new(limits(100));
        parent.observe_depth(8)?;
        let result = IssuedBudget::issue(&mut parent, limits(7))?
            .execute_local_child(saved_depth, |_, _| unreachable_callback());
        assert!(
            matches!(result, Err(f) if f.error == Error::Setup(StopReason::DepthLimit) && f.output.is_none())
        );
        assert_eq!(parent.poll(), Err(StopReason::DepthLimit));
        assert_eq!(parent.usage().depth, 8);
    }
    let mut parent = Budget::new(limits(100));
    let mut issued = IssuedBudget::issue(&mut parent, limits(7))?;
    assert!(
        issued
            .run_local(|b| {
                b.stop(StopReason::WorkLimit);
                Ok(())
            })
            .is_err()
    );
    let result = issued.execute_local_child(0, |_, _| unreachable_callback());
    assert!(matches!(result, Err(f) if f.error == Error::Setup(StopReason::WorkLimit)));
    assert_eq!(parent.poll(), Err(StopReason::WorkLimit));
    Ok(())
}

fn unreachable_callback() {
    std::panic::resume_unwind(Box::new("callback must not run"));
}

#[test]
fn observable_child_reset_is_rejected_before_depth_restoration() -> Result<(), StopReason> {
    use nepl3_suite::suspension::delegation::child::Error;
    let mut parent = Budget::new(limits(100));
    parent.record_observed_usage(usage(3))?;
    let result = IssuedBudget::issue(&mut parent, limits(20))?.execute_local_child(10, |b, _| {
        let observed = b.usage();
        *b = Budget::new(b.limits());
        // Even restoring historical counters cannot hide lost active depth.
        assert_eq!(b.record_observed_usage(observed), Ok(()));
        "retained"
    });
    assert!(
        matches!(result, Err(f) if f.error == Error::ChangedBudget && f.output == Some("retained"))
    );
    assert_eq!(parent.usage(), usage(3));
    assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[cfg(panic = "unwind")]
#[test]
fn unwound_child_cancels_unresolved_parent_without_fabricating_cost() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(100));
    parent.record_observed_usage(usage(3))?;
    let issued = IssuedBudget::issue(&mut parent, limits(20))?;
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = issued.execute_local_child(8, |b, _| {
            assert_eq!(b.charge(Resource::Work, 4), Ok(()));
            std::panic::resume_unwind(Box::new("child unwind"));
        });
    }));
    assert!(caught.is_err());
    assert_eq!(parent.usage(), usage(3));
    assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn child_zero_and_maximum_grants_and_changed_limits() -> Result<(), StopReason> {
    use nepl3_suite::suspension::delegation::child::Error;
    for n in [0, u64::MAX] {
        let mut parent = Budget::new(limits(u64::MAX));
        let result = IssuedBudget::issue(&mut parent, limits(n))?
            .execute_local_child(n, |b, _| {
                for (r, _) in resources() {
                    b.charge(r, n)?;
                }
                Ok::<_, StopReason>(())
            })
            .map_err(|_| StopReason::Cancelled)?;
        assert_eq!(result.output(), &Ok(()));
        assert_eq!(result.cumulative_usage(), usage(n));
        assert_eq!(parent.usage(), usage(n));
    }
    let mut parent = Budget::new(limits(100));
    let result = IssuedBudget::issue(&mut parent, limits(10))?.execute_local_child(0, |b, _| {
        *b = Budget::new(limits(100));
        5
    });
    assert!(matches!(result, Err(f) if f.error == Error::ChangedBudget && f.output == Some(5)));
    assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn child_counter_regression_is_rejected_in_every_field() -> Result<(), StopReason> {
    use nepl3_suite::suspension::delegation::child::Error;
    for field in 0..8 {
        let mut parent = Budget::new(limits(100));
        parent.record_observed_usage(usage(3))?;
        let result =
            IssuedBudget::issue(&mut parent, limits(10))?.execute_local_child(0, |b, _| {
                let mut lower = b.usage();
                match field {
                    0 => lower.source_bytes = 2,
                    1 => lower.work = 2,
                    2 => lower.depth = 2,
                    3 => lower.nodes = 2,
                    4 => lower.allocation_units = 2,
                    5 => lower.output_bytes = 2,
                    6 => lower.diagnostics = 2,
                    _ => lower.events = 2,
                }
                *b = Budget::new(b.limits());
                assert_eq!(b.record_observed_usage(lower), Ok(()));
                field
            });
        assert!(
            matches!(result, Err(f) if f.error == Error::ChangedBudget && f.output == Some(field))
        );
        assert_eq!(parent.usage(), usage(3));
        assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    }
    Ok(())
}

#[test]
fn child_admission_reuses_only_its_own_source_proof() -> Result<(), String> {
    use nepl3_core::source::{SourceAdmission, SourceId};
    let mut parent = Budget::new(limits(100_000));
    let mut parent_admission = SourceAdmission::default();
    let source = parent_admission
        .create(
            SourceId("source".into()),
            0,
            "memory:source".into(),
            b"abc".to_vec(),
            &mut parent,
        )
        .map_err(|e| format!("{e:?}"))?;
    let before = parent.usage();
    let child = IssuedBudget::issue(&mut parent, limits(10_000))
        .map_err(|e| format!("{e:?}"))?
        .execute_local_child(0, |b, a| {
            a.admit_existing(&source, b)?;
            a.admit_existing(&source, b)?;
            Ok::<_, nepl3_core::source::SourceError>(())
        })
        .map_err(|f| format!("{:?}", f.error))?;
    child.output().as_ref().map_err(|e| format!("{e:?}"))?;
    assert_eq!(child.settled_usage().source_bytes, 3);
    assert_eq!(parent.usage().source_bytes, before.source_bytes + 3);
    parent_admission
        .admit_existing(&source, &mut parent)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(parent.usage().source_bytes, before.source_bytes + 3);
    Ok(())
}

#[test]
fn saved_depth_preflight_and_child_depth_stop_are_distinct() -> Result<(), StopReason> {
    use nepl3_suite::suspension::delegation::child::Error;
    let mut parent = Budget::new(limits(100));
    parent.observe_depth(2)?;
    let result = IssuedBudget::issue(&mut parent, limits(7))?
        .execute_local_child(8, |_, _| unreachable_callback());
    assert!(
        matches!(result, Err(f) if f.error == Error::Setup(StopReason::DepthLimit) && f.output.is_none())
    );
    assert_eq!(parent.poll(), Err(StopReason::DepthLimit));
    assert_eq!(parent.usage().depth, 2);
    let mut parent = Budget::new(limits(100));
    parent.observe_depth(2)?;
    let execution = IssuedBudget::issue(&mut parent, limits(7))?
        .execute_local_child(7, |b, _| {
            b.charge(Resource::Work, 1)?;
            b.with_depth(|_| Ok::<_, StopReason>(91))
        })
        .map_err(|_| StopReason::Cancelled)?;
    assert_eq!(execution.output(), &Err(StopReason::DepthLimit));
    assert_eq!(execution.stopped(), Some(StopReason::DepthLimit));
    assert_eq!(parent.usage().depth, 7);
    assert_eq!(parent.usage().work, 1);
    assert_eq!(parent.poll(), Ok(()));
    Ok(())
}

#[test]
fn local_child_settlement_does_not_reobserve_historical_depth() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(100));
    parent.observe_depth(80)?;
    let (_, measured) = parent.measure_depth(|parent| {
        let issued = IssuedBudget::issue(
            parent,
            Limits {
                depth: 100,
                ..limits(0)
            },
        )?;
        let result = issued.execute_local_child(1, |_, _| ());
        assert!(result.is_ok());
        Ok::<_, StopReason>(())
    })?;
    assert_eq!(parent.usage().depth, 80);
    assert_eq!(measured, 1);
    Ok(())
}

#[test]
fn stopped_and_nested_children_settle_fresh_depth_not_history() -> Result<(), StopReason> {
    for stop in [false, true] {
        let mut parent = Budget::new(limits(100));
        parent.observe_depth(80)?;
        parent.with_depth_at_least(5, |parent| {
            let (_, measured) = parent.measure_depth(|parent| {
                let issued = IssuedBudget::issue(
                    parent,
                    Limits {
                        depth: 100,
                        ..limits(5)
                    },
                )?;
                let execution = issued
                    .execute_local_child(6, |child, _| {
                        child.observe_depth_scope(|child| {
                            child.charge(Resource::Work, 3)?;
                            if stop {
                                child.cancel();
                            }
                            Ok::<_, StopReason>(91)
                        })
                    })
                    .map_err(|_| StopReason::Cancelled)?;
                let nested = execution.output().as_ref().map_err(|e| *e)?;
                assert_eq!(nested.output, Ok(91));
                assert_eq!(nested.relative_depth, 0);
                assert_eq!(
                    execution.stopped(),
                    if stop {
                        Some(StopReason::Cancelled)
                    } else {
                        None
                    }
                );
                assert_eq!(execution.settled_usage().depth, 6);
                assert_eq!(execution.settled_usage().work, 3);
                assert_eq!(execution.cumulative_usage().depth, 80);
                Ok::<_, StopReason>(())
            })?;
            assert_eq!(measured, 1);
            Ok::<_, StopReason>(())
        })?;
        assert_eq!(parent.usage().depth, 80);
        assert_eq!(parent.usage().work, 3);
        assert_eq!(parent.poll(), Ok(()));
    }
    Ok(())
}

#[test]
fn empty_child_has_zero_fresh_depth_after_deep_history() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(100));
    parent.observe_depth(80)?;
    let (_, measured) = parent.measure_depth(|parent| {
        let execution = IssuedBudget::issue(
            parent,
            Limits {
                depth: 100,
                ..limits(0)
            },
        )?
        .execute_local_child(0, |_, _| 17)
        .map_err(|_| StopReason::Cancelled)?;
        assert_eq!(execution.output(), &17);
        assert_eq!(execution.settled_usage(), Usage::default());
        assert_eq!(execution.cumulative_usage().depth, 80);
        assert_eq!(execution.stopped(), None);
        Ok::<_, StopReason>(())
    })?;
    assert_eq!(measured, 0);
    assert_eq!(parent.usage().depth, 80);
    Ok(())
}
