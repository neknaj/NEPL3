use super::*;
use crate::echo_projection::Projection;
use nepl3_reader::{builtin::BuiltinReader, portable::PortableError, tokenizer::*};
use nepl3_wire::foundation::FoundationCodec;
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn response(kind: ProviderKind, b: &mut Budget, valid: bool) -> Result<ProviderReply, String> {
    if kind == ProviderKind::Transform {
        Ok(ProviderReply::Transform(Box::new(TransformReply {
            outcome: TransformOutcome::Complete {
                value: if valid {
                    NdfValue::Text("mapped".into())
                } else {
                    NdfValue::Bool(false)
                },
                view: ViewBundle {
                    elements: vec![],
                    roots: vec![],
                },
                facts: vec![],
            },
            sources: vec![],
            source_maps: vec![],
            report: Report {
                usage: b.usage(),
                ..Report::default()
            },
        })))
    } else {
        let mut reply = terminal("ab", 3, b).map_err(err)?;
        if !valid {
            let ProviderReply::Read(r) = &mut reply else {
                return Err("Read".into());
            };
            let ReadReply::Matched { value, .. } = r.as_mut() else {
                return Err("Matched".into());
            };
            *value = NdfValue::Bool(false);
        }
        Ok(reply)
    }
}
#[test]
fn canonical_tokenizer_echo_binds_outer_and_all_inner_fields_for_three_provider_kinds()
-> Result<(), String> {
    for projection in [Projection::Continuation, Projection::Reply] {
        canonical_tokenizer_echo_binds_outer_and_all_inner_fields_for_three_provider_kinds_case(
            projection,
        )?;
    }
    Ok(())
}
fn canonical_tokenizer_echo_binds_outer_and_all_inner_fields_for_three_provider_kinds_case(
    projection: Projection,
) -> Result<(), String> {
    let (registry, schema) = registry().map_err(err)?;
    for kind in [
        ProviderKind::Read,
        ProviderKind::Transform,
        ProviderKind::Dependent,
    ] {
        let sig = signature(&schema, kind);
        let root = match kind {
            ProviderKind::Read => ReaderExpr::Call(sig.operation.clone()),
            ProviderKind::Transform => ReaderExpr::Map {
                provider: sig.operation.clone(),
                body: ReaderId(0),
            },
            ProviderKind::Dependent => ReaderExpr::Then {
                provider: sig.operation.clone(),
                first: ReaderId(0),
            },
        };
        let mut p = plan(
            &schema,
            vec![ReaderExpr::Scalar(CharClass::Any), root],
            1,
            TypeDescriptor::Text,
        );
        p.providers.push(sig);
        let checked = p.check(&registry, &mut budget()).map_err(err)?;
        let modes = vec![ReaderMode {
            name: "test".into(),
            skip: vec![SkipRule {
                reader: TokenReader::Builtin(BuiltinReader::Trivia),
            }],
            take: vec![TakeRule {
                reader: TokenReader::Rule("entry".into()),
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
            }],
        }];
        let input = source(" ab").map_err(err)?;
        let mut store = SourceStore::default();
        store.insert(input.clone()).map_err(err)?;
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let mut session = TokenizationSession::new(
            "tokenizer-value".into(),
            &modes,
            &checked,
            &registry,
            &mut b,
        )
        .map_err(err)?;
        assert!(matches!(
            session.pending_read(),
            Err(ReaderError::NoPending)
        ));
        assert!(matches!(
            session.pending_transform(),
            Err(ReaderError::NoPending)
        ));
        let wait = {
            let raw = context(&schema, &registry).map_err(err)?;
            let ctx = check_context(&raw, &store, &registry, &mut b, &mut a).map_err(err)?;
            session
                .read(
                    TokenizationRequest {
                        snapshot: &input,
                        start: 0,
                        limit: 3,
                        final_input: true,
                        context: &ctx,
                        state: &NdfValue::Unit,
                    },
                    &store,
                    &mut b,
                    &mut a,
                )
                .map_err(err)?
        };
        let TokenizationOutcome::Await { continuation, .. } = wait.outcome else {
            return Err("Await".into());
        };
        let TokenizationWait::Provider {
            continuation: inner,
        } = &continuation.pending
        else {
            return Err("Provider".into());
        };
        assert!(continuation.usage.work > inner.usage.work);
        assert_eq!(continuation.reader_schema, schema);
        assert_eq!(wait.trivia.len(), 1);
        let mut c = FoundationCodec::new(&registry, &store, &mut a).map_err(err)?;
        let echo = projection.pending(&session, &mut c, &mut b).map_err(err)?;
        if matches!(projection, Projection::Reply) {
            crate::echo_projection::check_reply(&echo)?;
            for wrong in crate::echo_projection::changed_fields(&echo)? {
                let target = &mut b;
                assert!(matches!(
                    projection.resume(
                        &mut session,
                        &wrong,
                        response(kind, &mut budget(), true)?,
                        &store,
                        &mut c,
                        target
                    ),
                    Err(PortableError::Reader(ReaderError::Continuation))
                ));
                assert!(projection.pending(&session, &mut c, &mut b).is_ok());
            }
            for wrong in crate::echo_projection::malformed_headers(&echo)? {
                let mut cancelled = Budget::new(b.limits());
                cancelled.record_observed_usage(b.usage()).map_err(err)?;
                cancelled.cancel();
                let target = &mut cancelled;
                assert!(matches!(
                    projection.resume(
                        &mut session,
                        &wrong,
                        response(kind, &mut budget(), true)?,
                        &store,
                        &mut c,
                        target
                    ),
                    Err(PortableError::Reader(ReaderError::Continuation))
                ));
                assert!(projection.pending(&session, &mut c, &mut b).is_ok());
            }
            let mut measure = budget();
            session
                .pending_continuation_value(&mut c, &mut measure)
                .map_err(err)?;
            let mut reply_only_stop = Budget::new(Limits {
                work: measure.usage().work,
                ..budget().limits()
            });
            assert!(
                session
                    .pending_reply_value(&mut c, &mut reply_only_stop)
                    .is_err()
            );
            assert_eq!(reply_only_stop.poll(), Err(StopReason::WorkLimit));
            assert_eq!(
                projection.pending(&session, &mut c, &mut b).map_err(err)?,
                echo
            );
        }
        let NdfValue::Record(root) = projection.inner(&echo)? else {
            return Err("record".into());
        };
        assert_eq!(root.schema.package, "nepl3.reader");
        let NdfValue::Record(language) = &root.fields[2] else {
            return Err("plan schema".into());
        };
        assert_eq!(language.fields[0], NdfValue::Text("test".into()));
        let NdfValue::Record(outer_usage) = &root.fields[15] else {
            return Err("outer usage".into());
        };
        assert_eq!(
            outer_usage.fields[1],
            NdfValue::U64(continuation.usage.work)
        );
        let NdfValue::Record(outer_report) = &root.fields[16] else {
            return Err("outer report".into());
        };
        let NdfValue::Record(outer_report_usage) = &outer_report.fields[3] else {
            return Err("outer report usage".into());
        };
        assert_eq!(
            outer_report_usage.fields[1],
            NdfValue::U64(continuation.usage.work)
        );
        let NdfValue::Variant(pending) = &root.fields[13] else {
            return Err("wait".into());
        };
        let NdfValue::Record(inner_value) = &pending.fields[0] else {
            return Err("inner".into());
        };
        let NdfValue::Record(inner_usage) = &inner_value.fields[8] else {
            return Err("inner usage".into());
        };
        assert_eq!(inner_usage.fields[1], NdfValue::U64(inner.usage.work));
        let NdfValue::Record(inner_report) = &inner_value.fields[9] else {
            return Err("inner report".into());
        };
        let NdfValue::Record(inner_report_usage) = &inner_report.fields[3] else {
            return Err("inner report usage".into());
        };
        assert_eq!(
            inner_report_usage.fields[1],
            NdfValue::U64(inner.usage.work)
        );
        let echo = nepl3_wire::decode(&nepl3_wire::encode(&echo, &mut b).map_err(err)?, &mut b)
            .map_err(err)?;
        assert_eq!(
            projection.pending(&session, &mut c, &mut b).map_err(err)?,
            echo
        );
        for field in 0..27 {
            let mut wrong = echo.clone();
            let NdfValue::Record(root) = projection.inner_mut(&mut wrong)? else {
                return Err("root".into());
            };
            if field < 17 {
                root.fields[field] = NdfValue::Unit;
            } else {
                let NdfValue::Variant(pending) = &mut root.fields[13] else {
                    return Err("wait".into());
                };
                let NdfValue::Record(inner) = &mut pending.fields[0] else {
                    return Err("inner".into());
                };
                inner.fields[field - 17] = NdfValue::Unit;
            }
            let reply = response(kind, &mut b, true)?;
            assert!(matches!(
                projection.resume(&mut session, &wrong, reply, &store, &mut c, &mut b),
                Err(PortableError::Reader(ReaderError::Continuation))
            ));
            assert_eq!(
                projection.pending(&session, &mut c, &mut b).map_err(err)?,
                echo
            );
        }
        let reservation = SourceReservation {
            source_id: SourceId("must-not-create".into()),
            revision: 0,
            uri: "memory:must-not-create".into(),
        };
        assert!(matches!(
            projection.reserve(&mut session, &echo, &reservation, &store, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::Continuation))
        ));
        let mut foreign = Budget::new(b.limits());
        foreign.cancel();
        let reply = response(kind, &mut b, true)?;
        assert!(matches!(
            projection.resume(&mut session, &echo, reply, &store, &mut c, &mut foreign),
            Err(PortableError::Reader(ReaderError::Continuation))
        ));
        let bad = response(kind, &mut b, false)?;
        assert!(
            projection
                .resume(&mut session, &echo, bad, &store, &mut c, &mut b)
                .is_err()
        );
        assert_eq!(
            projection.pending(&session, &mut c, &mut b).map_err(err)?,
            echo
        );
        super::tokenizer_context::rejection_retains(&session, kind, &mut c, &mut b)?;
        let good = response(kind, &mut b, true)?;
        let mut baseline = response(kind, &mut b, true)?;
        match &mut baseline {
            ProviderReply::Read(r) => {
                let ReadReply::Matched { report, .. } = r.as_mut() else {
                    return Err("Matched".into());
                };
                report.usage = inner.usage;
            }
            ProviderReply::Transform(r) => r.report.usage = inner.usage,
        }
        let _ = super::tokenizer_context::roundtrip(&session, baseline, &mut c, &mut b)?;
        if kind != ProviderKind::Transform {
            let depth = match &inner.pending {
                ProviderCall::Read { depth_base, .. }
                | ProviderCall::Dependent { depth_base, .. } => *depth_base,
                _ => return Err("read kind".into()),
            };
            assert_eq!(
                session
                    .pending_read()
                    .map_err(err)?
                    .saved_depth()
                    .map_err(err)?,
                depth
            );
        }
        let good = super::tokenizer_context::roundtrip(&session, good, &mut c, &mut b)?;
        let done = projection
            .resume(&mut session, &echo, good, &store, &mut c, &mut b)
            .map_err(err)?;
        assert!(matches!(done.outcome, TokenizationOutcome::Token(_)));
        assert!(matches!(
            session.pending_read(),
            Err(ReaderError::NoPending)
        ));
        assert!(matches!(
            session.pending_transform(),
            Err(ReaderError::NoPending)
        ));
        assert_eq!(done.trivia, wait.trivia);
        let reply = response(kind, &mut b, true)?;
        assert!(matches!(
            projection.resume(&mut session, &echo, reply, &store, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::NoPending))
        ));
    }
    Ok(())
}

