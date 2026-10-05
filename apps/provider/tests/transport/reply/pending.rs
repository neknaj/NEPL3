use super::*;
use nepl3_provider::pending::{PendingFinishError, PendingReadError};
use nepl3_wire::operation::{ReplyAdmissionError, ReplyFrameError};
use std::{cell::Cell, rc::Rc};

struct Counted<T> {
    inner: T,
    calls: Rc<Cell<usize>>,
}
impl<T: Read> Read for Counted<T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        self.inner.read(bytes)
    }
}
impl<T: Write> Write for Counted<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        self.inner.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.calls.set(self.calls.get() + 1);
        self.inner.flush()
    }
}

#[test]
fn unfinished_receipts_cannot_perform_subsequent_stream_io() -> Result<(), String> {
    let (registry, request) = fixture()?;
    for forget in [false, true] {
        let (reader, writer) = connection(&complete(&request), &registry)?.into_parts();
        let reads = Rc::new(Cell::new(0));
        let writes = Rc::new(Cell::new(0));
        let mut c = Connection::new(
            Counted {
                inner: reader,
                calls: reads.clone(),
            },
            Counted {
                inner: writer,
                calls: writes.clone(),
            },
        );
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        {
            let pending = c
                .receive_pending_reply(&registry, &mut admission, &mut b)
                .map_err(error)?
                .ok_or("receipt")?;
            if forget {
                core::mem::forget(pending);
            }
        }
        let before = (reads.get(), writes.get());
        assert!(matches!(
            c.receive(&registry, &SourceStore::default(), &mut admission, &mut b),
            Err(TransportError::Closed)
        ));
        assert!(matches!(
            c.send(
                &ProviderFrame::Close,
                &registry,
                &SourceStore::default(),
                &mut admission,
                &mut b
            ),
            Err(TransportError::Closed)
        ));
        assert!(matches!(
            c.receive_pending_reply(&registry, &mut admission, &mut b),
            Err(PendingReadError::Transport(TransportError::Closed))
        ));
        assert!(matches!(
            c.receive_reply(
                &request,
                Digest::of(b"context"),
                &registry,
                &SourceStore::default(),
                &SourceStore::default(),
                &mut admission,
                &mut b,
                &mut budget()
            ),
            Err(ReplyError::Transport(TransportError::Closed))
        ));
        assert_eq!((reads.get(), writes.get()), before);
    }
    Ok(())
}

#[test]
fn internal_payload_trailing_data_is_not_a_following_frame() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let mut bytes = connection(&complete(&request), &registry)?
        .into_parts()
        .0
        .into_inner();
    bytes.push(0);
    let length = (bytes.len() - 8) as u64;
    bytes[..8].copy_from_slice(&length.to_be_bytes());
    let mut c = Connection::new(Cursor::new(bytes), Vec::new());
    assert!(matches!(
        c.receive_pending_reply(&registry, &mut SourceAdmission::default(), &mut budget()),
        Err(PendingReadError::Frame(ReplyFrameError::Wire(
            nepl3_wire::WireError::TrailingData
        )))
    ));
    assert_closed(&mut c, &registry)?;
    Ok(())
}

