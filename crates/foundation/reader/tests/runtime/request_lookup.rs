use super::*;
use nepl3_reader::{builtin::BuiltinReader, tokenizer::*};

#[test]
fn tokenizer_request_lookup_stops_at_catalog_boundary() -> Result<(), ReaderError> {
    let (registry, schema) = registry_with_padding(32)?;
    let p = provider_plan(&schema);
    let checked = p.check(&registry, &mut budget())?;
    let input = source("a")?;
    let mut store = SourceStore::default();
    store.insert(input.clone())?;
    let modes: Vec<_> = (0..32)
        .map(|i| ReaderMode {
            name: format!("common.mode.{i:02}"),
            skip: vec![],
            take: vec![],
        })
        .collect();
    let mut raw = context(&schema, &registry)?;
    raw.mode = "common.mode.zz".into();
    for builtin in [false, true] {
        let cutoffs: &[u64] = if builtin {
            &[82, 83, 84, 116]
        } else {
            &[82, 83, 112]
        };
        for &headroom in cutoffs {
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let proof = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
            let mut session =
                TokenizationSession::new("request".into(), &modes, &checked, &registry, &mut b)?;
            let scope = TokenizationScope {
                operation_id: "operation".into(),
                profile_digest: Digest([3; 32]),
                snapshot: input.reference(),
            };
            let accepted = AcceptedTokenizationReport::empty(scope.clone(), &mut b)?;
            let _ = admission.scope_with_budget(&mut b)?;
            // Scope operation(9), source identity twice(4+4), fixed comparison65.
            let remaining = b.limits().work - b.usage().work;
            b.charge(Resource::Work, remaining - headroom)?;
            let target = if builtin {
                let mut bad = schema.clone();
                bad.package = "missing".into();
                TokenTarget::Builtin {
                    reader: BuiltinReader::Name,
                    token_kind: KindRef {
                        schema: bad,
                        local_kind: 0,
                    },
                }
            } else {
                TokenTarget::Mode
            };
            let result = session.read_with_accepted_recover(
                ScopedTokenizationRequest {
                    scope: &scope,
                    target,
                    input: TokenizationRequest {
                        snapshot: &input,
                        start: 0,
                        limit: 1,
                        final_input: true,
                        context: &proof,
                        state: &NdfValue::Unit,
                    },
                },
                &store,
                &mut b,
                &mut admission,
                accepted,
            );
            let Ok(reply) = result else {
                return Err(ReaderError::Context);
            };
            assert!(matches!(
                reply.outcome,
                TokenizationOutcome::Stopped {
                    reason: StopReason::WorkLimit
                }
            ));
            assert_eq!(reply.cursor, 0);
            assert!(reply.new_state.is_none());
            assert!(reply.trivia.is_empty() && reply.facts.is_empty());
            assert_eq!(reply.accepted.report().usage, b.usage());
            assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        }
    }
    Ok(())
}

#[test]
fn tokenizer_request_mode_and_kind_misses_charge_independent_catalog_costs()
-> Result<(), ReaderError> {
    for padding in [0, 32] {
        let (registry, schema) = registry_with_padding(padding)?;
        let p = provider_plan(&schema);
        let checked = p.check(&registry, &mut budget())?;
        let input = source("a")?;
        let mut store = SourceStore::default();
        store.insert(input.clone())?;
        for long in [false, true] {
            let prefix = if long {
                "a".repeat(1024)
            } else {
                String::new()
            };
            let modes: Vec<_> = (0..32)
                .map(|i| ReaderMode {
                    name: format!("{prefix}common.mode.{i:02}"),
                    skip: vec![],
                    take: vec![],
                })
                .collect();
            let mut raw = context(&schema, &registry)?;
            raw.mode = format!("{prefix}common.mode.zz");
            for mutation in 0..5 {
                let mut b = budget();
                let mut admission = SourceAdmission::default();
                let proof = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
                let mut session = TokenizationSession::new(
                    "request".into(),
                    &modes,
                    &checked,
                    &registry,
                    &mut b,
                )?;
                let scope = TokenizationScope {
                    operation_id: "operation".into(),
                    profile_digest: Digest([3; 32]),
                    snapshot: input.reference(),
                };
                let accepted = AcceptedTokenizationReport::empty(scope.clone(), &mut b)?;
                let _ = admission.scope_with_budget(&mut b)?;
                let before = b.usage();
                let mut kind = KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                };
                match mutation {
                    0 => kind.schema.package = "missing".into(),
                    1 => kind.schema.revision += 1,
                    2 => kind.schema.digest.0[0] ^= 1,
                    3 => kind.local_kind = u64::MAX,
                    _ => {}
                }
                let target = if mutation == 4 {
                    TokenTarget::Mode
                } else {
                    TokenTarget::Builtin {
                        reader: BuiltinReader::Name,
                        token_kind: kind,
                    }
                };
                // Bad range remains lower priority than these lookup errors.
                let result = session.read_with_accepted_recover(
                    ScopedTokenizationRequest {
                        scope: &scope,
                        target,
                        input: TokenizationRequest {
                            snapshot: &input,
                            start: 0,
                            limit: 2,
                            final_input: true,
                            context: &proof,
                            state: &NdfValue::Unit,
                        },
                    },
                    &store,
                    &mut b,
                    &mut admission,
                    accepted,
                );
                let Err(AcceptedTokenizationFailure::Recoverable { error, accepted }) = result
                else {
                    return Err(ReaderError::ProviderContract);
                };
                assert_eq!(
                    error,
                    match mutation {
                        0..=2 => ReaderError::Schema(SchemaError::UnknownSchema),
                        3 => ReaderError::Schema(SchemaError::UnknownType),
                        _ => ReaderError::Context,
                    }
                );
                let work = if mutation == 4 {
                    82 + 1 + 32 * (2 * (prefix.len() as u64 + 14) + 1)
                } else {
                    let query = if mutation == 0 { 7 } else { 4 };
                    let fixed: u64 = [16, 12, 4].into_iter().map(|len| len + query + 9).sum();
                    let padded: u64 = (0..padding)
                        .map(|i| format!("unrelated.{i}").len() as u64 + query + 9)
                        .sum();
                    82 + 2 + fixed + padded + if mutation >= 2 { 45 } else { 0 }
                };
                let mut expected = before;
                expected.work += work;
                assert_eq!(b.usage(), expected);
                assert_eq!(accepted.report().usage, expected);
                assert_eq!(accepted.scope(), &scope);
            }
        }
    }
    Ok(())
}
