use nepl3_core::budget::{Budget, Limits, Resource, StopReason, Usage};
use nepl3_suite::suspension::delegation::{IssuedBudget, LocalFailure, SettlementError};

fn settled(result: Result<(), SettlementError>) -> Result<(), StopReason> {
    match result {
        Ok(()) => Ok(()),
        Err(SettlementError::Stopped(reason)) => Err(reason),
        Err(SettlementError::ObservationExceedsGrant) => Err(StopReason::Cancelled),
    }
}

fn limits(n: u64) -> Limits {
    Limits {
        source_bytes: n,
        work: n,
        depth: n,
        nodes: n,
        allocation_units: n,
        output_bytes: n,
        diagnostics: n,
        events: n,
    }
}
fn usage(n: u64) -> Usage {
    Usage {
        source_bytes: n,
        work: n,
        depth: n,
        nodes: n,
        allocation_units: n,
        output_bytes: n,
        diagnostics: n,
        events: n,
    }
}
fn resources() -> [(Resource, StopReason); 7] {
    [
        (Resource::SourceBytes, StopReason::SourceLimit),
        (Resource::Work, StopReason::WorkLimit),
        (Resource::Nodes, StopReason::NodeLimit),
        (Resource::AllocationUnits, StopReason::AllocationLimit),
        (Resource::OutputBytes, StopReason::OutputLimit),
        (Resource::Diagnostics, StopReason::DiagnosticLimit),
        (Resource::Events, StopReason::EventLimit),
    ]
}

#[test]
fn reserved_capacity_and_actual_usage_remain_distinct() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(10));
    parent.record_observed_usage(usage(3))?;
    let mut issued = IssuedBudget::issue(&mut parent, limits(4))?;
    assert_eq!(issued.parent_usage(), usage(3));
    assert_eq!(issued.limits(), limits(4));
    issued
        .run_local(|b| {
            for (resource, _) in resources() {
                b.charge(resource, 3)?;
            }
            b.observe_depth(5)
        })
        .map_err(|failure| failure.reason)?;
    settled(issued.settle(usage(4)))?;
    assert_eq!(
        parent.usage(),
        Usage {
            depth: 5,
            ..usage(10)
        }
    );
    assert_eq!(parent.limits(), limits(10));
    Ok(())
}

#[test]
fn local_exhaustion_keeps_remote_capacity_and_first_stop() -> Result<(), StopReason> {
    for (resource, reason) in resources() {
        let mut parent = Budget::new(limits(10));
        let mut issued = IssuedBudget::issue(&mut parent, limits(4))?;
        assert_eq!(
            issued.run_local(|b| b.charge(resource, 7)),
            Err(LocalFailure {
                reason,
                output: None
            })
        );
        // Authenticated work already completed remotely is still recorded.
        assert_eq!(
            issued.settle(usage(4)),
            Err(SettlementError::Stopped(reason))
        );
        assert_eq!(parent.usage(), usage(4));
        assert_eq!(parent.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn unused_capacity_returns_only_after_settlement() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(10));
    settled(IssuedBudget::issue(&mut parent, limits(8))?.settle(usage(2)))?;
    parent.charge(Resource::Work, 8)?;
    assert_eq!(parent.usage().work, 10);
    let mut cancelled = Budget::new(limits(10));
    drop(IssuedBudget::issue(&mut cancelled, limits(8))?);
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    assert_eq!(cancelled.usage(), Usage::default());
    Ok(())
}

#[test]
fn oversized_or_unknown_observations_never_enter_accounting() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(10));
    parent.record_observed_usage(usage(3))?;
    assert!(IssuedBudget::issue(&mut parent, limits(8)).is_err());
    assert_eq!(parent.usage(), usage(3));
    assert_eq!(parent.poll(), Err(StopReason::SourceLimit));
    let mut parent = Budget::new(limits(10));
    let grant = IssuedBudget::issue(&mut parent, limits(4))?;
    assert_eq!(
        grant.settle(usage(5)),
        Err(SettlementError::ObservationExceedsGrant)
    );
    assert_eq!(parent.usage(), Usage::default());
    let mut parent = Budget::new(limits(10));
    assert!(
        IssuedBudget::issue(
            &mut parent,
            Limits {
                depth: 11,
                ..limits(0)
            }
        )
        .is_err()
    );
    assert_eq!(parent.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

#[test]
fn nested_grants_cannot_duplicate_available_capacity() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(10));
    let mut outer = IssuedBudget::issue(&mut parent, limits(4))?;
    outer
        .run_local(|local| settled(IssuedBudget::issue(local, limits(6))?.settle(usage(6))))
        .map_err(|failure| failure.reason)?;
    settled(outer.settle(usage(4)))?;
    assert_eq!(
        parent.usage(),
        Usage {
            depth: 6,
            ..usage(10)
        }
    );
    Ok(())
}

#[test]
fn zero_maximum_and_prior_stop_boundaries() -> Result<(), StopReason> {
    let mut zero = Budget::new(limits(0));
    settled(IssuedBudget::issue(&mut zero, limits(0))?.settle(usage(0)))?;
    assert_eq!(zero.poll(), Ok(()));
    let mut maximum = Budget::new(limits(u64::MAX));
    settled(IssuedBudget::issue(&mut maximum, limits(u64::MAX))?.settle(usage(u64::MAX)))?;
    assert_eq!(maximum.usage(), usage(u64::MAX));
    assert_eq!(
        maximum.charge(Resource::Work, 1),
        Err(StopReason::WorkLimit)
    );
    assert!(IssuedBudget::issue(&mut maximum, limits(0)).is_err());
    assert_eq!(maximum.poll(), Err(StopReason::WorkLimit));
    Ok(())
}

#[test]
fn every_observation_field_is_checked_before_commit() -> Result<(), StopReason> {
    for field in 0..8 {
        let mut parent = Budget::new(limits(10));
        parent.record_observed_usage(usage(2))?;
        let grant = IssuedBudget::issue(&mut parent, limits(4))?;
        let mut observation = usage(1);
        match field {
            0 => observation.source_bytes = 5,
            1 => observation.work = 5,
            2 => observation.depth = 5,
            3 => observation.nodes = 5,
            4 => observation.allocation_units = 5,
            5 => observation.output_bytes = 5,
            6 => observation.diagnostics = 5,
            _ => observation.events = 5,
        }
        assert_eq!(
            grant.settle(observation),
            Err(SettlementError::ObservationExceedsGrant)
        );
        assert_eq!(parent.usage(), usage(2));
        assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    }
    Ok(())
}

#[test]
fn ancestor_scope_covers_local_and_delegated_work() -> Result<(), StopReason> {
    use nepl3_suite::suspension::execution::ExecutionScope;
    let mut parent = Budget::new(limits(100));
    let scope = ExecutionScope::root(&mut parent, limits(10))?;
    scope.run(&mut parent, |parent| {
        let mut issued = IssuedBudget::issue(parent, limits(6))?;
        issued
            .run_local(|local| local.charge(Resource::Work, 4))
            .map_err(|failure| failure.reason)?;
        let mut remote = Budget::new(issued.limits());
        remote.charge(Resource::Work, 6)?;
        remote.observe_depth(3)?;
        // A locally owned metered execution supplies an independent observation.
        settled(issued.settle(remote.usage()))
    })?;
    assert_eq!(parent.limits(), limits(100));
    assert_eq!(parent.usage().work, 10);
    assert_eq!(parent.usage().depth, 3);
    assert_eq!(
        scope.run(&mut parent, |b| b.charge(Resource::Work, 1)),
        Err(StopReason::WorkLimit)
    );
    Ok(())
}

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
