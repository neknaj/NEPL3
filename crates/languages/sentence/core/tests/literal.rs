use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
};
use nepl3_sentence_core::{
    literal::{self, SentenceCode, SentenceError, SentenceLiteral, SentenceOutcome},
    model::{InlineRef, Kind, Root, SentenceRef, SentenceValue},
};
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 2_000_000,
        work: 100_000_000,
        depth: 100_000,
        nodes: 2_000_000,
        allocation_units: 200_000_000,
        output_bytes: 2_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn source(text: &str) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId("sentence".into()),
        1,
        "memory:sentence".into(),
        text.as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)
}
fn parse(text: &str, r: &SchemaRegistry) -> Result<SentenceLiteral, String> {
    let s = source(text)?;
    let scan = literal::read(
        &s,
        0,
        s.text().len() as u64,
        true,
        r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    match scan.outcome {
        SentenceOutcome::Matched(v) => Ok(v),
        other => Err(err(other)),
    }
}

#[test]
fn referenced_literal_roundtrip_does_not_serialize_the_document_per_token() -> Result<(), String> {
    use nepl3_sentence_core::portable::literal as payload;
    use nepl3_wire::foundation::FoundationCodec;
    let r = registry()?;
    let empty = SourceStore::default();
    let mut sizes = vec![];
    for suffix in [String::new(), "x".repeat(100_000)] {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        let v = parse(&format!("\"[漢/かん]𝄞\" {suffix}"), &r)?;
        let raw = payload::to_value(&v.syntax, &r, &mut codec, &mut b()).map_err(err)?;
        let bytes = nepl3_wire::encode(&raw, &mut b()).map_err(err)?;
        sizes.push(bytes.len());
        let raw = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
        let actual = payload::from_value(
            &raw,
            &v.syntax.sources[0],
            &v.syntax.views[0].view,
            &r,
            &mut codec,
            &mut b(),
        )
        .map_err(err)?;
        assert_eq!(actual, v.syntax);
    }
    // The suffix is outside the literal. Only fixed-size snapshot identity
    // changes; no SourceContent/text from the rest of the document is sent.
    assert_eq!(sizes[0], sizes[1]);
    assert!(sizes[1] < 20_000);
    Ok(())
}

#[test]
fn migrated_doc_payload_preserves_annotations_and_exact_owner_after_cbor() -> Result<(), String> {
    use nepl3_sentence_core::portable::literal as payload;
    use nepl3_wire::foundation::FoundationCodec;
    let r = registry()?;
    let v = parse(
        &format!("\"前{{[文/ぶん]/note}}後\"{}", " ".repeat(65_536)),
        &r,
    )?;
    let owner = &v.syntax.sources[0];
    let mut ambient = SourceStore::default();
    ambient.insert(owner.clone()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &ambient, &mut admission).map_err(err)?;
    let raw = payload::to_value(&v.syntax, &r, &mut codec, &mut b()).map_err(err)?;
    let bytes = nepl3_wire::encode(&raw, &mut b()).map_err(err)?;
    let short = parse("\"前{[文/ぶん]/note}後\"", &r)?;
    let mut short_admission = SourceAdmission::default();
    let empty = SourceStore::default();
    let mut short_codec = FoundationCodec::new(&r, &empty, &mut short_admission).map_err(err)?;
    let short_raw =
        payload::to_value(&short.syntax, &r, &mut short_codec, &mut b()).map_err(err)?;
    let short_bytes = nepl3_wire::encode(&short_raw, &mut b()).map_err(err)?;
    // Adding 64 KiB outside this literal changes snapshot identity only. The
    // real reader's richer provenance has a different fixed cost from the old
    // hand-authored Doc fixture; no owner source bytes may enter the payload.
    assert_eq!(bytes.len(), short_bytes.len());
    let received = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let actual = payload::from_value(
        &received,
        owner,
        &v.syntax.views[0].view,
        &r,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    assert_eq!(actual, v.syntax);
    // Independent expected meaning, not only an encoder/decoder roundtrip.
    assert_eq!(
        actual.value.nodes,
        vec![
            Kind::Text { text: "前".into() },
            Kind::Text { text: "文".into() },
            Kind::Text {
                text: "ぶん".into()
            },
            Kind::Ruby {
                base: InlineRef(1),
                reading: InlineRef(2)
            },
            Kind::Text {
                text: "note".into()
            },
            Kind::InlineAnno {
                base: InlineRef(3),
                notes: vec![InlineRef(4)]
            },
            Kind::Text { text: "後".into() },
            Kind::Sentence {
                inlines: vec![InlineRef(0), InlineRef(5), InlineRef(6)]
            },
        ]
    );
    assert_eq!(actual.value.root, Root::Sentence(SentenceRef(7)));
    for wrong in [
        SourceSnapshot::new(
            SourceId("sentence".into()),
            2,
            "memory:sentence".into(),
            owner.text().as_bytes().to_vec(),
            &mut b(),
        )
        .map_err(err)?,
        source("different bytes")?,
    ] {
        // The correct ambient snapshot cannot replace the explicit owner.
        assert!(
            payload::from_value(
                &received,
                &wrong,
                &v.syntax.views[0].view,
                &r,
                &mut codec,
                &mut b()
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn referenced_literal_rejects_wrong_owner_even_when_ambient_source_is_correct() -> Result<(), String>
{
    use nepl3_sentence_core::portable::{self, literal as payload};
    use nepl3_wire::foundation::FoundationCodec;
    let r = registry()?;
    let v = parse("\"[漢/かん]\" tail", &r)?;
    let mut ambient = SourceStore::default();
    ambient.insert(v.syntax.sources[0].clone()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &ambient, &mut admission).map_err(err)?;
    let raw = payload::to_value(&v.syntax, &r, &mut codec, &mut b()).map_err(err)?;
    // Same id/revision and same literal, different document bytes.
    let wrong = source("\"[漢/かん]\" fail")?;
    assert!(
        payload::from_value(
            &raw,
            &wrong,
            &v.syntax.views[0].view,
            &r,
            &mut codec,
            &mut b()
        )
        .is_err()
    );
    let mut wrong_kind = v.syntax.clone();
    wrong_kind.value.nodes[0] = Kind::Code { text: "漢".into() };
    assert!(matches!(
        payload::to_value(&wrong_kind, &r, &mut codec, &mut b()),
        Err(portable::Error::Shape)
    ));
    let mut outside = v.syntax.clone();
    outside.locations[0].head = None;
    outside.locations[0].cover = Some(
        outside.sources[0]
            .span(v.head.end() + 1, outside.sources[0].text().len() as u64)
            .map_err(err)?,
    );
    assert!(matches!(
        payload::to_value(&outside, &r, &mut codec, &mut b()),
        Err(portable::Error::Shape)
    ));
    let mut forged = raw.clone();
    let nepl3_core::value::NdfValue::Record(record) = &mut forged else {
        return Err("record".into());
    };
    // Correct wire shape is insufficient: the view must belong to Sentence root.
    let nepl3_core::value::NdfValue::Record(view) = &mut record.fields[3] else {
        return Err("view".into());
    };
    view.fields[0] = nepl3_core::value::NdfValue::U64(0);
    assert!(
        payload::from_value(
            &forged,
            &v.syntax.sources[0],
            &v.syntax.views[0].view,
            &r,
            &mut codec,
            &mut b()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn literal_payload_binds_the_separately_supplied_complete_view() -> Result<(), String> {
    use nepl3_core::{
        source::Digest, value::NdfValue, value_codec::FoundationValueCodec, view::ViewBundle,
    };
    use nepl3_sentence_core::portable::{self, literal as payload};
    use nepl3_wire::foundation::FoundationCodec;
    let r = registry()?;
    let literal = parse("\"[漢/かん]\"", &r)?;
    let owner = &literal.syntax.sources[0];
    let view = &literal.syntax.views[0].view;
    let mut sources = SourceStore::default();
    sources.insert(owner.clone()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
    let raw = payload::to_value(&literal.syntax, &r, &mut codec, &mut b()).map_err(err)?;
    let NdfValue::Record(record) = &raw else {
        return Err("payload".into());
    };
    let NdfValue::Record(reference) = &record.fields[3] else {
        return Err("view reference".into());
    };
    assert_eq!(reference.kind, "SentenceLiteralView");
    // Independent preimage construction checks both the dedicated domain and
    // the complete canonical ViewBundle, rather than a roundtrip alone.
    let encoded = codec.encode_views(view, &mut b()).map_err(err)?;
    let mut preimage = b"NEPL3.Sentence.Literal.View.v1\0".to_vec();
    preimage.extend(nepl3_wire::encode(&encoded, &mut b()).map_err(err)?);
    assert_eq!(
        reference.fields[2],
        NdfValue::Bytes(Digest::of(&preimage).0.to_vec())
    );
    let empty = ViewBundle {
        roots: vec![],
        elements: vec![],
    };
    codec.encode_views(&empty, &mut b()).map_err(err)?;
    assert_eq!(
        payload::from_value(&raw, owner, &empty, &r, &mut codec, &mut b()),
        Err(portable::Error::LiteralViewMismatch)
    );
    for length in [31, 32, 33] {
        let mut forged = raw.clone();
        let NdfValue::Record(record) = &mut forged else {
            return Err("payload".into());
        };
        let NdfValue::Record(reference) = &mut record.fields[3] else {
            return Err("view reference".into());
        };
        reference.fields[2] = NdfValue::Bytes(vec![0; length]);
        let result = payload::from_value(&forged, owner, view, &r, &mut codec, &mut b());
        if length == 32 {
            assert_eq!(result, Err(portable::Error::LiteralViewMismatch));
        } else {
            assert!(matches!(
                result,
                Err(portable::Error::Schema(
                    nepl3_core::schema::SchemaError::WrongType
                ))
            ));
        }
    }
    let mut old = raw.clone();
    let NdfValue::Record(record) = &mut old else {
        return Err("payload".into());
    };
    let NdfValue::Record(reference) = &mut record.fields[3] else {
        return Err("view reference".into());
    };
    reference.kind = "SentenceView".into();
    reference.fields[2] = encoded;
    assert!(matches!(
        payload::from_value(&old, owner, view, &r, &mut codec, &mut b()),
        Err(portable::Error::Schema(
            nepl3_core::schema::SchemaError::WrongType
        ))
    ));
    Ok(())
}

#[test]
fn referenced_literal_encode_and_decode_preserve_stop_reasons() -> Result<(), String> {
    use nepl3_sentence_core::portable::{self, literal as payload};
    use nepl3_wire::foundation::FoundationCodec;
    let r = registry()?;
    let v = parse("\"a\"", &r)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let raw = payload::to_value(&v.syntax, &r, &mut codec, &mut b()).map_err(err)?;
    for reason in [
        StopReason::SourceLimit,
        StopReason::WorkLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::OutputLimit,
        StopReason::Cancelled,
    ] {
        for receiving in [false, true] {
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
            let mut limits = b().limits();
            match reason {
                StopReason::SourceLimit => limits.source_bytes = 0,
                StopReason::WorkLimit => limits.work = 0,
                StopReason::NodeLimit => limits.nodes = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::DepthLimit => limits.depth = 0,
                StopReason::OutputLimit => limits.output_bytes = 0,
                _ => {}
            }
            let mut budget = Budget::new(limits);
            if reason == StopReason::Cancelled {
                budget.cancel();
            }
            if receiving {
                assert_eq!(
                    payload::from_value(
                        &raw,
                        &v.syntax.sources[0],
                        &v.syntax.views[0].view,
                        &r,
                        &mut codec,
                        &mut budget
                    ),
                    Err(portable::Error::Stopped(reason))
                );
            } else {
                assert_eq!(
                    payload::to_value(&v.syntax, &r, &mut codec, &mut budget),
                    Err(portable::Error::Stopped(reason))
                );
            }
            assert_eq!(budget.poll(), Err(reason));
        }
    }
    Ok(())
}

#[test]
fn literal_has_independent_sentence_meaning_and_closed_source_provenance() -> Result<(), String> {
    let r = registry()?;
    let literal = parse("\"[漢/かん]{語/note/補足}\"", &r)?;
    // Expected constructor arena is authored independently of the reader.
    let expected = SentenceValue {
        root: Root::Sentence(SentenceRef(7)),
        nodes: vec![
            Kind::Text { text: "漢".into() },
            Kind::Text {
                text: "かん".into(),
            },
            Kind::Ruby {
                base: InlineRef(0),
                reading: InlineRef(1),
            },
            Kind::Text { text: "語".into() },
            Kind::Text {
                text: "note".into(),
            },
            Kind::Text {
                text: "補足".into(),
            },
            Kind::InlineAnno {
                base: InlineRef(3),
                notes: vec![InlineRef(4), InlineRef(5)],
            },
            Kind::Sentence {
                inlines: vec![InlineRef(2), InlineRef(6)],
            },
        ],
        embeds: vec![],
    };
    assert_eq!(literal.syntax.value, expected);
    let mut budget = b();
    literal
        .syntax
        .validate(&r, &mut budget, &mut SourceAdmission::default())
        .map_err(err)?;
    assert_eq!(
        budget.usage().source_bytes,
        literal.syntax.sources[0].text().len() as u64
    );
    assert_eq!(literal.syntax.views[0].owner, 7);
    assert_eq!(literal.syntax.locations.len(), expected.nodes.len());
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = nepl3_wire::foundation::FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let raw =
        nepl3_sentence_core::portable::syntax::to_value(&literal.syntax, &r, &mut c, &mut b())
            .map_err(err)?;
    let bytes = nepl3_wire::encode(&raw, &mut b()).map_err(err)?;
    let raw = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let actual = nepl3_sentence_core::portable::syntax::from_value(&raw, &r, &mut c, &mut b())
        .map_err(err)?;
    assert_eq!(actual, literal.syntax);
    Ok(())
}

#[test]
fn escapes_are_not_reparsed_and_keep_original_scalar_ranges() -> Result<(), String> {
    let r = registry()?;
    let v = parse(r#""\u{5b}x\/y\]\{a\}\n\r\t\"\\𝄞""#, &r)?;
    assert_eq!(
        v.syntax.value.nodes[0],
        Kind::Text {
            text: "[x/y]{a}\n\r\t\"\\𝄞".into()
        }
    );
    assert_eq!(v.syntax.value.nodes.len(), 2);
    let escape_kind = r
        .kind_id(
            &v.syntax.views[0].view.elements[0].kind.schema,
            "View:Escape",
        )
        .map_err(err)?;
    let escape = v.syntax.views[0]
        .view
        .elements
        .iter()
        .find(|v| v.kind.local_kind == escape_kind)
        .ok_or("escape view")?;
    assert_eq!(
        v.syntax.sources[0].slice(&escape.span).map_err(err)?,
        r"\u{5b}"
    );
    Ok(())
}

#[test]
fn literal_window_commits_exactly_one_head_and_incomplete_scans_return_no_tree()
-> Result<(), String> {
    let r = registry()?;
    let s = source("前 \"{[漢/かん]/補足}𝄞\" trailing")?;
    let start = "前 ".len() as u64;
    let end = s.text().find(" trailing").ok_or("end")? as u64;
    for limit in (start as usize..end as usize).filter(|i| s.text().is_char_boundary(*i)) {
        let scan = literal::read(
            &s,
            start,
            limit as u64,
            false,
            &r,
            &mut b(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        assert!(
            matches!(scan.outcome, SentenceOutcome::NeedMore),
            "limit {limit}"
        );
    }
    let scan = literal::read(
        &s,
        start,
        s.text().len() as u64,
        true,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let SentenceOutcome::Matched(v) = scan.outcome else {
        return Err("literal".into());
    };
    assert_eq!((v.head.start(), v.head.end()), (start, end));
    let scan = literal::read(
        &s,
        0,
        start,
        true,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert!(matches!(scan.outcome, SentenceOutcome::NoMatch));
    assert!(matches!(
        literal::read(
            &s,
            1,
            end,
            true,
            &r,
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(SentenceError::Source(_))
    ));
    Ok(())
}

#[test]
fn annotation_failures_preserve_exact_original_byte_ranges() -> Result<(), String> {
    let r = registry()?;
    // Byte offsets are counted in the original UTF-8 input: the opening '['
    // after '前' starts at byte 4, and EOF after 'ぶん' is byte 15. A CRLF
    // failure points at CR alone; empty parts retain a zero-width location.
    // These expectations migrate the former Doc reader's diagnostic contract.
    for (text, code, primary, opening) in [
        (
            "\"前[文/ぶん",
            SentenceCode::UnclosedAnnotation,
            (15, 15),
            Some((4, 5)),
        ),
        (
            "\"[x/y\"",
            SentenceCode::UnclosedAnnotation,
            (5, 6),
            Some((1, 2)),
        ),
        (
            "\"[x/y\r\n",
            SentenceCode::UnclosedAnnotation,
            (5, 6),
            Some((1, 2)),
        ),
        (
            "\"[/y]\"",
            SentenceCode::EmptyAnnotationPart,
            (2, 2),
            Some((1, 2)),
        ),
        (
            "\"[x/y/z]\"",
            SentenceCode::SeparatorCount,
            (5, 6),
            Some((1, 2)),
        ),
        ("\"]\"", SentenceCode::UnexpectedDelimiter, (1, 2), None),
    ] {
        let s = source(text)?;
        let scan = literal::read(
            &s,
            0,
            text.len() as u64,
            true,
            &r,
            &mut b(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        let SentenceOutcome::Failed(failure) = scan.outcome else {
            return Err(format!("expected diagnostic for {text:?}"));
        };
        assert_eq!(failure.code, code, "{text:?}");
        assert_eq!(
            (failure.primary.start(), failure.primary.end()),
            primary,
            "{text:?}"
        );
        s.slice(&failure.primary).map_err(err)?;
        if let Some(span) = &failure.opening {
            assert_eq!(s.slice(span).map_err(err)?, "[");
        }
        assert_eq!(
            failure.opening.map(|span| (span.start(), span.end())),
            opening,
            "{text:?}"
        );
    }
    Ok(())
}

#[test]
fn malformed_literals_have_typed_source_bound_failures() -> Result<(), String> {
    let r = registry()?;
    for (text, code) in [
        ("\"abc", SentenceCode::UnterminatedLiteral),
        ("\"[a/b\"", SentenceCode::UnclosedAnnotation),
        ("\"]\"", SentenceCode::UnexpectedDelimiter),
        ("\"[a/b/c]\"", SentenceCode::SeparatorCount),
        ("\"[a]\"", SentenceCode::SeparatorCount),
        ("\"[/b]\"", SentenceCode::EmptyAnnotationPart),
        ("\"{a/}\"", SentenceCode::EmptyAnnotationPart),
        ("\"a\nb\"", SentenceCode::DirectLineBreak),
        (r#""\q""#, SentenceCode::InvalidEscape),
        (r#""\u{d800}""#, SentenceCode::InvalidScalar),
        (r#""\u{110000}""#, SentenceCode::InvalidScalar),
        (r#""\u{}""#, SentenceCode::InvalidScalar),
    ] {
        let s = source(text)?;
        let scan = literal::read(
            &s,
            0,
            text.len() as u64,
            true,
            &r,
            &mut b(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        let SentenceOutcome::Failed(failure) = scan.outcome else {
            return Err(text.into());
        };
        assert_eq!(failure.code, code, "{text}");
        s.slice(&failure.primary).map_err(err)?;
        if let Some(opening) = failure.opening {
            assert!(matches!(s.slice(&opening).map_err(err)?, "[" | "{"));
        }
    }
    Ok(())
}

#[test]
fn migrated_doc_literal_cases_preserve_text_and_annotation_structure() -> Result<(), String> {
    let r = registry()?;
    for (text, expected, ruby, anno) in [
        ("\"\"", vec![], 0, 0),
        ("\"a/b\"", vec!["a/b"], 0, 0),
        (r#""\[a\/b\]""#, vec!["[a/b]"], 0, 0),
        (r#""\u{5B}x\u{2F}y\u{5D}""#, vec!["[x/y]"], 0, 0),
        (
            "\"これは{[文書/ぶんしょ]/document}を記述する。\"",
            vec!["これは", "文書", "ぶんしょ", "document", "を記述する。"],
            1,
            1,
        ),
        (r#""[ /\n]""#, vec![" ", "\n"], 1, 0),
    ] {
        let literal = parse(text, &r)?;
        let syntax = &literal.syntax;
        let actual: Vec<_> = syntax
            .value
            .nodes
            .iter()
            .filter_map(|kind| match kind {
                Kind::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(actual, expected, "{text}");
        assert_eq!(
            syntax
                .value
                .nodes
                .iter()
                .filter(|kind| matches!(kind, Kind::Ruby { .. }))
                .count(),
            ruby
        );
        assert_eq!(
            syntax
                .value
                .nodes
                .iter()
                .filter(|kind| matches!(kind, Kind::InlineAnno { .. }))
                .count(),
            anno
        );
        let mut operation = b();
        syntax
            .validate(&r, &mut operation, &mut SourceAdmission::default())
            .map_err(err)?;
        assert_eq!(operation.usage().source_bytes, text.len() as u64);
    }
    Ok(())
}

#[test]
fn nested_input_is_iterative_and_limits_stop_without_partial_success() -> Result<(), String> {
    let r = registry()?;
    let text = format!("\"{}x{}\"", "{".repeat(1000), "/y}".repeat(1000));
    let s = source(&text)?;
    let mut budget = b();
    let scan = literal::read(
        &s,
        0,
        text.len() as u64,
        true,
        &r,
        &mut budget,
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert!(matches!(scan.outcome, SentenceOutcome::Matched(_)));
    for reason in [
        StopReason::DepthLimit,
        StopReason::WorkLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::SourceLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::DepthLimit => limits.depth = 50,
            StopReason::WorkLimit => limits.work = 1000,
            StopReason::NodeLimit => limits.nodes = 10,
            StopReason::AllocationLimit => limits.allocation_units = 1000,
            StopReason::SourceLimit => limits.source_bytes = 1,
            _ => {}
        }
        let mut budget = Budget::new(limits);
        if reason == StopReason::Cancelled {
            budget.cancel();
        }
        assert!(
            matches!(literal::read(&s, 0, text.len() as u64, true, &r, &mut budget, &mut SourceAdmission::default()), Err(SentenceError::Stopped(s)) if s == reason)
        );
        assert_eq!(budget.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn literal_print_preserves_nested_content_and_escaped_delimiters() -> Result<(), String> {
    let r = registry()?;
    for text in [
        r#""""#,
        r#""a\n\r\t\\\"\u{5b}""#,
        "\"前{[漢/かん]/説明/別注}𝄞\"",
    ] {
        let value = parse(text, &r)?.syntax.value;
        let printed = literal::print(&value, &mut b()).map_err(err)?;
        let reparsed = parse(&printed, &r)?;
        assert_eq!(reparsed.syntax.value, value);
    }
    Ok(())
}

#[test]
fn literal_print_rejects_prefix_only_break_and_stops_expanding_shared_values() -> Result<(), String>
{
    let value = SentenceValue {
        root: Root::Sentence(SentenceRef(1)),
        nodes: vec![
            Kind::Break,
            Kind::Sentence {
                inlines: vec![InlineRef(0)],
            },
        ],
        embeds: vec![],
    };
    assert_eq!(
        literal::print(&value, &mut b()),
        Err(literal::PrintError::NotLiteral(InlineRef(0)))
    );
    // Only 21 arena nodes, but a million expanded characters: output limits
    // must count occurrences, not merely unique nodes of the DAG.
    let mut nodes = vec![Kind::Text { text: "x".into() }];
    for i in 0..20 {
        nodes.push(Kind::Concat {
            inlines: vec![InlineRef(i), InlineRef(i)],
        });
    }
    nodes.push(Kind::Sentence {
        inlines: vec![InlineRef(20)],
    });
    let value = SentenceValue {
        root: Root::Sentence(SentenceRef(21)),
        nodes,
        embeds: vec![],
    };
    let mut limits = b().limits();
    limits.output_bytes = 1000;
    let mut budget = Budget::new(limits);
    assert_eq!(
        literal::print(&value, &mut budget),
        Err(literal::PrintError::Stopped(StopReason::OutputLimit))
    );
    assert_eq!(budget.poll(), Err(StopReason::OutputLimit));
    Ok(())
}

#[test]
fn escape_prefixes_need_more_and_changed_schema_is_not_silently_accepted() -> Result<(), String> {
    let r = registry()?;
    let s = source(r#""\u{1d11e}\n""#)?;
    for end in 0..s.text().len() {
        let scan = literal::read(
            &s,
            0,
            end as u64,
            false,
            &r,
            &mut b(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        assert!(matches!(scan.outcome, SentenceOutcome::NeedMore));
    }
    let mut changed = nepl3_sentence_core::schema::descriptor(&mut b()).map_err(err)?;
    // Type inventory order is canonicalized; change a contract constraint.
    changed.types[0]
        .constraints
        .push("different-literal-contract".into());
    let mut wrong = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()).map_err(err)?,
        changed,
    ] {
        wrong
            .register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    wrong.finalize(&mut b()).map_err(err)?;
    assert!(matches!(
        literal::read(
            &s,
            0,
            s.text().len() as u64,
            true,
            &wrong,
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(SentenceError::SchemaIdentity)
    ));
    Ok(())
}
