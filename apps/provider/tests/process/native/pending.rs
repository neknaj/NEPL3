//! A real pipe carries a Report whose Unicode source is admitted after framing.
use super::*;
use nepl3_core::diagnostic::{Diagnostic, Severity};
use nepl3_provider::{TransportError, pending::PendingFinishError};
use nepl3_wire::{WireError, operation::ReplyAdmissionError};

fn generated() -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId("generated-process".into()),
        1,
        "memory:generated-process".into(),
        "生成 世界".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(error)
}
fn frame(request: &Invoke) -> Result<ProviderFrame, String> {
    Ok(ProviderFrame::Reply {
        request_id: request.request_id,
        reply: OperationReply::Result(OperationResult::Complete {
            value: request.input.clone(),
            report: Report {
                diagnostics: vec![Diagnostic {
                    schema: request.operation.schema.clone(),
                    code: "generated".into(),
                    severity: Severity::Warning,
                    stage: "process-test".into(),
                    arguments: request.input.clone(),
                    primary: Some(generated()?.span(7, 13).map_err(error)?),
                    related: vec![],
                    fixes: vec![],
                }],
                events: vec![],
                usage: Usage {
                    diagnostics: 1,
                    ..Usage::default()
                },
                trace_overflow: None,
            },
        }),
    })
}
pub fn child(following_close: bool) -> Result<(), String> {
    let (registry, request) = fixture()?;
    let mut sources = granted_sources()?;
    sources.insert(generated()?).map_err(error)?;
    let mut c = Connection::new(io::stdin().lock(), io::stdout().lock());
    let mut admission = SourceAdmission::default();
    c.send(
        &frame(&request)?,
        &registry,
        &sources,
        &mut admission,
        &mut budget(),
    )
    .map_err(error)?;
    if following_close {
        c.send(
            &ProviderFrame::Close,
            &registry,
            &sources,
            &mut admission,
            &mut budget(),
        )
        .map_err(error)?;
    }
    Ok(())
}
pub fn run() -> Result<(), String> {
    for mode in 0..3 {
        run_process(
            if mode == 0 {
                "--pending-source-child"
            } else {
                "--pending-source-only-child"
            },
            move |mut c| {
                let (registry, request) = fixture()?;
                let originals = granted_sources()?;
                // Trusted source fixture construction is outside the measured receipt.
                let mut trusted = SourceStore::default();
                if mode == 0 {
                    trusted.insert(generated()?).map_err(error)?;
                }
                let mut admission = SourceAdmission::default();
                let mut b = budget();
                let pending = c
                    .receive_pending_reply(&registry, &mut admission, &mut b)
                    .map_err(error)?
                    .ok_or("pending receipt")?;
                assert_eq!(pending.request_id(), request.request_id);
                let mut calls = 0;
                let mut at_policy = Usage::default();
                let result = pending.finish_with(&request, &originals, |_, _, _, _, b| {
                    calls += 1;
                    at_policy = b.usage();
                    if mode == 2 {
                        b.cancel();
                    }
                    Ok::<_, String>(trusted)
                });
                assert_eq!(calls, 1);
                assert!(at_policy.work > 0);
                assert!(at_policy.allocation_units > 0);
                if mode == 0 {
                    let result = result.map_err(error)?;
                    assert_eq!(result, frame(&request)?);
                    assert!(!c.is_closed());
                    let ProviderFrame::Reply {
                        reply: OperationReply::Result(OperationResult::Complete { report, .. }),
                        ..
                    } = result
                    else {
                        return Err("terminal reply".into());
                    };
                    let span = report.diagnostics[0]
                        .primary
                        .as_ref()
                        .ok_or("diagnostic span")?;
                    assert_eq!(generated()?.slice(span).map_err(error)?, "世界");
                    assert!(matches!(
                        c.receive(&registry, &originals, &mut admission, &mut b)
                            .map_err(error)?,
                        Some(ProviderFrame::Close)
                    ));
                } else {
                    if mode == 2 {
                        assert!(matches!(
                            result,
                            Err(PendingFinishError::Admission(ReplyAdmissionError::Wire(
                                WireError::Stopped(StopReason::Cancelled)
                            )))
                        ));
                        assert_eq!(b.usage(), at_policy);
                        assert_eq!(b.poll(), Err(StopReason::Cancelled));
                    } else {
                        assert!(matches!(
                            result,
                            Err(PendingFinishError::Admission(ReplyAdmissionError::Wire(
                                WireError::Source(SourceError::MissingSnapshot)
                            )))
                        ));
                        assert!(b.usage().work >= at_policy.work);
                        assert!(b.usage().allocation_units >= at_policy.allocation_units);
                    }
                    assert!(c.is_closed());
                    let consumed = b.usage();
                    assert!(matches!(
                        c.receive(&registry, &originals, &mut admission, &mut b),
                        Err(TransportError::Closed)
                    ));
                    assert_eq!(b.usage(), consumed);
                }
                Ok(())
            },
        )
        .map_err(|e| format!("staged source mode {mode}: {e}"))?;
    }
    Ok(())
}
