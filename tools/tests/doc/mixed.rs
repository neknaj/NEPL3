use super::*;
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_doc_core::{
    check::Category,
    lower::{self, LowerError},
};

struct UntrustedDecoder<'a, C> {
    codec: &'a mut C,
    mutation: usize,
}

#[test]
fn independent_sentence_payload_lowers_through_doc_consumer() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "paragraph cons \"literal [漢/かん]\" cons \"other\" nil",
        "Block",
        |tree, profile, b, a| {
            let original = &tree.tree().bundle;
            for case in 0..4 {
                let mut raw = original.clone();
                let mut literal_tokens = Vec::new();
                let empty = SourceStore::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
                for node in &raw.nodes {
                    if node.kind != "Leaf:SentenceLiteral" {
                        continue;
                    }
                    literal_tokens.push(node.token.ok_or("token")?.0 as usize);
                    let token = raw
                        .tokens
                        .get_mut(node.token.ok_or("token")?.0 as usize)
                        .ok_or("token index")?;
                    let owner = raw
                        .sources
                        .iter()
                        .find(|s| s.identity() == token.head.snapshot_ref())
                        .ok_or("owner")?;
                    let read = nepl3_sentence_core::literal::read(
                        owner,
                        token.head.start(),
                        token.head.end(),
                        true,
                        profile.registry(),
                        b,
                        codec.source_admission(),
                    )
                    .map_err(err)?;
                    let nepl3_sentence_core::literal::SentenceOutcome::Matched(literal) =
                        read.outcome
                    else {
                        return Err("literal".into());
                    };
                    let mut syntax = literal.syntax;
                    if case >= 2 {
                        let generated = SourceSnapshot::new(
                            SourceId(format!("generated-{}", token.head.start())),
                            1,
                            format!("memory:generated-{}", token.head.start()),
                            b"generated".to_vec(),
                            b,
                        )
                        .map_err(err)?;
                        let cover = generated.span(0, 9).map_err(err)?;
                        let nepl3_sentence_core::model::Root::Sentence(root) = syntax.value.root
                        else {
                            return Err("sentence root".into());
                        };
                        syntax.locations[root.0 as usize].head = Some(cover.clone());
                        syntax.locations[root.0 as usize].cover = Some(cover.clone());
                        syntax.source_maps.push(nepl3_core::origin::Mapping {
                            source: token.head.clone(),
                            target: cover,
                            kind: nepl3_core::origin::MappingKind::Transformed,
                        });
                        syntax.sources.push(generated);
                    }
                    token.payload = if case == 0 {
                        nepl3_sentence_core::portable::literal::to_value(
                            &syntax,
                            profile.registry(),
                            &mut codec,
                            b,
                        )
                    } else {
                        nepl3_sentence_core::portable::syntax::to_value(
                            &syntax,
                            profile.registry(),
                            &mut codec,
                            b,
                        )
                    }
                    .map_err(err)?;
                }
                if case == 3 {
                    let first = *literal_tokens.first().ok_or("first literal")?;
                    let last = *literal_tokens.last().ok_or("last literal")?;
                    if first == last {
                        return Err("need two literals".into());
                    }
                    raw.tokens[first].payload = raw.tokens[last].payload.clone();
                }
                let checked = raw
                    .validate_with_sources(profile.registry(), b, codec.source_admission())
                    .map_err(err)?;
                let result = nepl3_suite::adapters::sentence::literal::lower(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Block,
                    profile.registry(),
                    b,
                    &mut codec,
                );
                if case == 3 {
                    assert!(
                        matches!(
                            result,
                            Err(nepl3_suite::adapters::sentence::literal::Error::Lower(
                                LowerError::LiteralPayload { .. }
                            ))
                        ),
                        "wrong token: {result:?}"
                    );
                    continue;
                }
                let doc = result.map_err(err)?;
                assert_eq!(
                    &doc.origins[..original.origins.len()],
                    original.origins.as_slice()
                );
                assert_eq!(doc.views.len(), original.tokens.len());
                if case < 2 {
                    assert_eq!(doc.sources, original.sources);
                } else {
                    assert_eq!(
                        doc.sources.len(),
                        original.sources.len() + literal_tokens.len()
                    );
                }
                // The legacy Doc payload is not a fallback accepted by the new adapter.
                let mut retired = original.clone();
                for node in &retired.nodes {
                    if node.kind != "Leaf:SentenceLiteral" {
                        continue;
                    }
                    let token = retired
                        .tokens
                        .get_mut(node.token.ok_or("legacy token")?.0 as usize)
                        .ok_or("legacy token index")?;
                    let NdfValue::Record(record) = &mut token.payload else {
                        return Err("legacy payload".into());
                    };
                    record.schema = profile
                        .registry()
                        .selected("nepl3.doc", 1)
                        .ok_or("Doc schema")?
                        .clone();
                    record.kind = "SentencePayload".into();
                }
                assert!(
                    retired
                        .validate_with_sources(profile.registry(), b, codec.source_admission())
                        .is_err()
                );
            }
            Ok(())
        },
    )
}
impl<C: FoundationValueCodec> lower::LiteralDecoder for UntrustedDecoder<'_, C> {
    type Error = LowerError;
    fn admission(&mut self) -> &mut SourceAdmission {
        self.codec.source_admission()
    }
    fn read(
        &mut self,
        id: nepl3_core::syntax::NodeRef,
        node: &nepl3_core::syntax::SyntaxNode,
        bundle: &nepl3_core::syntax::SyntaxBundle,
        registry: &nepl3_core::schema::SchemaRegistry,
        b: &mut Budget,
    ) -> Result<nepl3_doc_core::model::DocumentSyntax, Self::Error> {
        use nepl3_doc_core::model::*;
        let token = node
            .token
            .and_then(|t| bundle.tokens.get(t.0 as usize))
            .ok_or(LowerError::LiteralPayload { node: id })?;
        let owner = bundle
            .sources
            .iter()
            .find(|s| s.identity() == token.head.snapshot_ref())
            .ok_or(LowerError::LiteralPayload { node: id })?;
        let syntax = nepl3_sentence_core::portable::literal::from_value(
            &token.payload,
            owner,
            registry,
            self.codec,
            b,
        )
        .map_err(|_| LowerError::LiteralPayload { node: id })?;
        let mut doc = nepl3_suite::adapters::sentence::document(
            &syntax,
            registry,
            b,
            self.codec.source_admission(),
        )
        .map_err(|_| LowerError::LiteralPayload { node: id })?;
        match self.mutation {
            0 => doc.value.root = DocRoot::Sentence(SentenceRef(doc.value.nodes.len() as u64)),
            1 => {
                doc.value.nodes[0].kind = DocKind::Concat {
                    inlines: vec![InlineRef(doc.value.nodes.len() as u64)],
                }
            }
            2 => {
                doc.value.nodes[0].origin =
                    Some(nepl3_core::origin::OriginId(doc.origins.len() as u64))
            }
            3 => doc.sources.clear(),
            4 => {
                b.stop(StopReason::Cancelled);
            }
            6 => {
                b.stop(StopReason::WorkLimit);
                return Err(LowerError::LiteralPayload { node: id });
            }
            _ => {}
        }
        Ok(doc)
    }
}

