use super::*;
#[path = "scheduler/admission.rs"]
mod admission;
#[path = "scheduler/outcomes.rs"]
mod outcomes;
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

fn stop_at_leaf(
    call: &Invoke,
    context: Digest,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    if number(&call.input) == Some(0) {
        return Err(b.stop(StopReason::Cancelled));
    }
    invoke(call, context, registry, b)
}

fn suspend_with(
    call: &Invoke,
    context: Digest,
    state: u64,
    children: &[u64],
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    let mut saved = call.input.clone_with_budget(b)?;
    if let TypedValue::Record(record) = &mut saved {
        record.fields[0] = NdfValue::U64(state);
    }
    let calls = children
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let mut child = call.clone();
            // Reverse ID order makes call order observable independently of ID sorting.
            child.request_id = call.request_id + 10 - index as u64;
            if let TypedValue::Record(record) = &mut child.input {
                record.fields[0] = NdfValue::U64(*value);
            }
            child
        })
        .collect();
    Ok(OperationReply::Await {
        continuation: Continuation {
            provider: call.operation.clone(),
            parent_request: call.request_id,
            snapshot_digest: context,
            state: saved,
        },
        calls,
        report: Report::default(),
    })
}

fn branching(
    call: &Invoke,
    context: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    match number(&call.input) {
        Some(99) => suspend_with(call, context, 99, &[10, 20], b),
        Some(77) => suspend_with(call, context, 77, &[], b),
        _ => Ok(OperationReply::Result(OperationResult::Complete {
            value: call.input.clone_with_budget(b)?,
            report: Report::default(),
        })),
    }
}

fn branching_resume(
    call: &Invoke,
    request: &Resume,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    if number(&request.continuation.state) == Some(77) {
        assert!(request.dependency_results.is_empty());
        return suspend_with(call, request.continuation.snapshot_digest, 78, &[33], b);
    }
    assert_eq!(
        number(&request.continuation.state),
        if number(&call.input) == Some(77) {
            Some(78)
        } else {
            Some(99)
        }
    );
    let mut combined = 0;
    for reply in &request.dependency_results {
        let OperationReply::Result(OperationResult::Complete { value, .. }) = reply else {
            return Ok(OperationReply::Result(OperationResult::Invalid {
                partial: None,
                report: Report::default(),
            }));
        };
        let Some(number) = number(value) else {
            return Ok(OperationReply::Result(OperationResult::Invalid {
                partial: None,
                report: Report::default(),
            }));
        };
        combined = combined * 100 + number;
    }
    let mut value = call.input.clone_with_budget(b)?;
    if let TypedValue::Record(record) = &mut value {
        record.fields[0] = NdfValue::U64(combined);
    }
    Ok(OperationReply::Result(OperationResult::Complete {
        value,
        report: Report::default(),
    }))
}

#[test]
fn scheduler_preserves_sibling_order_and_resumes_empty_and_repeated_await() -> Result<(), String> {
    for (input, expected, generations) in [(99, 1020, 1), (77, 33, 2)] {
        let (registry, mut root) = fixture()?;
        if let TypedValue::Record(record) = &mut root.input {
            record.fields[0] = NdfValue::U64(input);
        }
        let sources = SourceStore::default();
        let grants = Grants::new(&root.environment, &sources, &[], &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let identity = Digest::of(b"branch fixture");
        let registrations = [scheduler::Registration {
            invoke: suspending::Registration {
                operation: &root.operation,
                implementation: identity,
                invoke: &branching,
            },
            resume: resume::Registration {
                operation: &root.operation,
                implementation: identity,
                resume: &branching_resume,
            },
            grants: &grants,
            context: &context,
        }];
        let mut reports = Vec::new();
        let mut cancelled = Vec::new();
        let mut validation = budget();
        let result = scheduler::run(
            &registrations,
            &root,
            &registry,
            &mut budget(),
            &mut validation,
            |id, report| reports.push((id, report)),
            |id| cancelled.push(id),
        )
        .map_err(|e| format!("{e:?}"))?;
        let OperationResult::Complete { value, .. } = result else {
            return Err("expected Complete".into());
        };
        assert_eq!(number(&value), Some(expected));
        assert_eq!(reports.len(), generations);
        assert!(reports.iter().all(|(id, _)| *id == root.request_id));
        assert!(cancelled.is_empty());
        // The final charged validation operation finishes the parent. Rejecting
        // its last Work unit leaves only that parent eligible for cancellation;
        // its children have already committed their terminal results.
        let work = validation.usage().work;
        let mut limited = Budget::new(Limits {
            work: work - 1,
            ..budget().limits()
        });
        cancelled.clear();
        assert!(
            scheduler::run(
                &registrations,
                &root,
                &registry,
                &mut budget(),
                &mut limited,
                |_, _| {},
                |id| cancelled.push(id)
            )
            .is_err()
        );
        assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
        assert_eq!(cancelled, vec![root.request_id]);
        for allocation in [false, true] {
            let count = if allocation {
                validation.usage().allocation_units
            } else {
                work
            };
            for limit in 0..count {
                let mut limits = budget().limits();
                let reason = if allocation {
                    limits.allocation_units = limit;
                    StopReason::AllocationLimit
                } else {
                    limits.work = limit;
                    StopReason::WorkLimit
                };
                let mut limited = Budget::new(limits);
                cancelled.clear();
                assert!(
                    scheduler::run(
                        &registrations,
                        &root,
                        &registry,
                        &mut budget(),
                        &mut limited,
                        |_, _| {},
                        |id| cancelled.push(id)
                    )
                    .is_err(),
                    "limit {limit}"
                );
                assert_eq!(limited.poll(), Err(reason));
                assert!(cancelled.iter().all(|id| {
                    [root.request_id, root.request_id + 9, root.request_id + 10].contains(id)
                }));
                let notified = cancelled.len();
                cancelled.sort_unstable();
                cancelled.dedup();
                assert_eq!(cancelled.len(), notified);
            }
        }
    }
    Ok(())
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
    let configured_context = Digest::of(b"immutable run configuration");
    let context_calls = core::cell::Cell::new(0_usize);
    let borrowed_context = |_: &Invoke, _: Digest, b: &mut Budget| {
        b.charge(Resource::Work, 1)?;
        context_calls.set(context_calls.get() + 1);
        Ok(configured_context)
    };
    let mut registrations = [scheduler::Registration {
        invoke: suspending::Registration {
            operation: &root.operation,
            implementation: identity,
            invoke: &invoke,
        },
        resume: resume::Registration {
            operation: &root.operation,
            implementation: identity,
            resume: &resume,
        },
        grants: &grants,
        context: &borrowed_context,
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
    assert_eq!(context_calls.get(), 5);
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
    registrations[0].invoke.invoke = &stop_at_leaf;
    cancelled.clear();
    let failure = scheduler::run(
        &registrations,
        &root,
        &registry,
        &mut budget(),
        &mut budget(),
        |_, _| {},
        |id| cancelled.push(id),
    )
    .err()
    .ok_or("expected leaf stop")?;
    assert_eq!(failure.active_request_id(), root.request_id + 4);
    assert!(failure.accepted_results().next().is_none());
    cancelled.sort_unstable();
    assert_eq!(
        cancelled,
        (root.request_id..=root.request_id + 4).collect::<Vec<_>>()
    );
    Ok(())
}
