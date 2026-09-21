use super::*;
use nepl3_core::{
    diagnostic::{OperationResult, Report},
    operation::*,
    value::*,
};
use nepl3_provider::reply::ReplyError;
use nepl3_suite::grants::Grants;
#[path = "reply/control.rs"]
mod control;
#[path = "reply/resume.rs"]
mod resume;

fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

fn increment(
    request: &Invoke,
    _: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    let mut value = request.input.clone_with_budget(b)?;
    if let TypedValue::Record(record) = &mut value
        && let [NdfValue::U64(number)] = record.fields.as_mut_slice()
        && let Some(next) = number.checked_add(1)
    {
        *number = next;
        return Ok(OperationReply::Result(OperationResult::Complete {
            value,
            report: Report::default(),
        }));
    }
    Ok(OperationReply::Result(OperationResult::Invalid {
        partial: None,
        report: Report::default(),
    }))
}

#[test]
fn wire_invoke_executes_native_callback_and_returns_the_checked_result() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let sources = SourceStore::default();
    let context = Digest::of(b"host authorized context");
    let identity = Digest::of(b"native increment implementation");
    let registration = nepl3_suite::dispatch::suspending::Registration {
        operation: &request.operation,
        implementation: identity,
        invoke: increment,
    };
    let mut server = connection(&ProviderFrame::Invoke(request.clone()), &registry)?;
    let Some(ProviderFrame::Invoke(received)) = server
        .receive(
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?
    else {
        return Err("expected incoming Invoke".into());
    };
    let mut execution = budget();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let approved = grants.admit(&received, &mut budget()).map_err(error)?;
    let native = server
        .dispatch_invoke(
            &registration,
            identity,
            &approved,
            context,
            &registry,
            &sources,
            &sources,
            &mut SourceAdmission::default(),
            &mut execution,
            &mut budget(),
            &mut budget(),
        )
        .map_err(error)?;
    assert!(execution.usage().work > 0);
    let (_, bytes) = server.into_parts();
    let mut client = Connection::new(Cursor::new(bytes), Vec::new());
    let portable =
        receive(&mut client, &request, context, &registry, &mut budget()).map_err(error)?;
    assert_eq!(portable, native);
    let OperationReply::Result(OperationResult::Complete {
        value: TypedValue::Record(record),
        ..
    }) = portable
    else {
        return Err("expected complete record".into());
    };
    // Independent expectation: the callback increments the initial 41 once.
    assert_eq!(record.fields, vec![NdfValue::U64(42)]);
    Ok(())
}

#[test]
fn decoded_ungranted_environment_cannot_reach_dispatch() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut untrusted = request.clone();
    if let TypedValue::Record(record) = &mut untrusted.environment {
        record.fields[0] = NdfValue::U64(99);
    }
    // This is schema-valid wire data; permission comes from independent host state.
    let mut server = connection(&ProviderFrame::Invoke(untrusted), &registry)?;
    let Some(ProviderFrame::Invoke(received)) = server
        .receive(
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?
    else {
        return Err("expected decoded Invoke".into());
    };
    let identity = Digest::of(b"host implementation");
    let registration = nepl3_suite::dispatch::suspending::Registration {
        operation: &request.operation,
        implementation: identity,
        invoke: increment,
    };
    let mut execution = budget();
    let outcome = grants.admit(&received, &mut budget()).map(|approved| {
        server.dispatch_invoke(
            &registration,
            identity,
            &approved,
            identity,
            &registry,
            &sources,
            &sources,
            &mut SourceAdmission::default(),
            &mut execution,
            &mut budget(),
            &mut budget(),
        )
    });
    assert!(matches!(
        outcome,
        Err(nepl3_suite::grants::GrantError::Environment)
    ));
    assert_eq!(execution.usage().work, 0);
    assert!(server.into_parts().1.is_empty());
    Ok(())
}

#[test]
fn rejected_dispatch_and_closed_transport_do_not_execute_or_emit_a_reply() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let sources = SourceStore::default();
    let identity = Digest::of(b"native implementation");
    let registration = nepl3_suite::dispatch::suspending::Registration {
        operation: &request.operation,
        implementation: identity,
        invoke: increment,
    };
    let mut server = Connection::new(Cursor::new(Vec::<u8>::new()), Vec::<u8>::new());
    let mut execution = budget();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let approved = grants.admit(&request, &mut budget()).map_err(error)?;
    let failed = server.dispatch_invoke(
        &registration,
        Digest::of(b"different implementation"),
        &approved,
        identity,
        &registry,
        &sources,
        &sources,
        &mut SourceAdmission::default(),
        &mut execution,
        &mut budget(),
        &mut budget(),
    );
    assert!(matches!(
        failed,
        Err(nepl3_provider::dispatch::DispatchError::Operation(_))
    ));
    assert!(server.is_closed());
    assert_eq!(execution.usage().work, 0);
    let failed = server.dispatch_invoke(
        &registration,
        identity,
        &approved,
        identity,
        &registry,
        &sources,
        &sources,
        &mut SourceAdmission::default(),
        &mut execution,
        &mut budget(),
        &mut budget(),
    );
    assert!(matches!(
        failed,
        Err(nepl3_provider::dispatch::DispatchError::Transport(
            TransportError::Closed
        ))
    ));
    assert_eq!(execution.usage().work, 0);
    assert!(server.into_parts().1.is_empty());
    Ok(())
}

#[test]
fn failed_reply_write_keeps_execution_work_and_prevents_callback_retry() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let sources = SourceStore::default();
    let identity = Digest::of(b"native implementation");
    let registration = nepl3_suite::dispatch::suspending::Registration {
        operation: &request.operation,
        implementation: identity,
        invoke: increment,
    };
    let mut server = Connection::new(io::empty(), Broken);
    let mut execution = budget();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let approved = grants.admit(&request, &mut budget()).map_err(error)?;
    let result = server.dispatch_invoke(
        &registration,
        identity,
        &approved,
        identity,
        &registry,
        &sources,
        &sources,
        &mut SourceAdmission::default(),
        &mut execution,
        &mut budget(),
        &mut budget(),
    );
    assert!(matches!(
        result,
        Err(nepl3_provider::dispatch::DispatchError::Transport(
            TransportError::Io(_)
        ))
    ));
    assert!(server.is_closed());
    let work = execution.usage().work;
    assert!(work > 0);
    let retry = server.dispatch_invoke(
        &registration,
        identity,
        &approved,
        identity,
        &registry,
        &sources,
        &sources,
        &mut SourceAdmission::default(),
        &mut execution,
        &mut budget(),
        &mut budget(),
    );
    assert!(matches!(
        retry,
        Err(nepl3_provider::dispatch::DispatchError::Transport(
            TransportError::Closed
        ))
    ));
    assert_eq!(execution.usage().work, work);
    Ok(())
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
