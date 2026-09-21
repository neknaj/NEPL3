use super::*;
use nepl3_core::diagnostic::validation;
use nepl3_core::diagnostic::*;

fn fixture() -> Result<(SchemaRegistry, SourceStore, Vec<OperationReply>), String> {
    let (registry, request) = setup()?;
    let mut sources = SourceStore::default();
    sources.insert(request.sources[0].clone()).map_err(error)?;
    let span = request.sources[0].span(0, 6).map_err(error)?;
    let report = Report {
        diagnostics: vec![Diagnostic {
            schema: request.operation.schema.clone(),
            code: "example".into(),
            severity: Severity::Warning,
            stage: "test".into(),
            arguments: request.input.clone(),
            primary: Some(span.clone()),
            related: vec![],
            fixes: vec![],
        }],
        events: vec![Event {
            schema: request.operation.schema.clone(),
            kind: "observed".into(),
            operation_path: vec![17],
            span: Some(span),
            payload: request.environment.clone(),
        }],
        usage: Usage {
            work: 29,
            diagnostics: 1,
            events: 1,
            ..Usage::default()
        },
        trace_overflow: None,
    };
    let mut values = vec![
        OperationReply::Result(OperationResult::Complete {
            value: request.input.clone(),
            report: report.clone(),
        }),
        OperationReply::Result(OperationResult::Invalid {
            partial: None,
            report: report.clone(),
        }),
        OperationReply::Result(OperationResult::Invalid {
            partial: Some(request.input.clone()),
            report: report.clone(),
        }),
    ];
    for reason in [
        StopReason::Cancelled,
        StopReason::SourceLimit,
        StopReason::WorkLimit,
        StopReason::DepthLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
        StopReason::DiagnosticLimit,
        StopReason::EventLimit,
    ] {
        let mut stopped_report = report.clone();
        stopped_report.trace_overflow = Some(TraceOverflow { dropped: 2 });
        values.push(OperationReply::Result(OperationResult::Stopped {
            reason,
            partial: Some(request.environment.clone()),
            report: stopped_report,
        }));
    }
    values.push(OperationReply::Await {
        continuation: Continuation {
            provider: request.operation.clone(),
            parent_request: 17,
            snapshot_digest: Digest::of(b"snapshot"),
            state: request.input.clone(),
        },
        calls: vec![request],
        report,
    });
    Ok((registry, sources, values))
}

#[test]
fn reply_cases_preserve_reports_and_await_field_order() -> Result<(), String> {
    let (registry, sources, values) = fixture()?;
    for reply in &values {
        let bytes = encode_reply(
            reply,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        assert_eq!(
            decode_reply(
                &bytes,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .map_err(error)?,
            *reply
        );
        let raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
        let NdfValue::Variant(v) = &raw else {
            return Err("variant".into());
        };
        let (name, diagnostics, events) = match reply {
            OperationReply::Result(OperationResult::Complete { .. }) => ("Complete", 1, 2),
            OperationReply::Result(OperationResult::Invalid { .. }) => ("Invalid", 1, 2),
            OperationReply::Result(OperationResult::Stopped { .. }) => ("Stopped", 2, 3),
            OperationReply::Await { .. } => ("Await", 3, 2),
        };
        assert_eq!(v.type_name, "OperationReply");
        assert_eq!(v.variant, name);
        for (index, kind) in [(diagnostics, "Diagnostic"), (events, "Event")] {
            let NdfValue::List(entries) = &v.fields[index] else {
                return Err("report list".into());
            };
            let NdfValue::Record(entry) = &entries[0] else {
                return Err("report record".into());
            };
            assert_eq!(entry.kind, kind);
        }
    }
    Ok(())
}

#[test]
fn resume_preserves_dependency_order_and_rejects_forged_reply_metadata() -> Result<(), String> {
    let (registry, sources, values) = fixture()?;
    let OperationReply::Await { continuation, .. } = values.last().ok_or("await")? else {
        return Err("await".into());
    };
    let resume = Resume {
        request_id: 17,
        continuation: continuation.clone(),
        dependency_results: values,
    };
    let bytes = encode_resume(
        &resume,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    assert_eq!(
        decode_resume(
            &bytes,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(error)?,
        resume
    );
    let mut raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
    let NdfValue::Record(root) = &mut raw else {
        return Err("resume".into());
    };
    let NdfValue::List(replies) = &mut root.fields[2] else {
        return Err("replies".into());
    };
    let NdfValue::Variant(reply) = &mut replies[0] else {
        return Err("reply".into());
    };
    let NdfValue::Record(usage) = &mut reply.fields[3] else {
        return Err("usage".into());
    };
    // Structurally valid Usage contradicts the one actual diagnostic.
    usage.fields[6] = NdfValue::U64(0);
    let forged = nepl3_wire::encode(&raw, &mut budget()).map_err(error)?;
    assert!(matches!(
        decode_resume(
            &forged,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::Report(validation::ReportValidationError::Usage))
    ));
    Ok(())
}

#[test]
fn reply_source_authority_and_trace_overflow_are_checked() -> Result<(), String> {
    let (registry, sources, values) = fixture()?;
    let empty = SourceStore::default();
    for reply in &values {
        let bytes = encode_reply(
            reply,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        // In particular, a source declared inside an Await call does not grant
        // authority for its outer report, which belongs to the saved request.
        assert!(
            decode_reply(
                &bytes,
                &registry,
                &empty,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err()
        );
        assert!(
            encode_reply(
                reply,
                &registry,
                &empty,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err()
        );
    }
    for index in [0, 1, values.len() - 1] {
        let mut reply = values[index].clone();
        let report = match &mut reply {
            OperationReply::Result(
                OperationResult::Complete { report, .. } | OperationResult::Invalid { report, .. },
            )
            | OperationReply::Await { report, .. } => report,
            _ => return Err("case".into()),
        };
        report.trace_overflow = Some(TraceOverflow { dropped: 1 });
        assert_eq!(
            encode_reply(
                &reply,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget()
            ),
            Err(WireError::InvalidType)
        );
        let bytes = encode_reply(
            &values[index],
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        let mut raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
        let NdfValue::Variant(v) = &mut raw else {
            return Err("reply".into());
        };
        let tail = v.fields.last_mut().ok_or("tail")?;
        *tail = NdfValue::Some(Box::new(NdfValue::Record(Record {
            schema: v.schema.clone(),
            kind: "TraceOverflow".into(),
            fields: vec![NdfValue::U64(1)],
        })));
        let forged = nepl3_wire::encode(&raw, &mut budget()).map_err(error)?;
        assert_eq!(
            decode_reply(
                &forged,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget()
            ),
            Err(WireError::InvalidType)
        );
    }
    let OperationReply::Await { continuation, .. } = values.last().ok_or("await")? else {
        return Err("await".into());
    };
    let resume = Resume {
        request_id: 17,
        continuation: continuation.clone(),
        dependency_results: vec![],
    };
    let bytes = encode_resume(
        &resume,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    let mut stopped = budget();
    stopped.cancel();
    assert_eq!(
        decode_resume(
            &bytes,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut stopped
        ),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(
        encode_resume(
            &resume,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut stopped
        ),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}
