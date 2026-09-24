use super::*;
use crate::doc::export::pages::{composition, discovery};
use nepl3_core::budget::StopReason;

#[derive(Debug)]
struct Failure;
impl From<StopReason> for Failure {
    fn from(_: StopReason) -> Self {
        Self
    }
}

#[test]
fn discovered_rendering_checks_hidden_slots_and_stops_adapters() -> Result<(), String> {
    let compiled = compiled()?;
    let sources = [
        r#"article en sentence "Title" body cons paragraph cons parallel cons variant en sentence "Visible" cons variant ja sentence sentence cons link "javascript:alert(1)" text "Hidden" nil nil nil nil"#,
        r#"article en sentence "Title" body cons paragraph cons sentence sentence cons math add 1 2 cons math add 3 4 nil nil nil"#,
        r#"article en sentence "Title" body cons code Math add 1 2 cons code Math add 3 4 nil"#,
        r#"article en sentence "Title" body cons paragraph cons parallel cons variant en sentence "Visible" cons variant ja sentence sentence cons link "https://example.org/" text "Hidden" nil nil nil nil"#,
    ];
    for (case, source) in sources.into_iter().enumerate() {
        with_named_input(
            true,
            &compiled,
            source,
            "failure",
            "Article",
            |tree, profile, b, a| {
                let registry = profile.registry();
                let store = SourceStore::default();
                let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
                let input = tree
                    .tree()
                    .bundle
                    .validate_with_sources(registry, b, &mut SourceAdmission::default())
                    .map_err(err)?;
                let document = lower::document(
                    &input,
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let set = pages::PageSet {
                    pages: vec![pages::PageDocument {
                        registration: pages::PageRegistration {
                            id: "failure".into(),
                            source: "failure.nepld".into(),
                            route: "failure.html".into(),
                        },
                        document,
                    }],
                    files: vec![],
                };
                let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
                    kind: "Form:InlineMath",
                    guest_schema: &compiled.others[0].schema,
                    guest_category: "Expr",
                }];
                let found = discovery::collect(
                    &set.pages[0].document,
                    &compiled.others[3].schema,
                    &compiled.doc.package.schema,
                    &forms,
                    registry,
                    &mut codec,
                    b,
                )
                .map_err(err)?;
                let plan = discovery::namespace::inspect(
                    &found,
                    registry,
                    b,
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
                let refs = plan.member_refs(b).map_err(err)?;
                let checked = namespace::resolve(&refs, b).map_err(err)?;
                let namespaces = [&checked];
                let resolved =
                    scopes::resolve(&set, &namespaces, registry, &mut codec, b).map_err(err)?;
                let options = nepl3_doc_html::RenderOptions {
                    parallel: nepl3_doc_html::ParallelMode::Single {
                        language: "en".into(),
                        fallbacks: vec![],
                    },
                };
                let prepared = nepl3_doc_html::pages::namespace::prepare(&resolved, &options, b)
                    .map_err(err)?;
                for cancel in [false, true] {
                    let mut sentence_calls = 0;
                    let mut document_calls = 0;
                    let result = composition::render(
                        &plan,
                        &prepared,
                        0,
                        registry,
                        &mut |owner, slot, _, embed, b| {
                            sentence_calls += 1;
                            assert_eq!(owner.index(), 0);
                            assert_eq!(slot.0, 1);
                            assert_eq!(embed.0, 0);
                            if cancel {
                                b.cancel();
                            }
                            Err::<_, Failure>(Failure)
                        },
                        &mut |owner, _, _, b| {
                            document_calls += 1;
                            assert_eq!(owner.index(), 0);
                            if cancel {
                                b.cancel();
                            }
                            Err::<_, Failure>(Failure)
                        },
                        &mut budget(),
                        &mut SourceAdmission::default(),
                    );
                    if case == 3 {
                        let output = result.map_err(err)?;
                        assert_eq!((sentence_calls, document_calls), (0, 0));
                        let requests = [output.members()[0]
                            .document()
                            .output()
                            .fragment
                            .markup
                            .clone()];
                        let checked = nepl3_doc_html::pages::namespace::output::check(
                            &prepared,
                            &requests,
                            &mut budget(),
                        )
                        .map_err(err)?;
                        let html =
                            nepl3_markup::html::serialize_xhtml(&checked.pages()[0], &mut budget())
                                .map_err(err)?;
                        assert!(html.contains("Visible"));
                        assert!(!html.contains("Hidden"));
                        continue;
                    }
                    assert!(result.is_err());
                    match case {
                        0 => {
                            assert_eq!((sentence_calls, document_calls), (0, 0));
                            assert!(matches!(result, Err(composition::Error::Sentence {
                                slot: nepl3_doc_core::model::EmbedRef(2),
                                error: nepl3_suite::adapters::sentence::html::RenderFailure::Sentence(
                                    nepl3_suite::adapters::sentence::html::Error::Markup(
                                        nepl3_markup::html::HtmlError::Attribute { .. }
                                    )
                                ), ..
                            })));
                        }
                        1 => assert_eq!((sentence_calls, document_calls), (1, 0)),
                        2 => assert_eq!((sentence_calls, document_calls), (0, 1)),
                        _ => return Err("case".into()),
                    }
                    if cancel && case != 0 {
                        assert!(matches!(
                            result,
                            Err(composition::Error::Stopped(StopReason::Cancelled))
                        ));
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn discovered_rendering_preserves_recursive_shared_occurrences() -> Result<(), String> {
    let compiled = compiled()?;
    for (source, shared) in [
        (
            r#"article en sentence sentence cons doc anchor target text "Target" nil body cons paragraph cons sentence sentence cons doc anchor outer concat cons doc ref target text "Leaf" nil nil nil nil"#,
            false,
        ),
        (
            r#"article en sentence sentence cons doc anchor target text "Target" nil body cons paragraph cons sentence sentence cons doc ref target text "Leaf" nil nil nil"#,
            true,
        ),
    ] {
        with_named_input(
            true,
            &compiled,
            source,
            "recursive",
            "Article",
            |tree, profile, b, a| {
                let registry = profile.registry();
                let store = SourceStore::default();
                let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
                let input = tree
                    .tree()
                    .bundle
                    .validate_with_sources(registry, b, &mut SourceAdmission::default())
                    .map_err(err)?;
                let mut document = lower::document(
                    &input,
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                // Two display occurrences share a single paragraph and its nested
                // Sentence/Doc owners. References may repeat; the target occurs once.
                for node in document.value.nodes.iter_mut().filter(|_| shared) {
                    if let DocKind::Body { blocks } = &mut node.kind {
                        let first = *blocks.first().ok_or("paragraph")?;
                        blocks.push(first);
                    }
                }
                let set = pages::PageSet {
                    pages: vec![pages::PageDocument {
                        registration: pages::PageRegistration {
                            id: "recursive".into(),
                            source: "recursive.nepld".into(),
                            route: "recursive.html".into(),
                        },
                        document,
                    }],
                    files: vec![],
                };
                let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
                    kind: "Form:DocumentInline",
                    guest_schema: &compiled.doc.package.schema,
                    guest_category: "Inline",
                }];
                let found = discovery::collect(
                    &set.pages[0].document,
                    &compiled.others[3].schema,
                    &compiled.doc.package.schema,
                    &forms,
                    registry,
                    &mut codec,
                    b,
                )
                .map_err(err)?;
                let plan = discovery::namespace::inspect(
                    &found,
                    registry,
                    b,
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
                let refs = plan.member_refs(b).map_err(err)?;
                let checked = namespace::resolve(&refs, b).map_err(err)?;
                let namespaces = [&checked];
                let resolved =
                    scopes::resolve(&set, &namespaces, registry, &mut codec, b).map_err(err)?;
                let options = nepl3_doc_html::RenderOptions {
                    parallel: nepl3_doc_html::ParallelMode::Rows,
                };
                let prepared = nepl3_doc_html::pages::namespace::prepare(&resolved, &options, b)
                    .map_err(err)?;
                let output = composition::render(
                    &plan,
                    &prepared,
                    0,
                    registry,
                    &mut |_, _, _, _, _| Err::<_, Failure>(Failure),
                    &mut |_, _, _, _| Err::<_, Failure>(Failure),
                    b,
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
                assert_eq!(output.members().len(), if shared { 3 } else { 4 });
                assert_eq!(plan.occurrences().len(), 4);
                let root = output.members()[0].document().output();
                assert_eq!(root.foreign.len(), if shared { 3 } else { 2 });
                if shared {
                    assert_eq!(root.foreign[1].embed, root.foreign[2].embed);
                    assert_ne!(root.foreign[1].first_element, root.foreign[2].first_element);
                }
                assert_eq!(root.fragment.markup.fragment.nodes.iter().filter(|node| matches!(node, nepl3_markup::html::HtmlNode::Text { text } if text == "Leaf")).count(), if shared { 2 } else { 1 });
                let requests = [root.fragment.markup.clone()];
                let checked =
                    nepl3_doc_html::pages::namespace::output::check(&prepared, &requests, b)
                        .map_err(err)?;
                let html =
                    nepl3_markup::html::serialize_xhtml(&checked.pages()[0], b).map_err(err)?;
                assert_eq!(html.matches("Leaf").count(), if shared { 2 } else { 1 });
                assert_eq!(html.matches("id=\"n-746172676574\"").count(), 1);
                Ok(())
            },
        )?;
    }
    Ok(())
}
