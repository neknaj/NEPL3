use super::*;
use nepl3_core::diagnostic::{Diagnostic, Severity};
use std::cell::Cell;

#[test]
fn resume_preparation_retains_results_at_every_work_and_allocation_stop() -> Result<(), String> {
    let (registry, root) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&root.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let identity = Digest::of(b"resume retention");
    let resumed = Cell::new(0);
    let invoked = Cell::new(0);
    let fail_callback = Cell::new(false);
    let invoke = |call: &Invoke, context, _: &SchemaRegistry, b: &mut Budget| {
        invoked.set(invoked.get() + 1);
        if call.request_id == root.request_id {
            return suspend_with(call, context, 99, &[10, 11], b);
        }
        let mut report = Report::default();
        report.usage.diagnostics = 1;
        report.diagnostics.push(Diagnostic {
            schema: call.operation.schema.clone(),
            code: "retained-child".into(),
            severity: Severity::Information,
            stage: "invoke".into(),
            arguments: call.input.clone_with_budget(b)?,
            primary: None,
            related: vec![],
            fixes: vec![],
        });
        Ok(OperationReply::Result(OperationResult::Invalid {
            partial: Some(call.input.clone_with_budget(b)?),
            report,
        }))
    };
    let resume = |call: &Invoke, _: &Resume, _: &SchemaRegistry, b: &mut Budget| {
        resumed.set(resumed.get() + 1);
        if fail_callback.get() {
            return Err(b.stop(StopReason::Cancelled));
        }
        Ok(OperationReply::Result(OperationResult::Complete {
            value: call.input.clone_with_budget(b)?,
            report: Report::default(),
        }))
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
        resumed.set(0);
        scheduler::run(
            &registrations,
            &root,
            &registry,
            &mut budget(),
            validation,
            |_, _| {},
            |id| cancelled.push(id),
        )
    };
    let mut measured = budget();
    run(&mut measured, &mut vec![]).map_err(|e| format!("{e:?}"))?;
    assert_eq!(invoked.get(), 3);
    assert_eq!(resumed.get(), 1);
    let usage = measured.usage();
    let mut preparation_stops = [0, 0];
    for (axis, maximum) in [usage.work, usage.allocation_units].into_iter().enumerate() {
        for limit in 0..maximum {
            let mut limits = budget().limits();
            if axis == 0 {
                limits.work = limit;
            } else {
                limits.allocation_units = limit;
            }
            let mut cancelled = vec![];
            let result = run(&mut Budget::new(limits), &mut cancelled);
            let failure = result.expect_err("insufficient validation budget");
            assert!(resumed.get() <= 1);
            // Finished children receive no cancellation. If both are finished
            // and Resume has not begun, their complete outcomes remain owned
            // by the parent across extraction, allocation and revalidation.
            if invoked.get() == 3 && resumed.get() == 0 && cancelled == [root.request_id] {
                preparation_stops[axis] += 1;
                assert_eq!(failure.active_request_id(), root.request_id);
                let accepted = failure.accepted_results().collect::<Vec<_>>();
                assert_eq!(accepted.len(), 2, "axis {axis}, limit {limit}, {failure:?}");
                for ((call, result), (id, expected)) in accepted
                    .into_iter()
                    .zip([(root.request_id + 10, 10), (root.request_id + 9, 11)])
                {
                    assert_eq!(call.request_id, id);
                    assert_eq!(call.operation, root.operation);
                    let OperationResult::Invalid {
                        partial: Some(value),
                        report,
                    } = result
                    else {
                        return Err("missing retained partial".into());
                    };
                    assert_eq!(number(value), Some(expected));
                    assert_eq!(report.diagnostics.len(), 1);
                    assert_eq!(report.diagnostics[0].code, "retained-child");
                    assert_eq!(report.diagnostics[0].schema, call.operation.schema);
                    assert_eq!(number(&report.diagnostics[0].arguments), Some(expected));
                }
            }
            if resumed.get() == 1 {
                assert_eq!(failure.accepted_results().count(), 0);
            }
            let mut unique = cancelled.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), cancelled.len());
        }
    }
    assert!(preparation_stops.iter().all(|count| *count > 0));
    fail_callback.set(true);
    let mut cancelled = vec![];
    let failure = run(&mut budget(), &mut cancelled).expect_err("callback stop");
    assert_eq!(resumed.get(), 1);
    assert_eq!(failure.accepted_results().count(), 0);
    assert_eq!(cancelled, [root.request_id]);
    Ok(())
}
