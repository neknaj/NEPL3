use super::*;
use nepl3_reader::portable::{read, transform};
use nepl3_wire::foundation::FoundationCodec;
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

// These small test values have a fixed, shallow shape. Change every diagnostic
// echo, not arbitrary matching text, so report equality cannot explain rejection.
fn mutate(value: &mut NdfValue) -> usize {
    let mut count = 0;
    let children = match value {
        NdfValue::Record(record) => {
            if record.kind == "Diagnostic"
                && record.fields.get(1) == Some(&NdfValue::Text("ExpectedInput".into()))
            {
                record.fields[1] = NdfValue::Text("InventedReaderDiagnostic".into());
                count += 1;
            }
            &mut record.fields
        }
        NdfValue::Variant(variant) => &mut variant.fields,
        NdfValue::List(values) => values,
        NdfValue::Some(value) => return mutate(value),
        _ => return 0,
    };
    for child in children {
        count += mutate(child);
    }
    count
}
fn tampered(value: &NdfValue, copies: usize, b: &mut Budget) -> Result<NdfValue, String> {
    let mut value = value.clone();
    assert_eq!(mutate(&mut value), copies);
    let bytes = nepl3_wire::encode(&value, b).map_err(error)?;
    nepl3_wire::decode(&bytes, b).map_err(error)
}

