use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot},
    syntax::*,
};
use nepl3_sentence_core::{
    model::*,
    syntax::{NodeLocation, SentenceSyntax},
};
use nepl3_suite::adapters::sentence;

fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 10_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn all_standard_inline_kinds_keep_content_order_and_shared_references() -> Result<(), String> {
    use nepl3_doc_core::model as d;
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
    ] {
        let descriptor = descriptor.map_err(err)?;
        registry
            .register(
                descriptor.reference(&mut b()).map_err(err)?,
                descriptor,
                &mut b(),
            )
            .map_err(err)?;
    }
    registry.finalize(&mut b()).map_err(err)?;
    let nodes = vec![
        Kind::Text {
            text: "漢字".into(),
        },
        Kind::Text {
            text: "かんじ".into(),
        },
        Kind::Ruby {
            base: InlineRef(0),
            reading: InlineRef(1),
        },
        Kind::Code {
            text: "x<>&".into(),
        },
        Kind::Emphasis {
            inline: InlineRef(0),
        },
        Kind::Strong {
            inline: InlineRef(2),
        },
        Kind::Break,
        Kind::Concat {
            inlines: vec![InlineRef(4), InlineRef(6), InlineRef(5)],
        },
        Kind::InlineAnno {
            base: InlineRef(7),
            notes: vec![InlineRef(1), InlineRef(3)],
        },
        Kind::ExternalLink {
            uri: "https://example.org/".into(),
            label: InlineRef(8),
        },
        Kind::Sentence {
            inlines: (0..10).rev().map(InlineRef).collect(),
        },
    ];
    let input = SentenceSyntax {
        locations: vec![
            NodeLocation {
                origin: OriginId(0),
                head: None,
                cover: None
            };
            nodes.len()
        ],
        value: SentenceValue {
            root: Root::Sentence(SentenceRef(10)),
            nodes,
            embeds: vec![],
        },
        sources: vec![],
        views: vec![],
        source_maps: vec![],
        origins: vec![Origin::Synthetic {
            reason: "typed adapter input".into(),
            anchor: None,
        }],
    };
    let expected = vec![
        d::DocKind::Text {
            text: "漢字".into(),
        },
        d::DocKind::Text {
            text: "かんじ".into(),
        },
        d::DocKind::Ruby {
            base: d::InlineRef(0),
            reading: d::InlineRef(1),
        },
        d::DocKind::InlineCode {
            text: "x<>&".into(),
        },
        d::DocKind::Emphasis {
            inline: d::InlineRef(0),
        },
        d::DocKind::Strong {
            inline: d::InlineRef(2),
        },
        d::DocKind::Break,
        d::DocKind::Concat {
            inlines: vec![d::InlineRef(4), d::InlineRef(6), d::InlineRef(5)],
        },
        d::DocKind::Anno {
            base: d::InlineRef(7),
            notes: vec![d::InlineRef(1), d::InlineRef(3)],
        },
        d::DocKind::Link {
            target: d::LinkTarget::External {
                uri: "https://example.org/".into(),
            },
            label: d::InlineRef(8),
        },
        d::DocKind::Sentence {
            inlines: (0..10).rev().map(d::InlineRef).collect(),
        },
    ];
    let mut measured = b();
    let converted = sentence::document(
        &input,
        &registry,
        &mut measured,
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(
        converted.value.root,
        d::DocRoot::Sentence(d::SentenceRef(10))
    );
    assert_eq!(
        converted
            .value
            .nodes
            .iter()
            .map(|node| &node.kind)
            .collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>()
    );
    assert_eq!(converted.origins, input.origins);
    // Each resource stop returns no document and leaves the borrowed original
    // available; only the successful call publishes the fully checked result.
    for work in 0..measured.usage().work {
        let mut limited = Budget::new(Limits {
            work,
            ..b().limits()
        });
        assert_eq!(
            sentence::document(
                &input,
                &registry,
                &mut limited,
                &mut SourceAdmission::default()
            )
            .err(),
            Some(sentence::Error::Stopped(StopReason::WorkLimit))
        );
    }
    for allocation in 0..measured.usage().allocation_units {
        let mut limited = Budget::new(Limits {
            allocation_units: allocation,
            ..b().limits()
        });
        assert_eq!(
            sentence::document(
                &input,
                &registry,
                &mut limited,
                &mut SourceAdmission::default()
            )
            .err(),
            Some(sentence::Error::Stopped(StopReason::AllocationLimit))
        );
    }
    let mut invalid = input.clone();
    invalid.value.root = Root::Sentence(SentenceRef(0));
    assert!(matches!(
        sentence::document(
            &invalid,
            &registry,
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(sentence::Error::Sentence(_))
    ));
    let after = sentence::document(&input, &registry, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    assert_eq!(after, converted);
    Ok(())
}

#[test]
fn literal_bridge_and_typed_doc_share_normal_form_and_provenance() -> Result<(), String> {
    use nepl3_doc_core::model as d;
    use nepl3_sentence_core::literal::{self, SentenceOutcome};
    let mut r = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
    ] {
        let descriptor = descriptor.map_err(err)?;
        r.register(
            descriptor.reference(&mut b()).map_err(err)?,
            descriptor,
            &mut b(),
        )
        .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    let source = SourceSnapshot::new(
        SourceId("bridge-literal".into()),
        1,
        "memory:bridge-literal".into(),
        b"\"ab[x/y]\"".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let scan = literal::read(
        &source,
        0,
        source.text().len() as u64,
        true,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let SentenceOutcome::Matched(literal) = scan.outcome else {
        return Err("expected literal".into());
    };
    let doc = sentence::document(
        &literal.syntax,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    // Independent constructor input contains empty/adjacent Text and Concat;
    // normalization must yield the same meaning as the actual literal reader.
    let kinds = vec![
        d::DocKind::Text { text: "a".into() },
        d::DocKind::Text {
            text: String::new(),
        },
        d::DocKind::Text { text: "b".into() },
        d::DocKind::Concat {
            inlines: vec![d::InlineRef(0), d::InlineRef(1), d::InlineRef(2)],
        },
        d::DocKind::Text { text: "x".into() },
        d::DocKind::Text { text: "y".into() },
        d::DocKind::Ruby {
            base: d::InlineRef(4),
            reading: d::InlineRef(5),
        },
        d::DocKind::Sentence {
            inlines: vec![d::InlineRef(3), d::InlineRef(6)],
        },
    ];
    let prefix = d::DocumentSyntax {
        value: d::DocValue {
            root: d::DocRoot::Sentence(d::SentenceRef(7)),
            nodes: kinds
                .into_iter()
                .map(|kind| d::DocNode {
                    kind,
                    locations: vec![],
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
    let normal =
        nepl3_doc_core::normalize::document(&prefix, &r, &mut b(), &mut SourceAdmission::default())
            .map_err(err)?;
    assert_eq!(normal.value.root, doc.value.root);
    assert_eq!(
        normal
            .value
            .nodes
            .iter()
            .map(|node| &node.kind)
            .collect::<Vec<_>>(),
        doc.value
            .nodes
            .iter()
            .map(|node| &node.kind)
            .collect::<Vec<_>>()
    );
    let mut operation = b();
    let normalized = nepl3_doc_core::normalize::document(
        &doc,
        &r,
        &mut operation,
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(normalized.views, doc.views);
    assert_eq!(normalized.origins, doc.origins);
    assert_eq!(operation.usage().source_bytes, source.text().len() as u64);
    assert_eq!(doc.sources, literal.syntax.sources);
    assert_eq!(doc.origins, literal.syntax.origins);
    assert_eq!(doc.source_maps, literal.syntax.source_maps);
    for (node, location) in doc.value.nodes.iter().zip(&literal.syntax.locations) {
        assert_eq!(node.origin, Some(location.origin));
        assert_eq!(node.span, location.cover);
    }
    Ok(())
}

#[test]
fn doc_bridge_preserves_generated_shared_inline_and_rejects_unselected_foreign()
-> Result<(), String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    let location = NodeLocation {
        origin: OriginId(0),
        head: None,
        cover: None,
    };
    let mut input = SentenceSyntax {
        value: SentenceValue {
            root: Root::Inline(InlineRef(1)),
            embeds: vec![],
            nodes: vec![
                Kind::Break,
                Kind::Concat {
                    inlines: vec![InlineRef(0), InlineRef(0)],
                },
            ],
        },
        locations: vec![location.clone(), location.clone()],
        sources: vec![],
        views: vec![],
        source_maps: vec![],
        origins: vec![Origin::Synthetic {
            reason: "generated sentence".into(),
            anchor: None,
        }],
    };
    let out =
        sentence::document(&input, &r, &mut b(), &mut SourceAdmission::default()).map_err(err)?;
    use nepl3_doc_core::model as d;
    assert_eq!(out.value.root, d::DocRoot::Inline(d::InlineRef(1)));
    assert_eq!(
        out.value.nodes[1].kind,
        d::DocKind::Concat {
            inlines: vec![d::InlineRef(0), d::InlineRef(0)]
        }
    );
    assert!(
        out.value
            .nodes
            .iter()
            .all(|n| n.span.is_none() && n.origin == Some(OriginId(0)))
    );
    let mut budget = b();
    budget.cancel();
    assert_eq!(
        sentence::document(&input, &r, &mut budget, &mut SourceAdmission::default()).err(),
        Some(sentence::Error::Stopped(StopReason::Cancelled))
    );
    let mut limits = b().limits();
    limits.work = 0;
    assert_eq!(
        sentence::document(
            &input,
            &r,
            &mut Budget::new(limits),
            &mut SourceAdmission::default()
        )
        .err(),
        Some(sentence::Error::Stopped(StopReason::WorkLimit))
    );
    let foundation = r
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let env = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = nepl3_wire::environment::environment_digest(&env, &foundation, &r, &mut b())
        .map_err(err)?;
    // Structurally valid foreign syntax; no guest language or semantic role is
    // inferred from its category or foundation kind. The bridge must require
    // a selected adapter even after closure validation succeeds.
    let closure = ForeignClosure {
        syntax: ForeignSyntax {
            schema: foundation.clone(),
            category: "test-inline".into(),
            root: NodeRef(0),
            bundle: SyntaxBundle {
                sources: vec![],
                nodes: vec![SyntaxNode {
                    schema: foundation,
                    kind: "NodeRef".into(),
                    fields: vec![],
                    head: None,
                    cover: None,
                    origin: OriginId(0),
                    token: None,
                }],
                origins: input.origins.clone(),
                root: NodeRef(0),
                environments: vec![],
                tokens: vec![],
                source_maps: vec![],
            },
            environment: EnvironmentRef { id: 0, digest },
        },
        owner_environment: EnvironmentEntry {
            id: 0,
            digest,
            value: env,
        },
        owner_origins: vec![],
        owner_sources: vec![],
        owner_source_maps: vec![],
    };
    input.value.nodes = vec![Kind::ForeignInline {
        syntax: EmbedRef(0),
    }];
    input.value.root = Root::Inline(InlineRef(0));
    input.value.embeds = vec![closure.into()];
    input.locations = vec![location];
    input
        .validate(&r, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    assert_eq!(
        sentence::document(&input, &r, &mut b(), &mut SourceAdmission::default()).err(),
        Some(sentence::Error::ForeignAdapterRequired(EmbedRef(0)))
    );
    // The independent consumer keeps foreign meaning in its owner. A Doc
    // selection does not turn an unrelated category into a Doc inline.
    let surface = r.selected("nepl3.doc", 1).ok_or("Doc schema")?;
    let store = nepl3_core::source::SourceStore::default();
    let run = |input: &SentenceSyntax, budget: &mut Budget| {
        let mut admission = SourceAdmission::default();
        let mut codec = nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission)
            .map_err(err)?;
        Ok::<_, String>(sentence::document_guests::collect(
            input, surface, &r, &mut codec, budget,
        ))
    };
    let selected = run(&input, &mut b())?.map_err(err)?;
    assert!(selected.documents().is_empty());
    assert!(selected.occurrences().is_empty());
    let mut cancelled = b();
    cancelled.cancel();
    assert!(matches!(
        run(&input, &mut cancelled)?,
        Err(sentence::document_guests::Error::Stopped(
            StopReason::Cancelled
        ))
    ));
    input.locations.clear();
    assert!(matches!(
        run(&input, &mut b())?,
        Err(sentence::document_guests::Error::Sentence(
            nepl3_sentence_core::syntax::Error::LocationCount
        ))
    ));
    Ok(())
}
