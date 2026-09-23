use super::*;
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_doc_core::{
    check::Category,
    lower::{self, DocumentLowerError, LowerError},
};

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
                let doc = lower::document(
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
                let mut doc = nepl3_doc_core::portable::sentence::from_value(
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
                        doc.value.nodes[0].span = Some(raw.tokens[second].head.clone());
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
                        let at = doc.value.nodes[0].span.as_ref().ok_or("span")?;
                        doc.value.nodes[0].span =
                            Some(other.span(at.start(), at.end()).map_err(err)?);
                        if case >= 5 {
                            // A declared transformed map is evidence of the
                            // generated position, but must lead to this token.
                            doc.source_maps.push(nepl3_core::origin::Mapping {
                                source: raw.tokens[if case == 5 { first } else { second }]
                                    .head
                                    .clone(),
                                target: doc.value.nodes[0].span.clone().ok_or("generated span")?,
                                kind: nepl3_core::origin::MappingKind::Transformed,
                            });
                        }
                        doc.sources.push(other);
                    }
                }
                if case != 0 {
                    raw.tokens[first].payload =
                        nepl3_doc_core::portable::to_value(&doc, profile.registry(), &mut codec, b)
                            .map_err(err)?;
                }
                if case == 7 {
                    // The ambient host store has this snapshot. It must not
                    // complete an omitted payload declaration implicitly.
                    let NdfValue::Record(record) = &mut raw.tokens[first].payload else {
                        return Err("document record".into());
                    };
                    record.fields[1] = NdfValue::List(vec![]);
                }
                let checked = raw
                    .validate_with_sources(profile.registry(), b, codec.source_admission())
                    .map_err(err)?;
                let result = lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Block,
                    profile.registry(),
                    b,
                    &mut codec,
                );
                if case == 7 {
                    assert!(
                        matches!(result, Err(DocumentLowerError::Payload { .. })),
                        "missing declaration: {result:?}"
                    );
                } else if case == 5 {
                    assert!(result.is_ok(), "explicit local mapping: {result:?}");
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(DocumentLowerError::Lower(LowerError::LiteralPayload { .. }))
                        ),
                        "case {case}: {result:?}"
                    );
                }
            }
            let checked = original
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            lower::document(
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
