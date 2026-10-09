use super::*;
use nepl3_core::budget::StopReason;
use nepl3_doc_core::model::{BlockRef, DocKind, EmbedKind};
use nepl3_doc_html::{
    ForeignRenderError, LocalPreparationError, ParallelMode, RenderError, RenderOptions, guests,
};
use nepl3_markup::{html::*, mathml::Display};

#[test]
fn article_math_guests_keep_inline_display_and_structural_zero_denominator() -> Result<(), String> {
    let compiled = compiled()?;
    let input = "article en \"Math\" body cons paragraph cons sentence cons math Math frac 1 0 nil nil cons code Math frac 1 0 cons display Math frac 1 0 nil";
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            input,
            "Article",
            |tree, profile, b, _| {
                let registry = profile.registry();
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec =
                    FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                let mut doc = lower::document(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let display_node = doc
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
                    .ok_or("display")? as u64;
                let second_display_node = doc.value.nodes.len() as u64;
                doc.value
                    .nodes
                    .push(doc.value.nodes[display_node as usize].clone());
                let body = doc
                    .value
                    .nodes
                    .iter_mut()
                    .find_map(|n| {
                        if let DocKind::Body { blocks } = &mut n.kind {
                            Some(blocks)
                        } else {
                            None
                        }
                    })
                    .ok_or("body")?;
                body.push(BlockRef(display_node));
                body.push(BlockRef(second_display_node));
                let options = RenderOptions {
                    parallel: ParallelMode::Rows,
                };
                assert!(matches!(
                    nepl3_doc_html::code::prepare_code(&doc, &options, registry, &mut codec, b),
                    Err(LocalPreparationError::NeedsResolution(_))
                ));
                assert!(matches!(
                    nepl3_doc_html::prepare_local(&doc, &options, registry, &mut codec, b),
                    Err(LocalPreparationError::NeedsResolution(_))
                ));
                let prepared =
                    guests::prepare(&doc, &options, registry, &mut codec, b).map_err(err)?;
                let mut kinds = Vec::new();
                let mut measured = budget();
                let mut adapter =
                    |guest: &nepl3_doc_core::model::DocEmbed,
                     embed: nepl3_doc_core::model::EmbedRef,
                     b: &mut nepl3_core::budget::Budget| {
                        assert_eq!(guest, &doc.value.embeds[embed.0 as usize]);
                        kinds.push(guest.kind);
                        if guest.kind == EmbedKind::Code {
                            return super::code::render(guest, tree, profile, b);
                        }
                        let display = match guest.kind {
                            EmbedKind::InlineMath => Display::Inline,
                            EmbedKind::DisplayMath => Display::Block,
                            _ => return Err("unexpected guest".to_string()),
                        };
                        let mut host = crate::doc::math::MathDisplayHost {
                            registry,
                            math_surface: &compiled.others[0].schema,
                            sentence_surface: Some(&compiled.others[3].schema),
                            doc_surface: Some(&compiled.doc.package.schema),
                            codec: &mut codec,
                        };
                        Ok(host
                            .render(&guest.closure, display, b)
                            .map_err(err)?
                            .into_html(b)
                            .map_err(err)?
                            .markup)
                    };
                let result = guests::render(&prepared, &mut adapter, &mut measured).map_err(err)?;
                let mut contexts = Vec::new();
                let mut contextual_budget = budget();
                let contextual = guests::render_with_context(&prepared, &mut |context, b| {
                    assert!(core::ptr::eq(context.document(), &doc));
                    assert!(core::ptr::eq(context.options(), &options));
                    assert_eq!(context.document_digest(), result.fragment.document_digest);
                    assert!(core::ptr::eq(context.embed(), &doc.value.embeds[context.reference().0 as usize]));
                    assert_eq!(context.ordinal(), contexts.len() as u64);
                    assert!(matches!(&doc.value.nodes[context.node() as usize].kind,
                        DocKind::InlineMath { syntax } | DocKind::DisplayMath { syntax } | DocKind::Code { syntax }
                        if *syntax == context.reference()));
                    contexts.push((context.node(), context.reference(), context.ordinal()));
                    adapter(context.embed(), context.reference(), b)
                }, &mut contextual_budget).map_err(err)?;
                assert_eq!(contextual.fragment, result.fragment);
                assert_eq!(contextual.foreign, result.foreign);
                assert_eq!(contextual_budget.usage(), measured.usage());
                assert_eq!(contexts[2].0, display_node);
                assert_eq!(contexts[3].0, display_node);
                assert_eq!(contexts[4].0, second_display_node);
                assert_eq!(contexts[2].1, contexts[3].1);
                assert_eq!(contexts[3].1, contexts[4].1);
                assert_eq!(
                    kinds,
                    [
                        EmbedKind::InlineMath,
                        EmbedKind::Code,
                        EmbedKind::DisplayMath,
                        EmbedKind::DisplayMath,
                        EmbedKind::DisplayMath
                    ]
                    .repeat(2)
                );
                assert_eq!(result.foreign.len(), 5);
                assert_eq!(result.foreign[2].embed, result.foreign[3].embed);
                assert_ne!(
                    result.foreign[2].first_element,
                    result.foreign[3].first_element
                );
                assert_ne!(
                    result.foreign[0].first_element,
                    result.foreign[1].first_element
                );
                for p in &result.foreign {
                    assert!(p.elements > 0);
                    assert!(
                        p.first_element + p.elements
                            <= result.fragment.markup.fragment.nodes.len() as u64
                    );
                }
                let markup = &result.fragment.markup;
                let checked =
                    validate(&markup.fragment, markup.slot, &markup.policy, &mut measured)
                        .map_err(err)?;
                let html = serialize(&checked, &mut measured).map_err(err)?;
                assert_eq!(html.matches("<mfrac>").count(), 4);
                assert!(html.contains("display=\"inline\""));
                assert!(html.contains("display=\"block\""));
                assert!(html.contains("<div>"));
                assert!(!html.contains("<script"));
                assert!(markup.fragment.nodes.iter().any(|node| matches!(
                    node,
                    HtmlNode::Element {
                        tag: HtmlTag::Pre,
                        ..
                    }
                )));
                let code = markup
                    .fragment
                    .nodes
                    .iter()
                    .position(|node| {
                        matches!(
                            node,
                            HtmlNode::Element {
                                tag: HtmlTag::Code,
                                ..
                            }
                        )
                    })
                    .ok_or("code element")?;
                let mut pending = vec![code as u64];
                let mut retained = String::new();
                while let Some(index) = pending.pop() {
                    match &markup.fragment.nodes[index as usize] {
                        HtmlNode::Text { text } => retained.push_str(text),
                        HtmlNode::Element { children, .. }
                        | HtmlNode::MathElement { children, .. }
                        | HtmlNode::SvgElement { children, .. } => {
                            pending.extend(children.iter().rev().copied())
                        }
                    }
                }
                assert_eq!(retained.trim(), "frac 1 0");
                // A block callback result cannot cross the inline occurrence boundary.
                let bad = guests::render(
                    &prepared,
                    &mut |_, _, _| {
                        Ok::<_, String>(HtmlRequest {
                            fragment: HtmlFragment {
                                root: 0,
                                nodes: vec![HtmlNode::Element {
                                    tag: HtmlTag::Div,
                                    attributes: vec![],
                                    children: vec![],
                                }],
                            },
                            slot: HtmlSlot::Block,
                            policy: HtmlPolicy { classes: vec![] },
                        })
                    },
                    &mut budget(),
                );
                assert!(matches!(
                    bad,
                    Err(ForeignRenderError::Render(RenderError::Markup(_)))
                ));
                let mut stopped = budget();
                stopped.cancel();
                let mut called = false;
                let result = guests::render(
                    &prepared,
                    &mut |_, _, _| {
                        called = true;
                        Err::<HtmlRequest, _>("called")
                    },
                    &mut stopped,
                );
                assert!(matches!(
                    result,
                    Err(ForeignRenderError::Render(RenderError::Stopped(
                        nepl3_core::budget::StopReason::Cancelled
                    )))
                ));
                assert!(!called);
                for stopped_before in [true, false] {
                    let mut stopped = budget();
                    if stopped_before {
                        stopped.cancel();
                    }
                    let mut contextual_calls = 0;
                    let error = guests::render_with_context(
                        &prepared,
                        &mut |context, b| {
                            contextual_calls += 1;
                            assert_eq!(context.ordinal(), 0);
                            b.cancel();
                            Err::<HtmlRequest, _>("callback failure after cancellation")
                        },
                        &mut stopped,
                    );
                    assert!(matches!(
                        error,
                        Err(ForeignRenderError::Render(RenderError::Stopped(
                            StopReason::Cancelled
                        )))
                    ));
                    assert_eq!(contextual_calls, usize::from(!stopped_before));
                }
                let mut zero_work = nepl3_core::budget::Budget::new(nepl3_core::budget::Limits {
                    work: 0,
                    ..budget().limits()
                });
                let mut called = false;
                let error = guests::render_with_context(
                    &prepared,
                    &mut |_, _| {
                        called = true;
                        Err::<HtmlRequest, _>("unexpected callback")
                    },
                    &mut zero_work,
                );
                assert!(matches!(
                    error,
                    Err(ForeignRenderError::Render(RenderError::Stopped(
                        StopReason::WorkLimit
                    )))
                ));
                assert!(!called);
                let error = guests::render_with_context(
                    &prepared,
                    &mut |_, _| Err::<HtmlRequest, _>("typed guest failure"),
                    &mut budget(),
                );
                assert!(matches!(
                    error,
                    Err(ForeignRenderError::Foreign("typed guest failure"))
                ));
                let bad = guests::render_with_context(
                    &prepared,
                    &mut |_, _| {
                        Ok::<_, String>(HtmlRequest {
                            fragment: HtmlFragment {
                                root: 0,
                                nodes: vec![HtmlNode::Element {
                                    tag: HtmlTag::Div,
                                    attributes: vec![],
                                    children: vec![],
                                }],
                            },
                            slot: HtmlSlot::Block,
                            policy: HtmlPolicy { classes: vec![] },
                        })
                    },
                    &mut budget(),
                );
                assert!(matches!(
                    bad,
                    Err(ForeignRenderError::Render(RenderError::Markup(_)))
                ));
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn article_guest_route_preserves_unresolved_links_and_assets() -> Result<(), String> {
    let compiled = compiled()?;
    for input in [
        "article en \"Link\" body cons paragraph cons sentence cons link external \"https://example.test/\" text \"link\" nil nil nil",
        "article en \"Image\" body cons image asset \"figure\" none \"alt\" none nil",
    ] {
        with_input_route(true, &compiled, input, "Article", |tree, profile, b, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let doc = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            assert!(matches!(
                guests::prepare(&doc, &options, profile.registry(), &mut codec, b),
                Err(LocalPreparationError::NeedsResolution(_))
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn article_guest_wrappers_bound_temporary_callback_depth_and_stops() -> Result<(), String> {
    use nepl3_core::budget::StopReason;
    let compiled = compiled()?;
    for (input, parent_depth) in [
        ("article en \"Display\" body cons display Math 7 nil", 2u64),
        ("article en \"Code\" body cons code Math 7 nil", 4u64),
    ] {
        with_input_route(true, &compiled, input, "Article", |tree, profile, b, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let mut doc = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let prepared =
                guests::prepare(&doc, &options, profile.registry(), &mut codec, b).map_err(err)?;
            let shallow = || HtmlRequest {
                fragment: HtmlFragment {
                    root: 0,
                    nodes: vec![HtmlNode::Text { text: "7".into() }],
                },
                slot: HtmlSlot::Phrasing,
                policy: HtmlPolicy { classes: vec![] },
            };
            let peak = 5 + parent_depth + 20;
            let mut callback_nodes = None;
            for short in [false, true] {
                let mut limits = budget().limits();
                limits.depth = peak - u64::from(short);
                let mut measured = nepl3_core::budget::Budget::new(limits);
                let mut called = false;
                let result = measured
                    .with_depth_at_least(5, |b| {
                        Ok::<_, StopReason>(guests::render(
                            &prepared,
                            &mut |_, _, b| {
                                called = true;
                                assert_eq!(b.current_depth(), 5 + parent_depth);
                                callback_nodes = Some(b.usage().nodes);
                                b.observe_depth(20).map_err(err)?;
                                Ok::<_, String>(shallow())
                            },
                            b,
                        ))
                    })
                    .map_err(err)?;
                assert!(called);
                assert_eq!(measured.current_depth(), 0);
                if short {
                    assert!(matches!(
                        result,
                        Err(ForeignRenderError::Render(RenderError::Stopped(
                            StopReason::DepthLimit
                        )))
                    ));
                    assert_eq!(measured.poll(), Err(StopReason::DepthLimit));
                } else {
                    result.map_err(err)?;
                    assert_eq!(measured.usage().depth, peak);
                }
            }
            let mut measured = budget();
            let result = guests::render(
                &prepared,
                &mut |_, _, b| {
                    b.cancel();
                    Err::<HtmlRequest, _>("host renderer unavailable")
                },
                &mut measured,
            );
            assert!(matches!(
                result,
                Err(ForeignRenderError::Render(RenderError::Stopped(
                    StopReason::Cancelled
                )))
            ));
            assert_eq!(measured.poll(), Err(StopReason::Cancelled));
            let mut limits = budget().limits();
            limits.nodes = callback_nodes
                .ok_or("callback boundary")?
                .checked_sub(1)
                .ok_or("wrapper nodes")?;
            let wrapper_limit = limits.nodes;
            let mut measured = nepl3_core::budget::Budget::new(limits);
            let mut called = false;
            let result = guests::render(
                &prepared,
                &mut |_, _, _| {
                    called = true;
                    Ok::<_, String>(shallow())
                },
                &mut measured,
            );
            assert!(matches!(
                result,
                Err(ForeignRenderError::Render(RenderError::Stopped(
                    StopReason::NodeLimit
                )))
            ));
            assert!(!called);
            assert_eq!(measured.usage().nodes, wrapper_limit);
            // A structurally retained but unselected Circuit guest is not
            // implicitly dispatched by the new Code/Math preparation route.
            if parent_depth == 2 {
                use nepl3_doc_core::model::{DocRoot, SentenceRef};
                let DocRoot::Article(root) = doc.value.root else {
                    return Err("article".into());
                };
                let title = match doc.value.nodes[root.0 as usize].kind {
                    DocKind::Article { title, .. } => title,
                    _ => return Err("article node".into()),
                };
                let index = doc
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
                    .ok_or("display")?;
                let syntax = match doc.value.nodes[index].kind {
                    DocKind::DisplayMath { syntax } => syntax,
                    _ => return Err("display node".into()),
                };
                doc.value.embeds[syntax.0 as usize].kind = EmbedKind::CircuitFigure;
                doc.value.nodes[index].kind = DocKind::CircuitFigure {
                    caption: SentenceRef(title.0),
                    syntax,
                };
                assert!(matches!(
                    guests::prepare(&doc, &options, profile.registry(), &mut codec, b),
                    Err(LocalPreparationError::NeedsResolution(_))
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}