#[test]
fn reader_codes_reject_consistent_native_and_cbor_forgery_before_valid_retry() -> Result<(), String>
{
    let (registry, schema) = registry().map_err(error)?;
    let reader = registry
        .selected("nepl3.reader", 1)
        .ok_or("reader")?
        .clone();
    for case in 0..4 {
        let mut p = provider_plan(&schema);
        if case == 1 {
            let mut provider = signature(&schema, ProviderKind::Dependent);
            provider.value_input = TypeDescriptor::Unit;
            p = plan(
                &schema,
                vec![
                    ReaderExpr::Literal("".into()),
                    ReaderExpr::Then {
                        first: ReaderId(0),
                        provider: provider.operation.clone(),
                    },
                ],
                1,
                TypeDescriptor::Text,
            );
            p.providers.push(provider);
        } else if case >= 2 {
            let provider = signature(&schema, ProviderKind::Transform);
            let expression = if case == 2 {
                ReaderExpr::Map {
                    provider: provider.operation.clone(),
                    body: ReaderId(0),
                }
            } else {
                ReaderExpr::Decode {
                    provider: provider.operation.clone(),
                    body: ReaderId(0),
                }
            };
            p = plan(
                &schema,
                vec![ReaderExpr::Scalar(CharClass::Any), expression],
                1,
                TypeDescriptor::Text,
            );
            p.providers.push(provider);
        }
        let checked = p.check(&registry, &mut budget()).map_err(error)?;
        let mut b = budget();
        let mut session =
            ReaderSession::new("diagnostic-codes".into(), &checked, &registry, &mut b)
                .map_err(error)?;
        let input = source("a").map_err(error)?;
        let mut store = SourceStore::default();
        store.insert(input.clone()).map_err(error)?;
        let mut admission = SourceAdmission::default();
        let raw = context(&schema, &registry).map_err(error)?;
        let ctx = check_context(&raw, &store, &registry, &mut b, &mut admission).map_err(error)?;
        let ReadReply::Await { continuation, .. } = session
            .read(
                "entry",
                ReadRequest {
                    snapshot: &input,
                    start: 0,
                    limit: 1,
                    final_input: true,
                    context: &ctx,
                    state: &NdfValue::Unit,
                },
                &store,
                &mut b,
                &mut admission,
            )
            .map_err(error)?
        else {
            return Err("await".into());
        };
        b.charge(Resource::Diagnostics, 1).map_err(error)?;
        let usage = b.usage();
        let reply = |code: &str| {
            let diagnostic = Diagnostic {
                schema: reader.clone(),
                code: code.into(),
                severity: Severity::Error,
                stage: "reader".into(),
                arguments: TypedValue::Record(Record {
                    schema: reader.clone(),
                    kind: "ReaderDiagnosticArguments".into(),
                    fields: vec![NdfValue::List(vec![]), NdfValue::U64(0)],
                }),
                primary: None,
                related: vec![],
                fixes: vec![],
            };
            let report = Report {
                diagnostics: vec![diagnostic.clone()],
                usage,
                ..Report::default()
            };
            if case < 2 {
                ProviderReply::Read(Box::new(ReadReply::Failed {
                    diagnostic,
                    recovery: None,
                    sources: vec![],
                    source_maps: vec![],
                    report,
                }))
            } else {
                ProviderReply::Transform(Box::new(TransformReply {
                    outcome: TransformOutcome::Failed {
                        diagnostic: Box::new(diagnostic),
                        recovery: None,
                    },
                    sources: vec![],
                    source_maps: vec![],
                    report,
                }))
            }
        };
        assert_eq!(
            session.resume(
                &continuation,
                reply("InventedReaderDiagnostic"),
                &store,
                &mut b,
                &mut admission
            ),
            Err(ReaderError::ProviderContract)
        );
        let good = reply("ExpectedInput");
        {
            let mut codec =
                FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
            match &good {
                ProviderReply::Read(good) => {
                    let proof = session.pending_read().map_err(error)?;
                    let value =
                        read::reply_to_value(good, &proof, &mut codec, &mut b).map_err(error)?;
                    let bad = tampered(&value, 2, &mut b)?;
                    assert!(matches!(
                        read::reply_from_value(&bad, &proof, &mut codec, &mut b),
                        Err(nepl3_reader::portable::PortableError::Reader(
                            ReaderError::ProviderContract
                        ))
                    ));
                    let ReadReply::Failed { report, .. } = good.as_ref() else {
                        return Err("failed fixture".into());
                    };
                    for stopped in [false, true] {
                        let result = if stopped {
                            OperationResult::Stopped {
                                reason: StopReason::Cancelled,
                                partial: None,
                                report: report.clone(),
                            }
                        } else {
                            OperationResult::Invalid {
                                partial: None,
                                report: report.clone(),
                            }
                        };
                        let outer = nepl3_core::operation::OperationReply::Result(result);
                        let mut transport = SourceAdmission::default();
                        let bytes = nepl3_wire::operation::encode_reply(
                            &outer,
                            &registry,
                            &store,
                            &mut transport,
                            &mut b,
                        )
                        .map_err(error)?;
                        let value = nepl3_wire::decode(&bytes, &mut b).map_err(error)?;
                        let bad = tampered(&value, 1, &mut b)?;
                        let bytes = nepl3_wire::encode(&bad, &mut b).map_err(error)?;
                        let outer = nepl3_wire::operation::decode_reply(
                            &bytes,
                            &registry,
                            &store,
                            &mut transport,
                            &mut b,
                        )
                        .map_err(error)?;
                        assert!(matches!(
                            read::operation::from_reply(&outer, &proof, &mut codec, &mut b),
                            Err(nepl3_reader::portable::PortableError::Reader(
                                ReaderError::ProviderContract
                            ))
                        ));
                    }
                    let outer = read::operation::to_reply(good, &proof, &mut codec, &mut b)
                        .map_err(error)?;
                    let mut transport = SourceAdmission::default();
                    let bytes = nepl3_wire::operation::encode_reply(
                        &outer,
                        &registry,
                        &store,
                        &mut transport,
                        &mut b,
                    )
                    .map_err(error)?;
                    let value = nepl3_wire::decode(&bytes, &mut b).map_err(error)?;
                    let bad = tampered(&value, 3, &mut b)?;
                    let bytes = nepl3_wire::encode(&bad, &mut b).map_err(error)?;
                    let outer = nepl3_wire::operation::decode_reply(
                        &bytes,
                        &registry,
                        &store,
                        &mut transport,
                        &mut b,
                    )
                    .map_err(error)?;
                    assert!(matches!(
                        read::operation::from_reply(&outer, &proof, &mut codec, &mut b),
                        Err(nepl3_reader::portable::PortableError::Reader(
                            ReaderError::ProviderContract
                        ))
                    ));
                }
                ProviderReply::Transform(good) => {
                    let proof = session.pending_transform().map_err(error)?;
                    let value = transform::reply_to_value(good, &proof, &mut codec, &mut b)
                        .map_err(error)?;
                    let bad = tampered(&value, 2, &mut b)?;
                    assert!(matches!(
                        transform::reply_from_value(&bad, &proof, &mut codec, &mut b),
                        Err(nepl3_reader::portable::PortableError::Reader(
                            ReaderError::ProviderContract
                        ))
                    ));
                    for failure in [
                        transform::operation::DispatchFailure::Invalid,
                        transform::operation::DispatchFailure::Stopped(StopReason::Cancelled),
                    ] {
                        let value = transform::operation::rejection_to_value(
                            &failure,
                            &good.report,
                            &proof,
                            &mut codec,
                            &mut b,
                        )
                        .map_err(error)?;
                        let bad = tampered(&value, 1, &mut b)?;
                        assert!(matches!(
                            transform::operation::from_value(&bad, &proof, &mut codec, &mut b),
                            Err(nepl3_reader::portable::PortableError::Reader(
                                ReaderError::ProviderContract
                            ))
                        ));
                    }
                    let value = transform::operation::to_value(good, &proof, &mut codec, &mut b)
                        .map_err(error)?;
                    let bad = tampered(&value, 3, &mut b)?;
                    assert!(matches!(
                        transform::operation::from_value(&bad, &proof, &mut codec, &mut b),
                        Err(nepl3_reader::portable::PortableError::Reader(
                            ReaderError::ProviderContract
                        ))
                    ));
                }
            }
        }
        assert!(matches!(
            session
                .resume(&continuation, good, &store, &mut b, &mut admission)
                .map_err(error)?,
            ReadReply::Failed { .. }
        ));
    }
    Ok(())
}
