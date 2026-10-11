use super::*;
use nepl3_core::operation::request::RequestBindingError as Binding;
use nepl3_provider::delegation::SavedSettlementError as BoundError;
fn observed() -> Usage {
    Usage {
        source_bytes: 1,
        work: 7,
        depth: 12,
        nodes: 2,
        allocation_units: 3,
        output_bytes: 4,
        diagnostics: 1,
        events: 1,
    }
}
fn implementation() -> Digest {
    Digest::of(b"bound measurement fixture")
}
fn prepare() -> Result<(SchemaRegistry, Invoke), String> {
    let (r, mut q) = fixture()?;
    quota(&mut q);
    Ok((r, q))
}
#[test]
fn exact_saved_request_settles_comparison_and_all_observed_resources_at_active_depth()
-> Result<(), String> {
    let (registry, request) = prepare()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    for depth in [0, 7] {
        let mut parent = budget();
        parent.charge(Resource::Work, 91).map_err(error)?;
        parent
            .with_depth_at_least(depth, |b| {
                Ok::<_, StopReason>((|| -> Result<(), String> {
                    let issued = IssuedInvocation::issue(
                        grants.admit(&request, &mut budget()).map_err(error)?,
                        implementation(),
                        &registry,
                        b,
                        &mut budget(),
                    )
                    .map_err(error)?;
                    let context = issued.context();
                    let before = issued.parent_usage();
                    let mut oracle = budget();
                    oracle.record_observed_usage(before).map_err(error)?;
                    oracle
                        .with_depth_at_least(depth, |x| request.clone().check_saved(&request, x))
                        .map_err(error)?;
                    oracle.record_observed_usage(observed()).map_err(error)?;
                    issued
                        .settle_saved_request(
                            &request.clone(),
                            implementation(),
                            context,
                            observed(),
                        )
                        .map_err(error)?;
                    assert_eq!(b.usage(), oracle.usage());
                    assert_eq!(b.current_depth(), depth);
                    assert_eq!(b.poll(), Ok(()));
                    Ok(())
                })())
            })
            .map_err(error)??;
        assert_eq!(parent.current_depth(), 0);
    }
    Ok(())
}
#[test]
fn same_context_different_request_is_rejected_without_settling_observation() -> Result<(), String> {
    let (registry, request) = prepare()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    for field in 0..4 {
        let mut changed = request.clone();
        let expected = match field {
            0 => {
                changed.request_id += 1;
                Binding::RequestId
            }
            1 => {
                changed.operation.name.push('x');
                Binding::Operation
            }
            2 => {
                let TypedValue::Record(r) = &mut changed.input else {
                    return Err("record".into());
                };
                r.fields[0] = NdfValue::U64(77);
                Binding::Input
            }
            _ => {
                changed.limits.work -= 1;
                Binding::Limits
            }
        };
        let mut parent = budget();
        let issued = IssuedInvocation::issue(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent,
            &mut budget(),
        )
        .map_err(error)?;
        let context = issued.context();
        assert_eq!(
            nepl3_wire::operation::context_digest(
                &changed,
                implementation(),
                &registry,
                &mut budget()
            )
            .map_err(error)?,
            context
        );
        let mut oracle = budget();
        oracle
            .record_observed_usage(issued.parent_usage())
            .map_err(error)?;
        assert_eq!(changed.check_saved(&request, &mut oracle), Err(expected));
        assert!(
            matches!(issued.settle_saved_request(&changed,implementation(),context,observed()),Err(BoundError::RequestBinding(e)) if e==expected)
        );
        assert_eq!(parent.usage(), oracle.usage());
        assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    }
    Ok(())
}
#[test]
fn identity_and_excess_observation_fail_after_comparison_without_remote_charge()
-> Result<(), String> {
    let (registry, request) = prepare()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    for variant in 0..3 {
        let mut parent = budget();
        let issued = IssuedInvocation::issue(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent,
            &mut budget(),
        )
        .map_err(error)?;
        let context = issued.context();
        let mut oracle = budget();
        oracle
            .record_observed_usage(issued.parent_usage())
            .map_err(error)?;
        request.check_saved(&request, &mut oracle).map_err(error)?;
        let mut usage = observed();
        if variant == 2 {
            usage.work = request.limits.work + 1;
        }
        let result = issued.settle_saved_request(
            &request,
            if variant == 0 {
                Digest::of(b"wrong")
            } else {
                implementation()
            },
            if variant == 1 {
                Digest::of(b"wrong")
            } else {
                context
            },
            usage,
        );
        if variant == 2 {
            assert!(matches!(
                result,
                Err(BoundError::Settlement(Error::Settlement(
                    nepl3_suite::suspension::delegation::SettlementError::ObservationExceedsGrant
                )))
            ));
        } else {
            assert!(matches!(
                result,
                Err(BoundError::Settlement(Error::ObservationBinding))
            ));
        }
        assert_eq!(parent.usage(), oracle.usage());
        assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    }
    Ok(())
}
#[test]
fn matching_cannot_spend_reserved_quota_and_preserves_sticky_stop() -> Result<(), String> {
    let (registry, request) = prepare()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut direct = budget();
    request.check_saved(&request, &mut direct).map_err(error)?;
    for (limits, reason) in [
        (
            Limits {
                work: request.limits.work + direct.usage().work - 1,
                ..budget().limits()
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: request.limits.allocation_units,
                ..budget().limits()
            },
            StopReason::AllocationLimit,
        ),
    ] {
        let mut parent = Budget::new(limits);
        let issued = IssuedInvocation::issue(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent,
            &mut budget(),
        )
        .map_err(error)?;
        let context = issued.context();
        let local = Limits {
            work: limits.work - request.limits.work,
            allocation_units: limits.allocation_units - request.limits.allocation_units,
            ..limits
        };
        let mut oracle = Budget::new(local);
        assert_eq!(
            request.check_saved(&request, &mut oracle),
            Err(Binding::Stopped(reason))
        );
        assert!(
            matches!(issued.settle_saved_request(&request,implementation(),context,observed()),Err(BoundError::Stopped(e)) if e==reason)
        );
        assert_eq!(parent.usage(), oracle.usage());
        assert_eq!(parent.limits(), limits);
        assert_eq!(parent.poll(), Err(reason));
    }
    Ok(())
}
#[test]
fn already_stopped_new_entry_does_not_replace_compatibility_settlement() -> Result<(), String> {
    let (registry, request) = prepare()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    for checked in [false, true] {
        let mut parent = budget();
        let mut issued = IssuedInvocation::issue(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent,
            &mut budget(),
        )
        .map_err(error)?;
        let context = issued.context();
        let stopped: Result<(), _> = issued.run_local(|_| Err(StopReason::Cancelled));
        assert!(stopped.is_err());
        let before = issued.parent_usage();
        if checked {
            assert!(matches!(
                issued.settle_saved_request(&request, implementation(), context, observed()),
                Err(BoundError::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(parent.usage(), before);
        } else {
            assert!(matches!(
                issued.settle(request.request_id, implementation(), context, observed()),
                Err(Error::Settlement(
                    nepl3_suite::suspension::delegation::SettlementError::Stopped(
                        StopReason::Cancelled
                    )
                ))
            ));
            assert_eq!(parent.usage().work, before.work + observed().work);
        }
        assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    }
    Ok(())
}
#[test]
fn depth_stop_during_comparison_preserves_charges_without_settlement() -> Result<(), String> {
    let (registry, mut request) = prepare()?;
    request.limits.depth = 0;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let limits = Limits {
        depth: 0,
        ..budget().limits()
    };
    let mut parent = Budget::new(limits);
    let issued = IssuedInvocation::issue(
        grants.admit(&request, &mut budget()).map_err(error)?,
        implementation(),
        &registry,
        &mut parent,
        &mut budget(),
    )
    .map_err(error)?;
    let context = issued.context();
    let mut oracle = Budget::new(limits);
    assert_eq!(
        request.check_saved(&request, &mut oracle),
        Err(Binding::Stopped(StopReason::DepthLimit))
    );
    assert!(matches!(
        issued.settle_saved_request(&request, implementation(), context, observed()),
        Err(BoundError::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(parent.usage(), oracle.usage());
    assert_eq!(parent.poll(), Err(StopReason::DepthLimit));
    assert_eq!(parent.current_depth(), 0);
    assert_eq!(parent.limits(), limits);
    Ok(())
}
