use super::*;
use nepl3_suite::suspension::delegation::child::Error as ChildError;

#[test]
fn exact_request_binding_keeps_relative_grant_and_latest_parent_basis() -> Result<(), String> {
    for parent_paid in [false, true] {
        let (registry, mut request) = fixture()?;
        quota(&mut request);
        request.limits.work = 7;
        let original = request.clone();
        let sources = SourceStore::default();
        let grants =
            Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
        let implementation = Digest::of(b"trusted terminal child");
        // Independent oracle invokes validation primitives, not either issue API.
        let mut validation = budget();
        request
            .validate_input(&request.operation, &registry, &mut validation)
            .map_err(error)?;
        let context = nepl3_wire::operation::context_digest(
            &request,
            implementation,
            &registry,
            &mut validation,
        )
        .map_err(error)?;
        let cost = if parent_paid {
            validation.usage()
        } else {
            Usage::default()
        };
        let mut parent = budget();
        parent.charge(Resource::Work, 30).map_err(error)?;
        parent.observe_depth(80).map_err(error)?;
        let mut issued = if parent_paid {
            IssuedInvocation::issue_with_parent_validation(
                grants.admit(&request, &mut budget()).map_err(error)?,
                implementation,
                &registry,
                &mut parent,
            )
        } else {
            IssuedInvocation::issue(
                grants.admit(&request, &mut budget()).map_err(error)?,
                implementation,
                &registry,
                &mut parent,
                &mut budget(),
            )
        }
        .map_err(error)?;
        let expected_before_local = Usage {
            work: 30 + cost.work,
            depth: 80,
            ..cost
        };
        assert_eq!(issued.parent_usage(), expected_before_local);
        issued
            .run_local(|b| b.charge(Resource::Work, 3))
            .map_err(error)?;
        let basis = Usage {
            work: expected_before_local.work + 3,
            ..expected_before_local
        };
        let result = issued
            .execute_local_child(6, |bound, child, _| {
                assert!(std::ptr::eq(bound, &request));
                assert_eq!(bound, &original);
                assert_eq!(child.usage(), basis);
                assert!(basis.work > bound.limits.work);
                assert_eq!(child.limits().work, basis.work + bound.limits.work);
                assert_eq!(child.current_depth(), 6);
                child.charge(Resource::Work, bound.limits.work)?;
                Ok::<_, StopReason>(53)
            })
            .map_err(error)?;
        assert_eq!(result.request_id(), request.request_id);
        assert_eq!(result.implementation(), implementation);
        assert_eq!(result.context(), context);
        assert_eq!(result.execution().basis(), basis);
        let execution = result.into_execution();
        assert_eq!(execution.output(), &Ok(53));
        assert_eq!(execution.stopped(), None);
        assert_eq!(
            execution.settled_usage(),
            Usage {
                work: 7,
                depth: 6,
                ..Usage::default()
            }
        );
        assert_eq!(
            execution.cumulative_usage(),
            Usage {
                work: basis.work + 7,
                ..basis
            }
        );
        assert_eq!(parent.usage(), execution.cumulative_usage());
        assert_eq!(parent.poll(), Ok(()));
    }
    Ok(())
}

#[test]
fn child_semantic_error_and_stopped_output_remain_distinct() -> Result<(), String> {
    for stop in [false, true] {
        let (registry, mut request) = fixture()?;
        quota(&mut request);
        request.limits.work = 7;
        let sources = SourceStore::default();
        let grants =
            Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
        let mut parent = budget();
        parent.charge(Resource::Work, 30).map_err(error)?;
        let issued = IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            Digest::of(b"trusted child"),
            &registry,
            &mut parent,
        )
        .map_err(error)?;
        let basis = issued.parent_usage();
        let result = issued
            .execute_local_child(6, |_, child, _| {
                assert_eq!(child.charge(Resource::Work, 4), Ok(()));
                if stop {
                    assert_eq!(child.charge(Resource::Work, 4), Err(StopReason::WorkLimit));
                }
                Err::<(), _>("semantic failure")
            })
            .map_err(error)?;
        assert_eq!(result.execution().output(), &Err("semantic failure"));
        assert_eq!(
            result.execution().stopped(),
            if stop {
                Some(StopReason::WorkLimit)
            } else {
                None
            }
        );
        assert_eq!(result.execution().settled_usage().work, 4);
        assert_eq!(parent.usage().work, basis.work + 4);
        assert_eq!(parent.poll(), Ok(()));
    }
    Ok(())
}

#[test]
fn child_setup_failure_retains_parent_paid_cost_without_callback() -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    request.limits.depth = 7;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut parent = budget();
    parent.observe_depth(80).map_err(error)?;
    let issued = IssuedInvocation::issue_with_parent_validation(
        grants.admit(&request, &mut budget()).map_err(error)?,
        Digest::of(b"trusted child"),
        &registry,
        &mut parent,
    )
    .map_err(error)?;
    let basis = issued.parent_usage();
    assert!(basis.work > 0);
    let mut called = false;
    let result = issued.execute_local_child(1, |_, _, _| {
        called = true;
        17
    });
    let Err(failure) = result else {
        return Err("setup rejection required".into());
    };
    assert_eq!(failure.error, ChildError::Setup(StopReason::DepthLimit));
    assert_eq!(failure.output, None);
    assert!(!called);
    assert_eq!(parent.usage(), basis);
    assert_eq!(parent.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

#[test]
fn child_reset_failure_forwards_output_and_cancels_unresolved_reservation() -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut parent = budget();
    parent.charge(Resource::Work, 30).map_err(error)?;
    let issued = IssuedInvocation::issue(
        grants.admit(&request, &mut budget()).map_err(error)?,
        Digest::of(b"trusted child"),
        &registry,
        &mut parent,
        &mut budget(),
    )
    .map_err(error)?;
    let basis = issued.parent_usage();
    let result = issued.execute_local_child(0, |_, child, _| {
        *child = Budget::new(child.limits());
        19
    });
    let Err(failure) = result else {
        return Err("reset rejection required".into());
    };
    assert_eq!(failure.error, ChildError::ChangedBudget);
    assert_eq!(failure.output, Some(19));
    assert_eq!(parent.usage(), basis);
    assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    Ok(())
}
