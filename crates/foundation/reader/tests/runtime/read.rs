use super::*;
use nepl3_reader::portable::read;
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn terminal_read_replies_preserve_fields_and_reject_forged_cursor() -> Result<(), String> {
    for dependent in [false, true] {
        terminal_replies(dependent)?;
    }
    Ok(())
}

fn terminal_replies(dependent: bool) -> Result<(), String> {
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
    let ReadReply::Await { continuation, .. } = suspended else {
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
            diagnostics: vec![diagnostic],
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
        }
    }
    let reply = ProviderReply::Read(Box::new(received_match.ok_or("missing matched reply")?));
    assert!(matches!(
        checked!(session.resume(&continuation, reply, &store, &mut b, &mut admission)),
        ReadReply::Matched { end: 3, .. }
    ));
    Ok(())
}
