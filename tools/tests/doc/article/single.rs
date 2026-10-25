use super::*;
use nepl3_doc_core::{check::Category, model::*};
use nepl3_doc_html::{ParallelMode, RenderOptions};
use nepl3_markup::html::*;
use nepl3_tools::doc::annotations::{
    self, SentenceAnnotationRenderer,
    article::{self, render},
};

#[test]
fn single_article_keeps_structural_owners_and_rejects_hidden_guest_targets() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en Sentence "Title" body cons paragraph
cons parallel cons variant en Sentence sentence cons document anchor english text "English" cons math frac 1 2 nil cons variant ja Sentence sentence cons document anchor japanese text "Japanese" nil nil
cons code Math frac 3 4 cons Sentence sentence cons document ref english text "tail" nil nil nil"#;
    for (language, fallbacks, tail_target, expected_ids, success) in [
        ("en", vec![], "english", vec![0, 1, 3], true),
        ("fr", vec!["en".into()], "english", vec![0, 1, 3], true),
        ("ja", vec![], "japanese", vec![0, 2, 3], true),
        ("ja", vec![], "english", vec![], false),
    ] {
        let source = source.replace("ref english", &format!("ref {tail_target}"));
        with_input_route(
            true,
            &compiled,
            &source,
            "Article",
            |tree, profile, b, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                let root = nepl3_suite::adapters::sentence::literal::lower(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let mut host = SentenceAnnotationRenderer {
                    registry: profile.registry(),
                    surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                    math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                let selection = article::collect_structural(&root, &mut host, b).map_err(err)?;
                let output = render::render(
                    &selection,
                    &RenderOptions {
                        parallel: ParallelMode::Single {
                            language: language.into(),
                            fallbacks,
                        },
                    },
                    &mut host,
                    &mut |view, _, _, _| {
                        assert_eq!(view.kind, EmbedKind::Code);
                        Ok::<_, String>(HtmlRequest {
                            slot: HtmlSlot::Phrasing,
                            policy: HtmlPolicy { classes: vec![] },
                            fragment: HtmlFragment {
                                root: 0,
                                nodes: vec![HtmlNode::Text {
                                    text: "CODE".into(),
                                }],
                            },
                        })
                    },
                    b,
                );
                if !success {
                    assert!(matches!(
                        output,
                        Err(render::Error::Render(nepl3_doc_html::RenderError::Markup(
                            HtmlError::MissingFragment { .. }
                        )))
                    ));
                    return Ok(());
                }
                let output = output.map_err(err)?;
                assert_eq!(
                    output.resolution.adopted().len(),
                    4,
                    "namespace adoption remains all occurrences"
                );
                assert_eq!(
                    output
                        .sentences
                        .iter()
                        .map(|r| r.occurrence.0)
                        .collect::<Vec<_>>(),
                    expected_ids
                );
                assert_eq!(
                    output
                        .sentences
                        .iter()
                        .map(|r| r.placement)
                        .collect::<Vec<_>>(),
                    vec![0, 1, 3]
                );
                for record in &output.sentences {
                    assert_eq!(
                        selection.sentences()[record.occurrence.0].owner,
                        record.owner
                    );
                    for foreign in &record.foreign {
                        if let annotations::ForeignRecord::Document(doc) = foreign {
                            for origin in &doc.origins {
                                if let DocKind::Text { text } =
                                    &doc.document.value.nodes[origin.node as usize].kind
                                {
                                    assert!(
                                        matches!(&output.article.fragment.markup.fragment.nodes[origin.element as usize],HtmlNode::Text{text:actual} if actual==text)
                                    );
                                }
                            }
                        }
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn single_article_handles_empty_and_shared_owners_without_changing_adoption() -> Result<(), String>
{
    let compiled = compiled()?;
    let source = r#"article en Sentence "Title" body cons paragraph cons parallel cons variant en Sentence sentence cons document anchor hidden text "Hidden" nil cons variant ja Sentence "" nil cons Sentence "Tail" nil nil"#;
    for shared in [false, true] {
        let source = if shared {
            source.replace(
                "Sentence \"Title\"",
                "Sentence sentence cons document text \"Title\" nil",
            )
        } else {
            source.into()
        };
        with_input_route(
            true,
            &compiled,
            &source,
            "Article",
            |tree, profile, b, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                let mut root = nepl3_suite::adapters::sentence::literal::lower(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                if shared {
                    let DocRoot::Article(article) = root.value.root else {
                        return Err("root".into());
                    };
                    let DocKind::Article { title, .. } = root.value.nodes[article.0 as usize].kind
                    else {
                        return Err("title".into());
                    };
                    let old = root
                        .value
                        .nodes
                        .iter_mut()
                        .find_map(|node| {
                            if let DocKind::Variant { language, sentence } = &mut node.kind {
                                if language == "ja" {
                                    let old = *sentence;
                                    *sentence = title;
                                    Some(old)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .ok_or("ja")?;
                    let items = root
                        .value
                        .nodes
                        .iter_mut()
                        .find_map(|node| {
                            if let DocKind::Paragraph { items } = &mut node.kind {
                                Some(items)
                            } else {
                                None
                            }
                        })
                        .ok_or("paragraph")?;
                    items.push(FlowRef(old.0));
                }
                let mut host = SentenceAnnotationRenderer {
                    registry: profile.registry(),
                    surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                    math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                let selection = article::collect_structural(&root, &mut host, b).map_err(err)?;
                for language in ["en", "ja"] {
                    let result = render::render(
                        &selection,
                        &RenderOptions {
                            parallel: ParallelMode::Single {
                                language: language.into(),
                                fallbacks: vec![],
                            },
                        },
                        &mut host,
                        &mut |_, _, _, _| Err::<HtmlRequest, String>("unexpected".into()),
                        b,
                    )
                    .map_err(err)?;
                    let mut expected = vec![0, if language == "en" { 1 } else { 2 }, 3];
                    if shared {
                        expected.push(4);
                    }
                    assert_eq!(
                        result
                            .sentences
                            .iter()
                            .map(|r| r.occurrence.0)
                            .collect::<Vec<_>>(),
                        expected
                    );
                    assert_eq!(
                        result.resolution.adopted().len(),
                        if shared { 5 } else { 4 }
                    );
                    if shared && language == "ja" {
                        assert_eq!(result.sentences[0].owner, result.sentences[1].owner);
                        assert_ne!(
                            result.sentences[0].occurrence,
                            result.sentences[1].occurrence
                        );
                    }
                    let markup = result.article.fragment.markup;
                    let html = serialize(
                        &validate(&markup.fragment, markup.slot, &markup.policy, b).map_err(err)?,
                        b,
                    )
                    .map_err(err)?;
                    assert_eq!(html.contains("Hidden"), language == "en");
                    assert_eq!(
                        html.matches("Title").count(),
                        if shared && language == "ja" { 2 } else { 1 }
                    );
                }
                Ok(())
            },
        )?;
    }
    let duplicate = source.replace(
        "Sentence \"Title\"",
        "Sentence sentence cons document anchor hidden text \"Title\" nil",
    );
    with_input_route(
        true,
        &compiled,
        &duplicate,
        "Article",
        |tree, profile, b, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let root = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let mut host = SentenceAnnotationRenderer {
                registry: profile.registry(),
                surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            };
            let selection = article::collect_structural(&root, &mut host, b).map_err(err)?;
            let result = render::render(
                &selection,
                &RenderOptions {
                    parallel: ParallelMode::Single {
                        language: "ja".into(),
                        fallbacks: vec![],
                    },
                },
                &mut host,
                &mut |_, _, _, _| Err::<HtmlRequest, String>("unexpected".into()),
                b,
            );
            assert!(
                matches!(result, Err(render::Error::Resolve(_))),
                "hidden definitions still participate in explicit namespace adoption"
            );
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn single_article_trailing_hidden_members_and_shared_budget_remain_checked() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en Sentence "Title" body cons paragraph cons parallel cons variant en Sentence "English" cons variant ja Sentence sentence cons document anchor hidden text "Hidden" nil nil nil nil"#;
    with_input_route(true, &compiled, source, "Article", |tree, profile, b, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let root = nepl3_suite::adapters::sentence::literal::lower(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        let run = |budget: &mut Budget| -> Result<(), String> {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let mut host = SentenceAnnotationRenderer {
                registry: profile.registry(),
                surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            };
            let selection = article::collect_structural(&root, &mut host, budget).map_err(err)?;
            let output = render::render(
                &selection,
                &RenderOptions {
                    parallel: ParallelMode::Single {
                        language: "en".into(),
                        fallbacks: vec![],
                    },
                },
                &mut host,
                &mut |_, _, _, _| Err::<HtmlRequest, String>("unexpected".into()),
                budget,
            )
            .map_err(err)?;
            assert_eq!(output.resolution.adopted().len(), 3);
            assert_eq!(output.sentences.len(), 2);
            assert_eq!(output.sentences[1].occurrence.0, 1);
            Ok(())
        };
        let mut measured = budget();
        measured.charge(Resource::Work, 7).map_err(err)?;
        run(&mut measured)?;
        let mut limits = measured.limits();
        limits.work = measured.usage().work;
        let mut exact = Budget::new(limits);
        exact.charge(Resource::Work, 7).map_err(err)?;
        run(&mut exact)?;
        limits.work -= 1;
        let mut short = Budget::new(limits);
        short.charge(Resource::Work, 7).map_err(err)?;
        assert!(run(&mut short).is_err());
        assert_eq!(short.poll(), Err(StopReason::WorkLimit));
        let mut deeper = budget();
        deeper
            .with_depth_at_least(3, |b| Ok::<_, StopReason>(run(b)))
            .map_err(err)??;
        assert_eq!(deeper.current_depth(), 0);
        let mut depth_limits = measured.limits();
        depth_limits.depth = measured.usage().depth + 2;
        let mut shallow = Budget::new(depth_limits);
        assert!(
            shallow
                .with_depth_at_least(3, |b| Ok::<_, StopReason>(run(b)))
                .map_err(err)?
                .is_err()
        );
        assert_eq!(shallow.poll(), Err(StopReason::DepthLimit));
        let mut stopped = budget();
        stopped.cancel();
        assert!(run(&mut stopped).is_err());
        assert_eq!(stopped.poll(), Err(StopReason::Cancelled));
        Ok(())
    })
}

#[test]
fn planned_role_session_matches_legacy_asset_free_markup_and_placement() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en Sentence sentence cons document anchor title text "Title" nil body cons paragraph cons parallel cons variant en Sentence sentence cons document anchor english text "English" nil cons variant ja Sentence sentence cons document anchor japanese text "Japanese" nil nil cons code Math frac 1 2 cons Sentence "Tail" nil nil"#;
    for parallel in [
        ParallelMode::Rows,
        ParallelMode::Columns,
        ParallelMode::Single {
            language: "en".into(),
            fallbacks: vec![],
        },
        ParallelMode::Single {
            language: "fr".into(),
            fallbacks: vec!["ja".into()],
        },
    ] {
        let options = RenderOptions { parallel };
        with_input_route(true, &compiled, source, "Article", |tree, profile, b, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let root = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let mut host = SentenceAnnotationRenderer {
                registry: profile.registry(),
                surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            };
            let selection = article::collect_structural(&root, &mut host, b).map_err(err)?;
            let mut other =
                |_: &DocEmbedView<'_>, _: EmbedRef, _: &mut FoundationCodec<'_>, _: &mut Budget| {
                    Ok::<_, String>(HtmlRequest {
                        slot: HtmlSlot::Phrasing,
                        policy: HtmlPolicy { classes: vec![] },
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Text {
                                text: "CODE".into(),
                            }],
                        },
                    })
                };
            let legacy =
                render::render(&selection, &options, &mut host, &mut other, b).map_err(err)?;
            let plan = article::display::plan(&selection, &options, b).map_err(err)?;
            let planned = render::render_planned(&plan, &mut host, &mut other, b).map_err(err)?;
            assert_eq!(
                legacy.article.fragment.markup,
                planned.article.fragment.markup
            );
            assert_eq!(legacy.article.foreign, planned.article.foreign);
            assert_eq!(legacy.sentences.len(), planned.sentences.len());
            for (old, new) in legacy.sentences.iter().zip(&planned.sentences) {
                assert_eq!(
                    (old.occurrence, old.owner, old.placement),
                    (new.occurrence, new.owner, new.placement)
                );
                assert_eq!(old.origins, new.origins);
            }
            Ok(())
        })?;
    }
    Ok(())
}
