use super::{
    Digest, FoundationCodec, SourceAdmission, SourceStore, budget, compiled, err, retention,
    with_input,
};
use nepl3_core::{
    budget::{Budget, StopReason},
    origin::{Origin, OriginId},
};
use nepl3_doc_core::print::{PrintEntry, PrintMode};
use nepl3_doc_core::{check::Category, lower, model::*, portable};
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
    check_typed_list(false)
}

#[test]
fn typed_list_construction_preserves_order_and_inline_structure() -> Result<(), String> {
    check_typed_list(true)
}

fn check_typed_list(source_free: bool) -> Result<(), String> {
    let compiled = compiled()?;
    // Only the imported Doc Inline has source. The list, its two Sentence
    // values, Ruby and annotation are constructed as typed model values.
    let input = if source_free {
        "sentence sentence nil"
    } else {
        r#"sentence sentence cons doc link relative "guide.md" some "start" text "second" nil"#
    };
    with_input(&compiled, input, "Sentence", |tree, profile, b, a| {
        let registry = profile.registry();
        let store = SourceStore::default();
        let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
        let package = profile.language("Sentence", b).map_err(err)?;
        let forms = [ForeignInlineForm {
            kind: "Form:DocumentInline",
            guest_schema: &compiled.doc.package.schema,
            guest_category: "Inline",
        }];
        let closure = if source_free {
            let label = generated(SentenceValue {
                root: Root::Inline(sentence_model::InlineRef(0)),
                nodes: vec![Kind::Text {
                    text: "second".into(),
                }],
                embeds: vec![],
            });
            let inline = DocumentSyntax {
                value: DocValue {
                    root: DocRoot::Inline(InlineRef(0)),
                    nodes: vec![DocNode {
                        kind: DocKind::Link {
                            target: LinkTarget::Relative {
                                path: "guide.md".into(),
                                fragment: Some("start".into()),
                            },
                            label: EmbedRef(0),
                        },
                        locations: vec![],
                        origin: None,
                        span: None,
                    }],
                    embeds: vec![sentence::embed(&label, registry, &mut codec, b).map_err(err)?],
                },
                sources: vec![],
                origins: vec![],
                views: vec![],
                source_maps: vec![],
            };
            check_adapter_boundaries(&inline, &compiled.doc.package.schema, registry)?;
            check_meaning_depth(&inline, package, &compiled.doc.package.schema, registry)?;
            check_typed_namespace(
                &inline,
                &package.schema,
                &compiled.doc.package.schema,
                registry,
            )?;
            nepl3_suite::adapters::sentence::document_guests::embed(
                &inline, registry, &mut codec, b,
            )
            .map_err(err)?
        } else {
            let imported = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Sentence,
                registry,
                b,
                &mut codec,
            )
            .map_err(err)?;
            let imported = sentence::lower(
                &imported.value.embeds[0],
                &package.schema,
                &forms,
                registry,
                &mut codec,
                b,
            )
            .map_err(err)?;
            let content = imported.value.embeds.first().ok_or("Doc Inline")?.clone();
            check_value_source_closure(&content, &compiled.doc.package.schema, registry)?;
            content
        };
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
        assert_eq!(used.usage().source_bytes == 0, source_free);
        for (reason, used) in [
            (StopReason::WorkLimit, used.usage().work),
            (StopReason::AllocationLimit, used.usage().allocation_units),
            (StopReason::DepthLimit, used.usage().depth),
            (StopReason::SourceLimit, used.usage().source_bytes),
            (StopReason::OutputLimit, used.usage().output_bytes),
        ] {
            if source_free && reason == StopReason::SourceLimit {
                continue;
            }
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
            if source_free && reason == StopReason::SourceLimit {
                continue;
            }
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

fn check_adapter_boundaries(
    document: &DocumentSyntax,
    surface: &nepl3_core::value::SchemaRef,
    registry: &nepl3_core::schema::SchemaRegistry,
) -> Result<(), String> {
    use nepl3_core::value::{NdfValue, TypedValue};
    use nepl3_sentence_core::model::InlineContent;
    use nepl3_suite::adapters::sentence::document_guests::{self as guest, Error};
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let encoded = guest::embed(document, registry, &mut codec, &mut budget()).map_err(err)?;
    let actual =
        guest::decode(&encoded, surface, registry, &mut codec, &mut budget()).map_err(err)?;
    retention::assert_doc_retention(document, &actual)?;
    let mut shallow = generated(SentenceValue {
        root: Root::Sentence(sentence_model::SentenceRef(1)),
        nodes: vec![
            Kind::ForeignInline {
                syntax: sentence_model::EmbedRef(0),
            },
            Kind::Sentence {
                inlines: vec![sentence_model::InlineRef(0)],
            },
        ],
        embeds: vec![encoded.clone()],
    });
    let mut measured = budget();
    guest::collect(&shallow, surface, registry, &mut codec, &mut measured).map_err(err)?;
    let cap = measured.usage().depth + 8;
    let mut limits = budget().limits();
    limits.depth = cap;
    guest::decode(
        &encoded,
        surface,
        registry,
        &mut codec,
        &mut Budget::new(limits),
    )
    .map_err(err)?;
    // Visit the shared guest on the short path first. The second occurrence
    // puts the same guest inside hidden Ruby reading and Strong wrappers.
    shallow.value.nodes = vec![Kind::ForeignInline {
        syntax: sentence_model::EmbedRef(0),
    }];
    for index in 1..cap - 3 {
        shallow.value.nodes.push(Kind::Strong {
            inline: sentence_model::InlineRef(index - 1),
        });
    }
    let last = shallow.value.nodes.len() as u64 - 1;
    shallow.value.nodes.push(Kind::Ruby {
        base: sentence_model::InlineRef(0),
        reading: sentence_model::InlineRef(last),
    });
    let ruby = shallow.value.nodes.len() as u64 - 1;
    shallow.value.nodes.push(Kind::Sentence {
        inlines: vec![
            sentence_model::InlineRef(0),
            sentence_model::InlineRef(ruby),
        ],
    });
    shallow.value.root = Root::Sentence(sentence_model::SentenceRef(
        shallow.value.nodes.len() as u64 - 1,
    ));
    let deep = generated(shallow.value);
    deep.value
        .validate_shape(&mut Budget::new(limits))
        .map_err(err)?;
    let mut bounded = Budget::new(limits);
    let result = guest::collect(&deep, surface, registry, &mut codec, &mut bounded);
    assert!(matches!(
        result,
        Err(Error::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(bounded.current_depth(), 0);
    assert_eq!(bounded.poll(), Err(StopReason::DepthLimit));
    let mut forged = encoded.clone();
    let InlineContent::Value {
        value: TypedValue::Record(record),
    } = &mut forged
    else {
        return Err("Doc record".into());
    };
    record.schema.digest = Digest::of(b"changed Doc schema");
    assert!(matches!(
        guest::decode(&forged, surface, registry, &mut codec, &mut budget()),
        Err(Error::Selection)
    ));

    let paragraph = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Block(BlockRef(0)),
            nodes: vec![DocNode {
                kind: DocKind::Paragraph { items: vec![] },
                locations: vec![],
                origin: None,
                span: None,
            }],
            embeds: vec![],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    assert!(matches!(
        guest::embed(&paragraph, registry, &mut codec, &mut budget()),
        Err(Error::Category)
    ));
    let raw = portable::to_value(&paragraph, registry, &mut codec, &mut budget()).map_err(err)?;
    let NdfValue::Record(record) = &raw else {
        return Err("paragraph record".into());
    };
    let wrong_root = InlineContent::Value {
        value: TypedValue::Record(record.clone()),
    };
    assert!(matches!(
        guest::decode(&wrong_root, surface, registry, &mut codec, &mut budget()),
        Err(Error::Category)
    ));

    for encoding in [true, false] {
        let run = |limits, cancelled| {
            let mut b = Budget::new(limits);
            if cancelled {
                b.cancel();
            }
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
            let result = if encoding {
                guest::embed(document, registry, &mut codec, &mut b).map(|_| ())
            } else {
                guest::decode(&encoded, surface, registry, &mut codec, &mut b).map(|_| ())
            };
            assert_eq!(b.current_depth(), 0);
            Ok::<_, String>((result, b))
        };
        let (result, baseline) = run(budget().limits(), false)?;
        result.map_err(err)?;
        for (reason, usage) in [
            (StopReason::WorkLimit, baseline.usage().work),
            (
                StopReason::AllocationLimit,
                baseline.usage().allocation_units,
            ),
            (StopReason::NodeLimit, baseline.usage().nodes),
            (StopReason::DepthLimit, baseline.usage().depth),
        ] {
            assert!(usage > 0);
            for cap in [0, usage - 1, usage] {
                let mut limits = budget().limits();
                match reason {
                    StopReason::WorkLimit => limits.work = cap,
                    StopReason::AllocationLimit => limits.allocation_units = cap,
                    StopReason::NodeLimit => limits.nodes = cap,
                    StopReason::DepthLimit => limits.depth = cap,
                    _ => return Err("resource".into()),
                }
                let (result, bounded) = run(limits, false)?;
                if cap == usage {
                    result.map_err(err)?;
                } else {
                    assert!(
                        matches!(result, Err(Error::Stopped(actual)) if actual == reason),
                        "{encoding} {reason:?} {cap}: {result:?}"
                    );
                    assert_eq!(bounded.poll(), Err(reason));
                }
            }
        }
        let (result, bounded) = run(budget().limits(), true)?;
        assert!(matches!(result, Err(Error::Stopped(StopReason::Cancelled))));
        assert_eq!(bounded.poll(), Err(StopReason::Cancelled));
    }
    Ok(())
}

fn check_meaning_depth(
    template: &DocumentSyntax,
    package: &nepl3_engine::package::LanguagePackage,
    surface: &nepl3_core::value::SchemaRef,
    registry: &nepl3_core::schema::SchemaRegistry,
) -> Result<(), String> {
    use nepl3_suite::adapters::sentence::document_guests as guest;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let mut label = SentenceValue {
        root: Root::Inline(sentence_model::InlineRef(40)),
        nodes: vec![Kind::Text {
            text: "deep".into(),
        }],
        embeds: vec![],
    };
    for node in 1..=40 {
        label.nodes.push(Kind::Strong {
            inline: sentence_model::InlineRef(node - 1),
        });
    }
    let mut inline = template.clone();
    inline.value.embeds[0] =
        sentence::embed(&generated(label), registry, &mut codec, &mut budget()).map_err(err)?;
    let content = guest::embed(&inline, registry, &mut codec, &mut budget()).map_err(err)?;
    let mut value = SentenceValue {
        root: Root::Sentence(sentence_model::SentenceRef(31)),
        nodes: vec![Kind::ForeignInline {
            syntax: sentence_model::EmbedRef(0),
        }],
        embeds: vec![content.clone()],
    };
    for node in 1..=30 {
        value.nodes.push(Kind::Strong {
            inline: sentence_model::InlineRef(node - 1),
        });
    }
    value.nodes.push(Kind::Sentence {
        inlines: vec![sentence_model::InlineRef(0), sentence_model::InlineRef(30)],
    });
    let outer = generated(value);
    let mut limits = budget().limits();
    limits.depth = 64;
    outer
        .validate(
            registry,
            &mut Budget::new(limits),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
    guest::decode(
        &content,
        surface,
        registry,
        &mut codec,
        &mut Budget::new(limits),
    )
    .map_err(err)?;
    let document = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Sentence(SentenceRef(0)),
            nodes: vec![DocNode {
                kind: DocKind::Sentence {
                    syntax: EmbedRef(0),
                },
                locations: vec![],
                origin: None,
                span: None,
            }],
            embeds: vec![
                sentence::embed(&outer, registry, &mut codec, &mut budget()).map_err(err)?,
            ],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let run = |document| {
        let mut bounded = Budget::new(limits);
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let result = SentenceGuestPrinter {
            registry,
            sentence_package: package,
            math_surface: None,
            doc_surface: Some(surface),
            codec: &mut codec,
        }
        .document(document, PrintMode::Prefix, &mut bounded);
        assert_eq!(bounded.current_depth(), 0);
        Ok::<_, String>((result, bounded))
    };
    run(&inline)?.0.map_err(err)?;
    let (result, bounded) = run(&document)?;
    assert!(
        matches!(result, Err(Error::Stopped(StopReason::DepthLimit))),
        "{result:?}"
    );
    assert_eq!(bounded.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

fn check_value_source_closure(
    syntax: &sentence_model::InlineContent,
    surface: &nepl3_core::value::SchemaRef,
    registry: &nepl3_core::schema::SchemaRegistry,
) -> Result<(), String> {
    use nepl3_core::value::{NdfValue, TypedValue};
    use nepl3_suite::adapters::sentence::document_guests as guest;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let doc = guest::decode(syntax, surface, registry, &mut codec, &mut budget()).map_err(err)?;
    assert!(!doc.sources.is_empty());
    let mut value = guest::embed(&doc, registry, &mut codec, &mut budget()).map_err(err)?;
    let sentence_model::InlineContent::Value {
        value: TypedValue::Record(record),
    } = &mut value
    else {
        return Err("Doc value".into());
    };
    assert_eq!(record.kind, "DocumentSyntax");
    record.fields[1] = NdfValue::List(vec![]);
    let sentence_model::InlineContent::Value { value: typed } = &value else {
        return Err("Doc value".into());
    };
    registry.validate_typed(typed, &mut budget()).map_err(err)?;
    let mut ambient = SourceStore::default();
    for source in &doc.sources {
        ambient.insert(source.clone()).map_err(err)?;
    }
    let mut admission = SourceAdmission::default();
    let mut receiver = FoundationCodec::new(registry, &ambient, &mut admission).map_err(err)?;
    let result = guest::decode(&value, surface, registry, &mut receiver, &mut budget());
    assert!(matches!(result, Err(guest::Error::Value(_))), "{result:?}");
    Ok(())
}

fn check_typed_namespace(
    template: &DocumentSyntax,
    sentence_surface: &nepl3_core::value::SchemaRef,
    doc_surface: &nepl3_core::value::SchemaRef,
    registry: &nepl3_core::schema::SchemaRegistry,
) -> Result<(), String> {
    use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode};
    use nepl3_suite::adapters::sentence::document_guests;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let mut definition = template.clone();
    definition.value.nodes[0].kind = DocKind::Anchor {
        id: "target".into(),
        label: EmbedRef(0),
    };
    let mut reference = template.clone();
    reference.value.nodes[0].kind = DocKind::Reference {
        target: "target".into(),
        label: EmbedRef(0),
    };
    let syntax = generated(SentenceValue {
        root: Root::Sentence(sentence_model::SentenceRef(2)),
        nodes: vec![
            Kind::ForeignInline {
                syntax: sentence_model::EmbedRef(0),
            },
            Kind::ForeignInline {
                syntax: sentence_model::EmbedRef(1),
            },
            Kind::Sentence {
                inlines: vec![sentence_model::InlineRef(0), sentence_model::InlineRef(1)],
            },
        ],
        embeds: vec![
            document_guests::embed(&definition, registry, &mut codec, &mut budget())
                .map_err(err)?,
            document_guests::embed(&reference, registry, &mut codec, &mut budget()).map_err(err)?,
        ],
    });
    let raw = nepl3_sentence_core::portable::syntax::to_value(
        &syntax,
        registry,
        &mut codec,
        &mut budget(),
    )
    .map_err(err)?;
    let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut receiver = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let decoded = nepl3_sentence_core::portable::syntax::from_value(
        &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
        registry,
        &mut receiver,
        &mut budget(),
    )
    .map_err(err)?;
    assert_eq!(syntax, decoded);
    let mut expected = None;
    for input in [syntax, decoded] {
        let rendered = nepl3_tools::doc::annotations::SentenceAnnotationRenderer {
            registry,
            surface: sentence_surface,
            math_surface: None,
            doc_surface: Some(doc_surface),
            codec: &mut receiver,
        }
        .render_syntax(input, &mut budget())
        .map_err(err)?;
        let mut ids = vec![];
        let mut references = vec![];
        for node in &rendered.markup.fragment.nodes {
            if let HtmlNode::Element { attributes, .. } = node {
                for attr in attributes {
                    match attr {
                        HtmlAttribute::Id { value } => ids.push(value),
                        HtmlAttribute::Href {
                            value: HtmlHref::Fragment { id },
                        } => references.push(id),
                        _ => (),
                    }
                }
            }
        }
        assert_eq!(ids.len(), 1);
        assert_eq!(references, ids);
        assert_eq!(rendered.foreign.len(), 2);
        let checked = nepl3_markup::html::validate(
            &rendered.markup.fragment,
            rendered.markup.slot,
            &rendered.markup.policy,
            &mut budget(),
        )
        .map_err(err)?;
        let html = nepl3_markup::html::serialize(&checked, &mut budget()).map_err(err)?;
        if let Some(expected) = &expected {
            assert_eq!(&html, expected);
        } else {
            expected = Some(html);
        }
    }
    Ok(())
}