#[test]
fn public_literal_decoder_success_does_not_bypass_local_closure_or_stop() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "paragraph cons sentence cons text \"prefix\" nil cons \"literal\" nil",
        "Block",
        |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            for mutation in 0..7 {
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let mut operation = budget();
                let mut decoder = UntrustedDecoder {
                    codec: &mut codec,
                    mutation,
                };
                let result = operation.with_depth_at_least(5, |b| {
                    let caller_depth = b.current_depth();
                    let result = lower::document_with_decoder(
                        &checked,
                        &compiled.doc.package.schema,
                        Category::Block,
                        profile.registry(),
                        b,
                        &mut decoder,
                    );
                    assert_eq!(b.current_depth(), caller_depth);
                    result
                });
                if mutation == 5 {
                    result.map_err(err)?;
                    assert_eq!(
                        operation.usage().source_bytes,
                        tree.tree()
                            .bundle
                            .sources
                            .iter()
                            .map(|s| s.text().len() as u64)
                            .sum::<u64>()
                    );
                } else if mutation == 4 || mutation == 6 {
                    let reason = if mutation == 4 {
                        StopReason::Cancelled
                    } else {
                        StopReason::WorkLimit
                    };
                    assert_eq!(result.err(), Some(LowerError::Stopped(reason)));
                    assert_eq!(operation.poll(), Err(reason));
                } else if mutation == 1 {
                    assert!(
                        matches!(
                            result,
                            Err(LowerError::Structure(
                                nepl3_doc_core::check::StructureError::Shape(
                                    nepl3_doc_core::check::ShapeError::Reference(_)
                                )
                            ))
                        ),
                        "forged child: {result:?}"
                    );
                } else {
                    assert!(
                        matches!(result, Err(LowerError::Structure(_))),
                        "mutation {mutation}: {result:?}"
                    );
                }
                assert_eq!(operation.current_depth(), 0);
            }
            Ok(())
        },
    )
}

