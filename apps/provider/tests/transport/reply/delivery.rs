use super::*;
use nepl3_core::diagnostic::{Diagnostic, Severity};

fn stopped(
    request: &Invoke,
    _: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    let partial = request.input.clone_with_budget(b)?;
    let diagnostic = Diagnostic {
        schema: request.operation.schema.clone(),
        code: "retained-stop".into(),
        severity: Severity::Information,
        stage: "invoke".into(),
        arguments: request.input.clone_with_budget(b)?,
        primary: None,
        related: vec![],
        fixes: vec![],
    };
    b.charge(Resource::Diagnostics, 1)?;
    Ok(OperationReply::Result(OperationResult::Stopped {
        reason: StopReason::Cancelled,
        partial: Some(partial),
        report: Report {
            diagnostics: vec![diagnostic],
            usage: b.usage(),
            ..Report::default()
        },
    }))
}

#[test]
fn stopped_reply_survives_io_and_transport_budget_failure() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let approved = grants.admit(&request, &mut budget()).map_err(error)?;
    let identity = Digest::of(b"stopped provider");
    let registration = nepl3_suite::dispatch::suspending::Registration {
        operation: &request.operation,
        implementation: identity,
        invoke: &stopped,
    };
    for exhausted in [false, true] {
        let mut server = Connection::new(io::empty(), Broken);
        let mut execution = budget();
        let mut transport = if exhausted {
            Budget::new(Limits {
                work: 0,
                ..budget().limits()
            })
        } else {
            budget()
        };
        let delivered = server
            .dispatch_invoke(
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
                &mut transport,
            )
            .map_err(error)?;
        if exhausted {
            assert!(matches!(
                delivered.delivery,
                Err(TransportError::Stopped(StopReason::WorkLimit))
            ));
        } else {
            assert!(matches!(delivered.delivery, Err(TransportError::Io(_))));
        }
        assert_eq!(delivered.request_id, request.request_id);
        let OperationReply::Result(OperationResult::Stopped {
            reason,
            partial: Some(partial),
            report,
        }) = delivered.reply
        else {
            return Err("checked Stopped data lost on failed delivery".into());
        };
        assert_eq!(reason, StopReason::Cancelled);
        assert_eq!(partial, request.input);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "retained-stop");
        assert_eq!(report.diagnostics[0].arguments, request.input);
        assert_eq!(execution.poll(), Err(StopReason::Cancelled));
        assert!(server.is_closed());
    }
    Ok(())
}
