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
            Ok(())
        },
    )
}
