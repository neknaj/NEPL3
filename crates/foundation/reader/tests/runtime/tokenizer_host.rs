use super::*;
use nepl3_reader::tokenizer::*;

struct Host {
    mode: u8,
    calls: usize,
}
impl TokenizationHost for Host {
    fn provider(
        &mut self,
        call: &ProviderCall,
        b: &mut Budget,
        _: &mut SourceAdmission,
    ) -> Result<Option<ProviderReply>, ReaderError> {
        self.calls += 1;
        let ProviderCall::Read {
            request,
            depth_base,
            ..
        } = call
        else {
            return Err(ReaderError::ProviderContract);
        };
        assert_eq!(b.current_depth(), *depth_base);
        match self.mode {
            1 => return Ok(None),
            2 => return Err(ReaderError::Context),
            6 => return Err(ReaderError::Stopped(StopReason::Cancelled)),
            4 => {
                b.cancel();
                return Ok(None);
            }
            5 => {
                b.charge(Resource::Work, u64::MAX)?;
            }
            7 | 8 => {
                let mut limits = b.limits();
                limits.work += 1;
                let observed = b.usage();
                *b = Budget::new(limits);
                b.record_observed_usage(observed)?;
                if self.mode == 8 {
                    return Ok(None);
                }
            }
            _ => {}
        }
        let mut reply = terminal("a", request.start + 1, b)?;
        if self.mode == 3 {
            let ProviderReply::Read(reply) = &mut reply else {
                return Err(ReaderError::Context);
            };
            let ReadReply::Matched { value, .. } = reply.as_mut() else {
                return Err(ReaderError::Context);
            };
            *value = NdfValue::Unit;
        }
        Ok(Some(reply))
    }
    fn reservation(
        &mut self,
        _: &ReservationRequest,
        b: &mut Budget,
        _: &mut SourceAdmission,
    ) -> Result<Option<SourceReservation>, ReaderError> {
        self.calls += 1;
        match self.mode {
            1 => return Ok(None),
            2 => return Err(ReaderError::Context),
            6 => return Err(ReaderError::Stopped(StopReason::Cancelled)),
            4 => {
                b.cancel();
                return Ok(None);
            }
            _ => {}
        }
        Ok(Some(SourceReservation {
            source_id: SourceId("decoded".into()),
            revision: 0,
            uri: "memory:decoded".into(),
        }))
    }
}

#[test]
fn synchronous_text_reservation_preserves_decode_maps_and_fallback() -> Result<(), ReaderError> {
    let (r, schema) = registry()?;
    let p = provider_plan(&schema);
    let checked = p.check(&r, &mut budget())?;
    let input = source("\"a\\n\"")?;
    let mut store = SourceStore::default();
    store.insert(input.clone())?;
    let raw = context(&schema, &r)?;
    let modes = vec![ReaderMode {
        name: "test".into(),
        skip: vec![],
        take: vec![],
    }];
    let mut normal = None;
    for mode in [0, 1, 2, 4, 6] {
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let proof = check_context(&raw, &store, &r, &mut b, &mut a)?;
        let mut session = TokenizationSession::new("text".into(), &modes, &checked, &r, &mut b)?;
        let scope = TokenizationScope {
            operation_id: "text-operation".into(),
            profile_digest: Digest([3; 32]),
            snapshot: input.reference(),
        };
        let accepted = AcceptedTokenizationReport::empty(scope.clone(), &mut b)?;
        let mut host = Host { mode, calls: 0 };
        let reply = session.read_accepted_with_host(
            ScopedTokenizationRequest {
                scope: &scope,
                target: TokenTarget::Builtin {
                    reader: nepl3_reader::builtin::BuiltinReader::Text,
                    token_kind: KindRef {
                        schema: schema.clone(),
                        local_kind: 0,
                    },
                },
                input: TokenizationRequest {
                    snapshot: &input,
                    start: 0,
                    limit: 5,
                    final_input: true,
                    context: &proof,
                    state: &NdfValue::Unit,
                },
            },
            &store,
            &mut b,
            &mut a,
            accepted,
            &mut host,
        )?;
        assert_eq!(host.calls, 1);
        assert_eq!(reply.host_error.is_some(), matches!(mode, 2 | 6));
        let mut reply = reply.reply.into_raw();
        if matches!(mode, 4 | 6) {
            assert!(matches!(
                reply.outcome,
                TokenizationOutcome::Stopped {
                    reason: StopReason::Cancelled
                }
            ));
            continue;
        }
        if mode != 0 {
            let TokenizationOutcome::Reserve { continuation, .. } = &reply.outcome else {
                return Err(ReaderError::NoPending);
            };
            let reservation = SourceReservation {
                source_id: SourceId("decoded".into()),
                revision: 0,
                uri: "memory:decoded".into(),
            };
            reply = session.reserve(continuation, &reservation, &store, &mut b, &mut a)?;
        }
        assert!(
            matches!(&reply.outcome, TokenizationOutcome::Token(t) if t.payload == NdfValue::Text("a\n".into()))
        );
        assert_eq!(reply.sources.len(), 1);
        assert_eq!(reply.source_maps.len(), 2);
        assert_eq!(reply.sources[0].text(), "a\n");
        let result = (
            reply.outcome,
            reply.cursor,
            reply.sources,
            reply.source_maps,
        );
        match &normal {
            None => normal = Some(result),
            Some(normal) => assert_eq!(normal, &result),
        }
    }
    Ok(())
}

