use super::*;
use crate::echo_projection::Projection;
use nepl3_reader::{portable::PortableError, runtime::ProviderReply};
use nepl3_wire::foundation::FoundationCodec;
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn canonical_reservation_echo_preserves_trivia_target_and_owned_context() -> Result<(), String> {
    for projection in [Projection::Continuation, Projection::Reply] {
        canonical_reservation_echo_preserves_trivia_target_and_owned_context_case(projection)?;
    }
    Ok(())
}
fn canonical_reservation_echo_preserves_trivia_target_and_owned_context_case(
    projection: Projection,
) -> Result<(), String> {
    for explicit in [false, true] {
        let f = Fixture::new("\u{feff} \t#comment\n \"x\"").map_err(err)?;
        let p = token_plan(&f);
        let checked = p.check(&f.registry, &mut budget()).map_err(err)?;
        let modes = vec![ReaderMode {
            name: "default".into(),
            skip: vec![SkipRule {
                reader: TokenReader::Builtin(BuiltinReader::Trivia),
            }],
            take: vec![TakeRule {
                reader: TokenReader::Builtin(BuiltinReader::Text),
                kind: token_kind(&f).map_err(err)?,
            }],
        }];
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let mut session = TokenizationSession::new(
            "wire-reservation".into(),
            &modes,
            &checked,
            &f.registry,
            &mut b,
        )
        .map_err(err)?;
        let wait = {
            let raw = f.context.clone();
            let mut c = FoundationCodec::new(&f.registry, &f.store, &mut a).map_err(err)?;
            let ctx = raw
                .check(&mut c, &f.store, &f.registry, &mut b)
                .map_err(err)?;
            let input = TokenizationRequest {
                snapshot: &f.source,
                start: 0,
                limit: f.source.text().len() as u64,
                final_input: true,
                context: &ctx,
                state: &NdfValue::Unit,
            };
            if explicit {
                session.read_target(
                    TokenTarget::Builtin {
                        reader: BuiltinReader::Text,
                        token_kind: token_kind(&f).map_err(err)?,
                    },
                    input,
                    &f.store,
                    &mut b,
                    &mut a,
                )
            } else {
                session.read(input, &f.store, &mut b, &mut a)
            }
            .map_err(err)?
        };
        let TokenizationOutcome::Reserve { continuation, .. } = wait.outcome else {
            return Err("Reserve".into());
        };
        assert!(!wait.trivia.is_empty());
        let saved = continuation.usage;
        let reservation = SourceReservation {
            source_id: SourceId("decoded-value".into()),
            revision: 0,
            uri: "memory:decoded-value".into(),
        };
        let mut c = FoundationCodec::new(&f.registry, &f.store, &mut a).map_err(err)?;
        let echo = projection.pending(&session, &mut c, &mut b).map_err(err)?;
        if matches!(projection, Projection::Reply) {
            crate::echo_projection::check_reply(&echo)?;
            for wrong in crate::echo_projection::changed_fields(&echo)? {
                let target = &mut b;
                assert!(matches!(
                    projection.reserve(
                        &mut session,
                        &wrong,
                        &reservation,
                        &f.store,
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
                    projection.reserve(
                        &mut session,
                        &wrong,
                        &reservation,
                        &f.store,
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
        let NdfValue::Record(projected) = projection.inner(&echo)? else {
            return Err("projection".into());
        };
        assert_eq!(
            projected.fields[3],
            NdfValue::Bytes(continuation.reader_plan_digest.0.to_vec())
        );
        assert_eq!(
            projected.fields[4],
            NdfValue::Bytes(continuation.configuration_digest.0.to_vec())
        );
        assert_eq!(projected.fields[12], NdfValue::U64(continuation.furthest));
        assert_eq!(projected.fields[14], NdfValue::U64(continuation.depth_base));
        let NdfValue::Variant(target) = &projected.fields[7] else {
            return Err("target".into());
        };
        assert_eq!(target.variant, if explicit { "Builtin" } else { "Mode" });
        let NdfValue::Variant(phase) = &projected.fields[8] else {
            return Err("phase".into());
        };
        assert_eq!(phase.variant, "Take");
        let NdfValue::List(trivia) = &projected.fields[10] else {
            return Err("trivia".into());
        };
        let tags = trivia
            .iter()
            .map(|v| match v {
                NdfValue::Record(r) => match &r.fields[1] {
                    NdfValue::Variant(k) => Ok(k.variant.as_str()),
                    _ => Err("kind"),
                },
                _ => Err("trivia record"),
            })
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(tags, ["Bom", "Whitespace", "Comment", "Whitespace"]);
        assert!(matches!(
            session.pending_read(),
            Err(ReaderError::Continuation)
        ));
        assert!(matches!(
            session.pending_transform(),
            Err(ReaderError::Continuation)
        ));
        let bytes = nepl3_wire::encode(&echo, &mut b).map_err(err)?;
        let echo = nepl3_wire::decode(&bytes, &mut b).map_err(err)?;
        b.charge(Resource::Work, 19).map_err(err)?;
        assert_eq!(
            projection.pending(&session, &mut c, &mut b).map_err(err)?,
            echo
        );
        assert_eq!(continuation.usage, saved);
        assert_eq!(b.usage().diagnostics, saved.diagnostics);
        assert_eq!(b.usage().events, saved.events);
        for field in 0..17 {
            let mut wrong = echo.clone();
            let NdfValue::Record(r) = projection.inner_mut(&mut wrong)? else {
                return Err("record".into());
            };
            r.fields[field] = NdfValue::Unit;
            assert!(matches!(
                projection.reserve(&mut session, &wrong, &reservation, &f.store, &mut c, &mut b),
                Err(PortableError::Reader(ReaderError::Continuation))
            ));
            assert!(projection.pending(&session, &mut c, &mut b).is_ok());
        }
        let wrong_route = ProviderReply::Read(Box::new(ReadReply::Stopped {
            reason: StopReason::Cancelled,
            sources: vec![],
            source_maps: vec![],
            report: nepl3_core::diagnostic::Report::default(),
        }));
        assert!(matches!(
            projection.resume(&mut session, &echo, wrong_route, &f.store, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::Continuation))
        ));
        let mut foreign = Budget::new(b.limits());
        foreign.cancel();
        assert!(matches!(
            projection.reserve(
                &mut session,
                &echo,
                &reservation,
                &f.store,
                &mut c,
                &mut foreign
            ),
            Err(PortableError::Reader(ReaderError::Continuation))
        ));
        assert_eq!(
            projection.pending(&session, &mut c, &mut b).map_err(err)?,
            echo
        );
        let done = projection
            .reserve(&mut session, &echo, &reservation, &f.store, &mut c, &mut b)
            .map_err(err)?;
        let TokenizationOutcome::Token(token) = done.outcome else {
            return Err("Token".into());
        };
        assert_eq!(token.payload, NdfValue::Text("x".into()));
        assert_eq!(done.trivia, wait.trivia);
        assert_eq!(done.sources.len(), 1);
        assert_eq!(done.sources[0].identity().source, reservation.source_id);
        assert!(matches!(
            projection.reserve(&mut session, &echo, &reservation, &f.store, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::NoPending))
        ));
        session.close();
        assert!(matches!(session.pending_read(), Err(ReaderError::Closed)));
        assert!(matches!(
            session.pending_transform(),
            Err(ReaderError::Closed)
        ));
        assert!(matches!(
            projection.pending(&session, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::Closed))
        ));
    }
    Ok(())
}

#[test]
fn skip_reservation_stops_keep_generated_prefix_and_foreign_echo_cannot_consume_it()
-> Result<(), String> {
    for projection in [Projection::Continuation, Projection::Reply] {
        skip_reservation_stops_keep_generated_prefix_and_foreign_echo_cannot_consume_it_case(
            projection,
        )?;
    }
    Ok(())
}
fn skip_reservation_stops_keep_generated_prefix_and_foreign_echo_cannot_consume_it_case(
    projection: Projection,
) -> Result<(), String> {
    for mode in 0..7 {
        let f = Fixture::new("\"a\"\"b\"").map_err(err)?;
        let p = token_plan(&f);
        let checked = p.check(&f.registry, &mut budget()).map_err(err)?;
        let modes = vec![ReaderMode {
            name: "default".into(),
            skip: vec![SkipRule {
                reader: TokenReader::Builtin(BuiltinReader::Text),
            }],
            take: vec![],
        }];
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let mut session =
            TokenizationSession::new("skip-value".into(), &modes, &checked, &f.registry, &mut b)
                .map_err(err)?;
        let wait = {
            let raw = f.context.clone();
            let mut c = FoundationCodec::new(&f.registry, &f.store, &mut a).map_err(err)?;
            let ctx = raw
                .check(&mut c, &f.store, &f.registry, &mut b)
                .map_err(err)?;
            session
                .read(
                    TokenizationRequest {
                        snapshot: &f.source,
                        start: 0,
                        limit: 6,
                        final_input: true,
                        context: &ctx,
                        state: &NdfValue::Unit,
                    },
                    &f.store,
                    &mut b,
                    &mut a,
                )
                .map_err(err)?
        };
        assert!(matches!(wait.outcome, TokenizationOutcome::Reserve { .. }));
        let reservation = SourceReservation {
            source_id: SourceId("skip-first".into()),
            revision: 0,
            uri: "memory:skip-first".into(),
        };
        let mut c = FoundationCodec::new(&f.registry, &f.store, &mut a).map_err(err)?;
        let first = projection.pending(&session, &mut c, &mut b).map_err(err)?;
        let first = nepl3_wire::decode(&nepl3_wire::encode(&first, &mut b).map_err(err)?, &mut b)
            .map_err(err)?;
        let wait = projection
            .reserve(&mut session, &first, &reservation, &f.store, &mut c, &mut b)
            .map_err(err)?;
        let TokenizationOutcome::Reserve { continuation, .. } = &wait.outcome else {
            return Err("second Reserve".into());
        };
        assert!(matches!(continuation.phase, TokenizationPhase::Skip { .. }));
        assert_eq!(wait.trivia.len(), 1);
        assert_eq!(wait.trivia[0].kind, nepl3_core::view::TriviaKind::Skipped);
        assert_eq!(wait.sources.len(), 1);
        assert!(!wait.source_maps.is_empty());
        let echo = projection.pending(&session, &mut c, &mut b).map_err(err)?;
        let mut stopped_export = Budget::new(Limits {
            allocation_units: 0,
            ..b.limits()
        });
        assert!(
            projection
                .pending(&session, &mut c, &mut stopped_export)
                .is_err()
        );
        assert_eq!(
            projection.pending(&session, &mut c, &mut b).map_err(err)?,
            echo
        );
        let next = SourceReservation {
            source_id: SourceId("skip-next".into()),
            revision: 0,
            uri: "memory:skip-next".into(),
        };
        let reason = match mode {
            0 | 5 => {
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
                b.charge(Resource::Nodes, b.limits().nodes - b.usage().nodes)
                    .map_err(err)?;
                StopReason::NodeLimit
            }
            4 => StopReason::DepthLimit,
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
        if mode == 0 {
            let mut foreign = echo.clone();
            let NdfValue::Record(r) = projection.inner_mut(&mut foreign)? else {
                return Err("record".into());
            };
            r.fields[1] = NdfValue::Text("foreign-session".into());
            for _ in 0..2 {
                assert!(matches!(
                    projection.reserve(&mut session, &foreign, &next, &f.store, &mut c, &mut b),
                    Err(PortableError::Reader(ReaderError::Continuation))
                ));
            }
        }
        let result = if mode == 4 {
            b.with_depth_at_least(b.limits().depth, |b| {
                projection.reserve(&mut session, &echo, &next, &f.store, &mut c, b)
            })
            .map_err(err)?
        } else if mode == 5 {
            projection
                .resume(
                    &mut session,
                    &echo,
                    ProviderReply::Read(Box::new(ReadReply::Stopped {
                        reason: StopReason::Cancelled,
                        sources: vec![],
                        source_maps: vec![],
                        report: nepl3_core::diagnostic::Report::default(),
                    })),
                    &f.store,
                    &mut c,
                    &mut b,
                )
                .map_err(err)?
        } else {
            projection
                .reserve(&mut session, &echo, &next, &f.store, &mut c, &mut b)
                .map_err(err)?
        };
        assert!(
            matches!(result.outcome,TokenizationOutcome::Stopped{reason:actual} if actual==reason)
        );
        assert_eq!(result.trivia, wait.trivia);
        assert_eq!(result.sources, wait.sources);
        assert_eq!(result.source_maps, wait.source_maps);
        assert_eq!(result.facts, wait.facts);
        assert_eq!(result.cursor, wait.cursor);
        assert_eq!(b.poll(), Err(reason));
        assert!(matches!(
            projection.pending(&session, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::NoPending))
        ));
        // This is a new explicit operation, with its own budget and admission.
        drop(c);
        let mut fresh = budget();
        let mut admission = SourceAdmission::default();
        let pending = {
            let raw = f.context.clone();
            let mut c = FoundationCodec::new(&f.registry, &f.store, &mut admission).map_err(err)?;
            let ctx = raw
                .check(&mut c, &f.store, &f.registry, &mut fresh)
                .map_err(err)?;
            session
                .read(
                    TokenizationRequest {
                        snapshot: &f.source,
                        start: 0,
                        limit: 6,
                        final_input: true,
                        context: &ctx,
                        state: &NdfValue::Unit,
                    },
                    &f.store,
                    &mut fresh,
                    &mut admission,
                )
                .map_err(err)?
        };
        assert!(matches!(
            pending.outcome,
            TokenizationOutcome::Reserve { .. }
        ));
    }
    Ok(())
}
