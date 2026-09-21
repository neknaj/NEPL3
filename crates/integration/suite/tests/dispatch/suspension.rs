use super::*;
use nepl3_core::operation::{
    Continuation, OperationReply,
    lifetime::{RequestLifetimes, RequestPhase},
};
use nepl3_suite::suspension::{self, AwaitError};

fn resume_parent_result(
    parent: &Invoke,
    resume: &nepl3_core::operation::Resume,
    _: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason> {
    budget.charge(Resource::Work, 1)?;
    let [
        OperationReply::Result(OperationResult::Complete {
            value: TypedValue::Record(dependency),
            ..
        }),
    ] = resume.dependency_results.as_slice()
    else {
        return Ok(OperationResult::Invalid {
            partial: None,
            report: Report::default(),
        });
    };
    let [NdfValue::U64(right)] = dependency.fields.as_slice() else {
        return Ok(OperationResult::Invalid {
            partial: None,
            report: Report::default(),
        });
    };
    let mut value = parent.input.clone_with_budget(budget)?;
    if let TypedValue::Record(record) = &mut value
        && let [NdfValue::U64(left)] = record.fields.as_mut_slice()
        && let Some(sum) = left.checked_add(*right)
    {
        *left = sum;
        return Ok(OperationResult::Complete {
            value,
            report: Report::default(),
        });
    }
    Ok(OperationResult::Invalid {
        partial: None,
        report: Report::default(),
    })
}

fn resume_parent(
    parent: &Invoke,
    resume: &nepl3_core::operation::Resume,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<OperationReply, StopReason> {
    resume_parent_result(parent, resume, registry, budget).map(OperationReply::Result)
}

fn saved(parent: &Invoke, context: Digest) -> Continuation {
    Continuation {
        provider: parent.operation.clone(),
        parent_request: parent.request_id,
        snapshot_digest: context,
        state: parent.input.clone(),
    }
}

fn invoke_parent(
    parent: &Invoke,
    context: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 17)?;
    let mut child = parent.clone();
    child.request_id = 2;
    if let TypedValue::Record(record) = &mut child.input {
        record.fields[0] = NdfValue::U64(10);
    }
    Ok(OperationReply::Await {
        continuation: saved(parent, context),
        calls: vec![child],
        report: Report::default(),
    })
}

fn stopped_await(
    parent: &Invoke,
    context: Digest,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    let reply = invoke_parent(parent, context, registry, b)?;
    b.stop(StopReason::WorkLimit);
    Ok(reply)
}

fn await_again(
    parent: &Invoke,
    resume: &nepl3_core::operation::Resume,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 7)?;
    let mut continuation = saved(parent, resume.continuation.snapshot_digest);
    // Distinct provider state identifies the next suspension in this fixture.
    if let TypedValue::Record(record) = &mut continuation.state {
        record.fields[0] = NdfValue::U64(99);
    }
    Ok(OperationReply::Await {
        continuation,
        calls: vec![],
        report: Report::default(),
    })
}

fn invalid_await_after_resume(
    parent: &Invoke,
    resume: &nepl3_core::operation::Resume,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    let mut reply = await_again(parent, resume, registry, b)?;
    if let OperationReply::Await { continuation, .. } = &mut reply {
        continuation.parent_request = 999;
    }
    Ok(reply)
}

fn stopped_resume(
    parent: &Invoke,
    resume: &nepl3_core::operation::Resume,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    let reply = await_again(parent, resume, registry, b)?;
    b.stop(StopReason::Cancelled);
    Ok(reply)
}

