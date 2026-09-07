use super::*;
use nepl3_core::origin::{Mapping, MappingKind};
use nepl3_reader::portable::transform::*;
use nepl3_wire::foundation::FoundationCodec;
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn transform_all_outcomes_keep_sources_reports_and_retry_after_rejected_native_or_ndf_reply()
-> Result<(), String> {
    let (registry, schema) = registry().map_err(error)?;
    for decode in [false, true] {
        for branch in 0..3 {
            for portable in [false, true] {
                for abort in [false, true] {
                    let read = signature(&schema, ProviderKind::Read);
                    let transform = signature(&schema, ProviderKind::Transform);
                    let mapped = if decode {
                        ReaderExpr::Decode {
                            provider: transform.operation.clone(),
                            body: ReaderId(1),
                        }
                    } else {
                        ReaderExpr::Map {
                            provider: transform.operation.clone(),
                            body: ReaderId(1),
                        }
                    };
                    let mut p = plan(
                        &schema,
                        vec![
                            ReaderExpr::Call(read.operation.clone()),
                            ReaderExpr::Scalar(CharClass::Any),
                            mapped,
                            ReaderExpr::Seq(vec![ReaderId(0), ReaderId(2)]),
                        ],
                        3,
                        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
                    );
                    p.providers = vec![read, transform];
                    let checked = p.check(&registry, &mut budget()).map_err(error)?;
                    let mut b = budget();
                    let mut session = ReaderSession::new(
                        "transform-terminal".into(),
                        &checked,
                        &registry,
                        &mut b,
                    )
                    .map_err(error)?;
                    let input = source("ab").map_err(error)?;
                    let mut store = SourceStore::default();
                    store.insert(input.clone()).map_err(error)?;
                    let mut admission = SourceAdmission::default();
                    let raw = context(&schema, &registry).map_err(error)?;
                    let context = check_context(&raw, &store, &registry, &mut b, &mut admission)
                        .map_err(error)?;
                    let first = session
                        .read(
                            "entry",
                            ReadRequest {
                                snapshot: &input,
                                start: 0,
                                limit: 2,
                                final_input: true,
                                context: &context,
                                state: &NdfValue::Unit,
                            },
                            &store,
                            &mut b,
                            &mut admission,
                        )
                        .map_err(error)?;
                    let ReadReply::Await { continuation, .. } = first else {
                        return Err("first Await".into());
                    };
                    let prefix =
                        annotated_terminal(&schema, input.span(0, 1).map_err(error)?, 1, &mut b)
                            .map_err(error)?;
                    let wait = session
                        .resume(&continuation, prefix, &store, &mut b, &mut admission)
                        .map_err(error)?;
                    let ReadReply::Await {
                        continuation,
                        report: prefix,
                        ..
                    } = wait
                    else {
                        return Err("transform Await".into());
                    };
                    let generated = admission
                        .create(
                            SourceId("transform-generated".into()),
                            0,
                            "memory:transformed".into(),
                            b"g".to_vec(),
                            &mut b,
                        )
                        .map_err(error)?;
                    let aux = admission
                        .create(
                            SourceId("transform-aux".into()),
                            0,
                            "memory:aux".into(),
                            b"h".to_vec(),
                            &mut b,
                        )
                        .map_err(error)?;
                    let span = generated.span(0, 1).map_err(error)?;
                    let annotated =
                        annotated_terminal(&schema, span.clone(), 2, &mut b).map_err(error)?;
                    let ProviderReply::Read(annotated) = annotated else {
                        return Err("read report".into());
                    };
                    let ReadReply::Matched { mut report, .. } = *annotated else {
                        return Err("matched report".into());
                    };
                    let arguments = report.diagnostics[0].arguments.clone();
                    report.diagnostics[0].related.push(Related {
                        span: Some(aux.span(0, 1).map_err(error)?),
                        code: "aux".into(),
                        arguments,
                    });
                    let outcome = match branch {
                        0 => TransformOutcome::Complete {
                            value: NdfValue::Text("B".into()),
                            view: ViewBundle {
                                elements: vec![],
                                roots: vec![],
                            },
                            facts: vec![ReaderFact::Capture {
                                name: "decoded".into(),
                                span: span.clone(),
                            }],
                        },
                        1 => TransformOutcome::Failed {
                            diagnostic: Box::new(report.diagnostics[0].clone()),
                            recovery: Some(span.clone()),
                        },
                        _ => {
                            report.trace_overflow = Some(TraceOverflow { dropped: 1 });
                            TransformOutcome::Stopped {
                                reason: StopReason::Cancelled,
                            }
                        }
                    };
                    let expected = TransformReply {
                        outcome,
                        sources: vec![aux.clone(), generated.clone()],
                        source_maps: vec![Mapping {
                            source: input.span(1, 2).map_err(error)?,
                            target: span.clone(),
                            kind: MappingKind::Transformed,
                        }],
                        report,
                    };
                    let counts = b.usage();
                    for mutation in 0..4 {
                        let mut invalid = expected.clone();
                        match mutation {
                            0 => invalid.report.diagnostics[0].code.clear(),
                            1 => invalid.sources.push(generated.clone()),
                            2 => {
                                invalid.source_maps[0].target = source("outside")
                                    .map_err(error)?
                                    .span(0, 3)
                                    .map_err(error)?
                            }
                            _ => invalid.report.usage.events = 0,
                        }
                        assert!(
                            session
                                .resume(
                                    &continuation,
                                    ProviderReply::Transform(Box::new(invalid)),
                                    &store,
                                    &mut b,
                                    &mut admission
                                )
                                .is_err(),
                            "decode={decode} branch={branch} mutation={mutation}"
                        );
                        assert_eq!(b.poll(), Ok(()));
                        assert!(session.pending_transform().is_ok());
                        assert_eq!(b.usage().diagnostics, counts.diagnostics);
                        assert_eq!(b.usage().events, counts.events);
                    }
                    if abort {
                        b.cancel();
                        let stopped = session
                            .resume(
                                &continuation,
                                ProviderReply::Transform(Box::new(expected)),
                                &store,
                                &mut b,
                                &mut admission,
                            )
                            .map_err(error)?;
                        let ReadReply::Stopped {
                            reason,
                            report,
                            sources,
                            source_maps,
                        } = stopped
                        else {
                            return Err("cancelled transform".into());
                        };
                        assert_eq!(reason, StopReason::Cancelled);
                        assert_eq!(report.diagnostics, prefix.diagnostics);
                        assert_eq!(report.events, prefix.events);
                        assert!(sources.is_empty());
                        assert!(source_maps.is_empty());
                        assert_eq!(report.usage, b.usage());
                        assert!(matches!(
                            session.pending_transform(),
                            Err(ReaderError::NoPending)
                        ));
                        continue;
                    }
                    let reply = if portable {
                        let proof = session.pending_transform().map_err(error)?;
                        let empty = SourceStore::default();
                        let mut codec = FoundationCodec::new(&registry, &empty, &mut admission)
                            .map_err(error)?;
                        let value =
                            reply_to_value(&expected, &proof, &mut codec, &mut b).map_err(error)?;
                        let bytes = nepl3_wire::encode(&value, &mut b).map_err(error)?;
                        let decoded = nepl3_wire::decode(&bytes, &mut b).map_err(error)?;
                        let mut malformed = decoded.clone();
                        let NdfValue::Record(r) = &mut malformed else {
                            return Err("reply record".into());
                        };
                        let NdfValue::List(sources) = &mut r.fields[1] else {
                            return Err("source list".into());
                        };
                        sources.pop(); // The primary and mapping require the last source.
                        // Neither an ambient source nor a previous source admission grants
                        // permission to repair a missing declaration in this exact reply.
                        assert!(reply_from_value(&malformed, &proof, &mut codec, &mut b).is_err());
                        let reply = reply_from_value(&decoded, &proof, &mut codec, &mut b)
                            .map_err(error)?;
                        assert_eq!(reply, expected);
                        for resource in 0..5 {
                            let mut limits = budget().limits();
                            let reason = match resource {
                                0 => {
                                    limits.work = 0;
                                    StopReason::WorkLimit
                                }
                                1 => {
                                    limits.allocation_units = 0;
                                    StopReason::AllocationLimit
                                }
                                2 => {
                                    limits.depth = 0;
                                    StopReason::DepthLimit
                                }
                                3 => {
                                    limits.source_bytes = 0;
                                    StopReason::SourceLimit
                                }
                                _ => StopReason::Cancelled,
                            };
                            let mut stopped = Budget::new(limits);
                            if resource == 4 {
                                stopped.cancel();
                            }
                            let mut fresh_admission = SourceAdmission::default();
                            let mut fresh_codec =
                                FoundationCodec::new(&registry, &empty, &mut fresh_admission)
                                    .map_err(error)?;
                            assert!(
                                matches!(reply_from_value(&decoded,&proof,&mut fresh_codec,&mut stopped),Err(nepl3_reader::portable::PortableError::Stopped(r)) if r==reason)
                            );
                        }
                        let outer = operation::to_value(&expected, &proof, &mut codec, &mut b)
                            .map_err(error)?;
                        let outer_bytes = nepl3_wire::encode(&outer, &mut b).map_err(error)?;
                        let outer = nepl3_wire::decode(&outer_bytes, &mut b).map_err(error)?;
                        let completion = operation::from_value(&outer, &proof, &mut codec, &mut b)
                            .map_err(error)?;
                        assert_eq!(
                            completion,
                            operation::TransformCompletion::Reply(Box::new(expected.clone()))
                        );
                        let mut changed = outer.clone();
                        let NdfValue::Variant(v) = &mut changed else {
                            return Err("OperationReply".into());
                        };
                        v.fields[if branch == 2 { 3 } else { 2 }] = NdfValue::List(vec![]); // Drop outer events only.
                        assert!(
                            operation::from_value(&changed, &proof, &mut codec, &mut b).is_err()
                        );
                        if branch > 0 {
                            let mut missing = outer.clone();
                            let NdfValue::Variant(v) = &mut missing else {
                                return Err("OperationReply".into());
                            };
                            v.fields[if branch == 2 { 1 } else { 0 }] = NdfValue::None;
                            // Dropping partial also drops its generated source declarations.
                            assert!(
                                operation::from_value(&missing, &proof, &mut codec, &mut b)
                                    .is_err()
                            );
                            let mut rejected_report = expected.report.clone();
                            rejected_report.trace_overflow = None;
                            rejected_report.diagnostics[0].primary =
                                Some(input.span(1, 2).map_err(error)?);
                            rejected_report.diagnostics[0].related.clear();
                            rejected_report.events[0].span = Some(input.span(1, 2).map_err(error)?);
                            use nepl3_core::value_codec::FoundationValueCodec;
                            let encoded_report = {
                                let mut local = codec.scoped(&store);
                                local
                                    .encode_report(&rejected_report, &mut b)
                                    .map_err(error)?
                            };
                            let NdfValue::Record(r) = &encoded_report else {
                                return Err("Report".into());
                            };
                            let NdfValue::Variant(v) = &mut missing else {
                                return Err("OperationReply".into());
                            };
                            v.variant = "Invalid".into();
                            v.fields = vec![
                                NdfValue::None,
                                r.fields[0].clone(),
                                r.fields[1].clone(),
                                r.fields[3].clone(),
                                r.fields[2].clone(),
                            ];
                            let rejected =
                                operation::from_value(&missing, &proof, &mut codec, &mut b)
                                    .map_err(error)?;
                            let operation::TransformCompletion::Rejected(rejected) = rejected
                            else {
                                return Err("dispatch rejection".into());
                            };
                            assert_eq!(rejected.failure, operation::DispatchFailure::Invalid);
                            assert_eq!(rejected.report, rejected_report);
                            assert_eq!(rejected.sources, vec![input.clone()]);
                            assert!(rejected.source_maps.is_empty());
                            for failure in [
                                operation::DispatchFailure::Invalid,
                                operation::DispatchFailure::Stopped(StopReason::WorkLimit),
                            ] {
                                let value = operation::rejection_to_value(
                                    &failure,
                                    &rejected_report,
                                    &proof,
                                    &mut codec,
                                    &mut b,
                                )
                                .map_err(error)?;
                                let result =
                                    operation::from_value(&value, &proof, &mut codec, &mut b)
                                        .map_err(error)?;
                                let operation::TransformCompletion::Rejected(result) = result
                                else {
                                    return Err("dispatch rejection roundtrip".into());
                                };
                                assert_eq!(result.failure, failure);
                                assert_eq!(result.report, rejected_report);
                                assert_eq!(result.sources, vec![input.clone()]);
                            }
                        }
                        reply
                    } else {
                        expected
                    };
                    let result = session
                        .resume(
                            &continuation,
                            ProviderReply::Transform(Box::new(reply)),
                            &store,
                            &mut b,
                            &mut admission,
                        )
                        .map_err(error)?;
                    let (sources, maps, report) = match result {
                        ReadReply::Matched {
                            value,
                            sources,
                            source_maps,
                            report,
                            ..
                        } if branch == 0 => {
                            assert_eq!(
                                value,
                                NdfValue::List(vec![
                                    NdfValue::Text("a".into()),
                                    NdfValue::Text("B".into())
                                ])
                            );
                            (sources, source_maps, report)
                        }
                        ReadReply::Failed {
                            diagnostic,
                            recovery,
                            sources,
                            source_maps,
                            report,
                        } if branch == 1 => {
                            assert_eq!(diagnostic.primary, Some(span.clone()));
                            assert_eq!(recovery, Some(span.clone()));
                            (sources, source_maps, report)
                        }
                        ReadReply::Stopped {
                            reason,
                            sources,
                            source_maps,
                            report,
                        } if branch == 2 => {
                            assert_eq!(reason, StopReason::Cancelled);
                            assert_eq!(report.trace_overflow, Some(TraceOverflow { dropped: 1 }));
                            (sources, source_maps, report)
                        }
                        other => return Err(format!("unexpected {other:?}")),
                    };
                    assert_eq!(sources, vec![aux, generated]);
                    assert_eq!(maps.len(), 1);
                    assert_eq!(report.diagnostics.len(), 2);
                    assert_eq!(report.events.len(), 2);
                    assert_eq!(report.diagnostics[0], prefix.diagnostics[0]);
                    assert_eq!(report.events[0], prefix.events[0]);
                    assert_eq!(report.usage.source_bytes, 4); // Input2 + generated1 + auxiliary1, once.
                    assert_eq!(report.usage.diagnostics, 2);
                    assert_eq!(report.usage.events, 2);
                    assert!(matches!(
                        session.pending_transform(),
                        Err(ReaderError::NoPending)
                    ));
                }
            }
        }
    }
    Ok(())
}
