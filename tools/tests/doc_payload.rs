use nepl3_core::{source::*, value_codec::FoundationValueCodec};
use nepl3_doc_core::{check::Category, lower};
use nepl3_tools::doc::source::{budget, compiled, err};
use nepl3_wire::foundation::FoundationCodec;
#[test]
fn referenced_sentence_payloads_cross_the_owned_syntax_boundary_without_source_copies()
-> Result<(), String> {
    let compiled = compiled()?;
    let mut source = String::from("article en sentence \"Title\" body cons paragraph ");
    for i in 0..64 {
        source.push_str(&format!("cons sentence \"Sentence {i} [base/reading].\" "));
    }
    source.push_str("nil nil");
    source.push_str(&" ".repeat(65_536));
    {
        nepl3_tools::doc::source::with_input_route(
            true,
            &compiled,
            &source,
            "Article",
            |tree, profile, b, a| {
                let bundle = &tree.tree().bundle;
                let mut count = 0;
                let mut pending = vec![bundle];
                while let Some(bundle) = pending.pop() {
                    for node in &bundle.nodes {
                        for field in &node.fields {
                            if let nepl3_core::syntax::FieldValue::Foreign(guest) = field {
                                pending.push(&guest.bundle);
                            }
                        }
                        if node.kind != "Leaf:SentenceLiteral" {
                            continue;
                        }
                        let token = &bundle.tokens[node.token.ok_or("token")?.0 as usize];
                        let nepl3_core::value::NdfValue::Record(record) = &token.payload else {
                            return Err("record".into());
                        };
                        assert_eq!(record.kind, "SentenceLiteralPayload");
                        // Independent of the surrounding 64 KiB source, a short
                        // sentence must not serialize that source as its own payload.
                        {
                            assert!(
                                nepl3_wire::encode(&token.payload, &mut budget())
                                    .map_err(err)?
                                    .len()
                                    < source.len()
                            );
                        }
                        count += 1;
                    }
                }
                assert_eq!(count, 65);
                let empty = SourceStore::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
                let wire = codec.encode_syntax(bundle, b).map_err(err)?;
                let wire = nepl3_wire::decode(
                    &nepl3_wire::encode(&wire, &mut budget()).map_err(err)?,
                    &mut budget(),
                )
                .map_err(err)?;
                let mut fresh = SourceAdmission::default();
                let mut receiver =
                    FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
                let raw = receiver.decode_syntax(&wire, &mut budget()).map_err(err)?;
                let mut admitted = SourceAdmission::default();
                let checked = raw
                    .validate_with_sources(profile.registry(), &mut budget(), &mut admitted)
                    .map_err(err)?;
                let doc = lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    &mut budget(),
                    &mut receiver,
                )
                .map_err(err)?;
                use nepl3_doc_core::model::{DocKind, DocRoot};
                let DocRoot::Article(root) = doc.value.root else {
                    return Err("article".into());
                };
                let node = |id: u64| &doc.value.nodes[id as usize].kind;
                let DocKind::Article { body, .. } = node(root.0) else {
                    return Err("article node".into());
                };
                let DocKind::Body { blocks } = node(body.0) else {
                    return Err("body".into());
                };
                assert_eq!(blocks.len(), 1);
                let DocKind::Paragraph { items } = node(blocks[0].0) else {
                    return Err("paragraph".into());
                };
                assert_eq!(items.len(), 64);
                for (i, item) in items.iter().enumerate() {
                    let DocKind::Sentence { syntax } = node(item.0) else {
                        return Err("sentence".into());
                    };
                    let embed = &doc.value.embeds[syntax.0 as usize];
                    let sentence = nepl3_suite::adapters::document::sentence::lower(
                        embed,
                        embed.schema(),
                        &[],
                        profile.registry(),
                        &mut receiver,
                        &mut budget(),
                    )
                    .map_err(err)?;
                    use nepl3_sentence_core::model::{Kind, Root};
                    let Root::Sentence(root) = sentence.value.root else {
                        return Err("Sentence root".into());
                    };
                    let node = |id: u64| &sentence.value.nodes[id as usize];
                    let Kind::Sentence { inlines } = node(root.0) else {
                        return Err("Sentence node".into());
                    };
                    assert_eq!(inlines.len(), 3);
                    assert!(
                        matches!(node(inlines[0].0), Kind::Text { text } if text == &format!("Sentence {i} "))
                    );
                    let Kind::Ruby { base, reading } = node(inlines[1].0) else {
                        return Err("ruby".into());
                    };
                    assert!(matches!(node(base.0), Kind::Text { text } if text == "base"));
                    assert!(matches!(node(reading.0), Kind::Text { text } if text == "reading"));
                    assert!(matches!(node(inlines[2].0), Kind::Text { text } if text == "."));
                }
                assert_eq!(doc.sources.len(), 1);
                assert_eq!(doc.sources[0].text(), source);
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn sentence_payload_cannot_be_reassigned_to_another_doc_occurrence() -> Result<(), String> {
    use nepl3_doc_core::model::DocContent;
    use nepl3_sentence_core::lower::{literal, presentation};
    use nepl3_suite::adapters::document::sentence;
    let compiled = compiled()?;
    nepl3_tools::doc::source::with_input(
        &compiled,
        r#"paragraph cons sentence "one" cons sentence "two" nil"#,
        "Block",
        |tree, profile, b, a| {
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let document = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Block,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let [first, second] = document.value.embeds.as_slice() else {
                return Err("two independently owned Sentence occurrences".into());
            };
            let original = first.clone();
            let second_closure = second.syntax().ok_or("second syntax")?;
            let second_bundle = &second_closure.syntax.bundle;
            let second_root = second_bundle.node(second_bundle.root).map_err(err)?;
            let second_token =
                &second_bundle.tokens[second_root.token.ok_or("second token")?.0 as usize];
            // Both valid payloads name the same source snapshot. Only their
            // token ownership differs; swapping one must fail after decoding.
            sentence::lower(
                first,
                first.schema(),
                &[],
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            sentence::lower(
                second,
                second.schema(),
                &[],
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            // SentenceLiteralPayload fields are value, locations, origins and
            // view. Each substituted field remains schema-valid on its own.
            // Rejection must therefore come from its owner association.
            for field in [None, Some(1), Some(2), Some(3)] {
                let mut changed = first.clone();
                let DocContent::Syntax { closure } = &mut changed.content else {
                    return Err("first syntax".into());
                };
                let bundle = &mut closure.syntax.bundle;
                let root = bundle.root;
                let token = bundle.node(root).map_err(err)?.token.ok_or("first token")?;
                let payload = &mut bundle.tokens[token.0 as usize].payload;
                if let Some(field) = field {
                    let (
                        nepl3_core::value::NdfValue::Record(first),
                        nepl3_core::value::NdfValue::Record(second),
                    ) = (payload, &second_token.payload)
                    else {
                        return Err("Sentence literal records".into());
                    };
                    first.fields[field] = second.fields[field].clone();
                } else {
                    *payload = second_token.payload.clone();
                }
                let result = sentence::lower(
                    &changed,
                    changed.schema(),
                    &[],
                    profile.registry(),
                    &mut codec,
                    &mut budget(),
                );
                if field.is_none() {
                    assert!(
                        matches!(result, Err(sentence::Error::Lower(presentation::Error::Literal(literal::Error::TokenMismatch(id)))) if id == root),
                        "{result:?}"
                    );
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(sentence::Error::Lower(presentation::Error::Literal(
                                literal::Error::Payload(_)
                            )))
                        ),
                        "field {field:?}: {result:?}"
                    );
                }
                assert_eq!(first, &original);
            }
            // Keep the same owner but change the payload's view span. A valid
            // zero-width span must not replace the reader's original view.
            let mut changed = first.clone();
            let DocContent::Syntax { closure } = &mut changed.content else {
                return Err("first syntax".into());
            };
            let bundle = &mut closure.syntax.bundle;
            let root = bundle.root;
            let token_id = bundle.node(root).map_err(err)?.token.ok_or("token")?;
            let token = &mut bundle.tokens[token_id.0 as usize];
            let mut views = token.views.clone();
            let span = &views.elements[0].span;
            let source = bundle
                .sources
                .iter()
                .find(|s| s.identity() == span.snapshot_ref())
                .ok_or("view source")?;
            views.elements[0].span = source.span(span.start(), span.start()).map_err(err)?;
            let mut sources = SourceStore::default();
            for source in &bundle.sources {
                sources.insert(source.clone()).map_err(err)?;
            }
            let encoded = codec
                .scoped(&sources)
                .encode_views(&views, &mut budget())
                .map_err(err)?;
            let nepl3_core::value::NdfValue::Record(payload) = &mut token.payload else {
                return Err("payload".into());
            };
            let nepl3_core::value::NdfValue::Record(view) = &mut payload.fields[3] else {
                return Err("view".into());
            };
            view.fields[2] = encoded;
            assert!(
                matches!(sentence::lower(&changed, changed.schema(), &[], profile.registry(), &mut codec, &mut budget()), Err(sentence::Error::Lower(presentation::Error::Literal(literal::Error::TokenMismatch(id)))) if id == root)
            );
            Ok(())
        },
    )
}

#[test]
fn generated_sentence_views_keep_explicit_mapping_and_closed_sources() -> Result<(), String> {
    use nepl3_core::{
        origin::{Mapping, MappingKind},
        value::{NdfValue, TypedValue},
        view::{ViewField, ViewRef},
    };
    use nepl3_doc_core::model::{DocContent, DocEmbed, EmbedKind};
    use nepl3_sentence_core::{portable, syntax};
    use nepl3_suite::adapters::document::sentence;
    let compiled = compiled()?;
    nepl3_tools::doc::source::with_input(
        &compiled,
        r#"sentence "漢𝄞""#,
        "Sentence",
        |tree, profile, b, a| {
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let doc = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Sentence,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let [embed] = doc.value.embeds.as_slice() else {
                return Err("one Sentence".into());
            };
            let mut input = sentence::lower(
                embed,
                embed.schema(),
                &[],
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let mut child = input.views[0].view.elements[0].clone();
            let source_span = child.span.clone();
            let owner = input
                .sources
                .iter()
                .find(|source| source.identity() == source_span.snapshot_ref())
                .ok_or("owner")?;
            let content = owner
                .text()
                .get(source_span.start() as usize..source_span.end() as usize)
                .ok_or("source slice")?;
            let generated = SourceSnapshot::new(
                SourceId("generated-sentence".into()),
                0,
                "memory:generated-sentence".into(),
                content.as_bytes().to_vec(),
                &mut budget(),
            )
            .map_err(err)?;
            let target = generated.span(0, content.len() as u64).map_err(err)?;
            child.span = target.clone();
            let index = input.views[0].view.elements.len() as u64;
            input.views[0].view.elements[0].fields.push(ViewField {
                name: "generated".into(),
                children: vec![ViewRef(index)],
            });
            input.views[0].view.elements.push(child);
            input.sources.push(generated);
            assert!(matches!(
                input.validate(
                    profile.registry(),
                    &mut budget(),
                    &mut SourceAdmission::default()
                ),
                Err(syntax::Error::View(_))
            ));
            input.source_maps.push(Mapping {
                source: source_span,
                target,
                kind: MappingKind::Exact,
            });
            let mut wrong = input.clone();
            let mapping = &mut wrong.source_maps[0];
            let owner = wrong
                .sources
                .iter()
                .find(|s| s.identity() == mapping.source.snapshot_ref())
                .ok_or("mapping owner")?;
            mapping.source = owner
                .span(0, mapping.source.end() - mapping.source.start())
                .map_err(err)?;
            mapping.kind = MappingKind::Transformed;
            // Equal-length ranges in the same snapshot are insufficient: the
            // mapping must connect this generated view to its owning head.
            let rejected = wrong.validate(
                profile.registry(),
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(
                matches!(rejected, Err(syntax::Error::View(_))),
                "{:?}",
                rejected.err()
            );
            let encoded =
                portable::syntax::to_value(&input, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?;
            let bytes = nepl3_wire::encode(&encoded, &mut budget()).map_err(err)?;
            let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let NdfValue::Record(record) = &received else {
                return Err("SentenceSyntax record".into());
            };
            let placement = DocEmbed {
                kind: EmbedKind::Sentence,
                content: DocContent::Value {
                    value: TypedValue::Record(record.clone()),
                },
            };
            let mut admission = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let actual = sentence::lower(
                &placement,
                embed.schema(),
                &[],
                profile.registry(),
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(actual.value, input.value);
            assert_eq!(actual.locations, input.locations);
            assert_eq!(actual.origins, input.origins);
            assert_eq!(actual.views, input.views);
            assert_eq!(actual.source_maps, input.source_maps);
            assert_eq!(actual.sources.len(), input.sources.len());
            for source in &input.sources {
                assert!(actual.sources.contains(source));
            }
            // The ambient store contains both snapshots. An omitted declaration
            // must still fail at the closed SentenceSyntax boundary.
            let mut ambient = SourceStore::default();
            for source in &input.sources {
                ambient.insert(source.clone()).map_err(err)?;
            }
            let mut admission = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(profile.registry(), &ambient, &mut admission).map_err(err)?;
            let mut missing = placement;
            let DocContent::Value {
                value: TypedValue::Record(record),
            } = &mut missing.content
            else {
                return Err("record placement".into());
            };
            record.fields[2] = NdfValue::List(vec![]);
            assert!(matches!(
                sentence::lower(
                    &missing,
                    embed.schema(),
                    &[],
                    profile.registry(),
                    &mut receiver,
                    &mut budget()
                ),
                Err(sentence::Error::Value(_))
            ));
            Ok(())
        },
    )
}

#[test]
fn mixed_lower_stops_keep_original_tree_and_caller_depth() -> Result<(), String> {
    use nepl3_core::budget::{Budget, StopReason};
    let compiled = compiled()?;
    nepl3_tools::doc::source::with_input(
        &compiled,
        r#"paragraph cons sentence "[base/reading]" cons sentence sentence cons text "after" nil nil"#,
        "Block",
        |tree, profile, b, a| {
            let original = &tree.tree().bundle;
            let saved = original.clone_with_budget(b).map_err(err)?;
            let checked = original
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut stops = 0;
            let mut successes = 0;
            for resource in 0..5 {
                for cap in [0, 1, 8, 64, 512, 4096, 32768, 262144, 2097152] {
                    let mut limits = budget().limits();
                    let expected = match resource {
                        0 => {
                            limits.work = cap;
                            StopReason::WorkLimit
                        }
                        1 => {
                            limits.allocation_units = cap;
                            StopReason::AllocationLimit
                        }
                        2 => {
                            limits.source_bytes = cap;
                            StopReason::SourceLimit
                        }
                        3 => {
                            limits.nodes = cap;
                            StopReason::NodeLimit
                        }
                        _ => {
                            limits.depth = cap;
                            StopReason::DepthLimit
                        }
                    };
                    let mut operation = Budget::new(limits);
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &empty, &mut admission)
                            .map_err(err)?;
                    let result = operation.with_depth_at_least(7, |b| {
                        lower::document(
                            &checked,
                            &compiled.doc.package.schema,
                            Category::Block,
                            profile.registry(),
                            b,
                            &mut codec,
                        )
                    });
                    match result {
                        Ok(doc) => {
                            successes += 1;
                            doc.validate_structure(
                                profile.registry(),
                                &mut budget(),
                                &mut SourceAdmission::default(),
                            )
                            .map_err(err)?;
                        }
                        Err(lower::LowerError::Stopped(reason)) => {
                            stops += 1;
                            assert_eq!(reason, expected);
                            assert_eq!(operation.poll(), Err(expected));
                        }
                        Err(other) => {
                            return Err(format!("resource {resource} cap {cap}: {other:?}"));
                        }
                    }
                    assert_eq!(operation.current_depth(), 0);
                    assert_eq!(original, &saved);
                }
            }
            assert!(stops > 0 && successes > 0);
            let mut cancelled = budget();
            cancelled.cancel();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            assert!(matches!(
                lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Block,
                    profile.registry(),
                    &mut cancelled,
                    &mut codec
                ),
                Err(lower::LowerError::Stopped(StopReason::Cancelled))
            ));
            Ok(())
        },
    )
}