#[test]
fn resume_failure_preserves_or_consumes_generation_at_the_callback_boundary() -> Result<(), String>
{
    use nepl3_core::operation::Resume;
    use nepl3_suite::dispatch::resume as dispatch;
    let (registry, parent) = fixture()?;
    let context = Digest::of(b"context");
    let identity = Digest::of(b"provider");
    let sources = SourceStore::default();
    let continuation = saved(&parent, context);
    let request = Resume {
        request_id: parent.request_id,
        continuation: continuation.clone(),
        dependency_results: vec![],
    };
    let saved = dispatch::SavedAwait {
        parent: &parent,
        context,
        continuation: &continuation,
        calls: &[],
        sources: &[] as &[&SourceStore],
    };
    for callback in [
        invalid_await_after_resume as dispatch::ResumeOperation,
        stopped_resume,
    ] {
        let registration = dispatch::Registration {
            operation: &parent.operation,
            implementation: identity,
            resume: callback,
        };
        let mut lifetimes = RequestLifetimes::default();
        lifetimes
            .begin_call(&parent, context, None, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        lifetimes
            .suspend(parent.request_id, continuation.clone(), 0, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let mut execution = budget();
        let mut exhausted = Budget::new(Limits {
            work: 0,
            ..budget().limits()
        });
        assert!(matches!(
            dispatch::execute(
                &registration,
                identity,
                &saved,
                &request,
                &mut lifetimes,
                &registry,
                &sources,
                &mut execution,
                &mut exhausted
            ),
            Err(dispatch::ResumeError::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(
            lifetimes.phase(parent.request_id, &mut budget()),
            Ok(RequestPhase::Awaiting)
        );
        assert_eq!(execution.usage().work, 0);

        let result = dispatch::execute(
            &registration,
            identity,
            &saved,
            &request,
            &mut lifetimes,
            &registry,
            &sources,
            &mut execution,
            &mut budget(),
        );
        assert!(matches!(
            result,
            Err(dispatch::ResumeError::Await(AwaitError::Binding(_)))
                | Err(dispatch::ResumeError::Stopped(StopReason::Cancelled))
        ));
        // Execution began: even a rejected reply consumes the saved generation.
        assert_eq!(
            lifetimes.phase(parent.request_id, &mut budget()),
            Ok(RequestPhase::Running)
        );
        assert_eq!(execution.usage().work, 7);
        // A fresh execution budget cannot make the same Resume deliverable again.
        assert!(matches!(
            dispatch::execute(
                &registration,
                identity,
                &saved,
                &request,
                &mut lifetimes,
                &registry,
                &sources,
                &mut budget(),
                &mut budget()
            ),
            Err(dispatch::ResumeError::Lifetime(_))
        ));
        lifetimes
            .cancel(parent.request_id, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}

#[test]
fn resume_can_suspend_again_and_rejects_the_previous_generation() -> Result<(), String> {
    use nepl3_core::operation::Resume;
    use nepl3_suite::dispatch::resume as dispatch;
    let (registry, parent) = fixture()?;
    let context = Digest::of(b"context");
    let identity = Digest::of(b"provider");
    let sources = SourceStore::default();
    let first = saved(&parent, context);
    let request = Resume {
        request_id: parent.request_id,
        continuation: first.clone(),
        dependency_results: vec![],
    };
    let mut lifetimes = RequestLifetimes::default();
    lifetimes
        .begin_call(&parent, context, None, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    lifetimes
        .suspend(parent.request_id, first.clone(), 0, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut execution = budget();
    let registration = dispatch::Registration {
        operation: &parent.operation,
        implementation: identity,
        resume: await_again,
    };
    let saved_first = dispatch::SavedAwait {
        parent: &parent,
        context,
        continuation: &first,
        calls: &[],
        sources: &[] as &[&SourceStore],
    };
    let reply = dispatch::execute(
        &registration,
        identity,
        &saved_first,
        &request,
        &mut lifetimes,
        &registry,
        &sources,
        &mut execution,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let OperationReply::Await {
        continuation: second,
        ..
    } = reply
    else {
        return Err("expected second Await".into());
    };
    assert_eq!(execution.usage().work, 7);
    lifetimes
        .suspend(parent.request_id, second.clone(), 0, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let saved_second = dispatch::SavedAwait {
        continuation: &second,
        ..saved_first
    };
    assert!(matches!(
        dispatch::execute(
            &registration,
            identity,
            &saved_second,
            &request,
            &mut lifetimes,
            &registry,
            &sources,
            &mut execution,
            &mut budget()
        ),
        Err(dispatch::ResumeError::Binding(_))
    ));
    assert_eq!(execution.usage().work, 7);
    let second_request = Resume {
        continuation: second.clone(),
        ..request
    };
    let registration = dispatch::Registration {
        resume: resume_parent,
        ..registration
    };
    let reply = dispatch::execute(
        &registration,
        identity,
        &saved_second,
        &second_request,
        &mut lifetimes,
        &registry,
        &sources,
        &mut execution,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    // No dependency was supplied; this provider explicitly returns Invalid.
    assert!(matches!(
        reply,
        OperationReply::Result(OperationResult::Invalid { .. })
    ));
    assert_eq!(execution.usage().work, 8);
    lifetimes
        .finish(parent.request_id, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

#[test]
fn admitted_await_collects_dispatched_dependencies_and_resumes_saved_lifetime() -> Result<(), String>
{
    let (registry, parent) = fixture()?;
    let context = Digest::of(b"host admitted parent context");
    let identity = Digest::of(b"test provider");
    let mut execution = budget();
    let sources = SourceStore::default();
    let mut lifetimes = RequestLifetimes::default();
    lifetimes
        .begin_call(&parent, context, None, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    use nepl3_suite::dispatch::suspending;
    let registration = suspending::Registration {
        operation: &parent.operation,
        implementation: identity,
        invoke: invoke_parent,
    };
    let root_scope = suspension::execution::ExecutionScope::root(&mut execution, parent.limits)
        .map_err(|e| format!("{e:?}"))?;
    let reply = root_scope
        .run(&mut execution, |execution| {
            suspending::invoke(
                &registration,
                identity,
                &parent,
                context,
                &registry,
                &sources,
                execution,
                &mut budget(),
            )
        })
        .map_err(|e| format!("{e:?}"))?;
    use nepl3_suite::{
        grants::{
            Grants,
            dependencies::{OperationGrant, authorize},
        },
        suspension::host::activate_owned,
    };
    let grant = Grants::new(&parent.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let policy = [OperationGrant {
        operation: &parent.operation,
        grants: &grant,
    }];
    let active = activate_owned(
        &parent,
        context,
        reply,
        &policy,
        &[context],
        &registry,
        &sources,
        &mut lifetimes,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(active.report, Report::default());
    let mut pending = active.pending;
    // Authorization is reborrowed from immutable owned calls at dispatch time.
    let approved =
        authorize(pending.calls(), &policy, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let call = approved[0].invocation().request();
    let child_scope = root_scope
        .child(call.limits, &mut execution)
        .map_err(|e| format!("{e:?}"))?;
    // Preserve the actual parent callback's work during dependency execution.
    let parent_usage = execution.usage();
    let registrations = [Registration {
        operation: &call.operation,
        implementation: identity,
        invoke: increment,
    }];
    let result = child_scope
        .run(&mut execution, |execution| {
            invoke_terminal(
                &registrations,
                &call.operation,
                identity,
                call,
                &registry,
                &sources,
                execution,
                &mut budget(),
            )
        })
        .map_err(|e| format!("{e:?}"))?;
    assert!(execution.usage().work > parent_usage.work);
    let child_id = call.request_id;
    pending
        .accept_active(
            child_id,
            context,
            result,
            &registry,
            &sources,
            &mut lifetimes,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    let resume = pending
        .take_resume(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    use nepl3_suite::dispatch::resume as dispatch;
    let registration = dispatch::Registration {
        operation: &parent.operation,
        implementation: identity,
        resume: resume_parent,
    };
    let grants = [&sources];
    let saved = dispatch::SavedAwait {
        parent: &parent,
        context,
        continuation: pending.continuation(),
        calls: pending.calls(),
        sources: &grants,
    };
    let run = |request: &nepl3_core::operation::Resume,
               lifetimes: &mut RequestLifetimes,
               execution: &mut Budget| {
        root_scope.run(execution, |execution| {
            dispatch::execute(
                &registration,
                identity,
                &saved,
                request,
                lifetimes,
                &registry,
                &sources,
                execution,
                &mut budget(),
            )
        })
    };
    let mut wrong = resume.clone();
    wrong.continuation.parent_request = 99;
    assert!(matches!(
        run(&wrong, &mut lifetimes, &mut execution),
        Err(dispatch::ResumeError::Binding(_))
    ));
    let mut wrong = resume.clone();
    if let OperationReply::Result(OperationResult::Complete {
        value: TypedValue::Record(record),
        ..
    }) = &mut wrong.dependency_results[0]
    {
        record.fields.clear();
    }
    assert!(matches!(
        run(&wrong, &mut lifetimes, &mut execution),
        Err(dispatch::ResumeError::Dispatch(DispatchError::Output(_)))
    ));
    assert_eq!(
        lifetimes.phase(parent.request_id, &mut budget()),
        Ok(RequestPhase::Awaiting)
    );
    let completed = run(&resume, &mut lifetimes, &mut execution).map_err(|e| format!("{e:?}"))?;
    let OperationReply::Result(OperationResult::Complete {
        value: TypedValue::Record(record),
        ..
    }) = completed
    else {
        return Err("expected resumed result".into());
    };
    assert_eq!(record.fields, vec![NdfValue::U64(52)]);
    assert!(matches!(
        run(&resume, &mut lifetimes, &mut execution),
        Err(dispatch::ResumeError::Lifetime(_))
    ));
    assert_eq!(
        lifetimes.phase(parent.request_id, &mut budget()),
        Ok(RequestPhase::Running)
    );
    let [
        OperationReply::Result(OperationResult::Complete {
            value: TypedValue::Record(record),
            ..
        }),
    ] = resume.dependency_results.as_slice()
    else {
        return Err("expected one terminal dependency".into());
    };
    assert_eq!(record.fields, vec![NdfValue::U64(11)]);
    Ok(())
}

#[test]
fn await_admission_rejects_changed_binding_bad_calls_and_stops() -> Result<(), String> {
    let (registry, parent) = fixture()?;
    let context = Digest::of(b"context");
    let mut continuation = saved(&parent, context);
    let mut child = parent.clone();
    child.request_id = 2;
    let sources = SourceStore::default();
    let report = Report::default();
    continuation.parent_request = 99;
    assert!(matches!(
        suspension::prepare(
            &parent,
            context,
            &continuation,
            &[child.clone()],
            &report,
            &registry,
            &sources,
            &mut budget()
        ),
        Err(AwaitError::Binding(_))
    ));
    continuation.parent_request = parent.request_id;
    assert!(matches!(
        suspension::prepare(
            &parent,
            context,
            &continuation,
            &[child.clone(), child.clone()],
            &report,
            &registry,
            &sources,
            &mut budget()
        ),
        Err(AwaitError::Dependencies(_))
    ));
    if let TypedValue::Record(record) = &mut child.input {
        record.fields.clear();
    }
    assert!(matches!(
        suspension::prepare(
            &parent,
            context,
            &continuation,
            &[child],
            &report,
            &registry,
            &sources,
            &mut budget()
        ),
        Err(AwaitError::Call(_))
    ));
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        suspension::prepare(
            &parent,
            context,
            &continuation,
            &[],
            &report,
            &registry,
            &sources,
            &mut stopped
        ),
        Err(AwaitError::Stopped(StopReason::WorkLimit))
    ));
    Ok(())
}

#[test]
fn suspending_dispatch_rejects_an_await_after_execution_stopped() -> Result<(), String> {
    use nepl3_suite::dispatch::suspending;
    let (registry, parent) = fixture()?;
    let identity = Digest::of(b"provider");
    let registration = suspending::Registration {
        operation: &parent.operation,
        implementation: identity,
        invoke: stopped_await,
    };
    let mut execution = budget();
    let result = suspending::invoke(
        &registration,
        identity,
        &parent,
        Digest::of(b"context"),
        &registry,
        &SourceStore::default(),
        &mut execution,
        &mut budget(),
    );
    assert!(matches!(
        result,
        Err(suspending::Error::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(execution.poll(), Err(StopReason::WorkLimit));
    Ok(())
}
