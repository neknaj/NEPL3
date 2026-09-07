use nepl3_core::{
    budget::{Budget, Limits},
    origin::OriginGraph,
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
};
use nepl3_doc_core::{
    model::*,
    sentence::{self, SentenceCode, SentenceOutcome},
};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 100_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}

#[test]
fn typed_prefix_constructor_and_literal_share_the_normal_form() -> Result<(), String> {
    let mut b = budget();
    let registry = registry(&mut b)?;
    let source = source("\"ab[x/y]\"", &mut b)?;
    let literal = sentence::read(
        &source,
        0,
        source.text().len() as u64,
        true,
        &registry,
        &mut b,
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let SentenceOutcome::Matched(literal) = literal.outcome else {
        return Err("literal".into());
    };
    let kinds = vec![
        DocKind::Text { text: "a".into() },
        DocKind::Text {
            text: String::new(),
        },
        DocKind::Text { text: "b".into() },
        DocKind::Concat {
            inlines: vec![InlineRef(0), InlineRef(1), InlineRef(2)],
        },
        DocKind::Text { text: "x".into() },
        DocKind::Text { text: "y".into() },
        DocKind::Ruby {
            base: InlineRef(4),
            reading: InlineRef(5),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(3), InlineRef(6)],
        },
    ];
    let prefix = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Sentence(SentenceRef(7)),
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    origin: None,
                    span: None,
                })
                .collect(),
            embeds: vec![],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let normal = nepl3_doc_core::normalize::document(
        &prefix,
        &registry,
        &mut b,
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(normal.value.root, literal.value.root);
    assert_eq!(
        normal
            .value
            .nodes
            .iter()
            .map(|n| &n.kind)
            .collect::<Vec<_>>(),
        literal
            .value
            .nodes
            .iter()
            .map(|n| &n.kind)
            .collect::<Vec<_>>()
    );
    let doc = DocumentSyntax {
        value: literal.value,
        sources: vec![source],
        origins: literal.origins,
        views: vec![literal.view],
        source_maps: vec![],
    };
    let mut operation = budget();
    let mut admission = SourceAdmission::default();
    let normal =
        nepl3_doc_core::normalize::document(&doc, &registry, &mut operation, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
    assert_eq!(normal.views, doc.views);
    assert_eq!(normal.origins, doc.origins);
    assert_eq!(
        operation.usage().source_bytes,
        doc.sources[0].text().len() as u64
    );
    Ok(())
}

#[test]
fn text_merge_preserves_known_and_source_less_provenance() -> Result<(), String> {
    use nepl3_core::origin::{Origin, OriginId};
    let mut b = budget();
    let registry = registry(&mut b)?;
    let source = source("a", &mut b)?;
    let span = source.span(0, 1).map_err(|e| format!("{e:?}"))?;
    let input = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Sentence(SentenceRef(2)),
            nodes: vec![
                DocNode {
                    kind: DocKind::Text { text: "a".into() },
                    origin: Some(OriginId(0)),
                    span: Some(span.clone()),
                },
                DocNode {
                    kind: DocKind::Text { text: "b".into() },
                    origin: None,
                    span: None,
                },
                DocNode {
                    kind: DocKind::Sentence {
                        inlines: vec![InlineRef(0), InlineRef(1)],
                    },
                    origin: None,
                    span: None,
                },
            ],
            embeds: vec![],
        },
        sources: vec![source],
        origins: vec![Origin::Direct(span.clone())],
        views: vec![],
        source_maps: vec![],
    };
    let out = nepl3_doc_core::normalize::document(
        &input,
        &registry,
        &mut b,
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(out.value.nodes[0].kind, DocKind::Text { text: "ab".into() });
    assert_eq!(out.value.nodes[0].span, None);
    assert_eq!(out.origins[0], Origin::Direct(span));
    assert!(matches!(&out.origins[1],Origin::Synthetic{anchor:None,reason} if !reason.is_empty()));
    assert_eq!(
        out.origins[2],
        Origin::Composite(vec![OriginId(0), OriginId(1)])
    );
    assert_eq!(out.value.nodes[0].origin, Some(OriginId(2)));
    Ok(())
}
fn registry(b: &mut Budget) -> Result<SchemaRegistry, String> {
    let mut registry = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(b),
        nepl3_doc_core::schema::descriptor(b),
    ] {
        let d = d.map_err(|e| format!("{e:?}"))?;
        let r = d.reference(b).map_err(|e| format!("{e:?}"))?;
        registry.register(r, d, b).map_err(|e| format!("{e:?}"))?;
    }
    registry.finalize(b).map_err(|e| format!("{e:?}"))?;
    Ok(registry)
}
fn source(text: &str, b: &mut Budget) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId("sentence".into()),
        1,
        "memory:sentence".into(),
        text.as_bytes().to_vec(),
        b,
    )
    .map_err(|e| format!("{e:?}"))
}

