use super::*;
use nepl3_core::operation::request::InputValidationError;
fn implementation() -> Digest {
    Digest::of(b"parent validation fixture")
}
fn setup() -> Result<(SchemaRegistry, Invoke, SourceStore), String> {
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    let source = SourceSnapshot::new(
        SourceId("validation-source".into()),
        0,
        "memory:validation-source".into(),
        b"source".to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let mut sources = SourceStore::default();
    sources.insert(source.clone()).map_err(error)?;
    request.sources.push(source);
    Ok((registry, request, sources))
}
fn direct(request: &Invoke, registry: &SchemaRegistry, b: &mut Budget) -> Result<Digest, String> {
    request
        .validate_input(&request.operation, registry, b)
        .map_err(error)?;
    nepl3_wire::operation::context_digest(request, implementation(), registry, b).map_err(error)
}
fn set(l: &mut Limits, r: Resource, n: u64) {
    match r {
        Resource::SourceBytes => l.source_bytes = n,
        Resource::Work => l.work = n,
        Resource::Nodes => l.nodes = n,
        Resource::AllocationUnits => l.allocation_units = n,
        Resource::OutputBytes => l.output_bytes = n,
        Resource::Diagnostics => l.diagnostics = n,
        Resource::Events => l.events = n,
    }
}
fn used(u: Usage, r: Resource) -> u64 {
    match r {
        Resource::SourceBytes => u.source_bytes,
        Resource::Work => u.work,
        Resource::Nodes => u.nodes,
        Resource::AllocationUnits => u.allocation_units,
        Resource::OutputBytes => u.output_bytes,
        Resource::Diagnostics => u.diagnostics,
        Resource::Events => u.events,
    }
}
fn cap(l: Limits, r: Resource) -> u64 {
    match r {
        Resource::SourceBytes => l.source_bytes,
        Resource::Work => l.work,
        Resource::Nodes => l.nodes,
        Resource::AllocationUnits => l.allocation_units,
        Resource::OutputBytes => l.output_bytes,
        Resource::Diagnostics => l.diagnostics,
        Resource::Events => l.events,
    }
}
fn reason(r: Resource) -> StopReason {
    match r {
        Resource::SourceBytes => StopReason::SourceLimit,
        Resource::Work => StopReason::WorkLimit,
        Resource::Nodes => StopReason::NodeLimit,
        Resource::AllocationUnits => StopReason::AllocationLimit,
        Resource::OutputBytes => StopReason::OutputLimit,
        Resource::Diagnostics => StopReason::DiagnosticLimit,
        Resource::Events => StopReason::EventLimit,
    }
}
const ADDITIVE: [Resource; 7] = [
    Resource::SourceBytes,
    Resource::Work,
    Resource::Nodes,
    Resource::AllocationUnits,
    Resource::OutputBytes,
    Resource::Diagnostics,
    Resource::Events,
];

#[test]
fn parent_validation_matches_direct_work_at_actual_depth() -> Result<(), String> {
    let (registry, request, sources) = setup()?;
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    for depth in [0, 7] {
        let mut parent = budget();
        parent.charge(Resource::Work, 123).map_err(error)?;
        parent
            .with_depth_at_least(depth, |b| {
                Ok::<_, StopReason>((|| -> Result<(), String> {
                    let mut oracle = budget();
                    oracle.record_observed_usage(b.usage()).map_err(error)?;
                    let identity = oracle
                        .with_depth_at_least(depth, |o| {
                            Ok::<_, StopReason>(direct(&request, &registry, o))
                        })
                        .map_err(error)??;
                    let expected = oracle.usage();
                    let issued = IssuedInvocation::issue_with_parent_validation(
                        grants.admit(&request, &mut budget()).map_err(error)?,
                        implementation(),
                        &registry,
                        b,
                    )
                    .map_err(error)?;
                    assert_eq!(issued.context(), identity);
                    assert_eq!(issued.parent_usage(), expected);
                    assert!(std::ptr::eq(issued.request(), &request));
                    let execution = issued
                        .execute_local_child(depth, |_, child, _| child.charge(Resource::Work, 1))
                        .map_err(|f| error(f.error))?;
                    assert_eq!(execution.execution().basis(), expected);
                    assert!(execution.execution().output().is_ok());
                    assert_eq!(execution.execution().settled_usage().work, 1);
                    Ok(())
                })())
            })
            .map_err(error)??;
        assert_eq!(parent.current_depth(), 0);
        assert_eq!(parent.poll(), Ok(()));
    }
    Ok(())
}

#[test]
fn rejected_input_retains_cost_before_any_reservation_and_can_retry() -> Result<(), String> {
    let (registry, request, sources) = setup()?;
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    for unknown in [false, true] {
        let mut bad = request.clone();
        bad.limits.work = u64::MAX;
        if unknown {
            bad.operation.name = "unknown-operation".into();
        } else {
            let TypedValue::Record(input) = &mut bad.input else {
                return Err("record".into());
            };
            input.fields.clear();
        }
        let mut oracle = budget();
        assert!(direct(&bad, &registry, &mut oracle).is_err());
        let mut parent = budget();
        assert!(matches!(
            IssuedInvocation::issue_with_parent_validation(
                grants.admit(&bad, &mut budget()).map_err(error)?,
                implementation(),
                &registry,
                &mut parent
            ),
            Err(Error::Input(_))
        ));
        let spent = parent.usage();
        assert_eq!(spent, oracle.usage());
        assert!(spent.work > 0);
        assert_eq!(parent.poll(), Ok(()));
        let issued = IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent,
        )
        .map_err(error)?;
        assert!(issued.parent_usage().work > spent.work);
        let identity = issued.context();
        issued
            .settle(
                request.request_id,
                implementation(),
                identity,
                Usage::default(),
            )
            .map_err(error)?;
        assert_eq!(parent.poll(), Ok(()));
    }
    Ok(())
}

