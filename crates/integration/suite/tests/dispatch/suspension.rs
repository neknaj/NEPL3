use super::*;
use nepl3_core::operation::{
    Continuation, OperationReply,
    lifetime::{RequestLifetimes, RequestPhase},
};
use nepl3_suite::suspension::{self, AwaitError};

fn saved(parent: &Invoke, context: Digest) -> Continuation {
    Continuation {
        provider: parent.operation.clone(),
        parent_request: parent.request_id,
        snapshot_digest: context,
        state: parent.input.clone(),
    }
}

#[test]
fn admitted_await_collects_dispatched_dependencies_and_resumes_saved_lifetime() -> Result<(), String>
{
    let (registry, parent) = fixture()?;
    let context = Digest::of(b"host admitted parent context");
    let continuation = saved(&parent, context);
    let mut child = parent.clone();
    child.request_id = 2;
    // Different input makes the dependency distinct from its ancestor.
    if let TypedValue::Record(record) = &mut child.input {
        record.fields[0] = NdfValue::U64(10);
    }
    let calls = [child];
    let sources = SourceStore::default();
    let mut lifetimes = RequestLifetimes::default();
    lifetimes
        .begin_call(&parent, context, None, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut pending = suspension::prepare(
        &parent,
        context,
        &continuation,
        &calls,
        &Report::default(),
        &registry,
        &sources,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    lifetimes
        .suspend(
            parent.request_id,
            continuation.clone(),
            calls.len(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    lifetimes
        .begin_call(&calls[0], context, Some(parent.request_id), &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let identity = Digest::of(b"test provider");
    let mut execution = budget();
    // Preserve work already consumed by the suspended parent.
    execution
        .charge(Resource::Work, 17)
        .map_err(|e| format!("{e:?}"))?;
    let parent_usage = execution.usage();
    let registrations = [Registration {
        operation: &calls[0].operation,
        implementation: identity,
        invoke: increment,
    }];
    let result = invoke_terminal(
        &registrations,
        &calls[0].operation,
        identity,
        &calls[0],
        &registry,
        &sources,
        &mut execution,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(execution.usage().work > parent_usage.work);
    pending
        .accept(
            calls[0].request_id,
            result,
            &registry,
            &sources,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    lifetimes
        .finish(calls[0].request_id, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let resume = pending
        .take_resume(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    lifetimes
        .resume(&resume, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
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
