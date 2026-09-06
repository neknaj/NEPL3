use nepl3_core::{budget::*, schema::*, source::*, syntax::*, value::*};
use nepl3_reader::{builtin::*, model::*, runtime::ReaderError};
use nepl3_reader::{plan::*, tokenizer::*};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 1_000_000_000,
        depth: 512,
        nodes: 1_000_000,
        allocation_units: 1_000_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
fn registry() -> Result<SchemaRegistry, SchemaError> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut budget())?,
        nepl3_reader::schema::descriptor(&mut budget())?,
    ] {
        r.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    r.finalize(&mut budget())?;
    Ok(r)
}
struct Fixture {
    registry: SchemaRegistry,
    source: SourceSnapshot,
    store: SourceStore,
    context: ReaderContext,
}
impl Fixture {
    fn new(text: &str) -> Result<Self, ReaderError> {
        let registry = registry()?;
        let schema = registry
            .selected("nepl3.reader", 1)
            .ok_or(SchemaError::UnknownSchema)?
            .clone();
        let source = SourceSnapshot::new(
            SourceId("input".into()),
            0,
            "memory:input".into(),
            text.as_bytes().to_vec(),
            &mut budget(),
        )?;
        let mut store = SourceStore::default();
        store.insert(source.clone())?;
        let mut context = ReaderContext {
            schema,
            category: "Token".into(),
            mode: "default".into(),
            origins: vec![],
            environment: EnvironmentEntry {
                id: 0,
                digest: Digest([0; 32]),
                value: Environment {
                    bindings: vec![],
                    resources: vec![],
                },
            },
        };
        context.environment.digest = nepl3_wire::environment::environment_digest(
            &context.environment.value,
            registry
                .selected("nepl3.foundation", 1)
                .ok_or(SchemaError::UnknownSchema)?,
            &registry,
            &mut budget(),
        )
        .map_err(|_| ReaderError::Context)?;
        Ok(Self {
            registry,
            source,
            store,
            context,
        })
    }
    fn read(
        &self,
        kind: BuiltinReader,
        start: u64,
        limit: u64,
        final_input: bool,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ReadReply, ReaderError> {
        let mut proof_budget = budget();
        let mut proof_admission = SourceAdmission::default();
        let mut codec = nepl3_wire::foundation::FoundationCodec::new(
            &self.registry,
            &self.store,
            &mut proof_admission,
        )
        .map_err(|_| ReaderError::Context)?;
        let context = self
            .context
            .check(&mut codec, &self.store, &self.registry, &mut proof_budget)
            .map_err(|_| ReaderError::Context)?;
        let reservation = SourceReservation {
            source_id: SourceId("decoded".into()),
            revision: 0,
            uri: "memory:decoded".into(),
        };
        read(
            kind,
            ReadRequest {
                snapshot: &self.source,
                start,
                limit,
                final_input,
                context: &context,
                state: &NdfValue::Unit,
            },
            if kind == BuiltinReader::Text {
                Some(&reservation)
            } else {
                None
            },
            &self.registry,
            &self.store,
            b,
            admission,
        )
    }
    fn whole(&self, kind: BuiltinReader, final_input: bool) -> Result<ReadReply, ReaderError> {
        self.read(
            kind,
            0,
            self.source.text().len() as u64,
            final_input,
            &mut budget(),
            &mut SourceAdmission::default(),
        )
    }
}
fn failed(reply: ReadReply, code: &str) {
    assert!(
        matches!(reply, ReadReply::Failed { .. }),
        "expected failed {code}: {reply:?}"
    );
    let ReadReply::Failed {
        diagnostic, report, ..
    } = reply
    else {
        return;
    };
    assert_eq!(diagnostic.code, code);
    assert_eq!(report.diagnostics, vec![diagnostic]);
    assert_eq!(report.usage.diagnostics, 1);
}
#[test]
fn names_preserve_unicode_spelling_and_read_maximal_identifier() -> Result<(), ReaderError> {
    for name in ["letx", "_", "日本語", "a\u{301}", "\u{105c0}"] {
        let f = Fixture::new(&format!("{name} "))?;
        let ReadReply::Matched { value, end, .. } = f.whole(BuiltinReader::Name, false)? else {
            return Err(ReaderError::Context);
        };
        assert_eq!(value, NdfValue::Text(name.into()));
        assert_eq!(end, name.len() as u64);
    }
    for name in ["\u{1e6c0}", "1name", "\u{301}a"] {
        assert!(matches!(
            Fixture::new(name)?.whole(BuiltinReader::Name, true)?,
            ReadReply::NoMatch { .. }
        ));
    }
    Ok(())
}
#[test]
fn naturals_and_finite_decimals_are_exact_and_boundary_checked() -> Result<(), ReaderError> {
    for (text, numerator, denominator) in [
        ("0.1", "1", "10"),
        ("-12.340", "-617", "50"),
        ("-0.0", "0", "1"),
        ("123", "123", "1"),
    ] {
        let ReadReply::Matched {
            value: NdfValue::Rational(ref v),
            ..
        } = Fixture::new(text)?.whole(BuiltinReader::Number, true)?
        else {
            return Err(ReaderError::Context);
        };
        assert_eq!(v.numerator().as_bigint().to_string(), numerator);
        assert_eq!(v.denominator().to_string(), denominator);
    }
    let large = "9".repeat(120);
    let ReadReply::Matched {
        value: NdfValue::Integer(ref v),
        ..
    } = Fixture::new(&large)?.whole(BuiltinReader::Nat, true)?
    else {
        return Err(ReaderError::Context);
    };
    assert_eq!(v.as_bigint().to_string(), large);
    for text in ["00", "01", "1.", "1.x"] {
        failed(
            Fixture::new(text)?.whole(BuiltinReader::Number, true)?,
            "InvalidNumber",
        );
    }
    for text in ["1x", "1e2", "1\u{301}"] {
        failed(
            Fixture::new(text)?.whole(BuiltinReader::Nat, true)?,
            "BoundaryMismatch",
        );
    }
    assert!(matches!(
        Fixture::new("1.")?.whole(BuiltinReader::Number, false)?,
        ReadReply::NeedMore { .. }
    ));
    Ok(())
}
#[test]
fn maximal_lexemes_wait_at_every_nonfinal_scalar_boundary() -> Result<(), ReaderError> {
    for (kind, text) in [
        (BuiltinReader::Name, "日本_a"),
        (BuiltinReader::Nat, "123456"),
        (BuiltinReader::Number, "-12.340"),
        (BuiltinReader::Lang, "zh-Hant-CN-x-test"),
        (BuiltinReader::Trivia, "# 日本語"),
    ] {
        let f = Fixture::new(text)?;
        for limit in (0..=text.len()).filter(|i| text.is_char_boundary(*i)) {
            assert!(
                matches!(
                    f.read(
                        kind,
                        0,
                        limit as u64,
                        false,
                        &mut budget(),
                        &mut SourceAdmission::default()
                    )?,
                    ReadReply::NeedMore { .. }
                ),
                "{kind:?} {limit}"
            );
        }
    }
    Ok(())
}
#[test]
fn language_is_well_formed_abnf_not_registry_validity() -> Result<(), ReaderError> {
    for text in [
        "ja",
        "EN-latn-us",
        "x-a",
        "i-klingon",
        "SGN-be-FR",
        "zh-cmn-Hans-CN",
        "en-1234",
        "en-1-ab",
        "de-1901-1901",
        "en-a-ab-a-cd",
        "abc-def-ghi-jkl-Latn-123-abcde-x-a",
    ] {
        let ReadReply::Matched { value, .. } =
            Fixture::new(text)?.whole(BuiltinReader::Lang, true)?
        else {
            return Err(ReaderError::Context);
        };
        assert_eq!(value, NdfValue::Text(text.into()));
    }
    for text in [
        "x",
        "en-",
        "en-a",
        "en-abcdefghi",
        "a",
        "en--US",
        "en-x",
        "en-12",
    ] {
        failed(
            Fixture::new(text)?.whole(BuiltinReader::Lang, true)?,
            "InvalidLanguageTag",
        );
    }
    failed(
        Fixture::new("en日")?.whole(BuiltinReader::Lang, true)?,
        "BoundaryMismatch",
    );
    Ok(())
}
#[test]
fn text_decoding_retains_each_exact_or_transformed_source_range() -> Result<(), ReaderError> {
    let raw = "\"日\\n\\r\\t\\\\\\\"\\u{1f600}\\u{0}\\u{10ffff}\"tail";
    let f = Fixture::new(raw)?;
    let ReadReply::Matched {
        value,
        end,
        sources,
        source_maps,
        ..
    } = f.whole(BuiltinReader::Text, true)?
    else {
        return Err(ReaderError::Context);
    };
    let expected = "日\n\r\t\\\"😀\0\u{10ffff}";
    assert_eq!(value, NdfValue::Text(expected.into()));
    assert_eq!(sources[0].text(), expected);
    assert_eq!(end, (raw.len() - 4) as u64);
    assert_eq!(source_maps.len(), expected.chars().count());
    let mut previous = 0;
    for mapping in source_maps {
        assert_eq!(mapping.target.start(), previous);
        previous = mapping.target.end();
        let source = f.source.slice(&mapping.source)?;
        let target = sources[0].slice(&mapping.target)?;
        if mapping.kind == nepl3_core::origin::MappingKind::Exact {
            assert_eq!(source, target);
        } else {
            assert!(source.starts_with('\\'));
        }
    }
    assert_eq!(previous, expected.len() as u64);
    let ReadReply::Matched {
        sources,
        source_maps,
        ..
    } = Fixture::new("\"\"")?.whole(BuiltinReader::Text, true)?
    else {
        return Err(ReaderError::Context);
    };
    assert!(sources[0].text().is_empty());
    assert_eq!(source_maps[0].source.start(), 1);
    assert_eq!(source_maps[0].target.end(), 0);
    Ok(())
}
#[test]
fn committed_text_rejects_invalid_escapes_scalars_and_direct_lines() -> Result<(), ReaderError> {
    for text in ["\"\\q\"", "\"\\uX\"", "\"\\u{g}\""] {
        failed(
            Fixture::new(text)?.whole(BuiltinReader::Text, false)?,
            "InvalidEscape",
        );
    }
    for text in [
        "\"\\u{}\"",
        "\"\\u{d800}\"",
        "\"\\u{110000}\"",
        "\"\\u{0000000}\"",
    ] {
        failed(
            Fixture::new(text)?.whole(BuiltinReader::Text, true)?,
            "InvalidScalar",
        );
    }
    for text in ["\"a\nb\"", "\"a\r\nb\""] {
        failed(
            Fixture::new(text)?.whole(BuiltinReader::Text, false)?,
            "DirectLineBreak",
        );
    }
    let raw = "\"日本\\u{1f600}\\n\"";
    let f = Fixture::new(raw)?;
    for limit in (0..raw.len()).filter(|i| raw.is_char_boundary(*i)) {
        assert!(matches!(
            f.read(
                BuiltinReader::Text,
                0,
                limit as u64,
                false,
                &mut budget(),
                &mut SourceAdmission::default()
            )?,
            ReadReply::NeedMore { .. }
        ));
    }
    assert!(matches!(
        Fixture::new("\"abc")?.whole(BuiltinReader::Text, true)?,
        ReadReply::Failed { .. }
    ));
    Ok(())
}
#[test]
fn trivia_keeps_ascii_line_boundaries_and_bom_position() -> Result<(), ReaderError> {
    let f = Fixture::new("\u{feff} \r\n# comment\r\nname")?;
    let mut admission = SourceAdmission::default();
    let mut b = budget();
    for (start, end) in [(0, 3), (3, 6), (6, 15), (15, 17)] {
        let ReadReply::Matched {
            end: actual, value, ..
        } = f.read(
            BuiltinReader::Trivia,
            start,
            f.source.text().len() as u64,
            true,
            &mut b,
            &mut admission,
        )?
        else {
            return Err(ReaderError::Context);
        };
        assert_eq!(actual, end);
        assert_eq!(value, NdfValue::Unit);
    }
    for text in ["\u{a0}", "\u{3000}"] {
        assert!(matches!(
            Fixture::new(text)?.whole(BuiltinReader::Trivia, true)?,
            ReadReply::NoMatch { .. }
        ));
    }
    let f = Fixture::new("a\u{feff}")?;
    assert!(matches!(
        f.read(
            BuiltinReader::Trivia,
            1,
            4,
            true,
            &mut budget(),
            &mut SourceAdmission::default()
        )?,
        ReadReply::NoMatch { .. }
    ));
    Ok(())
}
#[test]
fn generated_source_admission_is_once_and_unsuccessful_text_has_no_source_charge()
-> Result<(), ReaderError> {
    for text in ["name", "\"partial", "\"\\q\""] {
        let f = Fixture::new(text)?;
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let reply = f.read(
            BuiltinReader::Text,
            0,
            text.len() as u64,
            false,
            &mut b,
            &mut admission,
        )?;
        assert!(!matches!(reply, ReadReply::Matched { .. }));
        assert_eq!(b.usage().source_bytes, text.len() as u64);
    }
    let f = Fixture::new("\"a\\n\"")?;
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    for _ in 0..2 {
        let reply = f.read(
            BuiltinReader::Text,
            0,
            f.source.text().len() as u64,
            true,
            &mut b,
            &mut admission,
        )?;
        assert!(matches!(reply, ReadReply::Matched { .. }));
        assert_eq!(b.usage().source_bytes, f.source.text().len() as u64 + 2);
    }
    assert_eq!(
        f.store.snapshots().len(),
        1,
        "builtin returns artifacts without mutating the caller store"
    );
    Ok(())
}
#[test]
fn builtin_budget_stops_are_typed_and_do_not_return_partial_tokens() -> Result<(), ReaderError> {
    for (kind, text) in [
        (BuiltinReader::Nat, "123456"),
        (BuiltinReader::Number, "0.123"),
        (BuiltinReader::Text, "\"a\\n\""),
        (BuiltinReader::Lang, "en-Latn"),
    ] {
        let f = Fixture::new(text)?;
        for reason in [
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::SourceLimit => limits.source_bytes = 0,
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::DepthLimit => limits.depth = 0,
                _ => unreachable!(),
            }
            let mut b = Budget::new(limits);
            let reply = f.read(
                kind,
                0,
                text.len() as u64,
                true,
                &mut b,
                &mut SourceAdmission::default(),
            )?;
            assert!(
                matches!(reply,ReadReply::Stopped { reason: actual,.. } if actual==reason),
                "{kind:?} {reply:?}"
            );
            assert_eq!(b.poll(), Err(reason));
        }
    }
    Ok(())
}
fn token_plan(f: &Fixture) -> ReaderPlan {
    ReaderPlan {
        schema: f.context.schema.clone(),
        state_type: TypeDescriptor::Unit,
        expressions: vec![
            ReaderExpr::Literal("a".into()),
            ReaderExpr::Literal("".into()),
        ],
        rules: vec![
            ReaderRule {
                name: "a".into(),
                root: ReaderId(0),
                output: TypeDescriptor::Unit,
            },
            ReaderRule {
                name: "empty".into(),
                root: ReaderId(1),
                output: TypeDescriptor::Unit,
            },
        ],
        providers: vec![],
    }
}
fn token_kind(f: &Fixture) -> Result<KindRef, ReaderError> {
    Ok(KindRef {
        schema: f.context.schema.clone(),
        local_kind: f.registry.kind_id(&f.context.schema, "BuiltinReader")?,
    })
}
#[test]
fn mode_take_is_ordered_choice_and_does_not_consume_trailing_trivia() -> Result<(), ReaderError> {
    let f = Fixture::new(" \r\nabc trailing")?;
    let p = token_plan(&f);
    let checked = p.check(&f.registry, &mut budget())?;
    let modes = vec![ReaderMode {
        name: "default".into(),
        skip: vec![SkipRule {
            reader: TokenReader::Builtin(BuiltinReader::Trivia),
        }],
        take: vec![
            TakeRule {
                reader: TokenReader::Rule("a".into()),
                kind: token_kind(&f)?,
            },
            TakeRule {
                reader: TokenReader::Builtin(BuiltinReader::Name),
                kind: token_kind(&f)?,
            },
        ],
    }];
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&f.registry, &f.store, &mut admission)
            .map_err(|_| ReaderError::Context)?;
    let context = f
        .context
        .check(&mut codec, &f.store, &f.registry, &mut b)
        .map_err(|_| ReaderError::Context)?;
    let mut session =
        TokenizationSession::new("tokens".into(), &modes, &checked, &f.registry, &mut b)?;
    let reply = session.read(
        TokenizationRequest {
            snapshot: &f.source,
            start: 0,
            limit: f.source.text().len() as u64,
            final_input: true,
            context: &context,
            state: &NdfValue::Unit,
        },
        &f.store,
        &mut b,
        &mut admission,
    )?;
    let TokenizationOutcome::Token(token) = reply.outcome else {
        return Err(ReaderError::Context);
    };
    assert_eq!(
        reply.cursor, 4,
        "first declared literal wins despite Name consuming abc"
    );
    assert_eq!(token.head.start(), 3);
    assert_eq!(token.payload, NdfValue::Unit);
    assert_eq!(reply.trivia.len(), 1);
    assert_eq!(reply.trivia, token.leading_trivia);
    let reply = session.read(
        TokenizationRequest {
            snapshot: &f.source,
            start: 4,
            limit: f.source.text().len() as u64,
            final_input: true,
            context: &context,
            state: &NdfValue::Unit,
        },
        &f.store,
        &mut b,
        &mut admission,
    )?;
    let TokenizationOutcome::Token(token) = reply.outcome else {
        return Err(ReaderError::Context);
    };
    assert_eq!(token.payload, NdfValue::Text("bc".into()));
    assert_eq!(
        reply.cursor, 6,
        "space after token belongs to next host read"
    );
    Ok(())
}
#[test]
fn mode_text_reservation_is_lazy_and_echo_is_checked() -> Result<(), ReaderError> {
    let f = Fixture::new("name \"a\\n\"")?;
    let p = token_plan(&f);
    let checked = p.check(&f.registry, &mut budget())?;
    let modes = vec![ReaderMode {
        name: "default".into(),
        skip: vec![SkipRule {
            reader: TokenReader::Builtin(BuiltinReader::Trivia),
        }],
        take: vec![
            TakeRule {
                reader: TokenReader::Builtin(BuiltinReader::Text),
                kind: token_kind(&f)?,
            },
            TakeRule {
                reader: TokenReader::Builtin(BuiltinReader::Name),
                kind: token_kind(&f)?,
            },
        ],
    }];
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&f.registry, &f.store, &mut admission)
            .map_err(|_| ReaderError::Context)?;
    let context = f
        .context
        .check(&mut codec, &f.store, &f.registry, &mut b)
        .map_err(|_| ReaderError::Context)?;
    let mut session =
        TokenizationSession::new("tokens".into(), &modes, &checked, &f.registry, &mut b)?;
    let reply = session.read(
        TokenizationRequest {
            snapshot: &f.source,
            start: 0,
            limit: f.source.text().len() as u64,
            final_input: true,
            context: &context,
            state: &NdfValue::Unit,
        },
        &f.store,
        &mut b,
        &mut admission,
    )?;
    assert!(matches!(reply.outcome, TokenizationOutcome::Token(_)));
    assert_eq!(reply.cursor, 4);
    assert!(reply.sources.is_empty());
    let reply = session.read(
        TokenizationRequest {
            snapshot: &f.source,
            start: 4,
            limit: f.source.text().len() as u64,
            final_input: true,
            context: &context,
            state: &NdfValue::Unit,
        },
        &f.store,
        &mut b,
        &mut admission,
    )?;
    let TokenizationOutcome::Reserve { request } = reply.outcome else {
        return Err(ReaderError::Context);
    };
    assert_eq!(request.start, 5);
    assert_eq!(reply.trivia.len(), 1);
    assert_eq!(reply.cursor, 5);
    let reservation = SourceReservation {
        source_id: SourceId("decoded".into()),
        revision: 0,
        uri: "memory:decoded".into(),
    };
    let mut forged = request.clone();
    forged.request_id += 1;
    assert_eq!(
        session.reserve(&forged, &reservation, &f.store, &mut b, &mut admission),
        Err(ReaderError::Continuation)
    );
    assert_eq!(
        session.reserve(
            &request,
            &reservation,
            &f.store,
            &mut Budget::new(b.limits()),
            &mut admission
        ),
        Err(ReaderError::Continuation),
        "a fresh budget cannot resume accepted skip state"
    );
    let reply = session.reserve(&request, &reservation, &f.store, &mut b, &mut admission)?;
    let TokenizationOutcome::Token(token) = reply.outcome else {
        return Err(ReaderError::Context);
    };
    assert_eq!(token.payload, NdfValue::Text("a\n".into()));
    assert_eq!(reply.sources.len(), 1);
    assert_eq!(reply.source_maps.len(), 2);
    assert_eq!(
        session.reserve(&request, &reservation, &f.store, &mut b, &mut admission),
        Err(ReaderError::NoPending)
    );
    let pending = session.read(
        TokenizationRequest {
            snapshot: &f.source,
            start: 4,
            limit: f.source.text().len() as u64,
            final_input: true,
            context: &context,
            state: &NdfValue::Unit,
        },
        &f.store,
        &mut b,
        &mut admission,
    )?;
    let TokenizationOutcome::Reserve { request } = pending.outcome else {
        return Err(ReaderError::Context);
    };
    let expected_trivia = pending.trivia;
    let expected_sources = pending.sources;
    b.cancel();
    let cancelled = session.reserve(&request, &reservation, &f.store, &mut b, &mut admission)?;
    assert!(matches!(
        cancelled.outcome,
        TokenizationOutcome::Stopped {
            reason: StopReason::Cancelled
        }
    ));
    assert_eq!(cancelled.cursor, 5);
    assert_eq!(cancelled.trivia, expected_trivia);
    assert_eq!(cancelled.sources, expected_sources);
    assert_eq!(cancelled.new_state, Some(NdfValue::Unit));
    assert_eq!(
        session.reserve(&request, &reservation, &f.store, &mut b, &mut admission),
        Err(ReaderError::NoPending)
    );
    let retry = session.read(
        TokenizationRequest {
            snapshot: &f.source,
            start: 4,
            limit: f.source.text().len() as u64,
            final_input: true,
            context: &context,
            state: &NdfValue::Unit,
        },
        &f.store,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    assert!(matches!(retry.outcome, TokenizationOutcome::Reserve { .. }));
    Ok(())
}
#[test]
fn reservation_locator_validation_obeys_work_limit_before_scanning() -> Result<(), ReaderError> {
    let f = Fixture::new("\"a\"")?;
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&f.registry, &f.store, &mut admission)
            .map_err(|_| ReaderError::Context)?;
    let context = f
        .context
        .check(&mut codec, &f.store, &f.registry, &mut budget())
        .map_err(|_| ReaderError::Context)?;
    for invalid in [false, true] {
        let reservation = SourceReservation {
            source_id: SourceId("decoded".into()),
            revision: 0,
            uri: format!(
                "memory:{}{}",
                "x".repeat(100_000),
                if invalid { " " } else { "" }
            ),
        };
        let mut limits = budget().limits();
        limits.work = 1000;
        let mut b = Budget::new(limits);
        let reply = read(
            BuiltinReader::Text,
            ReadRequest {
                snapshot: &f.source,
                start: 0,
                limit: 3,
                final_input: true,
                context: &context,
                state: &NdfValue::Unit,
            },
            Some(&reservation),
            &f.registry,
            &f.store,
            &mut b,
            &mut SourceAdmission::default(),
        )?;
        assert!(matches!(
            reply,
            ReadReply::Stopped {
                reason: StopReason::WorkLimit,
                ..
            }
        ));
        assert!(b.usage().work < 1000);
    }
    Ok(())
}
#[test]
fn empty_skip_and_take_readers_are_nonprogress_failures() -> Result<(), ReaderError> {
    let f = Fixture::new("a")?;
    let p = token_plan(&f);
    let checked = p.check(&f.registry, &mut budget())?;
    for skip in [true, false] {
        let modes = vec![ReaderMode {
            name: "default".into(),
            skip: if skip {
                vec![SkipRule {
                    reader: TokenReader::Rule("empty".into()),
                }]
            } else {
                vec![]
            },
            take: vec![TakeRule {
                reader: TokenReader::Rule("empty".into()),
                kind: token_kind(&f)?,
            }],
        }];
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let mut codec =
            nepl3_wire::foundation::FoundationCodec::new(&f.registry, &f.store, &mut admission)
                .map_err(|_| ReaderError::Context)?;
        let context = f
            .context
            .check(&mut codec, &f.store, &f.registry, &mut b)
            .map_err(|_| ReaderError::Context)?;
        let mut session =
            TokenizationSession::new("tokens".into(), &modes, &checked, &f.registry, &mut b)?;
        let reply = session.read(
            TokenizationRequest {
                snapshot: &f.source,
                start: 0,
                limit: 1,
                final_input: true,
                context: &context,
                state: &NdfValue::Unit,
            },
            &f.store,
            &mut b,
            &mut admission,
        )?;
        assert!(
            matches!(reply.outcome,TokenizationOutcome::Failed {diagnostic,..} if diagnostic.code=="NonProgress")
        );
        assert_eq!(reply.cursor, 0);
        assert!(reply.trivia.is_empty());
    }
    Ok(())
}
