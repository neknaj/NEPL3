use super::*;
use nepl3_core::{
    diagnostic::{OperationResult, Report},
    operation::*,
    value::*,
};
use nepl3_provider::reply::ReplyError;

fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn fixture() -> Result<(SchemaRegistry, Invoke), String> {
    let mut registry = SchemaRegistry::default();
    let foundation = foundation::descriptor(&mut budget()).map_err(error)?;
    let id = foundation.reference(&mut budget()).map_err(error)?;
    registry
        .register(id, foundation, &mut budget())
        .map_err(error)?;
    let ty = TypeDescriptor::Named(TypeRef {
        package: "test.reply".into(),
        revision: 1,
        name: "Number".into(),
    });
    let descriptor = SchemaDescriptor {
        package: "test.reply".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Number".into(),
            shape: TypeShape::Record {
                fields: vec![FieldDescriptor {
                    name: "value".into(),
                    ty: TypeDescriptor::U64,
                }],
            },
            constraints: vec![],
        }],
        operations: vec![OperationDescriptor {
            name: "identity".into(),
            input: ty.clone(),
            output: ty,
            pure: true,
        }],
    };
    let schema = descriptor.reference(&mut budget()).map_err(error)?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    let value = TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "Number".into(),
        fields: vec![NdfValue::U64(41)],
    });
    Ok((
        registry,
        Invoke {
            request_id: 17,
            operation: OperationRef {
                schema,
                name: "identity".into(),
            },
            input: value.clone(),
            environment: value,
            sources: vec![],
            resources: vec![],
            limits: budget().limits(),
        },
    ))
}
fn connection(
    frame: &ProviderFrame,
    registry: &SchemaRegistry,
) -> Result<Connection<Cursor<Vec<u8>>, Vec<u8>>, String> {
    let bytes = nepl3_wire::operation::encode_frame(
        frame,
        registry,
        &SourceStore::default(),
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    Ok(Connection::new(Cursor::new(bytes), Vec::new()))
}
fn receive(
    c: &mut Connection<Cursor<Vec<u8>>, Vec<u8>>,
    request: &Invoke,
    context: Digest,
    registry: &SchemaRegistry,
    validation: &mut Budget,
) -> Result<OperationReply, ReplyError> {
    let sources = SourceStore::default();
    c.receive_reply(
        request,
        context,
        registry,
        &sources,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
        validation,
    )
}

#[test]
fn sent_invoke_receives_checked_terminal_and_await_replies() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let context = Digest::of(b"admitted context");
    let terminal = OperationReply::Result(OperationResult::Complete {
        value: request.input.clone(),
        report: Report::default(),
    });
    let mut child = request.clone();
    child.request_id = 18;
    let awaiting = OperationReply::Await {
        continuation: Continuation {
            provider: request.operation.clone(),
            parent_request: request.request_id,
            snapshot_digest: context,
            state: request.input.clone(),
        },
        calls: vec![child],
        report: Report::default(),
    };
    for reply in [terminal, awaiting] {
        let mut c = connection(
            &ProviderFrame::Reply {
                request_id: 17,
                reply: reply.clone(),
            },
            &registry,
        )?;
        let frame = ProviderFrame::Invoke(request.clone());
        c.send(
            &frame,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        assert_eq!(
            receive(&mut c, &request, context, &registry, &mut budget()).map_err(error)?,
            reply
        );
        assert!(!c.is_closed());
        let (_, sent) = c.into_parts();
        let (decoded, rest) = nepl3_wire::operation::decode_frame(
            &sent,
            true,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?
        .ok_or("missing sent frame")?;
        assert_eq!(decoded, frame);
        assert!(rest.is_empty());
    }
    Ok(())
}

#[test]
fn mismatched_id_direction_and_eof_close_reply_exchange() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let context = Digest::of(b"context");
    let mut c = connection(
        &ProviderFrame::Reply {
            request_id: 99,
            reply: OperationReply::Result(OperationResult::Complete {
                value: request.input.clone(),
                report: Report::default(),
            }),
        },
        &registry,
    )?;
    assert!(matches!(
        receive(&mut c, &request, context, &registry, &mut budget()),
        Err(ReplyError::RequestId {
            expected: 17,
            received: 99
        })
    ));
    assert!(c.is_closed());
    let mut c = connection(&ProviderFrame::Cancel { request_id: 17 }, &registry)?;
    assert!(matches!(
        receive(&mut c, &request, context, &registry, &mut budget()),
        Err(ReplyError::UnexpectedFrame)
    ));
    assert!(c.is_closed());
    let mut c = Connection::new(Cursor::new(vec![]), vec![]);
    assert!(matches!(
        receive(&mut c, &request, context, &registry, &mut budget()),
        Err(ReplyError::Closed)
    ));
    assert!(c.is_closed());
    Ok(())
}

#[test]
fn changed_await_context_and_validation_stop_close_reply_exchange() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let context = Digest::of(b"context");
    let frame = ProviderFrame::Reply {
        request_id: 17,
        reply: OperationReply::Await {
            continuation: Continuation {
                provider: request.operation.clone(),
                parent_request: 17,
                snapshot_digest: context,
                state: request.input.clone(),
            },
            calls: vec![],
            report: Report::default(),
        },
    };
    let mut c = connection(&frame, &registry)?;
    assert!(matches!(
        receive(
            &mut c,
            &request,
            Digest::of(b"changed"),
            &registry,
            &mut budget()
        ),
        Err(ReplyError::Validation(
            nepl3_suite::dispatch::suspending::Error::Await(_)
        ))
    ));
    assert!(c.is_closed());
    let mut c = connection(&frame, &registry)?;
    let mut validation = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        receive(&mut c, &request, context, &registry, &mut validation),
        Err(ReplyError::Validation(
            nepl3_suite::dispatch::suspending::Error::Stopped(StopReason::WorkLimit)
        ))
    ));
    assert!(c.is_closed());
    Ok(())
}
