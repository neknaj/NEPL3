use super::*;
use nepl3_core::{operation::request::InputValidationError, value_codec::FoundationCodecError};

#[test]
fn parent_paid_issuance_retains_cost_before_reserving_unchanged_grant() -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    request.limits.work = 23;
    let original = request.clone();
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let implementation = Digest::of(b"selected implementation");
    // Independent oracle calls the two validation primitives, bypassing issuance.
    let mut measured = budget();
    request
        .validate_input(&request.operation, &registry, &mut measured)
        .map_err(error)?;
    let input_work = measured.usage().work;
    let context =
        nepl3_wire::operation::context_digest(&request, implementation, &registry, &mut measured)
            .map_err(error)?;
    let prior = 7;
    let cost = measured.usage();
    assert!(cost.work > 0);
    let exact = prior + cost.work + request.limits.work;
    for (limit, succeeds) in [(exact, true), (exact - 1, false)] {
        let mut parent = Budget::new(Limits {
            work: limit,
            ..budget().limits()
        });
        parent.charge(Resource::Work, prior).map_err(error)?;
        let approved = grants.admit(&request, &mut budget()).map_err(error)?;
        if succeeds {
            let issued = IssuedInvocation::issue_with_parent_validation(
                approved,
                implementation,
                &registry,
                &mut parent,
            )
            .map_err(error)?;
            assert_eq!(issued.request(), &original);
            assert_eq!(issued.context(), context);
            assert_eq!(
                issued.parent_usage(),
                Usage {
                    work: prior + cost.work,
                    ..cost
                }
            );
            // A reservation is capacity, not consumed remote usage.
            issued
                .settle(
                    request.request_id,
                    implementation,
                    context,
                    Usage {
                        work: 11,
                        ..Usage::default()
                    },
                )
                .map_err(error)?;
            assert_eq!(
                parent.usage(),
                Usage {
                    work: prior + cost.work + 11,
                    ..cost
                }
            );
            assert_eq!(parent.poll(), Ok(()));
        } else {
            assert!(matches!(
                IssuedInvocation::issue_with_parent_validation(
                    approved,
                    implementation,
                    &registry,
                    &mut parent,
                ),
                Err(Error::Stopped(StopReason::WorkLimit))
            ));
            assert_eq!(
                parent.usage(),
                Usage {
                    work: prior + cost.work,
                    ..cost
                }
            );
            assert_eq!(parent.poll(), Err(StopReason::WorkLimit));
        }
    }
    // Stops during either validation stage retain the original cause and consumed work.
    for (limit, in_input) in [
        (prior + input_work - 1, true),
        (prior + cost.work - 1, false),
    ] {
        let mut parent = Budget::new(Limits {
            work: limit,
            ..budget().limits()
        });
        parent.charge(Resource::Work, prior).map_err(error)?;
        let result = IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation,
            &registry,
            &mut parent,
        );
        if in_input {
            assert!(matches!(
                result,
                Err(Error::Input(InputValidationError::Stopped(
                    StopReason::WorkLimit
                )))
            ));
        } else {
            assert!(
                matches!(&result, Err(Error::Context(e)) if e.stop_reason() == Some(StopReason::WorkLimit))
            );
        }
        drop(result);
        assert_eq!(parent.poll(), Err(StopReason::WorkLimit));
        assert!(parent.usage().work >= prior);
        assert!(parent.usage().work <= limit);
        let mut oracle = Budget::new(parent.limits());
        oracle.charge(Resource::Work, prior).map_err(error)?;
        let expected = request
            .validate_input(&request.operation, &registry, &mut oracle)
            .map_err(Error::Input)
            .and_then(|_| {
                nepl3_wire::operation::context_digest(
                    &request,
                    implementation,
                    &registry,
                    &mut oracle,
                )
                .map_err(Error::Context)
            });
        assert!(expected.is_err());
        assert_eq!(parent.usage(), oracle.usage());
        assert_eq!(parent.poll(), oracle.poll());
    }
    let allocation_prior = 11;
    let allocation_exact =
        allocation_prior + cost.allocation_units + request.limits.allocation_units;
    for (limit, succeeds) in [(allocation_exact, true), (allocation_exact - 1, false)] {
        let mut parent = Budget::new(Limits {
            allocation_units: limit,
            ..budget().limits()
        });
        parent
            .charge(Resource::AllocationUnits, allocation_prior)
            .map_err(error)?;
        let approved = grants.admit(&request, &mut budget()).map_err(error)?;
        if succeeds {
            let issued = IssuedInvocation::issue_with_parent_validation(
                approved,
                implementation,
                &registry,
                &mut parent,
            )
            .map_err(error)?;
            assert_eq!(issued.request(), &original);
            issued
                .settle(
                    request.request_id,
                    implementation,
                    context,
                    Usage::default(),
                )
                .map_err(error)?;
            assert_eq!(parent.poll(), Ok(()));
        } else {
            assert!(matches!(
                IssuedInvocation::issue_with_parent_validation(
                    approved,
                    implementation,
                    &registry,
                    &mut parent,
                ),
                Err(Error::Stopped(StopReason::AllocationLimit))
            ));
            assert_eq!(parent.poll(), Err(StopReason::AllocationLimit));
        }
        assert_eq!(
            parent.usage(),
            Usage {
                allocation_units: allocation_prior + cost.allocation_units,
                ..cost
            }
        );
    }
    for (ceiling, succeeds) in [(exact, true), (exact - 1, false)] {
        let mut parent = budget();
        let outer = parent.limits();
        parent.charge(Resource::Work, prior).map_err(error)?;
        let approved = grants.admit(&request, &mut budget()).map_err(error)?;
        parent
            .with_ceiling(
                Limits {
                    work: ceiling,
                    ..outer
                },
                |parent| {
                    let result = IssuedInvocation::issue_with_parent_validation(
                        approved,
                        implementation,
                        &registry,
                        parent,
                    );
                    if succeeds {
                        let issued = result.map_err(|_| StopReason::Cancelled)?;
                        issued
                            .settle(
                                request.request_id,
                                implementation,
                                context,
                                Usage::default(),
                            )
                            .map_err(|_| StopReason::Cancelled)?;
                    } else {
                        assert!(matches!(result, Err(Error::Stopped(StopReason::WorkLimit))));
                    }
                    Ok::<_, StopReason>(())
                },
            )
            .map_err(error)?;
        assert_eq!(parent.limits(), outer);
        assert_eq!(
            parent.usage(),
            Usage {
                work: prior + cost.work,
                ..cost
            }
        );
        assert_eq!(
            parent.poll(),
            if succeeds {
                Ok(())
            } else {
                Err(StopReason::WorkLimit)
            }
        );
    }
    // Legacy issuance deliberately continues to charge its separate validation budget.
    let mut parent = budget();
    let mut validation = budget();
    let issued = IssuedInvocation::issue(
        grants.admit(&request, &mut budget()).map_err(error)?,
        implementation,
        &registry,
        &mut parent,
        &mut validation,
    )
    .map_err(error)?;
    assert_eq!(issued.context(), context);
    assert_eq!(issued.parent_usage(), Usage::default());
    assert_eq!(validation.usage(), cost);
    issued
        .settle(
            request.request_id,
            implementation,
            context,
            Usage::default(),
        )
        .map_err(error)?;
    Ok(())
}

#[test]
fn parent_paid_issuance_preserves_validation_failure_and_sticky_stop() -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    let TypedValue::Record(input) = &mut request.input else {
        return Err("record required".into());
    };
    input.fields.clear();
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let implementation = Digest::of(b"implementation");
    let mut measured = budget();
    assert!(
        request
            .validate_input(&request.operation, &registry, &mut measured)
            .is_err()
    );
    assert!(measured.usage().work > 0);
    let mut parent = budget();
    assert!(matches!(
        IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation,
            &registry,
            &mut parent,
        ),
        Err(Error::Input(_))
    ));
    assert_eq!(parent.usage(), measured.usage());
    assert_eq!(parent.poll(), Ok(()));
    for stop in [StopReason::Cancelled, StopReason::WorkLimit] {
        let mut parent = budget();
        parent.charge(Resource::Work, 7).map_err(error)?;
        parent.stop(stop);
        let before = parent.usage();
        assert!(matches!(IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?, implementation, &registry, &mut parent,
        ), Err(Error::Stopped(reason)) if reason == stop));
        assert_eq!(parent.usage(), before);
        assert_eq!(parent.poll(), Err(stop));
    }
    Ok(())
}