#[test]
fn literal_annotations_escapes_and_original_views() -> Result<(), String> {
    let mut b = budget();
    let registry = registry(&mut b)?;
    for (input, texts, ruby, anno) in [
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
        let source = source(input, &mut b)?;
        let mut op = budget();
        let mut admission = SourceAdmission::default();
        let result = sentence::read(
            &source,
            0,
            input.len() as u64,
            true,
            &registry,
            &mut op,
            &mut admission,
        )
        .map_err(|e| format!("{e:?}"))?;
        let SentenceOutcome::Matched(lit) = result.outcome else {
            return Err(format!("expected match {input}"));
        };
        let actual: Vec<_> = lit
            .value
            .nodes
            .iter()
            .filter_map(|n| {
                if let DocKind::Text { text } = &n.kind {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(actual, texts);
        assert_eq!(
            lit.value
                .nodes
                .iter()
                .filter(|n| matches!(n.kind, DocKind::Ruby { .. }))
                .count(),
            ruby
        );
        assert_eq!(
            lit.value
                .nodes
                .iter()
                .filter(|n| matches!(n.kind, DocKind::Anno { .. }))
                .count(),
            anno
        );
        assert_eq!(
            source.slice(&lit.head).map_err(|e| format!("{e:?}"))?,
            input
        );
        let mut store = SourceStore::default();
        store.insert(source).map_err(|e| format!("{e:?}"))?;
        lit.view
            .view
            .validate(&store, &registry, &mut op)
            .map_err(|e| format!("view {e:?}"))?;
        OriginGraph::validate_origins(&lit.origins, &store, &mut op)
            .map_err(|e| format!("origin {e:?}"))?;
        assert_eq!(op.usage().source_bytes, input.len() as u64);
    }
    Ok(())
}
#[test]
fn incomplete_and_failed_annotations_keep_raw_byte_positions() -> Result<(), String> {
    let mut b = budget();
    let registry = registry(&mut b)?;
    for (input, code, primary, opening) in [
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
        let source = source(input, &mut b)?;
        let mut op = budget();
        let result = sentence::read(
            &source,
            0,
            input.len() as u64,
            true,
            &registry,
            &mut op,
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let SentenceOutcome::Failed(failure) = result.outcome else {
            return Err(format!("expected failure {input}"));
        };
        assert_eq!(failure.code, code);
        assert_eq!((failure.primary.start(), failure.primary.end()), primary);
        assert_eq!(failure.opening.map(|s| (s.start(), s.end())), opening);
    }
    let text = "\"{[文/ぶん]/note}\"";
    let source = source(text, &mut b)?;
    for end in (1..text.len()).filter(|i| text.is_char_boundary(*i)) {
        let result = sentence::read(
            &source,
            0,
            end as u64,
            false,
            &registry,
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert!(matches!(result.outcome, SentenceOutcome::NeedMore));
    }
    Ok(())
}
