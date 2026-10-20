use super::*;
use nepl3_doc_core::{
    check::Category,
    labels::{DocLabelId, namespace::MemberId},
    model::*,
};
use nepl3_tools::doc::annotations::{
    SentenceAnnotationRenderer,
    article::{
        self,
        namespace::{self, MemberOrigin, SentenceOccurrenceId},
    },
};

#[test]
fn explicit_article_namespace_interleaves_outer_and_sentence_sites() -> Result<(), String> {
    let compiled = nepl3_tools::doc::source::compiled_with_sentence_forms(&[
        nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "document",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "Article namespace test",
        },
    ])?;
    let source = r#"article en sentence nil body cons paragraph
cons sentence cons ref inner text "root ref" nil
cons Sentence sentence cons document anchor inner text "guest def" nil
cons sentence cons anchor outer text "root def" nil
cons Sentence sentence cons document ref outer text "guest ref" nil
nil nil"#;
    for native in [false, true] {
        for scenario in 0..6 {
            let input = if scenario == 1 {
                source.replace("anchor outer", "anchor inner")
            } else if scenario == 4 {
                source.replace(
                    "document anchor inner text \"guest def\"",
                    "document concat cons anchor inner text \"a\" cons anchor inner text \"b\" nil",
                )
            } else if scenario == 5 {
                source.replace("Sentence sentence cons document anchor inner text \"guest def\" nil", "Sentence sentence cons document anchor inner text \"a\" cons document anchor inner text \"b\" nil")
            } else {
                source.into()
            };
            with_input_route(
                native,
                &compiled,
                &input,
                "Article",
                |tree, profile, b, a| {
                    let registry = profile.registry();
                    let checked = tree
                        .tree()
                        .bundle
                        .validate_with_sources(registry, b, a)
                        .map_err(err)?;
                    let store = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                    let mut doc = nepl3_suite::adapters::sentence::literal::lower(
                        &checked,
                        &compiled.doc.package.schema,
                        Category::Article,
                        registry,
                        b,
                        &mut codec,
                    )
                    .map_err(err)?;
                    if scenario == 2 || scenario == 3 {
                        let node = doc
                            .value
                            .nodes
                            .iter()
                            .enumerate()
                            .find_map(|(index, node)| match &node.kind {
                                DocKind::Sentence { inlines }
                                    if scenario == 2
                                        && inlines.iter().any(|item| {
                                            matches!(
                                                doc.value.nodes[item.0 as usize].kind,
                                                DocKind::Reference { .. }
                                            )
                                        }) =>
                                {
                                    Some(index)
                                }
                                DocKind::SentenceGuest { .. } if scenario == 3 => Some(index),
                                _ => None,
                            })
                            .ok_or("repeat node")?;
                        let items = doc
                            .value
                            .nodes
                            .iter_mut()
                            .find_map(|node| match &mut node.kind {
                                DocKind::Paragraph { items } => Some(items),
                                _ => None,
                            })
                            .ok_or("paragraph")?;
                        items.push(FlowRef(node as u64));
                    }
                    let mut host = SentenceAnnotationRenderer {
                        registry,
                        surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                        math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    let selected = article::collect_structural(&doc, &mut host, b).map_err(err)?;
                    let adopted: Vec<_> = (0..selected.sentences().len())
                        .map(SentenceOccurrenceId)
                        .collect();
                    let result = namespace::resolve(&selected, &adopted, &mut host, b);
                    if scenario == 1 || scenario == 3 || scenario == 5 {
                        let Err(namespace::Error::Label(diagnostic)) = result else {
                            return Err(
                                "duplicate namespace must return a source diagnostic".into()
                            );
                        };
                        assert!(diagnostic.diagnostic.primary.is_some());
                        assert!(!diagnostic.diagnostic.related.is_empty());
                        if scenario == 3 {
                            assert!(matches!(
                                diagnostic.primary.origin,
                                MemberOrigin::SentenceInline {
                                    sentence: SentenceOccurrenceId(2),
                                    ..
                                }
                            ));
                            assert!(matches!(
                                diagnostic.related.as_ref().map(|s| s.origin),
                                Some(MemberOrigin::SentenceInline {
                                    sentence: SentenceOccurrenceId(0),
                                    ..
                                })
                            ));
                        }
                        if scenario == 5 {
                            assert!(matches!(
                                diagnostic.primary.origin,
                                MemberOrigin::SentenceInline {
                                    sentence: SentenceOccurrenceId(0),
                                    inline_occurrence: 1,
                                    ..
                                }
                            ));
                            assert!(matches!(
                                diagnostic.related.as_ref().map(|s| s.origin),
                                Some(MemberOrigin::SentenceInline {
                                    sentence: SentenceOccurrenceId(0),
                                    inline_occurrence: 0,
                                    ..
                                })
                            ));
                        }
                        return Ok(());
                    }
                    if scenario == 4 {
                        assert!(matches!(
                            result,
                            Err(namespace::Error::Document {
                                context: namespace::DocumentContext::Member(
                                    MemberOrigin::SentenceInline {
                                        sentence: SentenceOccurrenceId(0),
                                        ..
                                    }
                                ),
                                ..
                            })
                        ));
                        return Ok(());
                    }
                    let resolved = result.map_err(err)?;
                    assert!(core::ptr::eq(resolved.input(), &selected));
                    assert!(core::ptr::eq(
                        resolved.document(MemberId(0)).ok_or("root")?,
                        &doc
                    ));
                    assert_eq!(resolved.adopted(), adopted);
                    assert_eq!(
                        resolved
                            .definitions()
                            .iter()
                            .map(|s| s.member)
                            .collect::<Vec<_>>(),
                        vec![MemberId(1), MemberId(0)],
                        "guest declaration precedes the later outer declaration"
                    );
                    assert_eq!(
                        resolved
                            .references()
                            .iter()
                            .map(|s| (s.reference.member, s.target))
                            .collect::<Vec<_>>(),
                        vec![(MemberId(0), DocLabelId(0)), (MemberId(2), DocLabelId(1))]
                    );
                    assert!(matches!(
                        resolved.origin(MemberId(1)),
                        Some(MemberOrigin::SentenceInline {
                            sentence: SentenceOccurrenceId(0),
                            ..
                        })
                    ));
                    assert_eq!(resolved.origin(MemberId(0)), Some(MemberOrigin::Article));
                    assert!(resolved.document(MemberId(99)).is_none());
                    for adopted in [vec![], vec![SentenceOccurrenceId(1)]] {
                        assert!(
                            matches!(
                                namespace::resolve(&selected, &adopted, &mut host, &mut budget()),
                                Err(namespace::Error::Label(_))
                            ),
                            "unadopted definition must not satisfy an outer reference"
                        );
                    }
                    for adopted in [
                        vec![SentenceOccurrenceId(1), SentenceOccurrenceId(0)],
                        vec![SentenceOccurrenceId(0), SentenceOccurrenceId(0)],
                    ] {
                        assert!(matches!(
                            namespace::resolve(&selected, &adopted, &mut host, &mut budget()),
                            Err(namespace::Error::AdoptionOrder { .. })
                        ));
                    }
                    assert!(matches!(
                        namespace::resolve(
                            &selected,
                            &[SentenceOccurrenceId(99)],
                            &mut host,
                            &mut budget()
                        ),
                        Err(namespace::Error::AdoptionIndex { .. })
                    ));
                    let mut stopped = budget();
                    stopped.cancel();
                    assert!(matches!(
                        namespace::resolve(&selected, &adopted, &mut host, &mut stopped),
                        Err(namespace::Error::Stopped(StopReason::Cancelled))
                    ));
                    let original_surface = host.surface;
                    host.surface = &compiled.doc.package.schema;
                    assert!(matches!(
                        namespace::resolve(&selected, &adopted, &mut host, &mut budget()),
                        Err(namespace::Error::SentenceSelection { .. })
                    ));
                    host.surface = original_surface;
                    let mut low = budget().limits();
                    low.work = 1;
                    assert!(matches!(
                        namespace::resolve(&selected, &adopted, &mut host, &mut Budget::new(low)),
                        Err(namespace::Error::Stopped(StopReason::WorkLimit))
                    ));
                    Ok(())
                },
            )?;
        }
    }
    Ok(())
}

