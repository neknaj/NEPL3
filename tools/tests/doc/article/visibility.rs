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