#[test]
fn synchronous_tokenizer_keeps_owned_fallback_and_validates_replies() -> Result<(), ReaderError> {
    let (r, schema) = registry()?;
    let p = provider_plan(&schema);
    let checked = p.check(&r, &mut budget())?;
    let input = source(&"a".repeat(20000))?;
    let mut store = SourceStore::default();
    store.insert(input.clone())?;
    let raw = context(&schema, &r)?;
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
    let mut successes = vec![];
    for mode in 0..9 {
        let mut b = budget();
        let original_limits = b.limits();
        let mut a = SourceAdmission::default();
        let proof = check_context(&raw, &store, &r, &mut b, &mut a)?;
        let mut session = TokenizationSession::new("host".into(), &modes, &checked, &r, &mut b)?;
        let scope = TokenizationScope {
            operation_id: "operation".into(),
            profile_digest: Digest([3; 32]),
            snapshot: input.reference(),
        };
        let accepted = AcceptedTokenizationReport::empty(scope.clone(), &mut b)?;
        let mut host = Host { mode, calls: 0 };
        let reply = session.read_accepted_with_host(
            ScopedTokenizationRequest {
                scope: &scope,
                target: TokenTarget::Mode,
                input: TokenizationRequest {
                    snapshot: &input,
                    start: 0,
                    limit: 20000,
                    final_input: true,
                    context: &proof,
                    state: &NdfValue::Unit,
                },
            },
            &store,
            &mut b,
            &mut a,
            accepted,
            &mut host,
        )?;
        assert_eq!(host.calls, 1);
        assert_eq!(b.current_depth(), 0);
        assert_eq!(
            reply.host_error.is_some(),
            matches!(mode, 2 | 3 | 5 | 6 | 7 | 8)
        );
        let mut reply = reply.reply.into_raw();
        if matches!(mode, 4..=6) {
            assert!(
                matches!(reply.outcome, TokenizationOutcome::Stopped { reason }
                if reason == if matches!(mode, 4 | 6) { StopReason::Cancelled } else { StopReason::WorkLimit })
            );
            continue;
        }
        if mode != 0 {
            if matches!(mode, 7 | 8) {
                assert_eq!(b.limits().work, original_limits.work + 1);
                let observed = b.usage();
                b = Budget::new(original_limits);
                b.record_observed_usage(observed)?;
            }
            let TokenizationOutcome::Await { continuation, .. } = &reply.outcome else {
                return Err(ReaderError::NoPending);
            };
            let mut forged = continuation.as_ref().clone();
            forged.request.limit -= 1;
            assert_eq!(
                session.resume(&forged, terminal("a", 1, &mut b)?, &store, &mut b, &mut a),
                Err(ReaderError::Continuation)
            );
            reply = session.resume(
                continuation,
                terminal("a", 1, &mut b)?,
                &store,
                &mut b,
                &mut a,
            )?;
        }
        assert_eq!(reply.cursor, 1);
        assert!(
            matches!(&reply.outcome, TokenizationOutcome::Token(t) if t.payload == NdfValue::Text("a".into()))
        );
        successes.push((
            reply.outcome,
            reply.trivia,
            reply.sources,
            reply.source_maps,
            b.usage(),
        ));
    }
    for item in &successes[1..] {
        assert_eq!(
            (&item.0, &item.1, &item.2, &item.3),
            (
                &successes[0].0,
                &successes[0].1,
                &successes[0].2,
                &successes[0].3
            )
        );
        assert!(successes[0].4.work < item.4.work);
        assert!(successes[0].4.allocation_units < item.4.allocation_units);
    }
    Ok(())
}