#[test]
fn article_adoption_does_not_import_math_annotation_definitions() -> Result<(), String> {
    let compiled = nepl3_tools::doc::source::compiled_with_sentence_forms(&[
        nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "document",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "Article Math scope isolation",
        },
    ])?;
    let source = r#"article en sentence nil body cons paragraph
cons sentence cons ref secret text "outer" nil
cons Sentence sentence cons document math Math label 7 Sentence sentence cons document anchor secret text "inner" nil nil
nil nil"#;
    for has_reference in [false, true] {
        let input = if has_reference {
            source.into()
        } else {
            source.replace("cons sentence cons ref secret text \"outer\" nil", "")
        };
        with_input_route(true, &compiled, &input, "Article", |tree, profile, b, a| {
            let registry = profile.registry();
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(registry, b, a)
                .map_err(err)?;
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
            let doc = nepl3_suite::adapters::sentence::literal::lower(
                &checked,
                &compiled.doc.package.schema,
                Category::Article,
                registry,
                b,
                &mut codec,
            )
            .map_err(err)?;
            let mut host = SentenceAnnotationRenderer {
                registry,
                surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            };
            let selected = article::collect_structural(&doc, &mut host, b).map_err(err)?;
            assert_eq!(selected.sentences().len(), 1);
            let result = namespace::resolve(&selected, &[SentenceOccurrenceId(0)], &mut host, b);
            if has_reference {
                let Err(namespace::Error::Label(failure)) = result else {
                    return Err("Math-local definition escaped its namespace".into());
                };
                assert_eq!(failure.primary.origin, MemberOrigin::Article);
                assert!(failure.related.is_none());
            } else {
                let result = result.map_err(err)?;
                assert!(result.definitions().is_empty());
                assert!(result.references().is_empty());
            }
            Ok(())
        })?;
    }
    Ok(())
}
