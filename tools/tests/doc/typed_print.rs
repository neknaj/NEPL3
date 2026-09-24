use super::*;
use nepl3_core::{
    budget::{Budget, StopReason},
    origin::{Origin, OriginId},
};
use nepl3_doc_core::print::{PrintEntry, PrintMode};
use nepl3_sentence_core::{
    lower::ForeignInlineForm,
    model::{self as sentence_model, Kind, Root, SentenceValue},
    syntax::{NodeLocation, SentenceSyntax},
};
use nepl3_suite::adapters::document::sentence;
use nepl3_tools::doc::printing::{Error, SentenceGuestPrinter};

fn generated(value: SentenceValue) -> SentenceSyntax {
    SentenceSyntax {
        locations: value
            .nodes
            .iter()
            .map(|_| NodeLocation {
                origin: OriginId(0),
                head: None,
                cover: None,
            })
            .collect(),
        value,
        origins: vec![Origin::Synthetic {
            reason: "typed list construction".into(),
            anchor: None,
        }],
        sources: vec![],
        views: vec![],
        source_maps: vec![],
    }
}

#[test]
fn typed_doc_printer_composes_meaning_slots_and_retained_foreign_syntax() -> Result<(), String> {
    let compiled = compiled()?;
    // Only the imported Doc Inline has source. The list, its two Sentence
    // values, Ruby and annotation are constructed as typed model values.
    let input =
        r#"sentence sentence cons doc link relative "guide.md" some "start" text "second" nil"#;
    with_input(&compiled, input, "Sentence", |tree, profile, b, a| {
        let registry = profile.registry();
        let store = SourceStore::default();
        let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
        let imported = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Sentence,
            registry,
            b,
            &mut codec,
        )
        .map_err(err)?;
        let package = profile.language("Sentence", b).map_err(err)?;
        let forms = [ForeignInlineForm {
            kind: "Form:DocumentInline",
            guest_schema: &compiled.doc.package.schema,
            guest_category: "Inline",
        }];
        let imported = sentence::lower(
            &imported.value.embeds[0],
            &package.schema,
            &forms,
            registry,
            &mut codec,
            b,
        )
        .map_err(err)?;
        let closure = imported.value.embeds.first().ok_or("Doc Inline")?.clone();
        let first = generated(SentenceValue {
            root: Root::Sentence(sentence_model::SentenceRef(5)),
            nodes: vec![
                Kind::Text { text: "字".into() },
                Kind::Text { text: "じ".into() },
                Kind::Ruby {
                    base: sentence_model::InlineRef(0),
                    reading: sentence_model::InlineRef(1),
                },
                Kind::Text {
                    text: "note".into(),
                },
                Kind::InlineAnno {
                    base: sentence_model::InlineRef(2),
                    notes: vec![sentence_model::InlineRef(3)],
                },
                Kind::Sentence {
                    inlines: vec![sentence_model::InlineRef(4)],
                },
            ],
            embeds: vec![],
        });
        let second = generated(SentenceValue {
            root: Root::Sentence(sentence_model::SentenceRef(1)),
            nodes: vec![
                Kind::ForeignInline {
                    syntax: sentence_model::EmbedRef(0),
                },
                Kind::Sentence {
                    inlines: vec![sentence_model::InlineRef(0)],
                },
            ],
            embeds: vec![closure.clone()],
        });
        let kinds = vec![
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
            DocKind::Paragraph {
                items: vec![FlowRef(0)],
            },
            DocKind::Body {
                blocks: vec![BlockRef(1)],
            },
            DocKind::ListItem {
                checked: None,
                body: BodyRef(2),
            },
            DocKind::Sentence {
                syntax: EmbedRef(1),
            },
            DocKind::Paragraph {
                items: vec![FlowRef(4)],
            },
            DocKind::Body {
                blocks: vec![BlockRef(5)],
            },
            DocKind::ListItem {
                checked: Some(false),
                body: BodyRef(6),
            },
            DocKind::List {
                kind: ListKind::Unordered,
                items: vec![ListItemRef(3), ListItemRef(7)],
            },
        ];
        let document = DocumentSyntax {
            value: DocValue {
                root: DocRoot::Block(BlockRef(8)),
                nodes: kinds
                    .iter()
                    .cloned()
                    .map(|kind| DocNode {
                        kind,
                        locations: vec![],
                        origin: None,
                        span: None,
                    })
                    .collect(),
                embeds: vec![
                    sentence::embed(&first, registry, &mut codec, b).map_err(err)?,
                    sentence::embed(&second, registry, &mut codec, b).map_err(err)?,
                ],
            },
            sources: vec![],
            origins: vec![],
            views: vec![],
            source_maps: vec![],
        };
        let saved = document.clone();
        let value = portable::to_value(&document, registry, &mut codec, b).map_err(err)?;
        let bytes = nepl3_wire::encode(&value, b).map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut receiver = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let received = portable::from_value(
            &nepl3_wire::decode(&bytes, b).map_err(err)?,
            registry,
            &mut receiver,
            b,
        )
        .map_err(err)?;
        retention::assert_doc_retention(&document, &received)?;
        for mode in [PrintMode::Prefix, PrintMode::Compact] {
            let mut expected = None;
            for owner in [&document, &received] {
                let artifact = SentenceGuestPrinter {
                    registry,
                    sentence_package: package,
                    math_surface: None,
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut receiver,
                }
                .document(owner, mode, &mut budget())
                .map_err(err)?;
                assert_eq!(artifact.entry, PrintEntry::Block);
                if let Some(expected) = &expected {
                    assert_eq!(&artifact, expected);
                } else {
                    expected = Some(artifact.clone());
                }
                with_input(&compiled, &artifact.text, "Block", |tree, profile, b, a| {
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &store, a).map_err(err)?;
                    let actual = lower::document(
                        tree.syntax(),
                        &compiled.doc.package.schema,
                        Category::Block,
                        profile.registry(),
                        b,
                        &mut codec,
                    )
                    .map_err(err)?;
                    // Fixed constructor references independently check list/item order.
                    assert_eq!(actual.value.root, DocRoot::Block(BlockRef(8)));
                    assert_eq!(
                        actual
                            .value
                            .nodes
                            .iter()
                            .map(|n| &n.kind)
                            .collect::<Vec<_>>(),
                        kinds.iter().collect::<Vec<_>>()
                    );
                    let actual_first = sentence::lower(
                        &actual.value.embeds[0],
                        &package.schema,
                        &forms,
                        profile.registry(),
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    assert_eq!(actual_first.value, first.value);
                    let actual_second = sentence::lower(
                        &actual.value.embeds[1],
                        &package.schema,
                        &forms,
                        profile.registry(),
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    assert_eq!(actual_second.value.nodes, second.value.nodes);
                    let selected = nepl3_suite::adapters::sentence::document_guests::collect(
                        &actual_second,
                        &compiled.doc.package.schema,
                        profile.registry(),
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    assert_eq!(selected.documents().len(), 1);
                    let link = &selected.documents()[0];
                    assert_eq!(
                        link.value.nodes[0].kind,
                        DocKind::Link {
                            target: LinkTarget::Relative {
                                path: "guide.md".into(),
                                fragment: Some("start".into())
                            },
                            label: EmbedRef(0),
                        }
                    );
                    let label = sentence::lower(
                        &link.value.embeds[0],
                        &package.schema,
                        &forms,
                        profile.registry(),
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    assert_eq!(label.value.root, Root::Inline(sentence_model::InlineRef(0)));
                    assert_eq!(
                        label.value.nodes,
                        [Kind::Text {
                            text: "second".into()
                        }]
                    );
                    Ok(())
                })?;
            }
        }
        let wrong = SentenceGuestPrinter {
            registry,
            sentence_package: package,
            math_surface: None,
            doc_surface: None,
            codec: &mut receiver,
        }
        .document(&document, PrintMode::Prefix, &mut budget());
        assert!(matches!(wrong, Err(Error::Selection)));
        let mut forged = document.clone();
        let DocContent::Value {
            value: nepl3_core::value::TypedValue::Record(record),
        } = &mut forged.value.embeds[0].content
        else {
            return Err("typed Sentence record".into());
        };
        record.schema.digest = Digest::of(b"unregistered Sentence identity");
        let invalid = SentenceGuestPrinter {
            registry,
            sentence_package: package,
            math_surface: None,
            doc_surface: Some(&compiled.doc.package.schema),
            codec: &mut receiver,
        }
        .document(&forged, PrintMode::Prefix, &mut budget());
        assert!(matches!(invalid, Err(Error::DocPortable(_))), "{invalid:?}");

        // A fresh admission ledger for every run makes exact limits independent
        // of preceding encoding, parsing and successful printing operations.
        let run = |limits| {
            let mut bounded = Budget::new(limits);
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
            let result = SentenceGuestPrinter {
                registry,
                sentence_package: package,
                math_surface: None,
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            }
            .document(&document, PrintMode::Prefix, &mut bounded);
            assert_eq!(bounded.current_depth(), 0);
            Ok::<_, String>((result, bounded))
        };
        let (baseline, used) = run(budget().limits())?;
        let baseline = baseline.map_err(err)?;
        for (reason, used) in [
            (StopReason::WorkLimit, used.usage().work),
            (StopReason::AllocationLimit, used.usage().allocation_units),
            (StopReason::DepthLimit, used.usage().depth),
            (StopReason::SourceLimit, used.usage().source_bytes),
            (StopReason::OutputLimit, used.usage().output_bytes),
        ] {
            assert!(used > 0);
            for cap in [used, used - 1] {
                let mut limits = budget().limits();
                match reason {
                    StopReason::WorkLimit => limits.work = cap,
                    StopReason::AllocationLimit => limits.allocation_units = cap,
                    StopReason::DepthLimit => limits.depth = cap,
                    StopReason::SourceLimit => limits.source_bytes = cap,
                    StopReason::OutputLimit => limits.output_bytes = cap,
                    _ => return Err("unexpected resource".into()),
                }
                let (result, bounded) = run(limits)?;
                if cap == used {
                    assert_eq!(result.map_err(err)?, baseline);
                } else {
                    assert!(
                        matches!(result, Err(Error::Stopped(actual)) if actual == reason),
                        "{reason:?} {cap}: {result:?}"
                    );
                    assert_eq!(bounded.poll(), Err(reason));
                }
            }
        }
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
            StopReason::SourceLimit,
            StopReason::OutputLimit,
            StopReason::Cancelled,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::DepthLimit => limits.depth = 0,
                StopReason::SourceLimit => limits.source_bytes = 0,
                StopReason::OutputLimit => limits.output_bytes = 0,
                _ => (),
            }
            let mut bounded = Budget::new(limits);
            if reason == StopReason::Cancelled {
                bounded.cancel();
            }
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
            let result = SentenceGuestPrinter {
                registry,
                sentence_package: package,
                math_surface: None,
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            }
            .document(&document, PrintMode::Prefix, &mut bounded);
            assert!(
                matches!(result, Err(Error::Stopped(actual)) if actual == reason),
                "{reason:?}: {result:?}"
            );
            assert_eq!(bounded.current_depth(), 0);
            assert_eq!(bounded.poll(), Err(reason));
        }
        assert_eq!(document, saved);
        assert!(document.sources.is_empty());
        assert!(first.sources.is_empty() && second.sources.is_empty());
        assert_eq!(second.value.embeds[0], closure);
        Ok(())
    })
}
