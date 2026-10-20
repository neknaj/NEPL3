use super::*;
use nepl3_doc_core::{check::Category, labels::namespace as labels, model::*};
use nepl3_doc_html::{ParallelMode, RenderOptions, namespace::display};
use nepl3_markup::html::*;

fn guest(id: &str, target: &str) -> DocumentSyntax {
    DocumentSyntax {
        value: DocValue {
            root: DocRoot::Inline(InlineRef(0)),
            nodes: vec![
                DocKind::Concat {
                    inlines: vec![InlineRef(1), InlineRef(2), InlineRef(3)],
                },
                DocKind::Anchor {
                    id: id.into(),
                    label: InlineRef(4),
                },
                DocKind::Reference {
                    target: target.into(),
                    label: InlineRef(4),
                },
                DocKind::Link {
                    target: LinkTarget::External {
                        uri: "https://example.test/guest".into(),
                    },
                    label: InlineRef(4),
                },
                DocKind::Text {
                    text: "label".into(),
                },
            ]
            .into_iter()
            .map(|kind| DocNode {
                kind,
                locations: vec![],
                origin: None,
                span: None,
            })
            .collect(),
            embeds: Default::default(),
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    }
}

#[test]
fn article_display_namespace_renders_pending_members_and_validates_whole_output()
-> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence cons ref target text "forward" cons anchor outer text "outer" cons link external "https://example.test/root" text "link" nil body cons paragraph cons Sentence "first slot" cons Sentence "second slot" nil nil"#;
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            source,
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
                let root = nepl3_suite::adapters::sentence::literal::lower(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                // This is the generic explicit-adapter boundary, not automatic
                // Sentence lowering: the host selects these exact immutable members.
                let first = guest("target", "other");
                let second = guest("other", "outer");
                let mut members = Vec::new();
                for doc in [&root, &first, &second] {
                    members.push(
                        labels::inspect(doc, registry, b, codec.source_admission()).map_err(err)?,
                    );
                }
                let refs: Vec<_> = members.iter().collect();
                let checked = labels::resolve(&refs, b).map_err(err)?;
                let options = RenderOptions {
                    parallel: ParallelMode::Rows,
                };
                assert!(matches!(
                    nepl3_doc_html::namespace::prepare_with_foreign(
                        &checked, &options, registry, &mut codec, b
                    ),
                    Err(nepl3_doc_html::LocalPreparationError::NeedsResolution(_))
                ));
                let prepared =
                    display::prepare(&checked, &options, registry, &mut codec, b).map_err(err)?;
                for scenario in 0..6 {
                    let mut seen = Vec::new();
                    let result = display::render_article(
                        &prepared,
                        labels::MemberId(0),
                        &mut |view, embed, b| {
                            assert_eq!(view.kind, EmbedKind::Sentence);
                            seen.push(embed);
                            if scenario == 4 {
                                return Err("adapter failed".to_string());
                            }
                            if scenario == 5 {
                                b.cancel();
                                return Err("cancelled".to_string());
                            }
                            let member = if scenario == 2 { 1 } else { seen.len() as u64 };
                            let part = display::render_part(
                                &prepared,
                                labels::MemberId(member),
                                &mut |_, _, _| Err("unexpected nested guest".to_string()),
                                b,
                            )
                            .map_err(err)?;
                            let (_, _, mut markup, _) = part.part.into_parts();
                            if scenario == 1 && seen.len() == 2 {
                                markup.fragment = HtmlFragment {
                                    root: 0,
                                    nodes: vec![HtmlNode::Text {
                                        text: "omitted".into(),
                                    }],
                                };
                            }
                            if scenario == 3
                                && let HtmlNode::Element { tag, .. } =
                                    &mut markup.fragment.nodes[markup.fragment.root as usize]
                            {
                                *tag = HtmlTag::Div;
                            }
                            Ok(markup)
                        },
                        &mut budget(),
                    );
                    match (scenario, result) {
                        (0, Ok(output)) => {
                            assert_eq!(seen, vec![EmbedRef(0), EmbedRef(1)]);
                            assert_eq!(output.foreign.len(), 2);
                            let m = output.fragment.markup;
                            let html = serialize(
                                &validate(&m.fragment, m.slot, &m.policy, b).map_err(err)?,
                                b,
                            )
                            .map_err(err)?;
                            assert!(html.contains("https://example.test/root"));
                            assert_eq!(html.matches("https://example.test/guest").count(), 2);
                            assert_eq!(html.matches("<article").count(), 1);
                        }
                        (
                            1,
                            Err(display::ArticleError::Render(
                                nepl3_doc_html::ForeignRenderError::Render(
                                    nepl3_doc_html::RenderError::Markup(
                                        HtmlError::MissingFragment(_),
                                    ),
                                ),
                            )),
                        ) => {}
                        (
                            2,
                            Err(display::ArticleError::Render(
                                nepl3_doc_html::ForeignRenderError::Render(
                                    nepl3_doc_html::RenderError::Markup(HtmlError::DuplicateId(_)),
                                ),
                            )),
                        ) => {}
                        (
                            3,
                            Err(display::ArticleError::Render(
                                nepl3_doc_html::ForeignRenderError::Render(
                                    nepl3_doc_html::RenderError::Markup(HtmlError::Content(_)),
                                ),
                            )),
                        ) => {}
                        (
                            4,
                            Err(display::ArticleError::Render(
                                nepl3_doc_html::ForeignRenderError::Foreign(message),
                            )),
                        ) => assert_eq!(message, "adapter failed"),
                        (
                            5,
                            Err(display::ArticleError::Render(
                                nepl3_doc_html::ForeignRenderError::Render(
                                    nepl3_doc_html::RenderError::Stopped(StopReason::Cancelled),
                                ),
                            )),
                        ) => {}
                        (scenario, Err(error)) => {
                            return Err(format!("scenario {scenario}: {error:?}"));
                        }
                        (scenario, Ok(_)) => {
                            return Err(format!("scenario {scenario}: unexpected success"));
                        }
                    }
                }
                assert!(matches!(
                    display::render_article(
                        &prepared,
                        labels::MemberId(1),
                        &mut |_, _, _| Err::<HtmlRequest, _>("must not run".to_string()),
                        b
                    ),
                    Err(display::ArticleError::ExpectedArticle(labels::MemberId(1)))
                ));
                let mut limits = budget().limits();
                limits.depth = 1;
                let mut limited = Budget::new(limits);
                assert!(matches!(
                    display::render_article(
                        &prepared,
                        labels::MemberId(0),
                        &mut |_, _, _| Err::<HtmlRequest, _>("must not run"),
                        &mut limited
                    ),
                    Err(display::ArticleError::Render(
                        nepl3_doc_html::ForeignRenderError::Render(
                            nepl3_doc_html::RenderError::Stopped(StopReason::DepthLimit)
                        )
                    ))
                ));
                // A part that fits alone must still consume its insertion
                // ancestors. Find that boundary for this fixed small fixture.
                let mut nested_exhaustion = false;
                for depth in 1..32 {
                    let mut limits = budget().limits();
                    limits.depth = depth;
                    if display::render_part(
                        &prepared,
                        labels::MemberId(1),
                        &mut |_, _, _| Err::<HtmlRequest, _>("unexpected"),
                        &mut Budget::new(limits),
                    )
                    .is_err()
                    {
                        continue;
                    }
                    let mut entered = false;
                    let mut guest_failed = false;
                    let mut index = 0;
                    let result = display::render_article(
                        &prepared,
                        labels::MemberId(0),
                        &mut |_, _, b| {
                            entered = true;
                            index += 1;
                            let part = display::render_part(
                                &prepared,
                                labels::MemberId(index),
                                &mut |_, _, _| Err::<HtmlRequest, _>("unexpected"),
                                b,
                            );
                            if part.is_err() && b.poll() == Err(StopReason::DepthLimit) {
                                guest_failed = true;
                            }
                            part.map(|part| part.part.into_parts().2).map_err(err)
                        },
                        &mut Budget::new(limits),
                    );
                    if entered && guest_failed {
                        assert!(matches!(
                            result,
                            Err(display::ArticleError::Render(
                                nepl3_doc_html::ForeignRenderError::Render(
                                    nepl3_doc_html::RenderError::Stopped(StopReason::DepthLimit)
                                )
                            ))
                        ));
                        nested_exhaustion = true;
                        break;
                    }
                }
                assert!(
                    nested_exhaustion,
                    "combined insertion must include ancestor depth"
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn display_namespace_keeps_resolution_and_uri_boundaries() -> Result<(), String> {
    let compiled = compiled()?;
    let registry = &compiled.doc.registry;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    for (number, target) in [
        LinkTarget::External {
            uri: "https://example.test/safe".into(),
        },
        LinkTarget::External {
            uri: "javascript:alert(1)".into(),
        },
        LinkTarget::Page {
            page: "elsewhere".into(),
            fragment: None,
        },
        LinkTarget::Relative {
            path: "elsewhere.html".into(),
            fragment: None,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut doc = guest("target", "target");
        doc.value.nodes[3].kind = DocKind::Link {
            target,
            label: InlineRef(4),
        };
        let member = labels::inspect(&doc, registry, &mut budget(), codec.source_admission())
            .map_err(err)?;
        let refs = [&member];
        let checked = labels::resolve(&refs, &mut budget()).map_err(err)?;
        let result = display::prepare(&checked, &options, registry, &mut codec, &mut budget());
        match (number, result) {
            (0, Ok(prepared)) => {
                let part = display::render_part(
                    &prepared,
                    labels::MemberId(0),
                    &mut |_, _, _| Err::<HtmlRequest, _>("unexpected"),
                    &mut budget(),
                )
                .map_err(err)?;
                let (_, _, markup, _) = part.part.into_parts();
                let html = serialize(
                    &validate(&markup.fragment, markup.slot, &markup.policy, &mut budget())
                        .map_err(err)?,
                    &mut budget(),
                )
                .map_err(err)?;
                assert!(html.contains("https://example.test/safe"));
                for reason in [
                    StopReason::WorkLimit,
                    StopReason::AllocationLimit,
                    StopReason::Cancelled,
                ] {
                    let mut limits = budget().limits();
                    match reason {
                        StopReason::WorkLimit => limits.work = 0,
                        StopReason::AllocationLimit => limits.allocation_units = 0,
                        _ => {}
                    }
                    let mut limited = Budget::new(limits);
                    if reason == StopReason::Cancelled {
                        limited.cancel();
                    }
                    assert!(
                        matches!(display::prepare(&checked, &options, registry, &mut codec, &mut limited), Err(nepl3_doc_html::LocalPreparationError::Stopped(actual)) if actual == reason)
                    );
                }
            }
            (1, Err(nepl3_doc_html::LocalPreparationError::InvalidExternalUri { node: 3 })) => {}
            (2 | 3, Err(nepl3_doc_html::LocalPreparationError::NeedsResolution(_))) => {}
            (_, Err(error)) => return Err(err(error)),
            (_, Ok(_)) => return Err("unexpected admission".into()),
        }
    }
    Ok(())
}

#[test]
fn display_namespace_keeps_foreign_roles_and_math_scope_separate() -> Result<(), String> {
    let compiled = compiled()?;
    for (source, supported) in [
        (
            r#"article en sentence cons anchor outer text "outer" nil body cons display Math frac 1 2 cons code Math frac 1 2 nil"#,
            true,
        ),
        (
            r#"article en sentence nil body cons circuit sentence nil Circuit design nil Main nil nil"#,
            false,
        ),
        (
            r#"article en sentence nil body cons image asset "missing" none sentence nil none nil"#,
            false,
        ),
    ] {
        with_input_route(true, &compiled, source, "Article", |tree, profile, b, a| {
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
            let member =
                labels::inspect(&doc, registry, b, codec.source_admission()).map_err(err)?;
            let refs = [&member];
            let checked = labels::resolve(&refs, b).map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let prepared = display::prepare(&checked, &options, registry, &mut codec, b);
            if !supported {
                assert!(matches!(
                    prepared,
                    Err(nepl3_doc_html::LocalPreparationError::NeedsResolution(_))
                ));
                return Ok(());
            }
            let prepared = prepared.map_err(err)?;
            let mut kinds = Vec::new();
            display::render_article(
                &prepared,
                labels::MemberId(0),
                &mut |view, _, _| {
                    kinds.push(view.kind);
                    Ok::<_, String>(HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Text {
                                text: "explicit adapter".into(),
                            }],
                        },
                        slot: if view.kind == EmbedKind::DisplayMath {
                            HtmlSlot::Block
                        } else {
                            HtmlSlot::Phrasing
                        },
                        policy: HtmlPolicy { classes: vec![] },
                    })
                },
                b,
            )
            .map_err(err)?;
            assert_eq!(kinds, vec![EmbedKind::DisplayMath, EmbedKind::Code]);
            // Math remains a closed namespace even when the outer Article has
            // a matching target. Derive the actual rendered ID, not its format.
            let mut outer = None;
            let rendered = display::render_article(
                &prepared,
                labels::MemberId(0),
                &mut |view, _, _| {
                    Ok::<_, String>(HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Text { text: "x".into() }],
                        },
                        slot: if view.kind == EmbedKind::DisplayMath {
                            HtmlSlot::Block
                        } else {
                            HtmlSlot::Phrasing
                        },
                        policy: HtmlPolicy { classes: vec![] },
                    })
                },
                b,
            )
            .map_err(err)?;
            for node in &rendered.fragment.markup.fragment.nodes {
                if let HtmlNode::Element { attributes, .. } = node {
                    for attr in attributes {
                        if let HtmlAttribute::Id { value } = attr {
                            outer = Some(value.clone());
                        }
                    }
                }
            }
            let outer = outer.ok_or("missing outer anchor")?;
            let result = display::render_article(
                &prepared,
                labels::MemberId(0),
                &mut |_, _, _| {
                    Ok::<_, String>(HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![
                                HtmlNode::Element {
                                    tag: HtmlTag::A,
                                    attributes: vec![HtmlAttribute::Href {
                                        value: HtmlHref::Fragment { id: outer.clone() },
                                    }],
                                    children: vec![1],
                                },
                                HtmlNode::Text {
                                    text: "cross-scope".into(),
                                },
                            ],
                        },
                        slot: HtmlSlot::Block,
                        policy: HtmlPolicy { classes: vec![] },
                    })
                },
                b,
            );
            assert!(matches!(
                result,
                Err(display::ArticleError::Render(
                    nepl3_doc_html::ForeignRenderError::Render(
                        nepl3_doc_html::RenderError::Markup(HtmlError::MissingFragment(_))
                    )
                ))
            ));
            assert!(matches!(
                display::render_part(
                    &prepared,
                    labels::MemberId(99),
                    &mut |_, _, _| Err::<HtmlRequest, _>("must not run"),
                    b
                ),
                Err(nepl3_doc_html::namespace::ForeignPartError::Member(
                    labels::MemberId(99)
                ))
            ));
            Ok(())
        })?;
    }
    Ok(())
}