fn resume_value(
    projection: Projection,
    session: &mut TokenizationSession<'_>,
    reply: ProviderReply,
    store: &SourceStore,
    registry: &SchemaRegistry,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<TokenizationReply, String> {
    let mut c = FoundationCodec::new(registry, store, a).map_err(err)?;
    let reply = super::tokenizer_context::roundtrip(session, reply, &mut c, b)?;
    let echo = projection.pending(session, &mut c, b).map_err(err)?;
    let echo = nepl3_wire::decode(&nepl3_wire::encode(&echo, b).map_err(err)?, b).map_err(err)?;
    projection
        .resume(session, &echo, reply, store, &mut c, b)
        .map_err(err)
}

#[test]
fn provider_echo_stops_keep_mapped_skip_prefix_and_release_inner_pending() -> Result<(), String> {
    for projection in [Projection::Continuation, Projection::Reply] {
        provider_echo_stops_keep_mapped_skip_prefix_and_release_inner_pending_case(projection)?;
    }
    Ok(())
}
fn provider_echo_stops_keep_mapped_skip_prefix_and_release_inner_pending_case(
    projection: Projection,
) -> Result<(), String> {
    use nepl3_core::origin::{Mapping, MappingKind};
    let (registry, schema) = registry().map_err(err)?;
    let mut p = provider_plan(&schema);
    p.state_type = TypeDescriptor::NdfValue;
    for provider in &mut p.providers {
        provider.state_type = TypeDescriptor::NdfValue;
    }
    let checked = p.check(&registry, &mut budget()).map_err(err)?;
    let modes = vec![ReaderMode {
        name: "test".into(),
        skip: vec![SkipRule {
            reader: TokenReader::Rule("entry".into()),
        }],
        take: vec![TakeRule {
            reader: TokenReader::Rule("entry".into()),
            kind: KindRef {
                schema: schema.clone(),
                local_kind: 0,
            },
        }],
    }];
    let input = source(" ab").map_err(err)?;
    let mut store = SourceStore::default();
    store.insert(input.clone()).map_err(err)?;
    let ambient = SourceSnapshot::new(
        SourceId("ambient-only".into()),
        0,
        "memory:ambient-only".into(),
        b"q".to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    store.insert(ambient.clone()).map_err(err)?;
    for mode in 0..8 {
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let mut session = TokenizationSession::new(
            "mapped-token-echo".into(),
            &modes,
            &checked,
            &registry,
            &mut b,
        )
        .map_err(err)?;
        let wait = {
            let raw = context(&schema, &registry).map_err(err)?;
            let ctx = check_context(&raw, &store, &registry, &mut b, &mut a).map_err(err)?;
            session
                .read(
                    TokenizationRequest {
                        snapshot: &input,
                        start: 0,
                        limit: 3,
                        final_input: true,
                        context: &ctx,
                        state: &NdfValue::Unit,
                    },
                    &store,
                    &mut b,
                    &mut a,
                )
                .map_err(err)?
        };
        let TokenizationOutcome::Await { continuation, .. } = wait.outcome else {
            return Err("Skip Await".into());
        };
        assert!(matches!(continuation.phase, TokenizationPhase::Skip { .. }));
        let generated = a
            .create(
                SourceId("token-prefix".into()),
                0,
                "memory:token-prefix".into(),
                b"g".to_vec(),
                &mut b,
            )
            .map_err(err)?;
        let span = generated.span(0, 1).map_err(err)?;
        let mut prefix = annotated_terminal(&schema, span.clone(), 1, &mut b).map_err(err)?;
        let ProviderReply::Read(r) = &mut prefix else {
            return Err("Read".into());
        };
        let ReadReply::Matched {
            sources,
            source_maps,
            view,
            facts,
            new_state,
            ..
        } = r.as_mut()
        else {
            return Err("Matched".into());
        };
        *new_state = NdfValue::Text("accepted-skip-state".into());
        sources.push(generated);
        source_maps.push(Mapping {
            source: input.span(0, 1).map_err(err)?,
            target: span.clone(),
            kind: MappingKind::Transformed,
        });
        view.elements = vec![
            ViewElement {
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
                span: input.span(0, 1).map_err(err)?,
                fields: vec![ViewField {
                    name: "mapped".into(),
                    children: vec![ViewRef(1)],
                }],
                roles: vec![],
                relations: vec![],
            },
            ViewElement {
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
                span,
                fields: vec![],
                roles: vec![],
                relations: vec![],
            },
        ];
        view.roots = vec![ViewRef(0)];
        facts.push(ReaderFact::Capture {
            name: "prefix".into(),
            span: input.span(0, 1).map_err(err)?,
        });
        let wait = resume_value(
            projection,
            &mut session,
            prefix,
            &store,
            &registry,
            &mut b,
            &mut a,
        )?;
        assert!(matches!(wait.outcome, TokenizationOutcome::Await { .. }));
        let miss = ProviderReply::Read(Box::new(ReadReply::NoMatch {
            expected: vec![],
            furthest: 1,
            sources: vec![],
            source_maps: vec![],
            report: Report {
                usage: b.usage(),
                ..Report::default()
            },
        }));
        let wait = resume_value(
            projection,
            &mut session,
            miss,
            &store,
            &registry,
            &mut b,
            &mut a,
        )?;
        let TokenizationOutcome::Await { continuation, .. } = &wait.outcome else {
            return Err("Take Await".into());
        };
        assert!(matches!(continuation.phase, TokenizationPhase::Take { .. }));
        assert_eq!(
            wait.new_state,
            Some(NdfValue::Text("accepted-skip-state".into()))
        );
        assert_eq!(
            continuation.current.state,
            NdfValue::Text("accepted-skip-state".into())
        );
        assert_eq!(continuation.request.state, NdfValue::Unit);
        assert_eq!(wait.trivia.len(), 1);
        assert_eq!(wait.sources.len(), 1);
        assert_eq!(wait.source_maps.len(), 1);
        assert_eq!(wait.facts.len(), 1);
        assert_eq!(wait.report.diagnostics.len(), 1);
        assert_eq!(wait.report.events.len(), 1);
        let mut c = FoundationCodec::new(&registry, &store, &mut a).map_err(err)?;
        let mut echo = projection.pending(&session, &mut c, &mut b).map_err(err)?;
        if matches!(projection, Projection::Reply) {
            crate::echo_projection::check_reply(&echo)?;
            let NdfValue::Record(r) = &echo else {
                return Err("reply".into());
            };
            assert_eq!(
                r.fields[2],
                NdfValue::Some(Box::new(NdfValue::Text("accepted-skip-state".into())))
            );
            assert_eq!(b.usage().diagnostics, wait.report.usage.diagnostics);
            assert_eq!(b.usage().events, wait.report.usage.events);
        }
        // A fresh provider diagnostic/event can refer to the saved prefix.
        // Use a separate codec-check Budget with observed history here: this
        // candidate is inspected, not resumed or emitted by the actual session.
        let mut checking = Budget::new(b.limits());
        checking.record_observed_usage(b.usage()).map_err(err)?;
        let source_reply = annotated_terminal(
            &schema,
            wait.sources[0].span(0, 1).map_err(err)?,
            3,
            &mut checking,
        )
        .map_err(err)?;
        let source_reply =
            super::tokenizer_context::roundtrip(&session, source_reply, &mut c, &mut checking)?;
        let ProviderReply::Read(mut outside) = source_reply else {
            return Err("read".into());
        };
        let ReadReply::Matched { report, .. } = outside.as_mut() else {
            return Err("matched".into());
        };
        report.diagnostics[0].primary = Some(ambient.span(0, 1).map_err(err)?);
        let proof = session.pending_read().map_err(err)?;
        assert!(
            nepl3_reader::portable::read::reply_to_value(&outside, &proof, &mut c, &mut checking)
                .is_err()
        );
        assert!(session.pending_read().is_ok());
        // Check the native reply-outcome gate before the tampered echo path.
        let mut forged = echo.clone();
        let NdfValue::Record(root) = projection.inner_mut(&mut forged)? else {
            return Err("record".into());
        };
        root.fields[3] = NdfValue::Unit;
        let malformed = ProviderReply::Read(Box::new(ReadReply::Matched {
            value: NdfValue::Text("ab".into()),
            end: 3,
            new_state: NdfValue::Unit,
            view: ViewBundle {
                elements: vec![],
                roots: vec![],
            },
            facts: vec![],
            sources: vec![],
            source_maps: vec![],
            report: Report {
                trace_overflow: Some(TraceOverflow { dropped: 1 }),
                usage: b.usage(),
                ..Report::default()
            },
        }));
        assert!(matches!(
            projection.resume(&mut session, &forged, malformed, &store, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::ProviderContract))
        ));
        assert_eq!(
            projection.pending(&session, &mut c, &mut b).map_err(err)?,
            echo
        );
        let preparation = if mode == 3 {
            let before = b.usage().work;
            let _ = projection.pending(&session, &mut c, &mut b).map_err(err)?;
            b.usage().work - before
        } else {
            0
        };
        let comparison = if mode == 3 {
            let mut measure = budget();
            assert!(echo.equal_with_budget(&echo, &mut measure).map_err(err)?);
            measure.usage().work
        } else {
            0
        };
        if mode == 3 {
            let NdfValue::Record(root) = projection.inner_mut(&mut echo)? else {
                return Err("record".into());
            };
            let NdfValue::Variant(wait) = &mut root.fields[13] else {
                return Err("wait".into());
            };
            let NdfValue::Record(inner) = &mut wait.fields[0] else {
                return Err("inner".into());
            };
            let NdfValue::Record(report) = &mut inner.fields[9] else {
                return Err("report".into());
            };
            report.schema.package = "hostile-nested-metadata".repeat(10_000);
        }
        let good = terminal("ab", 3, &mut b).map_err(err)?;
        if mode == 7 && matches!(projection, Projection::Reply) {
            let NdfValue::Record(r) = &mut echo else {
                return Err("reply".into());
            };
            r.fields[1] = NdfValue::U64(u64::MAX);
        }
        let reason = match mode {
            0 | 7 => {
                b.cancel();
                StopReason::Cancelled
            }
            1 => {
                b.charge(Resource::Work, b.limits().work - b.usage().work)
                    .map_err(err)?;
                StopReason::WorkLimit
            }
            2 => {
                b.charge(
                    Resource::AllocationUnits,
                    b.limits().allocation_units - b.usage().allocation_units,
                )
                .map_err(err)?;
                StopReason::AllocationLimit
            }
            3 => {
                b.charge(
                    Resource::Work,
                    b.limits().work - b.usage().work - preparation - comparison - 100,
                )
                .map_err(err)?;
                StopReason::WorkLimit
            }
            4 => {
                b.charge(Resource::Nodes, b.limits().nodes - b.usage().nodes)
                    .map_err(err)?;
                StopReason::NodeLimit
            }
            5 => StopReason::DepthLimit,
            _ => {
                b.charge(
                    Resource::OutputBytes,
                    b.limits().output_bytes - b.usage().output_bytes,
                )
                .map_err(err)?;
                assert!(nepl3_wire::encode(&echo, &mut b).is_err());
                StopReason::OutputLimit
            }
        };
        if mode == 2 {
            let proof = session.pending_read().map_err(err)?;
            let ProviderReply::Read(r) = &good else {
                return Err("read".into());
            };
            assert!(
                nepl3_reader::portable::read::reply_to_value(r, &proof, &mut c, &mut b).is_err()
            );
            assert!(session.pending_read().is_ok());
        }
        let before_resume = b.usage().work;
        let stopped = if mode == 0 {
            let mut foreign = echo.clone();
            let NdfValue::Record(r) = projection.inner_mut(&mut foreign)? else {
                return Err("record".into());
            };
            r.kind = "ForeignContinuation".into();
            let reservation = SourceReservation {
                source_id: SourceId("unused".into()),
                revision: 0,
                uri: "memory:unused".into(),
            };
            for _ in 0..2 {
                assert!(matches!(
                    projection.reserve(
                        &mut session,
                        &foreign,
                        &reservation,
                        &store,
                        &mut c,
                        &mut b
                    ),
                    Err(PortableError::Reader(ReaderError::Continuation))
                ));
            }
            projection
                .reserve(&mut session, &echo, &reservation, &store, &mut c, &mut b)
                .map_err(err)?
        } else if mode == 5 {
            b.with_depth_at_least(b.limits().depth, |b| {
                projection.resume(&mut session, &echo, good, &store, &mut c, b)
            })
            .map_err(err)?
        } else {
            projection
                .resume(&mut session, &echo, good, &store, &mut c, &mut b)
                .map_err(err)?
        };
        if mode == 3 {
            assert!(b.usage().work - before_resume > preparation);
        }
        assert!(
            matches!(stopped.outcome,TokenizationOutcome::Stopped{reason:actual} if actual==reason)
        );
        assert_eq!(stopped.trivia, wait.trivia);
        assert_eq!(stopped.sources, wait.sources);
        assert_eq!(stopped.source_maps, wait.source_maps);
        assert_eq!(stopped.facts, wait.facts);
        assert_eq!(stopped.report.diagnostics, wait.report.diagnostics);
        assert_eq!(stopped.report.events, wait.report.events);
        assert!(matches!(
            projection.pending(&session, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::NoPending))
        ));
        drop(c);
        let mut next = budget();
        let mut a = SourceAdmission::default();
        let new_wait = {
            let raw = context(&schema, &registry).map_err(err)?;
            let ctx = check_context(&raw, &store, &registry, &mut next, &mut a).map_err(err)?;
            session
                .read(
                    TokenizationRequest {
                        snapshot: &input,
                        start: 0,
                        limit: 3,
                        final_input: true,
                        context: &ctx,
                        state: &NdfValue::Unit,
                    },
                    &store,
                    &mut next,
                    &mut a,
                )
                .map_err(err)?
        };
        assert!(matches!(
            new_wait.outcome,
            TokenizationOutcome::Await { .. }
        ));
        session.discard_pending();
        assert!(matches!(
            session.pending_read(),
            Err(ReaderError::NoPending)
        ));
        assert!(matches!(
            session.pending_transform(),
            Err(ReaderError::NoPending)
        ));
        session.close();
        assert!(matches!(session.pending_read(), Err(ReaderError::Closed)));
        assert!(matches!(
            session.pending_transform(),
            Err(ReaderError::Closed)
        ));
    }
    Ok(())
}