#[test]
fn context_error_retains_cost_and_can_retry_with_complete_registry() -> Result<(), String> {
    let (registry, request, sources) = setup()?;
    let mut partial = SchemaRegistry::default();
    partial
        .register(
            request.operation.schema.clone(),
            registry
                .descriptor(&request.operation.schema)
                .ok_or("fixture descriptor")?
                .clone(),
            &mut budget(),
        )
        .map_err(error)?;
    partial.finalize(&mut budget()).map_err(error)?;
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut oracle = budget();
    assert!(direct(&request, &partial, &mut oracle).is_err());
    let mut parent = budget();
    assert!(matches!(
        IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &partial,
            &mut parent
        ),
        Err(Error::Context(_))
    ));
    assert_eq!(parent.usage(), oracle.usage());
    assert!(parent.usage().work > 0);
    assert_eq!(parent.poll(), Ok(()));
    let before = parent.usage();
    let issued = IssuedInvocation::issue_with_parent_validation(
        grants.admit(&request, &mut budget()).map_err(error)?,
        implementation(),
        &registry,
        &mut parent,
    )
    .map_err(error)?;
    assert!(issued.parent_usage().work > before.work);
    let context = issued.context();
    issued
        .settle(
            request.request_id,
            implementation(),
            context,
            Usage::default(),
        )
        .map_err(error)?;
    assert_eq!(parent.poll(), Ok(()));
    Ok(())
}

