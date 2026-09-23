use super::*;
use nepl3_core::budget::{Limits, Usage};
use nepl3_provider::delegation::{Error, IssuedInvocation};

fn quota(request: &mut Invoke) {
    let limit = request.limits;
    request.limits = Limits {
        source_bytes: limit.source_bytes / 4,
        work: limit.work / 4,
        depth: limit.depth,
        nodes: limit.nodes / 4,
        allocation_units: limit.allocation_units / 4,
        output_bytes: limit.output_bytes / 4,
        diagnostics: limit.diagnostics / 4,
        events: limit.events / 4,
    };
}

#[test]
fn framed_reply_claims_do_not_replace_independent_execution_measurement() -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let approved = grants.admit(&request, &mut budget()).map_err(error)?;
    let implementation = Digest::of(b"metered fixture executable");
    let mut parent = budget();
    let mut issued = IssuedInvocation::issue(
        approved,
        implementation,
        &registry,
        &mut parent,
        &mut budget(),
    )
    .map_err(error)?;
    let context = issued.context();
    let mut remote = Budget::new(request.limits);
    let registration = nepl3_suite::dispatch::suspending::Registration {
        operation: &request.operation,
        implementation,
        invoke: &increment,
    };
    let mut reply = nepl3_suite::dispatch::suspending::invoke(
        &registration,
        implementation,
        &request,
        context,
        &registry,
        &sources,
        &mut remote,
        &mut budget(),
    )
    .map_err(error)?;
    let independently_observed = remote.usage();
    assert!(independently_observed.work > 0);
    let OperationReply::Result(OperationResult::Complete { report, .. }) = &mut reply else {
        return Err("Complete required".into());
    };
    report.usage.work = u64::MAX;
    let mut transport = connection(
        &ProviderFrame::Reply {
            request_id: request.request_id,
            reply,
        },
        &registry,
    )?;
    let admitted = issued
        .run_local(|local| {
            Ok(transport.receive_reply(
                &request,
                context,
                &registry,
                &sources,
                &sources,
                &mut SourceAdmission::default(),
                local,
                &mut budget(),
            ))
        })
        .map_err(error)?
        .map_err(error)?;
    let OperationReply::Result(OperationResult::Complete { report, .. }) = admitted else {
        return Err("Complete required".into());
    };
    assert_eq!(report.usage.work, u64::MAX);
    let before = issued.parent_usage();
    assert!(before.work < request.limits.work);
    issued
        .settle(
            request.request_id,
            implementation,
            context,
            independently_observed,
        )
        .map_err(error)?;
    assert_eq!(
        parent.usage().work,
        before.work + independently_observed.work
    );
    assert_eq!(parent.poll(), Ok(()));
    Ok(())
}

#[test]
fn settlement_rejects_request_implementation_and_context_mismatch() -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let implementation = Digest::of(b"selected executable");
    for changed in 0..3 {
        let mut parent = budget();
        let issued = IssuedInvocation::issue(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation,
            &registry,
            &mut parent,
            &mut budget(),
        )
        .map_err(error)?;
        let context = issued.context();
        let result = issued.settle(
            if changed == 0 {
                request.request_id + 1
            } else {
                request.request_id
            },
            if changed == 1 {
                Digest::of(b"other executable")
            } else {
                implementation
            },
            if changed == 2 {
                Digest::of(b"other context")
            } else {
                context
            },
            Usage::default(),
        );
        assert!(matches!(result, Err(Error::ObservationBinding)));
        assert_eq!(parent.poll(), Err(StopReason::Cancelled));
        assert_eq!(parent.usage(), Usage::default());
    }
    Ok(())
}

#[test]
fn malformed_input_is_rejected_before_reserving_capacity() -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    let TypedValue::Record(input) = &mut request.input else {
        return Err("record required".into());
    };
    input.fields.clear();
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let approved = grants.admit(&request, &mut budget()).map_err(error)?;
    let mut parent = budget();
    assert!(matches!(
        IssuedInvocation::issue(
            approved,
            Digest::of(b"selected executable"),
            &registry,
            &mut parent,
            &mut budget()
        ),
        Err(Error::Input(_))
    ));
    assert_eq!(parent.usage(), Usage::default());
    assert_eq!(parent.poll(), Ok(()));
    Ok(())
}

fn stopped_with_partial(
    request: &Invoke,
    _: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    use nepl3_core::diagnostic::{Diagnostic, Severity};
    let partial = request.input.clone_with_budget(b)?;
    let diagnostic = Diagnostic {
        schema: request.operation.schema.clone(),
        code: "local-child-stopped".into(),
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
fn local_stop_retains_checked_partial_and_report_and_prevents_next_callback() -> Result<(), String>
{
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let implementation = Digest::of(b"stopped local provider");
    let mut parent = budget();
    let mut issued = IssuedInvocation::issue(
        grants.admit(&request, &mut budget()).map_err(error)?,
        implementation,
        &registry,
        &mut parent,
        &mut budget(),
    )
    .map_err(error)?;
    let context = issued.context();
    let registration = nepl3_suite::dispatch::suspending::Registration {
        operation: &request.operation,
        implementation,
        invoke: &stopped_with_partial,
    };
    let Err(failure) = issued.run_local(|local| {
        Ok(nepl3_suite::dispatch::suspending::invoke(
            &registration,
            implementation,
            &request,
            context,
            &registry,
            &sources,
            local,
            &mut budget(),
        ))
    }) else {
        return Err("checked Stopped reply must stop local execution".into());
    };
    assert_eq!(failure.reason, StopReason::Cancelled);
    let Some(Ok(OperationReply::Result(OperationResult::Stopped {
        reason,
        partial: Some(partial),
        report,
    }))) = failure.output
    else {
        return Err("checked partial and report were lost".into());
    };
    assert_eq!(reason, StopReason::Cancelled);
    assert_eq!(partial, request.input);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].code, "local-child-stopped");
    assert_eq!(report.diagnostics[0].arguments, request.input);
    let mut called = false;
    let Err(next) = issued.run_local(|_| {
        called = true;
        Ok(())
    }) else {
        return Err("stop must remain sticky".into());
    };
    assert!(!called, "callback after local stop");
    assert_eq!(next.reason, StopReason::Cancelled);
    assert_eq!(next.output, None);
    let before = issued.parent_usage();
    assert!(matches!(
        issued.settle(
            request.request_id,
            implementation,
            context,
            Usage {
                work: 1,
                ..Usage::default()
            }
        ),
        Err(Error::Settlement(
            nepl3_suite::suspension::delegation::SettlementError::Stopped(StopReason::Cancelled)
        ))
    ));
    assert_eq!(parent.usage().work, before.work + 1);
    assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    Ok(())
}