#[test]
fn nested_reader_projection_preserves_mapped_views_with_distinct_checkpoints() -> Result<(), String>
{
    for projection in [Projection::Continuation, Projection::Reply] {
        nested_reader_projection_preserves_mapped_views_with_distinct_checkpoints_case(projection)?;
    }
    Ok(())
}
fn nested_reader_projection_preserves_mapped_views_with_distinct_checkpoints_case(
    projection: Projection,
) -> Result<(), String> {
    use nepl3_core::origin::{Mapping, MappingKind};
    let (registry, schema) = registry().map_err(err)?;
    let mut sig = signature(&schema, ProviderKind::Read);
    sig.state_type = TypeDescriptor::NdfValue;
    let mut p = plan(
        &schema,
        vec![
            ReaderExpr::Call(sig.operation.clone()),
            ReaderExpr::Seq(vec![ReaderId(0), ReaderId(0)]),
        ],
        1,
        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    );
    p.state_type = TypeDescriptor::NdfValue;
    p.providers.push(sig);
    let checked = p.check(&registry, &mut budget()).map_err(err)?;
    let modes = vec![ReaderMode {
        name: "test".into(),
        skip: vec![],
        take: vec![TakeRule {
            reader: TokenReader::Rule("entry".into()),
            kind: KindRef {
                schema: schema.clone(),
                local_kind: 0,
            },
        }],
    }];
    let input = source("ab").map_err(err)?;
    let mut store = SourceStore::default();
    store.insert(input.clone()).map_err(err)?;
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let mut session =
        TokenizationSession::new("inner-map".into(), &modes, &checked, &registry, &mut b)
            .map_err(err)?;
    let first = {
        let raw = context(&schema, &registry).map_err(err)?;
        let ctx = check_context(&raw, &store, &registry, &mut b, &mut a).map_err(err)?;
        session
            .read(
                TokenizationRequest {
                    snapshot: &input,
                    start: 0,
                    limit: 2,
                    final_input: true,
                    context: &ctx,
                    state: &NdfValue::Text("outer-state".into()),
                },
                &store,
                &mut b,
                &mut a,
            )
            .map_err(err)?
    };
    assert!(matches!(first.outcome, TokenizationOutcome::Await { .. }));
    let generated = a
        .create(
            SourceId("inner-generated".into()),
            0,
            "memory:inner-generated".into(),
            b"g".to_vec(),
            &mut b,
        )
        .map_err(err)?;
    let span = generated.span(0, 1).map_err(err)?;
    let mut prefix = annotated_terminal(&schema, span.clone(), 1, &mut b).map_err(err)?;
    let ProviderReply::Read(r) = &mut prefix else {
        return Err("Read".into());
    };
    let ReadReply::Matched {
        sources,
        source_maps,
        view,
        new_state,
        ..
    } = r.as_mut()
    else {
        return Err("Matched".into());
    };
    *new_state = NdfValue::Text("inner-candidate-state".into());
    sources.push(generated);
    source_maps.push(Mapping {
        source: input.span(0, 1).map_err(err)?,
        target: span.clone(),
        kind: MappingKind::Transformed,
    });
    view.elements = vec![
        ViewElement {
            kind: KindRef {
                schema: schema.clone(),
                local_kind: 0,
            },
            span: input.span(0, 1).map_err(err)?,
            fields: vec![ViewField {
                name: "mapped".into(),
                children: vec![ViewRef(1)],
            }],
            roles: vec![],
            relations: vec![],
        },
        ViewElement {
            kind: KindRef {
                schema: schema.clone(),
                local_kind: 0,
            },
            span,
            fields: vec![],
            roles: vec![],
            relations: vec![],
        },
    ];
    view.roots = vec![ViewRef(0)];
    let wait = resume_value(
        projection,
        &mut session,
        prefix,
        &store,
        &registry,
        &mut b,
        &mut a,
    )?;
    let TokenizationOutcome::Await { continuation, .. } = wait.outcome else {
        return Err("inner Await".into());
    };
    // Native Await carries the nested report's source/map closure outward,
    // while only the inner reader owns the candidate's nonempty view.
    assert_eq!(continuation.current.sources.len(), 1);
    assert_eq!(continuation.current.source_maps.len(), 1);
    assert!(continuation.current.view.elements.is_empty());
    let TokenizationWait::Provider {
        continuation: inner,
    } = &continuation.pending
    else {
        return Err("inner continuation".into());
    };
    assert_eq!(
        continuation.current.state,
        NdfValue::Text("outer-state".into())
    );
    assert_eq!(
        inner.current.state,
        NdfValue::Text("inner-candidate-state".into())
    );
    assert_eq!(inner.current.sources.len(), 1);
    assert_eq!(inner.current.source_maps.len(), 1);
    assert_eq!(inner.current.view.elements.len(), 2);
    let mut c = FoundationCodec::new(&registry, &store, &mut a).map_err(err)?;
    let echo = projection.pending(&session, &mut c, &mut b).map_err(err)?;
    let echo = nepl3_wire::decode(&nepl3_wire::encode(&echo, &mut b).map_err(err)?, &mut b)
        .map_err(err)?;
    if matches!(projection, Projection::Reply) {
        crate::echo_projection::check_reply(&echo)?;
        let NdfValue::Record(reply) = &echo else {
            return Err("reply".into());
        };
        assert_eq!(
            reply.fields[2],
            NdfValue::Some(Box::new(NdfValue::Text("outer-state".into())))
        );
    }
    let NdfValue::Record(projected_outer) = projection.inner(&echo)? else {
        return Err("projected outer".into());
    };
    let NdfValue::Variant(projected_wait) = &projected_outer.fields[13] else {
        return Err("projected wait".into());
    };
    let NdfValue::Record(projected_inner) = &projected_wait.fields[0] else {
        return Err("projected inner".into());
    };
    let NdfValue::Record(projected_checkpoint) = &projected_inner.fields[6] else {
        return Err("projected checkpoint".into());
    };
    let NdfValue::Record(projected_view) = &projected_checkpoint.fields[2] else {
        return Err("projected view".into());
    };
    let NdfValue::List(projected_elements) = &projected_view.fields[0] else {
        return Err("projected elements".into());
    };
    let NdfValue::List(projected_roots) = &projected_view.fields[1] else {
        return Err("projected roots".into());
    };
    assert_eq!(projected_elements.len(), 2);
    assert_eq!(projected_roots.len(), 1);
    let mut wrong = echo.clone();
    let NdfValue::Record(root) = projection.inner_mut(&mut wrong)? else {
        return Err("root".into());
    };
    let NdfValue::Variant(wait) = &mut root.fields[13] else {
        return Err("wait".into());
    };
    let NdfValue::Record(inner) = &mut wait.fields[0] else {
        return Err("inner".into());
    };
    let NdfValue::Record(cp) = &mut inner.fields[6] else {
        return Err("checkpoint".into());
    };
    cp.fields[6] = NdfValue::List(vec![]);
    let reply = terminal("b", 2, &mut b).map_err(err)?;
    assert!(matches!(
        projection.resume(&mut session, &wrong, reply, &store, &mut c, &mut b),
        Err(PortableError::Reader(ReaderError::Continuation))
    ));
    let invalid = terminal("b", 0, &mut b).map_err(err)?;
    assert!(super::tokenizer_context::roundtrip(&session, invalid, &mut c, &mut b).is_err());
    assert!(session.pending_read().is_ok());
    let reply = terminal("b", 2, &mut b).map_err(err)?;
    let reply = super::tokenizer_context::roundtrip(&session, reply, &mut c, &mut b)?;
    let done = projection
        .resume(&mut session, &echo, reply, &store, &mut c, &mut b)
        .map_err(err)?;
    let TokenizationOutcome::Token(token) = done.outcome else {
        return Err("Token".into());
    };
    assert_eq!(token.views.elements.len(), 2);
    assert_eq!(done.sources.len(), 1);
    assert_eq!(done.source_maps.len(), 1);
    assert_eq!(done.report.diagnostics.len(), 1);
    assert_eq!(done.report.events.len(), 1);
    Ok(())
}