#[test]
fn pending_schema_exchange_rejects_before_any_stream_read() -> Result<(), String> {
    let (registry, _) = fixture()?;
    let reads = Rc::new(Cell::new(0));
    let mut c = Connection::new(
        Counted {
            inner: io::empty(),
            calls: reads.clone(),
        },
        Vec::new(),
    );
    c.send(
        &ProviderFrame::SchemaRequest { schemas: vec![] },
        &registry,
        &SourceStore::default(),
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    assert!(matches!(
        c.receive_pending_reply(&registry, &mut SourceAdmission::default(), &mut budget()),
        Err(PendingReadError::Transport(TransportError::ProtocolState))
    ));
    assert_eq!(reads.get(), 0);
    assert!(c.is_closed());
    Ok(())
}

#[test]
fn host_policy_admits_generated_report_sources_or_stays_closed() -> Result<(), String> {
    use nepl3_core::diagnostic::{Diagnostic, Severity};
    let (registry, request) = fixture()?;
    for provide_source in [false, true] {
        let source = SourceSnapshot::new(
            SourceId("generated".into()),
            1,
            "memory:generated".into(),
            b"new".to_vec(),
            &mut budget(),
        )
        .map_err(error)?;
        let span = source.span(0, 3).map_err(error)?;
        let mut sources = SourceStore::default();
        sources.insert(source).map_err(error)?;
        let frame = ProviderFrame::Reply {
            request_id: request.request_id,
            reply: OperationReply::Result(OperationResult::Complete {
                value: request.input.clone(),
                report: Report {
                    diagnostics: vec![Diagnostic {
                        schema: request.operation.schema.clone(),
                        code: "generated".into(),
                        severity: Severity::Warning,
                        stage: "test".into(),
                        arguments: request.input.clone(),
                        primary: Some(span),
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
        };
        let bytes = nepl3_wire::operation::encode_frame(
            &frame,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        let mut c = Connection::new(Cursor::new(bytes), Vec::new());
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let pending = c
            .receive_pending_reply(&registry, &mut admission, &mut b)
            .map_err(error)?
            .ok_or("receipt")?;
        let mut calls = 0;
        let result = pending.finish_with(&request, &SourceStore::default(), |_, _, _, _, _| {
            calls += 1;
            Ok::<_, ()>(if provide_source {
                sources
            } else {
                SourceStore::default()
            })
        });
        assert_eq!(calls, 1);
        if provide_source {
            assert_eq!(result.map_err(error)?, frame);
            assert!(!c.is_closed());
        } else {
            assert!(matches!(
                result,
                Err(PendingFinishError::Admission(ReplyAdmissionError::Wire(_)))
            ));
            assert_closed(&mut c, &registry)?;
        }
    }
    Ok(())
}

fn complete(request: &Invoke) -> ProviderFrame {
    ProviderFrame::Reply {
        request_id: request.request_id,
        reply: OperationReply::Result(OperationResult::Complete {
            value: request.input.clone(),
            report: Report::default(),
        }),
    }
}

fn assert_closed<R: Read, W: Write>(
    c: &mut Connection<R, W>,
    registry: &SchemaRegistry,
) -> Result<(), String> {
    assert!(c.is_closed());
    assert!(matches!(
        c.receive(
            registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(TransportError::Closed)
    ));
    assert!(matches!(
        c.send(
            &ProviderFrame::Close,
            registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(TransportError::Closed)
    ));
    assert!(matches!(
        c.receive_pending_reply(registry, &mut SourceAdmission::default(), &mut budget()),
        Err(PendingReadError::Transport(TransportError::Closed))
    ));
    Ok(())
}

#[test]
fn pending_drop_forget_and_finish_errors_leave_io_closed() -> Result<(), String> {
    let (registry, request) = fixture()?;
    for action in 0..5 {
        let (reader, writer) = connection(&complete(&request), &registry)?.into_parts();
        let reads = Rc::new(Cell::new(0));
        let writes = Rc::new(Cell::new(0));
        let mut c = Connection::new(
            Counted {
                inner: reader,
                calls: reads.clone(),
            },
            Counted {
                inner: writer,
                calls: writes.clone(),
            },
        );
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let pending = c
            .receive_pending_reply(&registry, &mut admission, &mut b)
            .map_err(error)?
            .ok_or("receipt")?;
        assert_eq!(pending.request_id(), request.request_id);
        match action {
            0 => {} // End its borrow without finishing; no Drop hook is required.
            1 => core::mem::forget(pending),
            2 => {
                let mut wrong = request.clone();
                wrong.request_id += 1;
                assert!(matches!(
                    pending.finish(&wrong, &SourceStore::default()),
                    Err(PendingFinishError::Admission(
                        ReplyAdmissionError::RequestId { .. }
                    ))
                ));
            }
            3 => assert!(matches!(
                pending.finish_with(&request, &SourceStore::default(), |_, _, _, _, _| Err::<
                    SourceStore,
                    _,
                >(
                    71u8
                )),
                Err(PendingFinishError::Admission(ReplyAdmissionError::Policy(
                    71
                )))
            )),
            _ => assert!(matches!(
                pending.finish_with(&request, &SourceStore::default(), |_, _, _, _, b| {
                    b.stop(StopReason::Cancelled);
                    Ok::<_, ()>(SourceStore::default())
                }),
                Err(PendingFinishError::Admission(ReplyAdmissionError::Wire(
                    nepl3_wire::WireError::Stopped(StopReason::Cancelled)
                )))
            )),
        }
        let before = (reads.get(), writes.get());
        assert_closed(&mut c, &registry)?;
        assert_eq!((reads.get(), writes.get()), before);
        let (_, written) = c.into_parts();
        assert!(written.inner.is_empty());
    }
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[allow(clippy::panic)] // Deliberately exercise a trusted callback unwinding.
fn pending_policy_panic_cannot_reopen_transport() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let (reader, writer) = connection(&complete(&request), &registry)?.into_parts();
    let reads = Rc::new(Cell::new(0));
    let writes = Rc::new(Cell::new(0));
    let mut c = Connection::new(
        Counted {
            inner: reader,
            calls: reads.clone(),
        },
        Counted {
            inner: writer,
            calls: writes.clone(),
        },
    );
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let pending = c
            .receive_pending_reply(&registry, &mut admission, &mut b)
            .map_err(error)?
            .ok_or("receipt")?;
        pending
            .finish_with(
                &request,
                &SourceStore::default(),
                |_, _, _, _, _| -> Result<SourceStore, ()> {
                    panic!("intentional trusted policy panic")
                },
            )
            .map_err(error)
    }));
    assert!(caught.is_err());
    let before = (reads.get(), writes.get());
    assert_closed(&mut c, &registry)?;
    assert_eq!((reads.get(), writes.get()), before);
    Ok(())
}

#[test]
fn pending_success_reopens_and_preserves_next_frame() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let frame = complete(&request);
    let c = connection(&frame, &registry)?;
    let (reader, _) = c.into_parts();
    let mut bytes = reader.into_inner();
    let c = connection(&ProviderFrame::Cancel { request_id: 91 }, &registry)?;
    bytes.extend(c.into_parts().0.into_inner());
    let mut c = Connection::new(Chunks(Cursor::new(bytes)), Vec::new());
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let pending = c
        .receive_pending_reply(&registry, &mut admission, &mut b)
        .map_err(error)?
        .ok_or("receipt")?;
    assert_eq!(
        pending
            .finish(&request, &SourceStore::default())
            .map_err(error)?,
        frame
    );
    assert!(!c.is_closed());
    assert_eq!(
        c.receive(&registry, &SourceStore::default(), &mut admission, &mut b)
            .map_err(error)?,
        Some(ProviderFrame::Cancel { request_id: 91 })
    );
    Ok(())
}

#[test]
fn pending_phase_precedes_io_and_close_remains_distinct() -> Result<(), String> {
    let (registry, _) = fixture()?;
    for incoming in [false, true] {
        let mut c = negotiating(&registry, incoming)?;
        assert!(matches!(
            c.receive_pending_reply(&registry, &mut SourceAdmission::default(), &mut budget()),
            Err(PendingReadError::Transport(TransportError::ProtocolState))
        ));
        assert!(c.is_closed());
    }
    for (frame, close) in [
        (ProviderFrame::Close, true),
        (ProviderFrame::Cancel { request_id: 1 }, false),
    ] {
        let mut c = connection(&frame, &registry)?;
        let result = c
            .receive_pending_reply(&registry, &mut SourceAdmission::default(), &mut budget())
            .err()
            .ok_or("expected error")?;
        assert!(matches!(
            (result, close),
            (PendingReadError::Frame(ReplyFrameError::PeerClose), true)
                | (
                    PendingReadError::Frame(ReplyFrameError::ExpectedReply),
                    false
                )
        ));
        assert_closed(&mut c, &registry)?;
    }
    Ok(())
}

#[test]
fn pending_eof_truncation_and_oversize_never_yield_receipts() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let bytes = connection(&complete(&request), &registry)?
        .into_parts()
        .0
        .into_inner();
    for end in [0, 4, bytes.len() - 1] {
        let mut c = Connection::new(Cursor::new(bytes[..end].to_vec()), Vec::new());
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let result = c.receive_pending_reply(&registry, &mut admission, &mut b);
        if end == 0 {
            assert!(matches!(result, Ok(None)));
        } else {
            assert!(matches!(
                result,
                Err(PendingReadError::Transport(TransportError::Truncated))
            ));
        }
        assert_closed(&mut c, &registry)?;
    }
    let mut c = Connection::new(Cursor::new(u64::MAX.to_be_bytes().to_vec()), Vec::new());
    assert!(
        c.receive_pending_reply(&registry, &mut SourceAdmission::default(), &mut budget())
            .is_err()
    );
    assert_closed(&mut c, &registry)?;
    Ok(())
}
