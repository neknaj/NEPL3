use super::*;
use nepl3_core::operation::{Continuation, OperationReply, Resume};
use nepl3_suite::{
    dispatch::{resume, suspending},
    grants::Grants,
    scheduler,
};

fn number(value: &TypedValue) -> Option<u64> {
    if let TypedValue::Record(record) = value
        && let [NdfValue::U64(n)] = record.fields.as_slice()
    {
        return Some(*n);
    }
    None
}
fn invoke(
    call: &Invoke,
    context: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    let Some(n) = number(&call.input) else {
        return Ok(OperationReply::Result(OperationResult::Invalid {
            partial: None,
            report: Report::default(),
        }));
    };
    if n == 0 {
        return Ok(OperationReply::Result(OperationResult::Complete {
            value: call.input.clone_with_budget(b)?,
            report: Report::default(),
        }));
    }
    let mut child = call.clone();
    child.request_id += 1;
    if let TypedValue::Record(record) = &mut child.input {
        record.fields[0] = NdfValue::U64(n - 1);
    }
    Ok(OperationReply::Await {
        continuation: Continuation {
            provider: call.operation.clone(),
            parent_request: call.request_id,
            snapshot_digest: context,
            state: call.input.clone_with_budget(b)?,
        },
        calls: vec![child],
        report: Report::default(),
    })
}
fn resume(
    _: &Invoke,
    request: &Resume,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    let [OperationReply::Result(OperationResult::Complete { value, .. })] =
        request.dependency_results.as_slice()
    else {
        return Ok(OperationReply::Result(OperationResult::Invalid {
            partial: None,
            report: Report::default(),
        }));
    };
    let mut value = value.clone_with_budget(b)?;
    if let TypedValue::Record(record) = &mut value
        && let [NdfValue::U64(n)] = record.fields.as_mut_slice()
    {
        *n += 1;
    }
    Ok(OperationReply::Result(OperationResult::Complete {
        value,
        report: Report::default(),
    }))
}
fn context(_: &Invoke, implementation: Digest, b: &mut Budget) -> Result<Digest, StopReason> {
    b.charge(Resource::Work, 1)?;
    // Fixture has one immutable empty grant set and one implementation.
    Ok(implementation)
}

#[test]
fn iterative_scheduler_resolves_nested_calls_and_cancels_on_execution_stop() -> Result<(), String> {
    let (registry, mut root) = fixture()?;
    if let TypedValue::Record(record) = &mut root.input {
        record.fields[0] = NdfValue::U64(4);
    }
    let sources = SourceStore::default();
    let grants = Grants::new(&root.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let identity = Digest::of(b"recursive native fixture");
    let registrations = [scheduler::Registration {
        invoke: suspending::Registration {
            operation: &root.operation,
            implementation: identity,
            invoke,
        },
        resume: resume::Registration {
            operation: &root.operation,
            implementation: identity,
            resume,
        },
        grants: &grants,
        context,
    }];
    let mut reports = Vec::new();
    let mut cancelled = Vec::new();
    let mut execution = budget();
    let result = scheduler::run(
        &registrations,
        &root,
        &registry,
        &mut execution,
        &mut budget(),
        |id, report| reports.push((id, report)),
        |id| cancelled.push(id),
    )
    .map_err(|e| format!("{e:?}"))?;
    let OperationResult::Complete { value, .. } = result else {
        return Err("expected Complete".into());
    };
    assert_eq!(number(&value), Some(4));
    assert_eq!(
        reports.iter().map(|v| v.0).collect::<Vec<_>>(),
        (root.request_id..root.request_id + 4).collect::<Vec<_>>()
    );
    assert!(cancelled.is_empty());
    // Five operation frames plus the record-value traversal in the leaf clone.
    assert_eq!(execution.usage().depth, 6);
    let measured = execution.usage().work;
    for work in [0, 1, measured - 1] {
        let mut execution = Budget::new(Limits {
            work,
            ..budget().limits()
        });
        cancelled.clear();
        assert!(
            scheduler::run(
                &registrations,
                &root,
                &registry,
                &mut execution,
                &mut budget(),
                |_, _| {},
                |id| cancelled.push(id)
            )
            .is_err()
        );
        assert_eq!(execution.poll(), Err(StopReason::WorkLimit));
        assert!(!cancelled.is_empty());
        let count = cancelled.len();
        cancelled.sort_unstable();
        cancelled.dedup();
        assert_eq!(cancelled.len(), count);
    }
    Ok(())
}
