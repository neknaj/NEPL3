use nepl3_core::{budget::*, diagnostic::*, schema::*, source::*, syntax::*, value::*, view::*};
use nepl3_provider::{Connection, TransportError};
use nepl3_reader::{model::*, plan::*, runtime::*};
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Write},
    rc::Rc,
};
#[path = "reader/model.rs"]
mod model;
use model::*;
use nepl3_core::{
    operation::{Invoke, OperationReply, ProviderFrame},
    value_codec::FoundationValueCodec,
};
use nepl3_reader::portable::{
    dispatch::{self, DispatchContext, ProviderInput},
    read,
};
use nepl3_wire::foundation::FoundationCodec;

fn error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

/// A generated-source diagnostic requires admission between frame decoding
/// and outer Report decoding. Domain validation still uses the saved Read call.
#[test]
fn generated_report_crosses_connection_without_consuming_rejected_pending_read()
-> Result<(), String> {
    let (registry, schema) = registry().map_err(error)?;
    let p = provider_plan(&schema);
    let checked_plan = p.check(&registry, &mut budget()).map_err(error)?;
    let mut b = budget();
    let mut session = ReaderSession::new("framed-read".into(), &checked_plan, &registry, &mut b)
        .map_err(error)?;
    let source = source("あ").map_err(error)?;
    let mut store = SourceStore::default();
    store.insert(source.clone()).map_err(error)?;
    let mut admission = SourceAdmission::default();
    let raw = context(&schema, &registry).map_err(error)?;
    let checked_context =
        check_context(&raw, &store, &registry, &mut b, &mut admission).map_err(error)?;
    let suspended = session
        .read(
            "entry",
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: 3,
                final_input: false,
                context: &checked_context,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut admission,
        )
        .map_err(error)?;
    let ReadReply::Await {
        call, continuation, ..
    } = suspended
    else {
        return Err("expected Await".into());
    };
    let ProviderCall::Read {
        call_id,
        operation,
        request,
        ..
    } = *call
    else {
        return Err("expected Read call".into());
    };
    let signature = signature(&schema, ProviderKind::Read);
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let input = dispatch::to_value(
        &ProviderInput::Read(Box::new(request.clone())),
        &DispatchContext {
            signature: &signature,
            sources: &store,
            mappings: &[],
            registry: &registry,
        },
        &mut codec,
        &mut b,
    )
    .map_err(error)?;
    let NdfValue::Record(ref environment) = codec
        .encode_environment(&request.context.environment, &mut b)
        .map_err(error)?
    else {
        return Err("expected environment record".into());
    };
    let saved = Invoke {
        request_id: call_id,
        operation,
        input,
        environment: TypedValue::Record(environment.clone()),
        sources: request.sources,
        resources: vec![],
        limits: b.limits(),
    };
    let generated = admission
        .create(
            SourceId("framed-generated".into()),
            0,
            "memory:framed-generated".into(),
            "生成😀".as_bytes().to_vec(),
            &mut b,
        )
        .map_err(error)?;
    let span = generated.span(0, 10).map_err(error)?;
    let mapping = nepl3_core::origin::Mapping {
        source: source.span(0, 3).map_err(error)?,
        target: span.clone(),
        kind: nepl3_core::origin::MappingKind::Transformed,
    };
    b.charge(Resource::Diagnostics, 1).map_err(error)?;
    let diagnostic = Diagnostic {
        schema: schema.clone(),
        code: "Generated".into(),
        severity: Severity::Warning,
        stage: "read".into(),
        arguments: TypedValue::Record(Record {
            schema: schema.clone(),
            kind: "Node".into(),
            fields: vec![],
        }),
        primary: Some(span),
        related: vec![],
        fixes: vec![],
    };
    let ProviderReply::Read(mut reply) = terminal("あ", 3, &mut b).map_err(error)? else {
        return Err("expected terminal read".into());
    };
    if let ReadReply::Matched {
        sources,
        source_maps,
        report,
        ..
    } = reply.as_mut()
    {
        sources.push(generated.clone());
        source_maps.push(mapping.clone());
        report.diagnostics.push(diagnostic.clone());
        report.usage = b.usage();
    }
    let receiving = session.pending_read().map_err(error)?;
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let operation =
        read::operation::to_reply(&reply, &receiving, &mut codec, &mut b).map_err(error)?;
    let mut sending = SourceStore::default();
    sending.insert(source.clone()).map_err(error)?;
    sending.insert(generated.clone()).map_err(error)?;
    // Mutations preserve structural framing while violating domain contracts.
    // Both are rejected before ReaderSession::resume consumes its pending slot.
    let mut received_match = None;
    for case in 0..3 {
        let mut candidate = operation.clone();
        let OperationReply::Result(OperationResult::Complete { value, report }) = &mut candidate
        else {
            return Err("expected complete".into());
        };
        if case == 0 {
            let TypedValue::Variant(value) = value else {
                return Err("expected ReadReply variant".into());
            };
            // Matched.sources is the sixth field in the registered Reader schema.
            value.fields[5] = NdfValue::List(vec![]);
        } else if case == 1 {
            report.usage.work += 1;
        }
        let frame = ProviderFrame::Reply {
            request_id: saved.request_id,
            reply: candidate,
        };
        let mut sender = Connection::new(io::empty(), Vec::new());
        sender
            .send(
                &frame,
                &registry,
                &sending,
                &mut SourceAdmission::default(),
                &mut b,
            )
            .map_err(error)?;
        let (_, encoded) = sender.into_parts();
        let mut closer = Connection::new(io::empty(), Vec::new());
        closer
            .send(
                &ProviderFrame::Close,
                &registry,
                &store,
                &mut SourceAdmission::default(),
                &mut b,
            )
            .map_err(error)?;
        let (_, following) = closer.into_parts();
        if case == 2 {
            let mut truncated = Connection::new(
                Cursor::new(encoded[..encoded.len() - 1].to_vec()),
                Vec::new(),
            );
            let mut truncated_admission = SourceAdmission::default();
            let mut truncated_budget = budget();
            assert!(matches!(
                truncated.receive_pending_reply(
                    &registry,
                    &mut truncated_admission,
                    &mut truncated_budget,
                ),
                Err(nepl3_provider::pending::PendingReadError::Transport(
                    TransportError::Truncated
                ))
            ));
            assert!(truncated.is_closed());
            assert!(session.pending_read().is_ok());
            let mut missing_admission = SourceAdmission::default();
            let mut missing_budget = budget();
            let mut missing_connection = Connection::new(Cursor::new(encoded.clone()), Vec::new());
            let pending = missing_connection
                .receive_pending_reply(&registry, &mut missing_admission, &mut missing_budget)
                .map_err(error)?
                .ok_or("missing negative-control frame")?;
            let missing = pending.finish(&saved, &store);
            assert!(
                matches!(
                    missing,
                    Err(nepl3_provider::pending::PendingFinishError::Admission(
                        nepl3_wire::operation::ReplyAdmissionError::Wire(
                            nepl3_wire::WireError::Source(SourceError::MissingSnapshot)
                        )
                    ))
                ),
                "unexpected missing-source result: {missing:?}"
            );
            assert!(missing_connection.is_closed());
            assert!(session.pending_read().is_ok());
        }
        let mut transport_admission = SourceAdmission::default();
        let read_bytes = Rc::new(Cell::new(0));
        let read_calls = Rc::new(Cell::new(0));
        let write_calls = Rc::new(Cell::new(0));
        let mut bytes = encoded.clone();
        // A following Close frame must remain unread, even on a policy failure.
        bytes.extend_from_slice(&following);
        let mut connection = Connection::new(
            Chunks {
                input: Cursor::new(bytes),
                read_bytes: read_bytes.clone(),
                calls: read_calls.clone(),
            },
            Writes {
                bytes: Vec::new(),
                calls: write_calls.clone(),
            },
        );
        let pending = connection
            .receive_pending_reply(&registry, &mut transport_admission, &mut b)
            .map_err(error)?
            .ok_or("missing pending frame")?;
        let received = pending.finish_with(
            &saved,
            &store,
            |payload, registry, original, admission, budget| {
                let mut codec =
                    FoundationCodec::new(registry, original, admission).map_err(error)?;
                let decoded = read::reply_from_value(payload.value, &receiving, &mut codec, budget)
                    .map_err(error)?;
                let ReadReply::Matched { sources, .. } = decoded else {
                    return Err("expected matched payload".into());
                };
                let mut closure = SourceStore::default();
                for snapshot in original.snapshots().iter().chain(sources.iter()) {
                    closure
                        .insert_ref_with_budget(snapshot, budget)
                        .map_err(error)?;
                }
                Ok::<_, String>(closure)
            },
        );
        assert_eq!(read_bytes.get(), encoded.len());
        if case == 0 {
            assert!(received.is_err());
            assert!(connection.is_closed());
            assert!(session.pending_read().is_ok());
            let before = (read_calls.get(), write_calls.get());
            assert!(matches!(
                connection.receive(&registry, &store, &mut transport_admission, &mut b),
                Err(TransportError::Closed)
            ));
            assert_eq!(read_bytes.get(), encoded.len());
            assert!(matches!(
                connection.send(
                    &ProviderFrame::Close,
                    &registry,
                    &store,
                    &mut transport_admission,
                    &mut b,
                ),
                Err(TransportError::Closed)
            ));
            assert_eq!((read_calls.get(), write_calls.get()), before);
            let (_, written) = connection.into_parts();
            assert!(written.bytes.is_empty());
            continue;
        }
        assert!(!connection.is_closed());
        let ProviderFrame::Reply {
            reply: received, ..
        } = received.map_err(error)?
        else {
            return Err("expected reply frame".into());
        };
        let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
        let decoded = read::operation::from_reply(&received, &receiving, &mut codec, &mut b);
        if case == 1 {
            // Domain rejection occurs after transport admission. The documented
            // transport boundary does not own the Reader continuation.
            assert!(!connection.is_closed());
            assert!(decoded.is_err());
            assert!(session.pending_read().is_ok());
            continue;
        }
        let OperationResult::Complete { value, report } = decoded.map_err(error)? else {
            return Err("expected complete Reader result".into());
        };
        assert_eq!(value, *reply);
        received_match = Some(value);
        assert_eq!(report.diagnostics, vec![diagnostic.clone()]);
        // The successful frame does not consume the domain continuation either.
        assert!(session.pending_read().is_ok());
        assert!(matches!(
            connection
                .receive(&registry, &store, &mut transport_admission, &mut b)
                .map_err(error)?,
            Some(ProviderFrame::Close)
        ));
        assert_eq!(read_bytes.get(), encoded.len() + following.len());
        assert!(connection.is_closed());
        assert!(session.pending_read().is_ok());
    }
    let final_reply = session
        .resume(
            &continuation,
            ProviderReply::Read(Box::new(received_match.ok_or("missing admitted reply")?)),
            &store,
            &mut b,
            &mut admission,
        )
        .map_err(error)?;
    let ReadReply::Matched {
        value,
        end,
        sources,
        source_maps,
        report,
        ..
    } = final_reply
    else {
        return Err("expected final matched".into());
    };
    assert_eq!(value, NdfValue::Text("あ".into()));
    assert_eq!(end, 3);
    assert!(sources.contains(&generated));
    assert!(source_maps.contains(&mapping));
    assert!(report.diagnostics.contains(&diagnostic));
    assert!(session.pending_read().is_err());
    Ok(())
}

struct Chunks {
    input: Cursor<Vec<u8>>,
    read_bytes: Rc<Cell<usize>>,
    calls: Rc<Cell<usize>>,
}
impl Read for Chunks {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        let len = output.len().min(2);
        let read = self.input.read(&mut output[..len])?;
        self.read_bytes.set(self.read_bytes.get() + read);
        Ok(read)
    }
}

struct Writes {
    bytes: Vec<u8>,
    calls: Rc<Cell<usize>>,
}
impl Write for Writes {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.calls.set(self.calls.get() + 1);
        Ok(())
    }
}