#[test]
fn input_and_context_resource_stops_keep_their_stage_and_spent_prefix() -> Result<(), String> {
    let (registry, request, sources) = setup()?;
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut input = budget();
    request
        .validate_input(&request.operation, &registry, &mut input)
        .map_err(error)?;
    let mut complete = budget();
    direct(&request, &registry, &mut complete)?;
    let cases = [
        (Resource::Work, input.usage().work - 1, false),
        (Resource::Nodes, 0, false),
        (
            Resource::Work,
            input.usage().work + (complete.usage().work - input.usage().work) / 2,
            true,
        ),
        (Resource::SourceBytes, 0, true),
        (Resource::Nodes, input.usage().nodes, true),
        (
            Resource::AllocationUnits,
            input.usage().allocation_units,
            true,
        ),
        (Resource::OutputBytes, 0, true),
    ];
    for (resource, limit, context) in cases {
        let mut limits = budget().limits();
        set(&mut limits, resource, limit);
        let mut oracle = Budget::new(limits);
        assert!(direct(&request, &registry, &mut oracle).is_err());
        let expected_prefix = oracle.usage();
        let mut parent = Budget::new(limits);
        let result = IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent,
        );
        let failure = result.err().ok_or("expected validation stop")?;
        if context {
            assert!(
                matches!(failure, Error::Context(_)),
                "resource={resource:?} limit={limit} failure={failure:?}"
            );
        } else {
            assert!(
                matches!(failure, Error::Input(_)),
                "resource={resource:?} limit={limit} failure={failure:?}"
            );
        }
        assert_eq!(parent.poll(), Err(reason(resource)));
        let spent = parent.usage();
        assert_eq!(spent, expected_prefix);
        assert_eq!(parent.current_depth(), 0);
        if matches!(resource, Resource::OutputBytes) {
            assert_eq!(spent.source_bytes, request.sources[0].text().len() as u64);
        }
        assert!(
            spent.work > 0,
            "resource={resource:?} context={context} limit={limit}"
        );
        if context {
            assert!(spent.work >= input.usage().work);
        }
        assert!(
            matches!(IssuedInvocation::issue_with_parent_validation(grants.admit(&request,&mut budget()).map_err(error)?,implementation(),&registry,&mut parent),Err(Error::Stopped(s)) if s==reason(resource))
        );
        assert_eq!(parent.usage(), spent);
    }
    assert!(complete.usage().depth > input.usage().depth);
    let mut context_limits = budget().limits();
    context_limits.depth = input.usage().depth;
    let mut context_oracle = Budget::new(context_limits);
    assert!(direct(&request, &registry, &mut context_oracle).is_err());
    let mut context_parent = Budget::new(context_limits);
    assert!(matches!(
        IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut context_parent
        ),
        Err(Error::Context(_))
    ));
    assert_eq!(context_parent.poll(), Err(StopReason::DepthLimit));
    assert_eq!(context_parent.usage(), context_oracle.usage());
    assert_eq!(context_parent.current_depth(), 0);
    assert!(context_parent.usage().work >= input.usage().work);
    let mut limits = budget().limits();
    limits.depth = 0;
    let mut parent = Budget::new(limits);
    assert!(matches!(
        IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent
        ),
        Err(Error::Input(InputValidationError::Stopped(
            StopReason::DepthLimit
        )))
    ));
    assert_eq!(parent.poll(), Err(StopReason::DepthLimit));
    let mut cancelled = budget();
    cancelled.charge(Resource::Work, 17).map_err(error)?;
    cancelled.cancel();
    let before = cancelled.usage();
    assert!(matches!(
        IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut cancelled
        ),
        Err(Error::Stopped(StopReason::Cancelled))
    ));
    assert_eq!(cancelled.usage(), before);
    Ok(())
}

