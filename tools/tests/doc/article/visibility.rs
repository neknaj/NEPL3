use super::*;
use nepl3_doc_core::{check::Category, labels::namespace as labels, model::*};
use nepl3_doc_html::{ParallelMode, RenderOptions, namespace::display, visibility};
use nepl3_markup::html::*;
fn lower(compiled: &Compiled, source: &str) -> Result<DocumentSyntax, String> {
    with_input_route(true, compiled, source, "Article", |tree, profile, b, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        nepl3_suite::adapters::sentence::literal::lower(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)
    })
}
const SOURCE: &str = r#"article en Sentence "Title" body cons paragraph cons parallel cons variant en Sentence "English" cons variant ja Sentence "日本語" nil cons Sentence "Tail" nil nil"#;
#[test]
fn selected_sentence_inventory_matches_actual_callbacks_and_shared_occurrences()
-> Result<(), String> {
    let compiled = compiled()?;
    let mut doc = lower(&compiled, SOURCE)?;
    let DocRoot::Article(root) = doc.value.root else {
        return Err("article".into());
    };
    let DocKind::Article { title, .. } = doc.value.nodes[root.0 as usize].kind else {
        return Err("article node".into());
    };
    let paragraph = doc
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
    paragraph.push(FlowRef(title.0));
    let shape = doc.value.validate_shape(&mut budget()).map_err(err)?;
    let structural = shape.structural_occurrences(&mut budget()).map_err(err)?;
    let registry = &compiled.doc.registry;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let member =
        labels::inspect(&doc, registry, &mut budget(), codec.source_admission()).map_err(err)?;
    let members = [&member];
    let namespace = labels::resolve(&members, &mut budget()).map_err(err)?;
    for (mode, count) in [
        (ParallelMode::Rows, 5),
        (ParallelMode::Columns, 5),
        (
            ParallelMode::Single {
                language: "EN".into(),
                fallbacks: vec![],
            },
            4,
        ),
        (
            ParallelMode::Single {
                language: "fr".into(),
                fallbacks: vec!["ja".into()],
            },
            4,
        ),
    ] {
        let options = RenderOptions { parallel: mode };
        let selected =
            visibility::sentence_occurrences(&shape, &options, &mut budget()).map_err(err)?;
        assert_eq!(selected.len(), count);
        assert_eq!(
            selected.first().map(|x| x.syntax),
            selected.last().map(|x| x.syntax)
        );
        assert_ne!(
            selected.first().map(|x| x.structural_index),
            selected.last().map(|x| x.structural_index)
        );
        for entry in &selected {
            let original = structural[entry.structural_index as usize];
            assert_eq!((entry.node, entry.depth), (original.node, original.depth));
        }
        let prepared = display::prepare(&namespace, &options, registry, &mut codec, &mut budget())
            .map_err(err)?;
        let mut seen = Vec::new();
        display::render_article(
            &prepared,
            labels::MemberId(0),
            &mut |_, index, _| {
                seen.push(index);
                Ok::<_, String>(HtmlRequest {
                    slot: HtmlSlot::Phrasing,
                    policy: HtmlPolicy { classes: vec![] },
                    fragment: HtmlFragment {
                        root: 0,
                        nodes: vec![HtmlNode::Text {
                            text: "slot".into(),
                        }],
                    },
                })
            },
            &mut budget(),
        )
        .map_err(err)?;
        assert_eq!(
            seen,
            selected
                .iter()
                .map(|entry| entry.syntax)
                .collect::<Vec<_>>()
        );
    }
    Ok(())
}
#[test]
fn image_alt_is_not_a_display_callback_but_caption_is() -> Result<(), String> {
    let compiled = compiled()?;
    let doc = lower(
        &compiled,
        r#"article en Sentence "Title" body cons image asset "logo" none Sentence "Alt" some Sentence "Caption" cons paragraph cons sentence cons image asset "logo" none Sentence "Inline alt" nil nil nil"#,
    )?;
    let shape = doc.value.validate_shape(&mut budget()).map_err(err)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let selected =
        visibility::sentence_occurrences(&shape, &options, &mut budget()).map_err(err)?;
    assert_eq!(selected.len(), 2);
    let DocRoot::Article(root) = doc.value.root else {
        return Err("root".into());
    };
    let DocKind::Article { title, .. } = doc.value.nodes[root.0 as usize].kind else {
        return Err("title".into());
    };
    let caption = doc
        .value
        .nodes
        .iter()
        .find_map(|node| {
            if let DocKind::Image {
                caption: Some(caption),
                ..
            } = node.kind
            {
                Some(caption)
            } else {
                None
            }
        })
        .ok_or("caption")?;
    assert_eq!(
        selected.iter().map(|entry| entry.node).collect::<Vec<_>>(),
        vec![title.0, caption.0]
    );

    let all = shape.structural_occurrences(&mut budget()).map_err(err)?;
    assert_eq!(
        all.iter()
            .filter(|entry| matches!(
                doc.value.nodes[entry.node as usize].kind,
                DocKind::SentenceGuest { .. }
            ))
            .count(),
        4
    );
    let mut admission = SourceAdmission::default();
    let store = SourceStore::default();
    let mut codec =
        FoundationCodec::new(&compiled.doc.registry, &store, &mut admission).map_err(err)?;
    let member = labels::inspect(
        &doc,
        &compiled.doc.registry,
        &mut budget(),
        codec.source_admission(),
    )
    .map_err(err)?;
    let members = [&member];
    let namespace = labels::resolve(&members, &mut budget()).map_err(err)?;
    assert!(
        display::prepare(
            &namespace,
            &options,
            &compiled.doc.registry,
            &mut codec,
            &mut budget()
        )
        .is_err(),
        "inventory does not resolve the asset or alt projection"
    );
    Ok(())
}
#[test]
fn display_inventory_preserves_option_failures_and_cumulative_budget() -> Result<(), String> {
    let compiled = compiled()?;
    let doc = lower(&compiled, SOURCE)?;
    let shape = doc.value.validate_shape(&mut budget()).map_err(err)?;
    for (language, fallbacks) in [("bad_tag", vec![]), ("en", vec!["EN".into()])] {
        assert!(matches!(
            visibility::sentence_occurrences(
                &shape,
                &RenderOptions {
                    parallel: ParallelMode::Single {
                        language: language.into(),
                        fallbacks
                    }
                },
                &mut budget()
            ),
            Err(visibility::Error::Language)
        ));
    }
    assert!(matches!(
        visibility::sentence_occurrences(
            &shape,
            &RenderOptions {
                parallel: ParallelMode::Single {
                    language: "fr".into(),
                    fallbacks: vec![]
                }
            },
            &mut budget()
        ),
        Err(visibility::Error::MissingVariant { .. })
    ));
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let mut measured = budget();
    measured.charge(Resource::Work, 7).map_err(err)?;
    let expected =
        visibility::sentence_occurrences(&shape, &options, &mut measured).map_err(err)?;
    let mut limits = measured.limits();
    limits.work = measured.usage().work;
    let mut exact = Budget::new(limits);
    exact.charge(Resource::Work, 7).map_err(err)?;
    assert_eq!(
        visibility::sentence_occurrences(&shape, &options, &mut exact).map_err(err)?,
        expected
    );
    let mut depth_limits = budget().limits();
    depth_limits.depth = measured.usage().depth + 3;
    let mut deeper = Budget::new(depth_limits);
    assert_eq!(
        deeper
            .with_depth_at_least(3, |b| visibility::sentence_occurrences(&shape, &options, b))
            .map_err(err)?,
        expected
    );
    assert_eq!(deeper.current_depth(), 0);
    depth_limits.depth -= 1;
    let mut shallow = Budget::new(depth_limits);
    assert!(matches!(
        shallow.with_depth_at_least(3, |b| visibility::sentence_occurrences(&shape, &options, b)),
        Err(visibility::Error::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(shallow.current_depth(), 0);
    let mut allocation_limits = budget().limits();
    allocation_limits.allocation_units = measured.usage().allocation_units;
    assert_eq!(
        visibility::sentence_occurrences(&shape, &options, &mut Budget::new(allocation_limits))
            .map_err(err)?,
        expected
    );
    allocation_limits.allocation_units -= 1;
    assert!(matches!(
        visibility::sentence_occurrences(&shape, &options, &mut Budget::new(allocation_limits)),
        Err(visibility::Error::Stopped(StopReason::AllocationLimit))
    ));
    limits.work -= 1;
    let mut short = Budget::new(limits);
    short.charge(Resource::Work, 7).map_err(err)?;
    assert!(matches!(
        visibility::sentence_occurrences(&shape, &options, &mut short),
        Err(visibility::Error::Stopped(StopReason::WorkLimit))
    ));
    let mut stopped = budget();
    stopped.cancel();
    assert!(matches!(
        visibility::sentence_occurrences(&shape, &options, &mut stopped),
        Err(visibility::Error::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}

const ROLE_SOURCE: &str = r#"article en Sentence "Title" body cons image asset "logo" none Sentence "Alt" some Sentence "Caption" cons paragraph cons parallel cons variant en Sentence "English" cons variant ja Sentence "Japanese" nil cons sentence cons image asset "logo" none Sentence "Inline alt" nil cons Sentence "Tail" nil nil"#;

fn with_role_selection(
    source: &str,
    action: impl FnOnce(
        &nepl3_tools::doc::annotations::article::Selection<'_>,
        &mut Budget,
    ) -> Result<(), String>,
) -> Result<(), String> {
    use nepl3_tools::doc::annotations::{SentenceAnnotationRenderer, article};
    let compiled = compiled()?;
    let doc = lower(&compiled, source)?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&compiled.doc.registry, &store, &mut admission).map_err(err)?;
    let mut b = budget();
    let selection = article::collect_structural(
        &doc,
        &mut SentenceAnnotationRenderer {
            registry: &compiled.doc.registry,
            surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
            math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
            doc_surface: Some(&compiled.doc.package.schema),
            codec: &mut codec,
        },
        &mut b,
    )
    .map_err(err)?;
    action(&selection, &mut b)
}
#[test]
fn sentence_role_plan_keeps_original_ids_and_separates_adoption_from_insertion()
-> Result<(), String> {
    use nepl3_tools::doc::annotations::article::display;
    with_role_selection(ROLE_SOURCE, |selection, b| {
        assert_eq!(selection.sentences().len(), 7);
        assert_eq!(selection.owners().len(), 7);
        let before = selection.structure().to_vec();
        let source_bytes = b.usage().source_bytes;
        let owner_addresses: Vec<_> = selection
            .owners()
            .iter()
            .map(|owner| owner.syntax() as *const _)
            .collect();
        for (mode, ids) in [
            (ParallelMode::Rows, vec![0, 2, 3, 4, 6]),
            (ParallelMode::Columns, vec![0, 2, 3, 4, 6]),
            (
                ParallelMode::Single {
                    language: "EN".into(),
                    fallbacks: vec![],
                },
                vec![0, 2, 3, 6],
            ),
            (
                ParallelMode::Single {
                    language: "fr".into(),
                    fallbacks: vec!["ja".into()],
                },
                vec![0, 2, 4, 6],
            ),
        ] {
            let options = RenderOptions { parallel: mode };
            let plan = display::plan(selection, &options, b).map_err(err)?;
            assert!(std::ptr::eq(plan.selection(), selection));
            assert!(std::ptr::eq(plan.options(), &options));
            assert_eq!(
                plan.adoption().iter().map(|id| id.0).collect::<Vec<_>>(),
                vec![0, 2, 3, 4, 6]
            );
            assert_eq!(
                plan.insertion().iter().map(|id| id.0).collect::<Vec<_>>(),
                ids
            );
            assert_eq!(plan.selection().structure(), before);
            assert_eq!(plan.selection().owners().len(), 7);
            assert_eq!(b.usage().source_bytes, source_bytes);
            assert_eq!(
                plan.selection()
                    .owners()
                    .iter()
                    .map(|owner| owner.syntax() as *const _)
                    .collect::<Vec<_>>(),
                owner_addresses
            );
        }
        for (case, source_options) in [
            ParallelMode::Single {
                language: "bad_tag".into(),
                fallbacks: vec![],
            },
            ParallelMode::Single {
                language: "en".into(),
                fallbacks: vec!["EN".into()],
            },
            ParallelMode::Single {
                language: "fr".into(),
                fallbacks: vec![],
            },
        ]
        .into_iter()
        .enumerate()
        {
            let options = RenderOptions {
                parallel: source_options,
            };
            let result = display::plan(selection, &options, b);
            if case < 2 {
                assert!(matches!(
                    result,
                    Err(display::Error::Visibility(visibility::Error::Language))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(display::Error::Visibility(
                        visibility::Error::MissingVariant { .. }
                    ))
                ));
            }
        }
        Ok(())
    })
}
#[test]
fn sentence_role_plan_alt_only_is_empty_but_owners_remain() -> Result<(), String> {
    use nepl3_tools::doc::annotations::article::display;
    with_role_selection(
        r#"article en "Title" body cons image asset "logo" none Sentence "Alt" none cons paragraph cons sentence cons image asset "logo" none Sentence "Inline alt" nil nil nil"#,
        |selection, b| {
            assert_eq!(selection.sentences().len(), 2);
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let plan = display::plan(selection, &options, b).map_err(err)?;
            assert!(plan.adoption().is_empty() && plan.insertion().is_empty());
            assert_eq!(plan.selection().owners().len(), 2);
            Ok(())
        },
    )
}
#[test]
fn sentence_role_plan_resources_are_cumulative_and_sticky() -> Result<(), String> {
    use nepl3_tools::doc::annotations::article::display;
    with_role_selection(ROLE_SOURCE, |selection, _| {
        let options = RenderOptions {
            parallel: ParallelMode::Single {
                language: "en".into(),
                fallbacks: vec![],
            },
        };
        let run = |b: &mut Budget| -> Result<(), display::Error> {
            b.charge(Resource::Work, 13)?;
            b.charge(Resource::AllocationUnits, 17)?;
            b.with_depth_at_least(5, |b| display::plan(selection, &options, b))
                .map(|_| ())
        };
        let mut baseline = budget();
        run(&mut baseline).map_err(err)?;
        let usage = baseline.usage();
        assert_eq!(baseline.current_depth(), 0);
        for resource in 0..3 {
            for short in [false, true] {
                let mut limits = budget().limits();
                let less = u64::from(short);
                match resource {
                    0 => limits.work = usage.work - less,
                    1 => limits.allocation_units = usage.allocation_units - less,
                    _ => limits.depth = usage.depth - less,
                }
                let mut b = Budget::new(limits);
                let result = run(&mut b);
                if short {
                    let expected = [
                        StopReason::WorkLimit,
                        StopReason::AllocationLimit,
                        StopReason::DepthLimit,
                    ][resource];
                    assert!(
                        matches!(result, Err(display::Error::Stopped(reason)) if reason == expected)
                    );
                    assert_eq!(b.poll(), Err(expected));
                    assert!(
                        matches!(display::plan(selection, &options, &mut b), Err(display::Error::Stopped(reason)) if reason == expected)
                    );
                } else {
                    result.map_err(err)?;
                }
                assert_eq!(b.current_depth(), 0);
            }
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            display::plan(selection, &options, &mut cancelled),
            Err(display::Error::Stopped(StopReason::Cancelled))
        ));
        Ok(())
    })
}

#[test]
fn sentence_role_plan_shared_owner_is_filtered_by_occurrence_not_embed() -> Result<(), String> {
    use nepl3_tools::doc::annotations::{SentenceAnnotationRenderer, article};
    let compiled = compiled()?;
    let mut doc = lower(&compiled, ROLE_SOURCE)?;
    let DocRoot::Article(root) = doc.value.root else {
        return Err("root".into());
    };
    let DocKind::Article { title, .. } = doc.value.nodes[root.0 as usize].kind else {
        return Err("article".into());
    };
    let old_alts: Vec<_> = doc
        .value
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            DocKind::Image { alt, .. } | DocKind::InlineImage { alt, .. } => Some(alt),
            _ => None,
        })
        .collect();
    let mut caption = None;
    for node in &mut doc.value.nodes {
        if let DocKind::Image {
            alt,
            caption: image_caption,
            ..
        } = &mut node.kind
        {
            *alt = title;
            caption = *image_caption;
        }
    }
    let caption = caption.ok_or("caption")?;
    for node in &mut doc.value.nodes {
        if let DocKind::InlineImage { alt, .. } = &mut node.kind {
            *alt = caption;
        }
        if let DocKind::Paragraph { items } = &mut node.kind {
            items.push(FlowRef(title.0));
            // Keep the original alt-only nodes and embeds reachable; the same
            // immutable shape must not be made valid by dropping source owners.
            items.extend(old_alts.iter().map(|alt| FlowRef(alt.0)));
        }
    }
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&compiled.doc.registry, &store, &mut admission).map_err(err)?;
    let mut b = budget();
    let selection = article::collect_structural(
        &doc,
        &mut SentenceAnnotationRenderer {
            registry: &compiled.doc.registry,
            surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
            math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
            doc_surface: Some(&compiled.doc.package.schema),
            codec: &mut codec,
        },
        &mut b,
    )
    .map_err(err)?;
    assert_eq!(selection.sentences().len(), 10);
    assert_eq!(selection.owners().len(), 7);
    assert_eq!(
        selection.sentences()[0].owner,
        selection.sentences()[1].owner
    );
    assert_eq!(
        selection.sentences()[0].owner,
        selection.sentences()[7].owner
    );
    assert_eq!(
        selection.sentences()[2].owner,
        selection.sentences()[5].owner
    );
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let plan = article::display::plan(&selection, &options, &mut b).map_err(err)?;
    assert_eq!(
        plan.adoption().iter().map(|id| id.0).collect::<Vec<_>>(),
        vec![0, 2, 3, 4, 6, 7, 8, 9]
    );
    assert_eq!(plan.insertion(), plan.adoption());
    Ok(())
}

#[test]
fn sentence_role_plan_alt_labels_do_not_enter_display_namespace() -> Result<(), String> {
    use nepl3_tools::doc::annotations::{SentenceAnnotationRenderer, article};
    let compiled = compiled()?;
    for (source, succeeds) in [
        (
            r#"article en "Title" body cons image asset "logo" none Sentence sentence cons document anchor target text "alt" nil none cons paragraph cons Sentence sentence cons document ref target text "body" nil nil nil"#,
            false,
        ),
        (
            r#"article en "Title" body cons image asset "logo" none Sentence sentence cons document anchor target text "alt" nil some Sentence sentence cons document anchor target text "caption" nil nil"#,
            true,
        ),
        (
            r#"article en "Title" body cons image asset "logo" none Sentence "alt" none cons paragraph cons parallel cons variant en Sentence sentence cons document anchor target text "en" nil cons variant ja Sentence sentence cons document anchor target text "ja" nil nil nil nil"#,
            false,
        ),
    ] {
        let doc = lower(&compiled, source)?;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&compiled.doc.registry, &store, &mut admission).map_err(err)?;
        let mut b = budget();
        let mut host = SentenceAnnotationRenderer {
            registry: &compiled.doc.registry,
            surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
            math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
            doc_surface: Some(&compiled.doc.package.schema),
            codec: &mut codec,
        };
        let selection = article::collect_structural(&doc, &mut host, &mut b).map_err(err)?;
        let options = RenderOptions {
            parallel: ParallelMode::Single {
                language: "en".into(),
                fallbacks: vec![],
            },
        };
        let plan = article::display::plan(&selection, &options, &mut b).map_err(err)?;
        let result =
            article::namespace::resolve(plan.selection(), plan.adoption(), &mut host, &mut b);
        if succeeds {
            result.map_err(err)?;
        } else {
            assert!(matches!(result, Err(article::namespace::Error::Label(_))));
        }
    }
    Ok(())
}
