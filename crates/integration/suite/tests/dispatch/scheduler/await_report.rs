use super::*;
use nepl3_core::diagnostic::{Diagnostic, Severity};
use std::cell::Cell;

#[test]
fn checked_await_report_survives_prepublication_stops() -> Result<(), String> {
    let (registry, root) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&root.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let identity = Digest::of(b"Await report retention");
    let invoked = Cell::new(0);
    let contexts = Cell::new(0);
    let delivered = Cell::new(0);
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
    let invoke = |call: &Invoke, context, _: &SchemaRegistry, b: &mut Budget| {
        invoked.set(invoked.get() + 1);
        if call.request_id == root.request_id {
            let mut reply = suspend_with(call, context, 99, &[10], b)?;
            if let OperationReply::Await { report, .. } = &mut reply {
                *report = expected.clone();
                if invalid.get() {
                    report.usage.diagnostics = 0;
                }
            }
            Ok(reply)
        } else {
            Ok(OperationReply::Result(OperationResult::Complete {
                value: call.input.clone_with_budget(b)?,
                report: Report::default(),
            }))
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
            resume: &resume,
        },
        grants: &grants,
        context: &context,
    }];
    let run = |validation: &mut Budget, cancelled: &mut Vec<u64>| {
        invoked.set(0);
        contexts.set(0);
        delivered.set(0);
        scheduler::run(
            &registrations,
            &root,
            &registry,
            &mut budget(),
            validation,
            |id, report| {
                assert_eq!(id, root.request_id);
                assert_eq!(report, expected);
                delivered.set(delivered.get() + 1);
            },
            |id| cancelled.push(id),
        )
    };
    let mut measured = budget();
    run(&mut measured, &mut vec![]).map_err(|e| format!("{e:?}"))?;
    assert_eq!(invoked.get(), 2);
    assert_eq!(delivered.get(), 1);
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
                retained_stops[axis] += 1;
                assert_eq!(retained.len(), 1);
                let saved = &retained[0];
                assert_eq!(saved.request_id, root.request_id);
                assert_eq!(saved.operation, &root.operation);
                assert_eq!(saved.context, identity);
                assert_eq!(saved.report, &expected);
                assert_eq!(saved.sources.snapshots().len(), 0);
                assert_eq!(invoked.get(), 1);
                assert_eq!(delivered.get(), 0);
                assert_eq!(cancelled, [root.request_id]);
            }
            // Child context construction follows full reply preparation. Any
            // subsequent failure before publication must retain that report.
            if contexts.get() > 0 && delivered.get() == 0 {
                assert_eq!(retained.len(), 1, "axis {axis}, limit {limit}");
            }
            if delivered.get() > 0 {
                assert!(retained.is_empty());
            }
            assert!(delivered.get() <= 1);
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
    assert_eq!(delivered.get(), 0);
    Ok(())
}
