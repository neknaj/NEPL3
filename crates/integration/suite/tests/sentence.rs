use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::*,
};
use nepl3_doc_core::{model as d, portable};
use nepl3_sentence_core::{
    literal::{self, SentenceOutcome},
    model::*,
    syntax::{NodeLocation, SentenceSyntax},
};
use nepl3_suite::adapters::{document::sentence as slot, sentence};
use nepl3_wire::foundation::FoundationCodec;

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
fn registry() -> Result<SchemaRegistry, String> {
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
    Ok(r)
}
fn generated(value: SentenceValue) -> SentenceSyntax {
    SentenceSyntax {
        locations: vec![
            NodeLocation {
                origin: OriginId(0),
                head: None,
                cover: None
            };
            value.nodes.len()
        ],
        value,
        sources: vec![],
        views: vec![],
        source_maps: vec![],
        origins: vec![Origin::Synthetic {
            reason: "typed Sentence slot".into(),
            anchor: None,
        }],
    }
}

#[test]
fn guest_free_selection_uses_only_complete_sentence_validation() -> Result<(), String> {
    let registry = registry()?;
    let surface = registry.selected("nepl3.doc", 1).ok_or("Doc")?;
    let store = SourceStore::default();
    for count in [16, 128, 1024] {
        let mut nodes = vec![Kind::Sentence {
            inlines: (1..=count).map(InlineRef).collect(),
        }];
        nodes.extend((0..count).map(|_| Kind::Text { text: "文".into() }));
        let mut input = generated(SentenceValue {
            root: Root::Sentence(SentenceRef(0)),
            nodes,
            embeds: vec![],
        });
        let mut validation = b();
        input
            .validate(&registry, &mut validation, &mut SourceAdmission::default())
            .map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(err)?;
        let mut selection = b();
        let result = sentence::document_guests::collect(
            &input,
            surface,
            &registry,
            &mut codec,
            &mut selection,
        )
        .map_err(err)?;
        assert!(result.documents().is_empty() && result.occurrences().is_empty());
        // No second traversal/allocation for absent guests. Full validation's
        // resource accounting and depth remain mandatory at every input size.
        assert_eq!(selection.usage(), validation.usage());
        let mut limits = b().limits();
        limits.work = validation.usage().work - 1;
        let mut short = Budget::new(limits);
        assert!(matches!(
            sentence::document_guests::collect(&input, surface, &registry, &mut codec, &mut short),
            Err(sentence::document_guests::Error::Stopped(
                StopReason::WorkLimit
            ))
        ));
        let mut cancelled = b();
        cancelled.cancel();
        assert!(matches!(
            sentence::document_guests::collect(
                &input,
                surface,
                &registry,
                &mut codec,
                &mut cancelled
            ),
            Err(sentence::document_guests::Error::Stopped(
                StopReason::Cancelled
            ))
        ));
        let original = input.value.nodes[1].clone();
        input.value.nodes[1] = Kind::ForeignInline {
            syntax: EmbedRef(0),
        };
        assert!(
            sentence::document_guests::collect(&input, surface, &registry, &mut codec, &mut b())
                .is_err()
        );
        input.value.nodes[1] = original;
        input.locations.clear();
        assert!(matches!(
            sentence::document_guests::collect(&input, surface, &registry, &mut codec, &mut b()),
            Err(sentence::document_guests::Error::Sentence(
                nepl3_sentence_core::syntax::Error::LocationCount
            ))
        ));
    }
    Ok(())
}

