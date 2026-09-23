use super::*;
use nepl3_core::diagnostic::{Diagnostic, Severity};
use std::cell::Cell;

#[test]
fn checked_await_report_survives_prepublication_stops() -> Result<(), String> {
    retention(false)
}

#[test]
fn resumed_await_report_survives_prepublication_stops() -> Result<(), String> {
    retention(true)
}

fn retention(after_resume: bool) -> Result<(), String> {
    let (registry, root) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&root.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let identity = Digest::of(b"Await report retention");
    let invoked = Cell::new(0);
    let contexts = Cell::new(0);
    let delivered = Cell::new(0);
    let resumed = Cell::new(0);
    let invalid = Cell::new(false);
    let mut expected = Report::default();
    expected.usage.diagnostics = 1;
    expected.diagnostics.push(Diagnostic {
        schema: root.operation.schema.clone(),
        code: "checked-await".into(),
        severity: Severity::Information,
        stage: "invoke".into(),
        arguments: root.input.clone(),
        primary: None,
        related: vec![],
        fixes: vec![],
    });
    let checked_reply = |call: &Invoke, context, b: &mut Budget| {
        let mut reply = suspend_with(call, context, 99, &[10], b)?;
        if let OperationReply::Await { report, .. } = &mut reply {
            *report = expected.clone();
            if invalid.get() {
                report.usage.diagnostics = 0;
            }
        }
        Ok(reply)
    };
    let invoke = |call: &Invoke, context, _: &SchemaRegistry, b: &mut Budget| {
        invoked.set(invoked.get() + 1);
        if call.request_id == root.request_id {
            if after_resume {
                suspend_with(call, context, 77, &[], b)
            } else {
                checked_reply(call, context, b)
            }
        } else {
            Ok(OperationReply::Result(OperationResult::Complete {
                value: call.input.clone_with_budget(b)?,
                report: Report::default(),
            }))
        }
    };
    let resume_callback =
        |call: &Invoke, request: &Resume, registry: &SchemaRegistry, b: &mut Budget| {
            resumed.set(resumed.get() + 1);
            if after_resume && number(&request.continuation.state) == Some(77) {
                assert!(request.dependency_results.is_empty());
                checked_reply(call, request.continuation.snapshot_digest, b)
            } else {
                resume(call, request, registry, b)
            }
        };
    let context = |call: &Invoke, implementation, b: &mut Budget| {
        if call.request_id != root.request_id {
            contexts.set(contexts.get() + 1);
        }
        super::context(call, implementation, b)
    };
    let registrations = [scheduler::Registration {
        invoke: suspending::Registration {
            operation: &root.operation,
            implementation: identity,
            invoke: &invoke,
        },
        resume: resume::Registration {
            operation: &root.operation,
            implementation: identity,
            resume: &resume_callback,
        },
        grants: &grants,
        context: &context,
    }];
    let run = |validation: &mut Budget, cancelled: &mut Vec<u64>| {
        invoked.set(0);
        contexts.set(0);
        delivered.set(0);
        resumed.set(0);
        scheduler::run(
            &registrations,
            &root,
            &registry,
            &mut budget(),
            validation,
            |id, report| {
                assert_eq!(id, root.request_id);
                if after_resume && delivered.get() == 0 {
                    assert_eq!(report, Report::default());
                } else {
                    assert_eq!(report, expected);
                }
                delivered.set(delivered.get() + 1);
            },
            |id| cancelled.push(id),
        )
    };
    let mut measured = budget();
    run(&mut measured, &mut vec![]).map_err(|e| format!("{e:?}"))?;
    assert_eq!(invoked.get(), 2);
    let previous_reports = usize::from(after_resume);
    assert_eq!(delivered.get(), previous_reports + 1);
    assert_eq!(resumed.get(), previous_reports + 1);
    let mut retained_stops = [0, 0];
    for (axis, maximum) in [measured.usage().work, measured.usage().allocation_units]
        .into_iter()
        .enumerate()
    {
        for limit in 0..maximum {
            let mut limits = budget().limits();
            if axis == 0 {
                limits.work = limit;
            } else {
                limits.allocation_units = limit;
            }
            let mut cancelled = vec![];
            let Err(failure) = run(&mut Budget::new(limits), &mut cancelled) else {
                return Err("insufficient validation budget must stop".into());
            };
            let retained = failure.uncommitted_await_reports().collect::<Vec<_>>();
            if !retained.is_empty() {
                let initial_empty = after_resume && resumed.get() == 0;
                if !initial_empty {
                    retained_stops[axis] += 1;
                }
                assert_eq!(retained.len(), 1);
                let saved = &retained[0];
                assert_eq!(saved.request_id, root.request_id);
                assert_eq!(saved.operation, &root.operation);
                assert_eq!(saved.context, identity);
                if initial_empty {
                    assert_eq!(saved.report, &Report::default());
                } else {
                    assert_eq!(saved.report, &expected);
                }
                assert_eq!(saved.sources.snapshots().len(), 0);
                assert_eq!(invoked.get(), 1);
                let prior = if initial_empty { 0 } else { previous_reports };
                assert_eq!(delivered.get(), prior);
                assert_eq!(resumed.get(), prior);
                assert_eq!(failure.accepted_results().count(), 0);
                assert_eq!(cancelled, [root.request_id]);
            }
            // Child context construction follows full reply preparation. Any
            // subsequent failure before publication must retain that report.
            if contexts.get() > 0 && delivered.get() == previous_reports {
                assert_eq!(retained.len(), 1, "axis {axis}, limit {limit}");
            }
            if delivered.get() > previous_reports {
                assert!(retained.is_empty());
            }
            assert!(delivered.get() <= previous_reports + 1);
            let mut unique = cancelled.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), cancelled.len());
        }
    }
    assert!(retained_stops.iter().all(|count| *count > 0));
    invalid.set(true);
    let Err(failure) = run(&mut budget(), &mut vec![]) else {
        return Err("invalid Report accepted".into());
    };
    assert_eq!(failure.uncommitted_await_reports().count(), 0);
    assert_eq!(contexts.get(), 0);
    assert_eq!(invoked.get(), 1);
    assert_eq!(delivered.get(), previous_reports);
    assert_eq!(resumed.get(), previous_reports);
    Ok(())
}
