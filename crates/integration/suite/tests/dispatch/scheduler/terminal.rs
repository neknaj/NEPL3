use super::*;
use nepl3_core::diagnostic::{Diagnostic, Severity};
use std::cell::Cell;

#[test]
fn checked_terminal_results_survive_root_and_child_acceptance_stops() -> Result<(), String> {
    // Both root delivery and child acceptance must retain all terminal kinds.
    for (child, mode) in [
        (false, 0),
        (false, 1),
        (false, 2),
        (true, 0),
        (true, 1),
        (true, 2),
    ] {
        let (registry, root) = fixture()?;
        let sources = SourceStore::default();
        let grants = Grants::new(&root.environment, &sources, &[], &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let identity = Digest::of(b"terminal retention");
        let resumed = Cell::new(0);
        let corrupt = Cell::new(false);
        let invoke = |call: &Invoke, context, _: &SchemaRegistry, b: &mut Budget| {
            if child && call.request_id == root.request_id {
                return suspend_with(call, context, 99, &[10], b);
            }
            let mut value = call.input.clone_with_budget(b)?;
            if corrupt.get()
                && let TypedValue::Record(record) = &mut value
            {
                record.fields.clear();
            }
            let mut report = Report::default();
            report.usage.diagnostics = 1;
            report.diagnostics.push(Diagnostic {
                schema: call.operation.schema.clone(),
                code: "terminal-retained".into(),
                severity: Severity::Information,
                stage: "invoke".into(),
                arguments: call.input.clone_with_budget(b)?,
                primary: None,
                related: vec![],
                fixes: vec![],
            });
            Ok(OperationReply::Result(match mode {
                1 => OperationResult::Invalid {
                    partial: Some(value),
                    report,
                },
                2 => OperationResult::Stopped {
                    reason: StopReason::Cancelled,
                    partial: Some(value),
                    report,
                },
                _ => OperationResult::Complete { value, report },
            }))
        };
        let resume = |call: &Invoke, _: &Resume, _: &SchemaRegistry, b: &mut Budget| {
            resumed.set(resumed.get() + 1);
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
        let mut full = budget();
        let mut cancelled = vec![];
        let result = run(&mut full, &mut cancelled);
        assert_eq!(result.is_ok(), !child || mode != 2);
        assert_eq!(resumed.get(), usize::from(child && mode != 2));
        if !child {
            let result = result.as_ref().map_err(|e| format!("{e:?}"))?;
            let (value, report) = match (mode, result) {
                (0, OperationResult::Complete { value, report }) => (value, report),
                (
                    1,
                    OperationResult::Invalid {
                        partial: Some(value),
                        report,
                    },
                ) => (value, report),
                (
                    2,
                    OperationResult::Stopped {
                        reason: StopReason::Cancelled,
                        partial: Some(value),
                        report,
                    },
                ) => (value, report),
                _ => return Err("root terminal kind or partial changed".into()),
            };
            assert_eq!(value, &root.input);
            assert_eq!(report.diagnostics.len(), 1);
            assert_eq!(report.diagnostics[0].code, "terminal-retained");
            assert!(cancelled.is_empty());
        } else if mode == 2 {
            let failure = result
                .as_ref()
                .err()
                .ok_or("stopped child must prevent Resume")?;
            assert_eq!(failure.accepted_results().count(), 1);
            assert_eq!(cancelled, [root.request_id]);
        }
        let mut retained = 0;
        for work in 0..full.usage().work {
            let mut cancelled = vec![];
            let Err(failure) = run(
                &mut Budget::new(Limits {
                    work,
                    ..budget().limits()
                }),
                &mut cancelled,
            ) else {
                return Err("insufficient validation Work must stop".into());
            };
            let outcomes: Vec<_> = failure.uncommitted_results().collect();
            assert!(outcomes.len() <= 1);
            for outcome in outcomes {
                assert_eq!(failure.active_request_id(), outcome.request_id);
                assert_eq!(outcome.operation, &root.operation);
                assert_eq!(outcome.context, identity);
                assert_eq!(outcome.sources.snapshots().len(), 0);
                assert_eq!(failure.accepted_results().count(), 0);
                let (value, report) = match outcome.result {
                    OperationResult::Complete { value, report } => (value, report),
                    OperationResult::Invalid {
                        partial: Some(value),
                        report,
                    } => (value, report),
                    OperationResult::Stopped {
                        reason: StopReason::Cancelled,
                        partial: Some(value),
                        report,
                    } => (value, report),
                    _ => return Err("lost terminal partial".into()),
                };
                if !child || outcome.request_id != root.request_id {
                    retained += 1;
                    assert_eq!(resumed.get(), 0);
                    assert!(matches!(
                        (mode, outcome.result),
                        (0, OperationResult::Complete { .. })
                            | (1, OperationResult::Invalid { .. })
                            | (2, OperationResult::Stopped { .. })
                    ));
                    assert_eq!(
                        number(value),
                        if !child {
                            number(&root.input)
                        } else {
                            Some(10)
                        }
                    );
                    assert_eq!(report.diagnostics.len(), 1);
                    assert_eq!(report.diagnostics[0].code, "terminal-retained");
                    assert_eq!(report.diagnostics[0].arguments, *value);
                }
                assert!(cancelled.contains(&outcome.request_id));
            }
            let mut unique = cancelled.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), cancelled.len());
        }
        assert!(
            retained > 0,
            "child={child}, mode={mode}: acceptance stop not exercised"
        );
        corrupt.set(true);
        let Err(failure) = run(&mut budget(), &mut vec![]) else {
            return Err("invalid output must fail".into());
        };
        assert_eq!(failure.uncommitted_results().count(), 0);
        assert_eq!(failure.accepted_results().count(), 0);
        assert_eq!(resumed.get(), 0);
    }
    Ok(())
}
