use super::*;
use nepl3_core::diagnostic::{Diagnostic, Severity};
use std::sync::atomic::{AtomicUsize, Ordering};

static RESUMED: AtomicUsize = AtomicUsize::new(0);

fn outcome(
    call: &Invoke,
    context: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    match number(&call.input) {
        Some(99) => return suspend_with(call, context, 99, &[10], b),
        Some(98) => return suspend_with(call, context, 98, &[20], b),
        _ => (),
    }
    let partial = Some(call.input.clone_with_budget(b)?);
    let mut report = Report::default();
    report.usage.diagnostics = 1;
    report.diagnostics.push(Diagnostic {
        schema: call.operation.schema.clone(),
        code: "child-outcome".into(),
        severity: Severity::Information,
        stage: "invoke".into(),
        arguments: call.input.clone_with_budget(b)?,
        primary: None,
        related: vec![],
        fixes: vec![],
    });
    Ok(OperationReply::Result(if number(&call.input) == Some(10) {
        OperationResult::Invalid { partial, report }
    } else {
        OperationResult::Stopped {
            reason: StopReason::Cancelled,
            partial,
            report,
        }
    }))
}

fn receive_invalid(
    _: &Invoke,
    request: &Resume,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    RESUMED.fetch_add(1, Ordering::SeqCst);
    b.charge(Resource::Work, 1)?;
    let [
        OperationReply::Result(OperationResult::Invalid {
            partial: Some(value),
            report,
        }),
    ] = request.dependency_results.as_slice()
    else {
        return Ok(OperationReply::Result(OperationResult::Invalid {
            partial: None,
            report: Report::default(),
        }));
    };
    assert_eq!(number(value), Some(10));
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].code, "child-outcome");
    assert_eq!(number(&report.diagnostics[0].arguments), Some(10));
    assert_eq!(report.usage.diagnostics, 1);
    Ok(OperationReply::Result(OperationResult::Invalid {
        partial: Some(value.clone_with_budget(b)?),
        report: report.clone(),
    }))
}

#[test]
fn invalid_keeps_partial_and_diagnostic_while_stopped_prevents_parent_callback()
-> Result<(), String> {
    for input in [99, 98] {
        RESUMED.store(0, Ordering::SeqCst);
        let (registry, mut root) = fixture()?;
        if let TypedValue::Record(record) = &mut root.input {
            record.fields[0] = NdfValue::U64(input);
        }
        let sources = SourceStore::default();
        let grants = Grants::new(&root.environment, &sources, &[], &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let identity = Digest::of(b"outcome fixture");
        let registrations = [scheduler::Registration {
            invoke: suspending::Registration {
                operation: &root.operation,
                implementation: identity,
                invoke: outcome,
            },
            resume: resume::Registration {
                operation: &root.operation,
                implementation: identity,
                resume: receive_invalid,
            },
            grants: &grants,
            context,
        }];
        let mut execution = budget();
        let mut cancelled = Vec::new();
        let result = scheduler::run_with_failure(
            &registrations,
            &root,
            &registry,
            &mut execution,
            &mut budget(),
            |_, _| {},
            |id| cancelled.push(id),
        );
        if input == 99 {
            let OperationResult::Invalid {
                partial: Some(value),
                report,
            } = result.map_err(|e| format!("{e:?}"))?
            else {
                return Err("expected retained Invalid".into());
            };
            assert_eq!(number(&value), Some(10));
            assert_eq!(report.diagnostics[0].code, "child-outcome");
            assert_eq!(RESUMED.load(Ordering::SeqCst), 1);
            assert_eq!(execution.poll(), Ok(()));
            assert!(cancelled.is_empty());
        } else {
            let failure = match result {
                Ok(_) => return Err("stopped child unexpectedly completed".into()),
                Err(failure) => failure,
            };
            assert!(matches!(failure.cause, scheduler::Error::Stopped(_)));
            let accepted = failure.accepted_results().collect::<Vec<_>>();
            assert_eq!(accepted.len(), 1);
            let OperationResult::Stopped {
                partial: Some(value),
                ..
            } = accepted[0].1
            else {
                return Err("expected retained stopped result".into());
            };
            assert_eq!(number(value), Some(20));
            assert_eq!(execution.poll(), Err(StopReason::Cancelled));
            assert_eq!(RESUMED.load(Ordering::SeqCst), 0);
            assert_eq!(cancelled, vec![root.request_id]);
        }
    }
    Ok(())
}