#[test]
fn referenced_sentence_payloads_cross_the_owned_syntax_boundary_without_source_copies()
-> Result<(), String> {
    let compiled = compiled()?;
    let mut source = String::from("article en \"Title\" body cons paragraph ");
    for i in 0..64 {
        source.push_str(&format!("cons \"Sentence {i} [base/reading].\" "));
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
                for node in &bundle.nodes {
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
                let doc = nepl3_suite::adapters::sentence::literal::lower(
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
                    let DocKind::Sentence { inlines } = node(item.0) else {
                        return Err("sentence".into());
                    };
                    assert_eq!(inlines.len(), 3);
                    assert!(
                        matches!(node(inlines[0].0), DocKind::Text { text } if text == &format!("Sentence {i} "))
                    );
                    let DocKind::Ruby { base, reading } = node(inlines[1].0) else {
                        return Err("ruby".into());
                    };
                    assert!(matches!(node(base.0), DocKind::Text { text } if text == "base"));
                    assert!(matches!(node(reading.0), DocKind::Text { text } if text == "reading"));
                    assert!(matches!(node(inlines[2].0), DocKind::Text { text } if text == "."));
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
fn literal_and_prefix_share_one_lower_with_source_and_local_view_retention() -> Result<(), String> {
    let compiled = compiled()?;
    let mixed = r#"article en "Title" body cons paragraph cons "Before [base/reading]" cons sentence cons text "after" nil nil nil"#;
    let prefix = r#"article en sentence cons text "Title" nil body cons paragraph cons sentence cons text "Before " cons ruby text "base" text "reading" nil cons sentence cons text "after" nil nil nil"#;
    let mut values = Vec::new();
    for source in [mixed, prefix] {
        values.push(with_input(
            &compiled,
            source,
            "Article",
            |tree, profile, b, a| {
                let raw = &tree.tree().bundle;
                let checked = raw
                    .validate_with_sources(profile.registry(), b, a)
                    .map_err(err)?;
                let empty = SourceStore::default();
                let mut operation = budget();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let doc = nepl3_suite::adapters::sentence::literal::lower(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    &mut operation,
                    &mut codec,
                )
                .map_err(err)?;
                assert_eq!(&doc.origins[..raw.origins.len()], raw.origins.as_slice());
                // Every original token view remains in its own local ID space;
                // decoding its payload must not publish a duplicate owner.
                assert_eq!(doc.views.len(), raw.tokens.len());
                assert_eq!(
                    operation.usage().source_bytes,
                    raw.sources
                        .iter()
                        .map(|s| s.text().len() as u64)
                        .sum::<u64>()
                );
                let value = nepl3_doc_core::portable::to_value(
                    &doc,
                    profile.registry(),
                    &mut codec,
                    &mut operation,
                )
                .map_err(err)?;
                let bytes = nepl3_wire::encode(&value, &mut operation).map_err(err)?;
                let mut fresh = SourceAdmission::default();
                let mut receiver =
                    FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
                let received = nepl3_doc_core::portable::from_value(
                    &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                    profile.registry(),
                    &mut receiver,
                    &mut budget(),
                )
                .map_err(err)?;
                assert_doc_retention(&doc, &received);
                Ok(doc.value)
            },
        )?);
    }
    assert_eq!(values[0].root, values[1].root);
    assert_eq!(
        values[0].nodes.iter().map(|n| &n.kind).collect::<Vec<_>>(),
        values[1].nodes.iter().map(|n| &n.kind).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn mixed_lower_rejects_other_token_view_and_semantic_positions_without_mutation()
-> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        r#"paragraph cons "one" cons "two" nil"#,
        "Block",
        |tree, profile, b, a| {
            let original = &tree.tree().bundle;
            let tokens = original
                .nodes
                .iter()
                .filter(|n| n.kind == "Leaf:SentenceLiteral")
                .map(|n| n.token.ok_or("token"))
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(tokens.len(), 2);
            let first = tokens[0].0 as usize;
            let second = tokens[1].0 as usize;
            let mut empty = SourceStore::default();
            for source in &original.sources {
                empty.insert(source.clone()).map_err(err)?;
            }
            for case in 0..8 {
                let mut raw = original.clone_with_budget(b).map_err(err)?;
                let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
                let owner = original
                    .sources
                    .iter()
                    .find(|s| s.identity() == raw.tokens[first].head.snapshot_ref())
                    .ok_or("owner source")?;
                let mut doc = nepl3_sentence_core::portable::literal::from_value(
                    &raw.tokens[first].payload,
                    owner,
                    profile.registry(),
                    &mut codec,
                    b,
                )
                .map_err(err)?;
                match case {
                    0 => {
                        raw.tokens[first].payload = raw.tokens[second].payload.clone();
                    }
                    1 => {
                        doc.locations[0].cover = Some(raw.tokens[second].head.clone());
                        if doc.locations[0].head.is_some() {
                            doc.locations[0].head = doc.locations[0].cover.clone();
                        }
                    }
                    2 => {
                        doc.origins[0] =
                            nepl3_core::origin::Origin::Direct(raw.tokens[second].head.clone());
                    }
                    3 => {
                        let at = doc.views[0].view.elements[0].span.start();
                        doc.views[0].view.elements[0].span =
                            doc.sources[0].span(at, at).map_err(err)?;
                    }
                    7 => {}
                    _ => {
                        let other = SourceSnapshot::new(
                            SourceId("different-source".into()),
                            0,
                            "memory:other".into(),
                            original.sources[0].text().as_bytes().to_vec(),
                            b,
                        )
                        .map_err(err)?;
                        let at = doc.locations[0].cover.as_ref().ok_or("span")?;
                        doc.locations[0].cover =
                            Some(other.span(at.start(), at.end()).map_err(err)?);
                        if doc.locations[0].head.is_some() {
                            doc.locations[0].head = doc.locations[0].cover.clone();
                        }
                        if case >= 5 {
                            // A declared transformed map is evidence of the
                            // generated position, but must lead to this token.
                            doc.source_maps.push(nepl3_core::origin::Mapping {
                                source: raw.tokens[if case == 5 { first } else { second }]
                                    .head
                                    .clone(),
                                target: doc.locations[0].cover.clone().ok_or("generated span")?,
                                kind: nepl3_core::origin::MappingKind::Transformed,
                            });
                        }
                        doc.sources.push(other);
                    }
                }
                if case != 0 {
                    raw.tokens[first].payload = nepl3_sentence_core::portable::syntax::to_value(
                        &doc,
                        profile.registry(),
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                }
                if case == 7 {
                    // The ambient host store has this snapshot. It must not
                    // complete an omitted payload declaration implicitly.
                    let NdfValue::Record(record) = &mut raw.tokens[first].payload else {
                        return Err("document record".into());
                    };
                    record.fields[2] = NdfValue::List(vec![]);
                }
                let checked = raw
                    .validate_with_sources(profile.registry(), b, codec.source_admission())
                    .map_err(err)?;
                let result = nepl3_suite::adapters::sentence::literal::lower(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Block,
                    profile.registry(),
                    b,
                    &mut codec,
                );
                if case == 7 {
                    assert!(
                        matches!(
                            result,
                            Err(nepl3_suite::adapters::sentence::literal::Error::Payload { .. })
                        ),
                        "missing declaration: {result:?}"
                    );
                } else if case == 5 {
                    assert!(result.is_ok(), "explicit local mapping: {result:?}");
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(nepl3_suite::adapters::sentence::literal::Error::Lower(
                                LowerError::LiteralPayload { .. }
                            ))
                        ),
                        "case {case}: {result:?}"
                    );
                }
            }
            let checked = original
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            nepl3_suite::adapters::sentence::literal::lower(
                &checked,
                &compiled.doc.package.schema,
                Category::Block,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            Ok(())
        },
    )
}

#[test]
fn mixed_lower_stops_keep_original_tree_and_caller_depth() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        r#"paragraph cons "[base/reading]" cons sentence cons text "after" nil nil"#,
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
                        nepl3_suite::adapters::sentence::literal::lower(
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
                        Err(nepl3_suite::adapters::sentence::literal::Error::Stopped(reason)) => {
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
                nepl3_suite::adapters::sentence::literal::lower(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Block,
                    profile.registry(),
                    &mut cancelled,
                    &mut codec
                ),
                Err(nepl3_suite::adapters::sentence::literal::Error::Stopped(
                    StopReason::Cancelled
                ))
            ));
            Ok(())
        },
    )
}

#[test]
fn mixed_lower_keeps_doc_guest_syntax_without_meaning_check() -> Result<(), String> {
    use nepl3_doc_core::{check::ShapeError, model::*};
    let compiled = compiled()?;
    let source = r#"paragraph cons code Doc article en "Guest title" body cons paragraph cons sentence cons ruby text "" text "r" nil nil nil cons "host-tail" nil"#;
    with_input(&compiled, source, "Block", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
        let doc = nepl3_suite::adapters::sentence::literal::lower(
            &checked,
            &compiled.doc.package.schema,
            Category::Block,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        assert_eq!(doc.value.embeds.len(), 1);
        assert!(
            doc.value
                .nodes
                .iter()
                .any(|n| matches!(&n.kind,DocKind::Text{text} if text=="host-tail"))
        );
        let guest = doc.value.embeds[0]
            .closure
            .syntax
            .bundle
            .validate_with_sources(profile.registry(), b, codec.source_admission())
            .map_err(err)?;
        assert!(matches!(
            nepl3_suite::adapters::sentence::literal::lower(
                &guest,
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                b,
                &mut codec
            ),
            Err(nepl3_suite::adapters::sentence::literal::Error::Lower(
                LowerError::Shape(ShapeError::EmptyAnnotationPart(_))
            ))
        ));
        let wire = nepl3_doc_core::portable::to_value(&doc, profile.registry(), &mut codec, b)
            .map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut receiver =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let received =
            nepl3_doc_core::portable::from_value(&wire, profile.registry(), &mut receiver, b)
                .map_err(err)?;
        assert_doc_retention(&doc, &received);
        Ok(())
    })
}