#[test]
fn reservation_uses_remaining_capacity_after_validation_for_all_additive_resources()
-> Result<(), String> {
    let (registry, request, sources) = setup()?;
    let limits = budget().limits();
    let mut oracle = budget();
    direct(&request, &registry, &mut oracle)?;
    let expected = oracle.usage();
    for resource in ADDITIVE {
        for extra in [0, 1] {
            let mut request = request.clone();
            set(
                &mut request.limits,
                resource,
                cap(limits, resource) - used(expected, resource) + extra,
            );
            let grants =
                Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
            let mut parent = Budget::new(limits);
            let result = IssuedInvocation::issue_with_parent_validation(
                grants.admit(&request, &mut budget()).map_err(error)?,
                implementation(),
                &registry,
                &mut parent,
            );
            if extra == 0 {
                let issued = result.map_err(error)?;
                assert_eq!(issued.parent_usage(), expected);
                let context = issued.context();
                issued
                    .settle(
                        request.request_id,
                        implementation(),
                        context,
                        Usage::default(),
                    )
                    .map_err(error)?;
                assert_eq!(parent.poll(), Ok(()));
            } else {
                let failure = result.err().ok_or("expected capacity rejection")?;
                assert!(matches!(failure,Error::Stopped(s) if s==reason(resource)));
                assert_eq!(parent.poll(), Err(reason(resource)));
            }
            assert_eq!(parent.usage(), expected);
        }
    }
    for maximal in [false, true] {
        let mut request = request.clone();
        for r in ADDITIVE {
            set(&mut request.limits, r, if maximal { u64::MAX } else { 0 });
        }
        let grants =
            Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
        let mut parent = budget();
        let result = IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent,
        );
        if maximal {
            assert!(result.err().is_some());
        } else {
            let issued = result.map_err(error)?;
            let context = issued.context();
            issued
                .settle(
                    request.request_id,
                    implementation(),
                    context,
                    Usage::default(),
                )
                .map_err(error)?;
        }
        assert_eq!(parent.usage(), expected);
    }
    Ok(())
}

#[test]
fn validation_respects_ancestor_limits_and_child_setup_keeps_depth_checks() -> Result<(), String> {
    let (registry, mut request, sources) = setup()?;
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut parent = budget();
    let original = parent.limits();
    let mut ceiling = original;
    ceiling.work = 200;
    parent
        .with_ceiling(ceiling, |b| {
            assert!(
                IssuedInvocation::issue_with_parent_validation(
                    grants
                        .admit(&request, &mut budget())
                        .map_err(|_| StopReason::Cancelled)?,
                    implementation(),
                    &registry,
                    b
                )
                .is_err()
            );
            Ok::<_, StopReason>(())
        })
        .map_err(error)?;
    assert_eq!(parent.limits(), original);
    assert_eq!(parent.current_depth(), 0);
    assert!(parent.usage().work > 0);
    assert_eq!(parent.poll(), Err(StopReason::WorkLimit));
    request.limits.depth = 0;
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut parent = budget();
    let issued = IssuedInvocation::issue_with_parent_validation(
        grants.admit(&request, &mut budget()).map_err(error)?,
        implementation(),
        &registry,
        &mut parent,
    )
    .map_err(error)?;
    let validated = issued.parent_usage();
    assert!(validated.depth > 0);
    let mut called = false;
    let result = issued.execute_local_child(0, |_, _, _| {
        called = true;
    });
    assert!(result.is_err());
    assert!(!called);
    assert_eq!(parent.poll(), Err(StopReason::DepthLimit));
    assert_eq!(parent.usage(), validated);
    Ok(())
}

#[cfg(panic = "unwind")]
#[test]
fn unwinding_before_and_after_issue_preserves_cost_and_reservation_lifetime() -> Result<(), String>
{
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let (registry, request, sources) = setup()?;
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    for issued in [false, true] {
        let mut bad = request.clone();
        if !issued {
            let TypedValue::Record(input) = &mut bad.input else {
                return Err("record".into());
            };
            input.fields.clear();
        }
        let mut parent = budget();
        let limits = parent.limits();
        let mut oracle = budget();
        assert_eq!(direct(&bad, &registry, &mut oracle).is_ok(), issued);
        let authorized = grants.admit(&bad, &mut budget()).map_err(error)?;
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let result = IssuedInvocation::issue_with_parent_validation(
                authorized,
                implementation(),
                &registry,
                &mut parent,
            );
            assert_eq!(result.is_ok(), issued);
            std::panic::resume_unwind(Box::new("outer test scope"));
        }));
        assert!(panic.is_err());
        assert_eq!(parent.usage(), oracle.usage());
        assert!(parent.usage().work > 0);
        assert_eq!(parent.limits(), limits);
        assert_eq!(parent.current_depth(), 0);
        assert_eq!(
            parent.poll(),
            if issued {
                Err(StopReason::Cancelled)
            } else {
                Ok(())
            }
        );
    }
    Ok(())
}
