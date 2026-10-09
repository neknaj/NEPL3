use super::*;
use nepl3_reader::{builtin::BuiltinReader, tokenizer::*};

#[test]
fn tokenizer_reservation_restoration_meters_unrelated_modes() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = provider_plan(&schema);
    let checked = p.check(&registry, &mut budget())?;
    let input = source("\"a\"")?;
    let mut store = SourceStore::default();
    store.insert(input.clone())?;
    let raw = context(&schema, &registry)?;
    let mut measurements = vec![];
    for padding in [0, 32] {
        let mut modes: Vec<_> = (0..padding)
            .map(|i| ReaderMode {
                name: format!("A{i:02}"),
                skip: vec![],
                take: vec![],
            })
            .collect();
        modes.push(ReaderMode {
            name: "test".into(),
            skip: vec![],
            take: vec![],
        });
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let proof = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
        let mut session =
            TokenizationSession::new("restore".into(), &modes, &checked, &registry, &mut b)?;
        let wait = session.read_target(
            TokenTarget::Builtin {
                reader: BuiltinReader::Text,
                token_kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
            },
            TokenizationRequest {
                snapshot: &input,
                start: 0,
                limit: 3,
                final_input: true,
                context: &proof,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let TokenizationOutcome::Reserve { continuation, .. } = wait.outcome else {
            return Err(ReaderError::Context);
        };
        let before = b.usage();
        let reply = session.reserve(
            &continuation,
            &SourceReservation {
                source_id: SourceId("decoded".into()),
                revision: 0,
                uri: "memory:decoded".into(),
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        assert!(matches!(reply.outcome, TokenizationOutcome::Token(_)));
        measurements.push((
            b.usage().work - before.work,
            b.usage().allocation_units - before.allocation_units,
        ));
    }
    assert_eq!(
        measurements[1],
        (measurements[0].0 + 32 * (3 + 4 + 1), measurements[0].1)
    );
    Ok(())
}

#[test]
fn tokenizer_restore_lookup_stop_keeps_nonempty_prefix_and_consumes_pending()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = provider_plan(&schema);
    let checked = p.check(&registry, &mut budget())?;
    for provider in [false, true] {
        let input = source(if provider { "\"s\"p" } else { "\"s\"\"t\"" })?;
        let mut store = SourceStore::default();
        store.insert(input.clone())?;
        let raw = context(&schema, &registry)?;
        let modes = vec![ReaderMode {
            name: "test".into(),
            skip: vec![SkipRule {
                reader: TokenReader::Builtin(BuiltinReader::Text),
            }],
            take: vec![TakeRule {
                reader: TokenReader::Rule("entry".into()),
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
            }],
        }];
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let proof = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
        let mut session =
            TokenizationSession::new("restore".into(), &modes, &checked, &registry, &mut b)?;
        let first = session.read(
            TokenizationRequest {
                snapshot: &input,
                start: 0,
                limit: input.text().len() as u64,
                final_input: true,
                context: &proof,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let TokenizationOutcome::Reserve {
            continuation: first,
            ..
        } = first.outcome
        else {
            return Err(ReaderError::Context);
        };
        let wait = session.reserve(
            &first,
            &SourceReservation {
                source_id: SourceId("first".into()),
                revision: 0,
                uri: "memory:first".into(),
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let echo = match &wait.outcome {
            TokenizationOutcome::Await { continuation, .. } if provider => continuation,
            TokenizationOutcome::Reserve { continuation, .. } if !provider => continuation,
            _ => return Err(ReaderError::Context),
        };
        assert_eq!(wait.sources.len(), 1);
        assert!(!wait.source_maps.is_empty());
        assert_eq!(wait.trivia.len(), 1);
        let reservation = SourceReservation {
            source_id: SourceId("second".into()),
            revision: 0,
            uri: "memory:second".into(),
        };
        let response = |usage| {
            ProviderReply::Read(Box::new(ReadReply::NoMatch {
                furthest: 3,
                expected: vec![],
                sources: vec![],
                source_maps: vec![],
                report: Report {
                    usage,
                    ..Report::default()
                },
            }))
        };
        let missing = if provider {
            session.resume(
                echo,
                response(b.usage()),
                &SourceStore::default(),
                &mut b,
                &mut admission,
            )
        } else {
            session.reserve(
                echo,
                &reservation,
                &SourceStore::default(),
                &mut b,
                &mut admission,
            )
        };
        assert!(matches!(
            missing,
            Err(ReaderError::Source(SourceError::MissingSnapshot))
        ));
        if provider {
            assert!(session.pending_read().is_ok());
        }
        let mut measure = budget();
        echo.charge_clone(&mut measure)?;
        let headroom = measure.usage().work + 1;
        let remaining = b.limits().work - b.usage().work;
        b.charge(Resource::Work, remaining - headroom)?;
        let before = b.usage();
        let stopped = if provider {
            session.resume(echo, response(b.usage()), &store, &mut b, &mut admission)?
        } else {
            session.reserve(echo, &reservation, &store, &mut b, &mut admission)?
        };
        assert!(matches!(
            stopped.outcome,
            TokenizationOutcome::Stopped {
                reason: StopReason::WorkLimit
            }
        ));
        assert_eq!(b.usage().work - before.work, headroom);
        assert_eq!(
            b.usage().allocation_units - before.allocation_units,
            measure.usage().allocation_units
        );
        assert_eq!(stopped.sources, wait.sources);
        assert_eq!(stopped.source_maps, wait.source_maps);
        assert_eq!(stopped.trivia, wait.trivia);
        assert_eq!(stopped.report.usage, b.usage());
        assert_eq!(stopped.cursor, wait.cursor);
        let retry = if provider {
            session.resume(echo, response(b.usage()), &store, &mut b, &mut admission)
        } else {
            session.reserve(echo, &reservation, &store, &mut b, &mut admission)
        };
        assert!(matches!(retry, Err(ReaderError::NoPending)));
    }
    Ok(())
}

#[test]
fn tokenizer_restore_foundation_search_is_metered_before_rejected_provider_reply()
-> Result<(), ReaderError> {
    let (original, schema) = registry()?;
    let mut measurements = vec![];
    for padding in [0, 32] {
        let mut registry = SchemaRegistry::default();
        for i in 0..padding {
            let d = SchemaDescriptor {
                package: format!("padding.{i:02}"),
                revision: 1,
                types: vec![],
                operations: vec![],
            };
            registry.register(d.reference(&mut budget())?, d, &mut budget())?;
        }
        for package in ["nepl3.foundation", "nepl3.reader", "test"] {
            let r = original
                .selected(package, 1)
                .ok_or(SchemaError::UnknownSchema)?;
            let d = original
                .descriptor(r)
                .ok_or(SchemaError::UnknownSchema)?
                .clone();
            registry.register(r.clone(), d, &mut budget())?;
        }
        registry.finalize(&mut budget())?;
        let p = provider_plan(&schema);
        let checked = p.check(&registry, &mut budget())?;
        let input = source("a")?;
        let mut store = SourceStore::default();
        store.insert(input.clone())?;
        let raw = context(&schema, &registry)?;
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
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let proof = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
        let mut session =
            TokenizationSession::new("restore".into(), &modes, &checked, &registry, &mut b)?;
        let wait = session.read(
            TokenizationRequest {
                snapshot: &input,
                start: 0,
                limit: 1,
                final_input: true,
                context: &proof,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let TokenizationOutcome::Await { continuation, .. } = wait.outcome else {
            return Err(ReaderError::Context);
        };
        let wrong = ProviderReply::Transform(Box::new(TransformReply {
            outcome: TransformOutcome::Complete {
                value: NdfValue::Unit,
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
        }));
        let before = b.usage();
        assert!(matches!(
            session.resume(&continuation, wrong, &store, &mut b, &mut admission),
            Err(ReaderError::ProviderContract)
        ));
        measurements.push((
            b.usage().work - before.work,
            b.usage().allocation_units - before.allocation_units,
        ));
        assert!(session.pending_read().is_ok());
        let valid = terminal("a", 1, &mut b)?;
        let result = session.resume(&continuation, valid, &store, &mut b, &mut admission)?;
        assert!(matches!(result.outcome, TokenizationOutcome::Token(_)));
    }
    // Restore selects foundation once. The outer tokenizer request and inner
    // reader request each admit foundation plus their test-package context.
    // Wrong reply-kind rejection still precedes typed provider payload validation.
    let per_padding_schema = (10 + 16 + 9) + 2 * ((10 + 16 + 9) + (10 + 4 + 9));
    assert_eq!(
        measurements[1],
        (
            measurements[0].0 + 32 * per_padding_schema,
            measurements[0].1
        )
    );
    Ok(())
}