/// Doc owns one slot. Sentence retains its entire arena and provenance.
/// The first receiver starts with an empty ambient SourceStore.
fn doc_roundtrip(input: &SentenceSyntax, r: &SchemaRegistry) -> Result<SentenceSyntax, String> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(r, &store, &mut admission).map_err(err)?;
    let embed = slot::embed(input, r, &mut codec, &mut b()).map_err(err)?;
    let (root, kind, role) = match input.value.root {
        Root::Sentence(_) => (
            d::DocRoot::Sentence(d::SentenceRef(0)),
            d::DocKind::Sentence {
                syntax: d::EmbedRef(0),
            },
            d::EmbedKind::Sentence,
        ),
        Root::Inline(_) => (
            d::DocRoot::Inline(d::InlineRef(0)),
            d::DocKind::Anchor {
                id: "label".into(),
                label: d::EmbedRef(0),
            },
            d::EmbedKind::SentenceInline,
        ),
    };
    assert_eq!(embed.kind, role);
    let document = d::DocumentSyntax {
        value: d::DocValue {
            root,
            nodes: vec![d::DocNode {
                kind,
                locations: vec![],
                origin: None,
                span: None,
            }],
            embeds: vec![embed],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let normalized = nepl3_doc_core::normalize::document(
        &document,
        r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(normalized, document);
    let raw = portable::to_value(&document, r, &mut codec, &mut b()).map_err(err)?;
    let wire = nepl3_wire::encode(&raw, &mut b()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut receiver = FoundationCodec::new(r, &store, &mut admission).map_err(err)?;
    let received = portable::from_value(
        &nepl3_wire::decode(&wire, &mut b()).map_err(err)?,
        r,
        &mut receiver,
        &mut b(),
    )
    .map_err(err)?;
    assert_eq!(received, document);
    // Value decoding uses the meaning contract, with no surface form lookup.
    let surface = r.selected("nepl3.sentence", 1).ok_or("Sentence schema")?;
    let mut decode = b();
    let actual = slot::lower(
        &received.value.embeds[0],
        surface,
        &[],
        r,
        &mut receiver,
        &mut decode,
    )
    .map_err(err)?;
    assert_eq!(&actual, input);
    assert_eq!(
        decode.usage().source_bytes,
        input
            .sources
            .iter()
            .map(|source| source.text().len() as u64)
            .sum()
    );
    let mut wrong_role = received.value.embeds[0].clone();
    wrong_role.kind = match role {
        d::EmbedKind::Sentence => d::EmbedKind::SentenceInline,
        _ => d::EmbedKind::Sentence,
    };
    assert!(matches!(
        slot::lower(&wrong_role, surface, &[], r, &mut receiver, &mut b()),
        Err(slot::Error::Category)
    ));
    Ok(actual)
}

#[test]
fn all_standard_inline_kinds_keep_content_order_and_shared_references() -> Result<(), String> {
    let r = registry()?;
    let input = generated(SentenceValue {
        root: Root::Sentence(SentenceRef(10)),
        nodes: vec![
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
        ],
        embeds: vec![],
    });
    let actual = doc_roundtrip(&input, &r)?;
    assert_eq!(actual.value.nodes, input.value.nodes);
    let store = SourceStore::default();
    let run = |input: &SentenceSyntax, limits, cancelled| {
        let mut budget = Budget::new(limits);
        if cancelled {
            budget.cancel();
        }
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let result = slot::embed(input, &r, &mut codec, &mut budget);
        assert_eq!(budget.current_depth(), 0);
        Ok::<_, String>((result, budget))
    };
    let (result, measured) = run(&input, b().limits(), false)?;
    result.map_err(err)?;
    for (reason, used) in [
        (StopReason::WorkLimit, measured.usage().work),
        (
            StopReason::AllocationLimit,
            measured.usage().allocation_units,
        ),
        (StopReason::NodeLimit, measured.usage().nodes),
        (StopReason::DepthLimit, measured.usage().depth),
    ] {
        assert!(used > 0);
        for cap in [0, used / 2, used - 1, used] {
            let mut limits = b().limits();
            match reason {
                StopReason::WorkLimit => limits.work = cap,
                StopReason::AllocationLimit => limits.allocation_units = cap,
                StopReason::NodeLimit => limits.nodes = cap,
                StopReason::DepthLimit => limits.depth = cap,
                _ => return Err("resource".into()),
            }
            let (result, budget) = run(&input, limits, false)?;
            if cap == used {
                result.map_err(err)?;
            } else {
                assert!(
                    matches!(result, Err(slot::Error::Stopped(actual)) if actual == reason),
                    "{result:?}"
                );
                assert_eq!(budget.poll(), Err(reason));
            }
        }
    }
    assert!(matches!(
        run(&input, b().limits(), true)?.0,
        Err(slot::Error::Stopped(StopReason::Cancelled))
    ));
    let mut invalid = input.clone();
    invalid.value.root = Root::Sentence(SentenceRef(0));
    assert!(matches!(
        run(&invalid, b().limits(), false)?.0,
        Err(slot::Error::Value(_))
    ));
    assert_eq!(doc_roundtrip(&input, &r)?, actual);
    Ok(())
}

#[test]
fn literal_and_typed_slots_preserve_sentence_meaning_and_provenance() -> Result<(), String> {
    let r = registry()?;
    let source = SourceSnapshot::new(
        SourceId("slot-literal".into()),
        1,
        "memory:slot-literal".into(),
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
        return Err("literal".into());
    };
    let actual = doc_roundtrip(&literal.syntax, &r)?;
    assert_eq!(actual.sources, literal.syntax.sources);
    assert_eq!(actual.locations, literal.syntax.locations);
    assert_eq!(actual.views, literal.syntax.views);
    assert_eq!(actual.origins, literal.syntax.origins);
    assert_eq!(actual.source_maps, literal.syntax.source_maps);
    // Typed empty/adjacent Text and Concat retain their arena. Sentence's
    // literal projection supplies the content oracle; Doc keeps owner data.
    let typed = generated(SentenceValue {
        root: Root::Sentence(SentenceRef(7)),
        nodes: vec![
            Kind::Text { text: "a".into() },
            Kind::Text {
                text: String::new(),
            },
            Kind::Text { text: "b".into() },
            Kind::Concat {
                inlines: vec![InlineRef(0), InlineRef(1), InlineRef(2)],
            },
            Kind::Text { text: "x".into() },
            Kind::Text { text: "y".into() },
            Kind::Ruby {
                base: InlineRef(4),
                reading: InlineRef(5),
            },
            Kind::Sentence {
                inlines: vec![InlineRef(3), InlineRef(6)],
            },
        ],
        embeds: vec![],
    });
    let received = doc_roundtrip(&typed, &r)?;
    assert_eq!(received.value, typed.value);
    assert_eq!(
        literal::print(&received.value, &mut b()).map_err(err)?,
        "\"ab[x/y]\""
    );
    assert_eq!(
        literal::print(&actual.value, &mut b()).map_err(err)?,
        "\"ab[x/y]\""
    );
    Ok(())
}

#[test]
fn doc_slot_preserves_shared_inline_and_requires_selection_for_foreign_output() -> Result<(), String>
{
    let r = registry()?;
    let mut input = generated(SentenceValue {
        root: Root::Inline(InlineRef(1)),
        nodes: vec![
            Kind::Break,
            Kind::Concat {
                inlines: vec![InlineRef(0), InlineRef(0)],
            },
        ],
        embeds: vec![],
    });
    assert_eq!(doc_roundtrip(&input, &r)?, input);
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
        provenance: nepl3_core::syntax::OwnerProvenance::from_parts(vec![], vec![], vec![]),
    };
    input = generated(SentenceValue {
        root: Root::Inline(InlineRef(0)),
        nodes: vec![Kind::ForeignInline {
            syntax: EmbedRef(0),
        }],
        embeds: vec![closure.into()],
    });
    let actual = doc_roundtrip(&input, &r)?;
    // Storage preserves opaque guests; output requires a selected operation.
    let prepared = nepl3_sentence_core::print::prepare(
        &actual.value,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(
        prepared.render(&[], &mut b()),
        Err(nepl3_sentence_core::print::Error::AdapterRequired(
            EmbedRef(0)
        ))
    );
    let surface = r.selected("nepl3.doc", 1).ok_or("Doc")?;
    let store = SourceStore::default();
    let run = |input: &SentenceSyntax, budget: &mut Budget| {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        Ok::<_, String>(sentence::document_guests::collect(
            input, surface, &r, &mut codec, budget,
        ))
    };
    let selected = run(&actual, &mut b())?.map_err(err)?;
    assert!(selected.documents().is_empty());
    assert!(selected.occurrences().is_empty());
    let mut cancelled = b();
    cancelled.cancel();
    assert!(matches!(
        run(&actual, &mut cancelled)?,
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
