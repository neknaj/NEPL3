use super::*;
use nepl3_reader::portable::read;
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn terminal_read_replies_preserve_fields_and_reject_forged_cursor() -> Result<(), String> {
    for dependent in [false, true] {
        terminal_replies(dependent, false)?;
    }
    Ok(())
}

#[test]
fn terminal_source_closure_is_explicit_provisional_and_metered() -> Result<(), String> {
    for dependent in [false, true] {
        terminal_replies(dependent, true)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn staged_source_roundtrip(
    reply: &ReadReply,
    value: &NdfValue,
    receiving: &read::ReadReplyContext<'_>,
    call: &ProviderCall,
    signature: &ProviderSignature,
    registry: &SchemaRegistry,
    store: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<ReadReply, String> {
    use nepl3_core::{
        operation::{Invoke, ProviderFrame},
        value_codec::FoundationValueCodec,
    };
    use nepl3_reader::portable::{
        PortableError,
        dispatch::{DispatchContext, ProviderInput},
    };
    macro_rules! checked {
        ($v:expr) => {
            $v.map_err(|e| format!("{e:?}"))?
        };
    }
    // Use the real session-issued call and selected signature. This is an
    // in-process codec comparison; native execution supplied trustworthy usage.
    // It does not authenticate a remote execution or transfer a continuation.
    let (id, operation, input, request) = match call {
        ProviderCall::Read {
            call_id,
            operation,
            request,
            ..
        } => (
            *call_id,
            operation,
            ProviderInput::Read(Box::new(request.clone())),
            request,
        ),
        ProviderCall::Dependent {
            call_id,
            operation,
            request,
            ..
        } => (
            *call_id,
            operation,
            ProviderInput::Dependent(Box::new(request.clone())),
            &request.request,
        ),
        _ => return Err("unexpected call".into()),
    };
    let context = DispatchContext {
        signature,
        sources: store,
        mappings: &[],
        registry,
    };
    let (invoke, operation_reply, closure) = {
        let mut codec = checked!(FoundationCodec::new(registry, store, admission));
        let input = checked!(nepl3_reader::portable::dispatch::to_value(
            &input, &context, &mut codec, b
        ));
        let environment = checked!(codec.encode_environment(&request.context.environment, b));
        let NdfValue::Record(environment) = &environment else {
            return Err("environment".into());
        };
        let invoke = Invoke {
            request_id: id,
            operation: operation.clone(),
            input,
            environment: TypedValue::Record(environment.clone()),
            sources: request.sources.clone(),
            resources: vec![],
            limits: b.limits(),
        };
        let operation_reply = checked!(read::operation::to_reply(reply, receiving, &mut codec, b));
        let closure = checked!(read::reply_source_closure(value, receiving, &mut codec, b));
        (invoke, operation_reply, closure)
    };
    let frame = ProviderFrame::Reply {
        request_id: id,
        reply: operation_reply,
    };
    let bytes = checked!(nepl3_wire::operation::encode_frame(
        &frame, registry, &closure, admission, b
    ));
    // The outer diagnostic names the generated snapshot, absent from store.
    assert!(
        nepl3_wire::operation::decode_frame(&bytes, true, registry, store, admission, b).is_err()
    );
    let (pending, rest) = checked!(nepl3_wire::operation::decode_pending_reply_frame(
        &bytes, true, registry, admission, b
    ))
    .ok_or("pending frame")?;
    assert!(rest.is_empty());
    assert_eq!(pending.request_id(), id);
    let mut calls = 0;
    let received = checked!(pending.finish_with(
        &invoke,
        store,
        |payload, registry, original, admission, b| {
            calls += 1;
            let mut codec = FoundationCodec::new(registry, original, admission)
                .map_err(PortableError::Boundary)?;
            read::reply_source_closure(payload.value, receiving, &mut codec, b)
        }
    ));
    assert_eq!(calls, 1);
    assert_eq!(received, frame);
    let ProviderFrame::Reply {
        reply: received, ..
    } = received
    else {
        return Err("Reply".into());
    };
    let mut codec = checked!(FoundationCodec::new(registry, store, admission));
    let OperationResult::Complete {
        value: accepted, ..
    } = checked!(read::operation::from_reply(
        &received, receiving, &mut codec, b
    ))
    else {
        return Err("terminal".into());
    };
    assert_eq!(&accepted, reply);
    // terminal_replies resumes the same still-pending native slot afterward.
    Ok(accepted)
}

#[allow(clippy::too_many_arguments)]
fn source_closure_checks(
    value: &NdfValue,
    case: &str,
    receiving: &read::ReadReplyContext<'_>,
    registry: &SchemaRegistry,
    store: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<(), String> {
    use nepl3_core::value_codec::FoundationValueCodec;
    macro_rules! checked {
        ($v:expr) => {
            $v.map_err(|e| format!("{e:?}"))?
        };
    }
    let mut codec = checked!(FoundationCodec::new(registry, store, admission));
    let closure = checked!(read::reply_source_closure(value, receiving, &mut codec, b));
    assert_eq!(
        closure.snapshots().len(),
        if case == "Matched" { 2 } else { 1 }
    );
    for saved in store.snapshots() {
        assert!(closure.resolve(&saved.reference()).is_some());
    }
    for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
        let mut limits = budget().limits();
        if reason == StopReason::WorkLimit {
            limits.work = 0;
        } else {
            limits.allocation_units = 0;
        }
        let mut stopped = Budget::new(limits);
        assert!(read::reply_source_closure(value, receiving, &mut codec, &mut stopped).is_err());
        assert_eq!(stopped.poll(), Err(reason));
    }
    if case != "Matched" {
        return Ok(());
    }

    // Valid source identities alone say nothing about cursor or mapping validity.
    for field in [1, 6] {
        let mut malformed = value.clone();
        let NdfValue::Variant(v) = &mut malformed else {
            return Err("reply".into());
        };
        v.fields[field] = if field == 1 {
            NdfValue::U64(1)
        } else {
            NdfValue::List(vec![])
        };
        assert!(read::reply_source_closure(&malformed, receiving, &mut codec, b).is_ok());
        assert!(read::reply_from_value(&malformed, receiving, &mut codec, b).is_err());
    }
    let mut forged_usage = value.clone();
    let NdfValue::Variant(v) = &mut forged_usage else {
        return Err("reply".into());
    };
    let NdfValue::Record(report) = &mut v.fields[7] else {
        return Err("report".into());
    };
    let NdfValue::Record(usage) = &mut report.fields[3] else {
        return Err("usage".into());
    };
    usage.fields[1] = NdfValue::U64(u64::MAX);
    assert!(read::reply_source_closure(&forged_usage, receiving, &mut codec, b).is_ok());
    assert!(read::reply_from_value(&forged_usage, receiving, &mut codec, b).is_err());

    for defect in 0..5 {
        let mut malformed = value.clone();
        let NdfValue::Variant(v) = &mut malformed else {
            return Err("reply".into());
        };
        match defect {
            0 => v.schema.digest = nepl3_core::source::Digest::of(b"wrong reader"),
            1 => {
                v.variant = "Await".into();
                v.fields.clear();
            }
            2..=4 => {
                let NdfValue::List(entries) = &mut v.fields[5] else {
                    return Err("sources".into());
                };
                if defect == 2 || defect == 4 {
                    let NdfValue::Record(content) = &mut entries[0] else {
                        return Err("source".into());
                    };
                    if defect == 4 {
                        content.fields[1] = NdfValue::Text("memory:conflicting-locator".into());
                    } else {
                        let NdfValue::Record(reference) = &mut content.fields[0] else {
                            return Err("reference".into());
                        };
                        reference.fields[2] = NdfValue::Bytes(vec![0; 32]);
                    }
                } else {
                    entries.push(entries[0].clone());
                }
            }
            _ => return Err("defect".into()),
        }
        assert!(read::reply_source_closure(&malformed, receiving, &mut codec, b).is_err());
    }

    // Construct a source only on the producer ledger; it has not been admitted
    // to this receiving ledger by fixture setup or reply encoding.
    let mut producer_budget = budget();
    let extra = checked!(SourceSnapshot::new(
        SourceId("zz-new".into()),
        0,
        "memory:zz-new".into(),
        b"new".to_vec(),
        &mut producer_budget
    ));
    let mut producer_admission = SourceAdmission::default();
    let mut producer = checked!(FoundationCodec::new(
        registry,
        store,
        &mut producer_admission
    ));
    let NdfValue::List(ref mut extra_entries) =
        checked!(producer.encode_sources(core::slice::from_ref(&extra), &mut producer_budget))
    else {
        return Err("source list".into());
    };
    let entry = extra_entries.pop().ok_or("source entry")?;
    let mut expanded = value.clone();
    let NdfValue::Variant(v) = &mut expanded else {
        return Err("reply".into());
    };
    let NdfValue::List(entries) = &mut v.fields[5] else {
        return Err("sources".into());
    };
    entries.push(entry);
    let before = b.usage();
    let mut late_failure = expanded.clone();
    let NdfValue::Variant(v) = &mut late_failure else {
        return Err("reply".into());
    };
    let NdfValue::List(entries) = &mut v.fields[5] else {
        return Err("sources".into());
    };
    let mut invalid = entries.last().cloned().ok_or("last source")?;
    let NdfValue::Record(content) = &mut invalid else {
        return Err("source".into());
    };
    let NdfValue::Record(reference) = &mut content.fields[0] else {
        return Err("reference".into());
    };
    reference.fields[0] = NdfValue::Text("zzz-invalid".into());
    reference.fields[2] = NdfValue::Bytes(vec![0; 32]);
    entries.push(invalid);
    assert!(read::reply_source_closure(&late_failure, receiving, &mut codec, b).is_err());
    // The fresh valid prefix is retained despite the later digest rejection.
    assert_eq!(b.usage().source_bytes, before.source_bytes + 3);
    let after_failure = b.usage();
    let expanded_closure = checked!(read::reply_source_closure(
        &expanded, receiving, &mut codec, b
    ));
    assert!(expanded_closure.resolve(&extra.reference()).is_some());
    assert_eq!(b.usage().source_bytes, after_failure.source_bytes);
    let before = b.usage();
    checked!(read::reply_source_closure(
        &expanded, receiving, &mut codec, b
    ));
    assert_eq!(b.usage().source_bytes, before.source_bytes);
    assert!(b.usage().work > before.work);
    assert!(b.usage().allocation_units > before.allocation_units);

    let mut reordered = expanded.clone();
    let NdfValue::Variant(v) = &mut reordered else {
        return Err("reply".into());
    };
    let NdfValue::List(entries) = &mut v.fields[5] else {
        return Err("sources".into());
    };
    entries.swap(0, 1);
    assert!(read::reply_source_closure(&reordered, receiving, &mut codec, b).is_err());

    let total_bytes: u64 = expanded_closure
        .snapshots()
        .iter()
        .map(|s| s.text().len() as u64)
        .sum();
    for exact in [false, true] {
        let mut limits = budget().limits();
        limits.source_bytes = total_bytes - u64::from(!exact);
        let mut limited = Budget::new(limits);
        let mut fresh = SourceAdmission::default();
        let mut fresh_codec = checked!(FoundationCodec::new(registry, store, &mut fresh));
        let result =
            read::reply_source_closure(&expanded, receiving, &mut fresh_codec, &mut limited);
        if exact {
            assert_eq!(
                checked!(result).snapshots().len(),
                expanded_closure.snapshots().len()
            );
        } else {
            assert!(result.is_err());
            assert_eq!(limited.poll(), Err(StopReason::SourceLimit));
        }
    }

    // A source in the codec's ambient store is not part of this saved dispatch.
    let mut ambient = SourceStore::default();
    for saved in store.snapshots() {
        checked!(ambient.insert(saved.clone()));
    }
    checked!(ambient.insert(extra.clone()));
    let mut ambient_admission = SourceAdmission::default();
    let mut ambient_codec = checked!(FoundationCodec::new(
        registry,
        &ambient,
        &mut ambient_admission
    ));
    let original = checked!(read::reply_source_closure(
        value,
        receiving,
        &mut ambient_codec,
        b
    ));
    assert!(original.resolve(&extra.reference()).is_none());
    // The caller still owns the real pending slot and can fully accept a correction.
    checked!(read::reply_from_value(value, receiving, &mut codec, b));
    Ok(())
}

fn terminal_replies(dependent: bool, closure_checks: bool) -> Result<(), String> {
    macro_rules! checked {
        ($value:expr) => {
            $value.map_err(|e| format!("{e:?}"))?
        };
    }
    let (registry, schema) = checked!(registry());
    let p = if dependent {
        let mut signature = signature(&schema, ProviderKind::Dependent);
        signature.value_input = TypeDescriptor::Unit;
        let mut p = plan(
            &schema,
            vec![
                ReaderExpr::Literal("".into()),
                ReaderExpr::Then {
                    first: ReaderId(0),
                    provider: signature.operation.clone(),
                },
            ],
            1,
            TypeDescriptor::Text,
        );
        p.providers.push(signature);
        p
    } else {
        provider_plan(&schema)
    };
    let plan = checked!(p.check(&registry, &mut budget()));
    let mut b = budget();
    let mut session = checked!(ReaderSession::new(
        "read-wire".into(),
        &plan,
        &registry,
        &mut b
    ));
    let source = checked!(source("あ"));
    let mut store = SourceStore::default();
    checked!(store.insert(source.clone()));
    let mut admission = SourceAdmission::default();
    let raw = checked!(context(&schema, &registry));
    let context = checked!(check_context(
        &raw,
        &store,
        &registry,
        &mut b,
        &mut admission
    ));
    let suspended = checked!(session.read(
        "entry",
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: 3,
            final_input: false,
            context: &context,
            state: &NdfValue::Unit,
        },
        &store,
        &mut b,
        &mut admission
    ));
    let ReadReply::Await {
        continuation, call, ..
    } = suspended
    else {
        return Err("await".into());
    };
    let ProviderReply::Read(mut matched) = checked!(terminal("あ", 3, &mut b)) else {
        return Err("read".into());
    };
    let generated = checked!(admission.create(
        SourceId("read-generated".into()),
        0,
        "memory:read-generated".into(),
        b"a".to_vec(),
        &mut b
    ));
    let generated_span = checked!(generated.span(0, 1));
    if let ReadReply::Matched {
        sources,
        source_maps,
        facts,
        view,
        report,
        ..
    } = matched.as_mut()
    {
        sources.push(generated.clone());
        source_maps.push(nepl3_core::origin::Mapping {
            source: checked!(source.span(0, 3)),
            target: generated_span.clone(),
            kind: nepl3_core::origin::MappingKind::Transformed,
        });
        facts.push(ReaderFact::Capture {
            name: "decoded".into(),
            span: generated_span.clone(),
        });
        view.elements.push(ViewElement {
            kind: KindRef {
                schema: schema.clone(),
                local_kind: checked!(registry.kind_id(&schema, "Node")),
            },
            span: generated_span,
            fields: vec![],
            roles: vec![],
            relations: vec![],
        });
        view.roots.push(ViewRef(0));
        if closure_checks {
            checked!(b.charge(Resource::Diagnostics, 1));
            report.diagnostics.push(Diagnostic {
                schema: schema.clone(),
                code: "Failure".into(),
                severity: Severity::Warning,
                stage: "read".into(),
                arguments: TypedValue::Record(Record {
                    schema: schema.clone(),
                    kind: "Node".into(),
                    fields: vec![],
                }),
                primary: Some(checked!(generated.span(0, 1))),
                related: vec![],
                fixes: vec![],
            });
        }
        report.usage = b.usage();
    }
    let report = Report {
        usage: b.usage(),
        ..Report::default()
    };
    let expected = vec![
        Expectation::Literal("あ".into()),
        Expectation::ScalarClass(CharClass::Range {
            lo: 'あ', hi: 'ん'
        }),
        Expectation::EndOfInput,
        Expectation::TokenBoundary,
        Expectation::Provider {
            operation: signature(&schema, ProviderKind::Read).operation,
            arguments: TypedValue::Record(Record {
                schema: schema.clone(),
                kind: "Node".into(),
                fields: vec![],
            }),
        },
    ];
    let no_match = ReadReply::NoMatch {
        expected: expected.clone(),
        furthest: 3,
        sources: vec![],
        source_maps: vec![],
        report: report.clone(),
    };
    let need_more = ReadReply::NeedMore {
        expected,
        sources: vec![],
        source_maps: vec![],
        report: report.clone(),
    };
    let stopped = ReadReply::Stopped {
        reason: StopReason::Cancelled,
        sources: vec![],
        source_maps: vec![],
        report,
    };
    checked!(b.charge(Resource::Diagnostics, 1));
    let diagnostic = Diagnostic {
        schema: schema.clone(),
        code: "Failure".into(),
        severity: Severity::Error,
        stage: "read".into(),
        arguments: TypedValue::Record(Record {
            schema: schema.clone(),
            kind: "Node".into(),
            fields: vec![],
        }),
        primary: Some(checked!(source.span(0, 3))),
        related: vec![],
        fixes: vec![],
    };
    let failed = ReadReply::Failed {
        diagnostic: diagnostic.clone(),
        recovery: Some(checked!(source.span(0, 3))),
        sources: vec![],
        source_maps: vec![],
        report: Report {
            diagnostics: vec![diagnostic.clone()],
            usage: b.usage(),
            ..Report::default()
        },
    };
    let receiving = checked!(session.pending_read());
    let mut received_match = None;
    for (reply, case) in [
        (*matched, "Matched"),
        (no_match, "NoMatch"),
        (need_more, "NeedMore"),
        (stopped, "Stopped"),
        (failed, "Failed"),
    ] {
        let mut codec = checked!(FoundationCodec::new(&registry, &store, &mut admission));
        let value = checked!(read::reply_to_value(&reply, &receiving, &mut codec, &mut b));
        if closure_checks {
            source_closure_checks(
                &value,
                case,
                &receiving,
                &registry,
                &store,
                &mut admission,
                &mut b,
            )?;
            if case == "Matched" {
                received_match = Some(staged_source_roundtrip(
                    &reply,
                    &value,
                    &receiving,
                    &call,
                    p.providers.first().ok_or("provider")?,
                    &registry,
                    &store,
                    &mut admission,
                    &mut b,
                )?);
            }
            continue;
        }

        for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                _ => return Err("unexpected limit".into()),
            }
            let mut stopped = Budget::new(limits);
            assert!(read::reply_to_value(&reply, &receiving, &mut codec, &mut stopped).is_err());
            assert_eq!(stopped.poll(), Err(reason));
            let mut stopped = Budget::new(limits);
            assert!(read::reply_from_value(&value, &receiving, &mut codec, &mut stopped).is_err());
            assert_eq!(stopped.poll(), Err(reason));
        }
        let NdfValue::Variant(encoded) = &value else {
            return Err("variant".into());
        };
        assert_eq!(encoded.variant, case);
        if case == "Matched" {
            assert_eq!(encoded.fields[0], NdfValue::Text("あ".into()));
            assert_eq!(encoded.fields[1], NdfValue::U64(3));
            assert_eq!(encoded.fields[2], NdfValue::Unit);
            let mut forged = value.clone();
            if let NdfValue::Variant(v) = &mut forged {
                v.fields[1] = NdfValue::U64(1);
            }
            assert!(read::reply_from_value(&forged, &receiving, &mut codec, &mut b).is_err());
            let mut undeclared = value.clone();
            if let NdfValue::Variant(v) = &mut undeclared {
                v.fields[5] = NdfValue::List(vec![]);
            }
            assert!(read::reply_from_value(&undeclared, &receiving, &mut codec, &mut b).is_err());
            let mut missing_mapping = value.clone();
            if let NdfValue::Variant(v) = &mut missing_mapping {
                v.fields[6] = NdfValue::List(vec![]);
            }
            assert!(
                read::reply_from_value(&missing_mapping, &receiving, &mut codec, &mut b).is_err()
            );
        }
        let bytes = checked!(nepl3_wire::encode(&value, &mut b));
        let value = checked!(nepl3_wire::decode(&bytes, &mut b));
        let decoded = checked!(read::reply_from_value(
            &value, &receiving, &mut codec, &mut b
        ));
        assert_eq!(decoded, reply);
        let operation = checked!(read::operation::to_reply(
            &reply, &receiving, &mut codec, &mut b
        ));
        let mut transport_admission = SourceAdmission::default();
        let bytes = checked!(nepl3_wire::operation::encode_reply(
            &operation,
            &registry,
            &store,
            &mut transport_admission,
            &mut b
        ));
        let operation = checked!(nepl3_wire::operation::decode_reply(
            &bytes,
            &registry,
            &store,
            &mut transport_admission,
            &mut b
        ));
        let restored = checked!(read::operation::from_reply(
            &operation, &receiving, &mut codec, &mut b
        ));
        match (case, restored) {
            ("Matched" | "NoMatch" | "NeedMore", OperationResult::Complete { value, .. }) => {
                assert_eq!(value, reply)
            }
            (
                "Failed",
                OperationResult::Invalid {
                    partial: Some(value),
                    ..
                },
            ) => assert_eq!(value, reply),
            (
                "Stopped",
                OperationResult::Stopped {
                    reason: StopReason::Cancelled,
                    partial: Some(value),
                    ..
                },
            ) => assert_eq!(value, reply),
            _ => return Err("operation outcome".into()),
        }
        let mut forged = operation.clone();
        if let nepl3_core::operation::OperationReply::Result(
            OperationResult::Complete { report, .. }
            | OperationResult::Invalid { report, .. }
            | OperationResult::Stopped { report, .. },
        ) = &mut forged
        {
            report.usage.work += 1;
        }
        assert!(read::operation::from_reply(&forged, &receiving, &mut codec, &mut b).is_err());
        // The outer outcome must describe the encoded reader outcome exactly.
        // A valid payload and report do not authorize changing its result tag.
        let mut wrong_outcome = operation.clone();
        if let nepl3_core::operation::OperationReply::Result(result) = &mut wrong_outcome {
            *result = match result.clone() {
                OperationResult::Complete { value, report } => OperationResult::Invalid {
                    partial: Some(value),
                    report,
                },
                OperationResult::Invalid {
                    partial: Some(value),
                    report,
                } => OperationResult::Complete { value, report },
                OperationResult::Stopped {
                    partial, report, ..
                } => OperationResult::Stopped {
                    reason: StopReason::WorkLimit,
                    partial,
                    report,
                },
                _ => return Err("missing terminal payload".into()),
            };
        }
        assert!(
            read::operation::from_reply(&wrong_outcome, &receiving, &mut codec, &mut b).is_err()
        );
        if case == "Matched" {
            received_match = Some(decoded);
        }
    }
    {
        use nepl3_core::operation::OperationReply;
        let mut codec = checked!(FoundationCodec::new(&registry, &store, &mut admission));
        for stopped in [false, true] {
            let report = Report {
                usage: b.usage(),
                ..Report::default()
            };
            let result = if stopped {
                OperationResult::Stopped {
                    reason: StopReason::Cancelled,
                    partial: None,
                    report,
                }
            } else {
                OperationResult::Invalid {
                    partial: None,
                    report,
                }
            };
            let envelope = OperationReply::Result(result);
            let decoded = checked!(read::operation::from_reply(
                &envelope, &receiving, &mut codec, &mut b
            ));
            assert!(matches!(
                (stopped, decoded),
                (false, OperationResult::Invalid { partial: None, .. })
                    | (
                        true,
                        OperationResult::Stopped {
                            reason: StopReason::Cancelled,
                            partial: None,
                            ..
                        }
                    )
            ));
            let mut stopped_budget = Budget::new(Limits {
                work: 0,
                ..budget().limits()
            });
            assert!(
                read::operation::from_reply(&envelope, &receiving, &mut codec, &mut stopped_budget)
                    .is_err()
            );
            assert_eq!(stopped_budget.poll(), Err(StopReason::WorkLimit));
            // A failure without a domain partial has no generated-source grant.
            // This source exists locally but is absent from the saved dispatch.
            let mut foreign_report = envelope.clone();
            if let OperationReply::Result(
                OperationResult::Invalid { report, .. } | OperationResult::Stopped { report, .. },
            ) = &mut foreign_report
            {
                let mut foreign_diagnostic = diagnostic.clone();
                foreign_diagnostic.primary = Some(checked!(generated.span(0, 1)));
                report.diagnostics.push(foreign_diagnostic);
            }
            assert!(
                read::operation::from_reply(&foreign_report, &receiving, &mut codec, &mut b)
                    .is_err()
            );
        }
    }
    let reply = ProviderReply::Read(Box::new(received_match.ok_or("missing matched reply")?));
    assert!(matches!(
        checked!(session.resume(&continuation, reply, &store, &mut b, &mut admission)),
        ReadReply::Matched { end: 3, .. }
    ));
    Ok(())
}
