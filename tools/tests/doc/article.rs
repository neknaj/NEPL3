use super::*;
use nepl3_doc_html::{
    ParallelMode, RenderOptions, prepare_article_with_foreign, render_article_with_foreign,
};
use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode, serialize, validate};

#[test]
fn article_requires_resolution_for_assets() -> Result<(), String> {
    let compiled = compiled()?;
    {
        let block = "image asset \"logo\" none sentence \"alt\" none";
        // An independent parser fixture retains the unresolved asset request.
        let source = format!("article en sentence \"Title\" body cons {block} nil");
        with_input(&compiled, &source, "Article", |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let document = lower::document(
                &checked,
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let expected =
                inspect(&document, profile.registry(), &mut codec, &mut budget()).map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            match prepare_article_with_foreign(
                &document,
                &options,
                profile.registry(),
                &mut codec,
                &mut budget(),
            ) {
                Err(nepl3_doc_html::LocalPreparationError::NeedsResolution(plan)) => {
                    assert_eq!(plan, expected)
                }
                _ => return Err(format!("missing resolution requirement: {block}")),
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn article_places_block_guest_results_with_caption_and_owner() -> Result<(), String> {
    use nepl3_markup::html::{HtmlFragment, HtmlPolicy, HtmlRequest, HtmlSlot, HtmlTag};
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body
      cons display Math add 1 2
      cons code Math add 3 4
      cons circuit sentence "Caption" Circuit design nil Main nil
      nil"#;
    with_input(&compiled, source, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let document = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let options = RenderOptions {
            parallel: ParallelMode::Rows,
        };
        let prepared = prepare_article_with_foreign(
            &document,
            &options,
            profile.registry(),
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let mut calls = Vec::new();
        let rendered = render_article_with_foreign(
            &prepared,
            &mut |slot, embed, b| {
                calls.push((slot.kind, embed));
                if slot.kind == EmbedKind::Sentence {
                    let sentence = nepl3_suite::adapters::document::sentence::lower(
                        slot,
                        slot.schema(),
                        &[],
                        profile.registry(),
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    return nepl3_suite::adapters::sentence::html::render(
                        &sentence,
                        profile.registry(),
                        b,
                        &mut SourceAdmission::default(),
                    )
                    .map(|v| v.into_markup())
                    .map_err(err);
                }
                // Backend contract fixture: a selected producer supplies block markup.
                // This asserts placement, not evaluation of Math or Circuit syntax.
                let text = match slot.kind {
                    EmbedKind::DisplayMath => "display result",
                    EmbedKind::Code => "code result",
                    EmbedKind::CircuitFigure => "circuit result",
                    _ => return Err("unexpected role".into()),
                };
                Ok(HtmlRequest {
                    fragment: HtmlFragment {
                        root: 0,
                        nodes: vec![
                            HtmlNode::Element {
                                tag: HtmlTag::Div,
                                attributes: vec![],
                                children: vec![1],
                            },
                            HtmlNode::Text { text: text.into() },
                        ],
                    },
                    slot: HtmlSlot::Block,
                    policy: HtmlPolicy { classes: vec![] },
                })
            },
            &mut budget(),
        )
        .map_err(err)?;
        assert_eq!(
            calls.iter().map(|v| v.0).collect::<Vec<_>>(),
            [
                EmbedKind::Sentence,
                EmbedKind::DisplayMath,
                EmbedKind::Code,
                EmbedKind::CircuitFigure,
                EmbedKind::Sentence
            ]
        );
        assert_eq!(
            rendered.foreign.iter().map(|p| p.embed).collect::<Vec<_>>(),
            calls.iter().map(|v| v.1).collect::<Vec<_>>()
        );
        for placement in &rendered.foreign {
            let owner = document.value.nodes.iter().position(|node| {
                matches!(&node.kind,
                    DocKind::Sentence { syntax } | DocKind::DisplayMath { syntax }
                    | DocKind::Code { syntax } | DocKind::CircuitFigure { syntax, .. }
                    if *syntax == placement.embed)
            });
            // Each imported node is attributed to the Doc occurrence that requested it.
            let owner = owner.ok_or("guest owner")? as u64;
            assert!(placement.elements > 0);
            assert_eq!(
                rendered
                    .fragment
                    .origins
                    .iter()
                    .filter(|origin| origin.element >= placement.first_element
                        && origin.element < placement.first_element + placement.elements)
                    .count() as u64,
                placement.elements
            );
            assert!(
                rendered
                    .fragment
                    .origins
                    .iter()
                    .filter(|origin| origin.element >= placement.first_element
                        && origin.element < placement.first_element + placement.elements)
                    .all(|origin| origin.node == owner)
            );
        }
        let markup = &rendered.fragment.markup;
        let html = serialize(
            &validate(&markup.fragment, markup.slot, &markup.policy, &mut budget()).map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        let positions = [
            "Title",
            "display result",
            "code result",
            "circuit result",
            "Caption",
        ]
        .map(|text| html.find(text).ok_or("missing content"));
        let positions = positions.into_iter().collect::<Result<Vec<_>, _>>()?;
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(html.contains("<figure>"));
        assert!(html.contains("<figcaption>"));
        let invalid = render_article_with_foreign(
            &prepared,
            &mut |slot, _, _| {
                Ok::<_, String>(HtmlRequest {
                    fragment: HtmlFragment {
                        root: 0,
                        nodes: vec![HtmlNode::Element {
                            tag: if slot.kind == EmbedKind::Sentence {
                                HtmlTag::Span
                            } else {
                                HtmlTag::Tr
                            },
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
            invalid,
            Err(nepl3_doc_html::ForeignRenderError::Render(
                nepl3_doc_html::RenderError::Markup(_)
            ))
        ));
        // Article, Figure and content Div add three ancestors to the guest.
        for guest_depth in [253, 254] {
            let mut deep_budget = nepl3_core::budget::Budget::new(nepl3_core::budget::Limits {
                depth: 1024,
                ..budget().limits()
            });
            let deep = render_article_with_foreign(
                &prepared,
                &mut |slot, _, _| {
                    let count = if slot.kind == EmbedKind::CircuitFigure {
                        guest_depth
                    } else {
                        1
                    };
                    Ok::<_, String>(HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: (0..count)
                                .map(|index| HtmlNode::Element {
                                    tag: if slot.kind == EmbedKind::Sentence {
                                        HtmlTag::Span
                                    } else {
                                        HtmlTag::Div
                                    },
                                    attributes: vec![],
                                    children: if index + 1 < count {
                                        vec![index + 1]
                                    } else {
                                        vec![]
                                    },
                                })
                                .collect(),
                        },
                        slot: if slot.kind == EmbedKind::Sentence {
                            HtmlSlot::Phrasing
                        } else {
                            HtmlSlot::Block
                        },
                        policy: HtmlPolicy { classes: vec![] },
                    })
                },
                &mut deep_budget,
            );
            let circuit_node = document
                .value
                .nodes
                .iter()
                .position(|node| matches!(node.kind, DocKind::CircuitFigure { .. }))
                .ok_or("circuit")? as u64;
            if guest_depth == 253 {
                deep.map_err(err)?;
            } else {
                assert!(
                    matches!(deep, Err(nepl3_doc_html::ForeignRenderError::Render(nepl3_doc_html::RenderError::OutputDepth { node })) if node == circuit_node)
                );
            }
        }
        Ok(())
    })
}

#[test]
fn article_renders_independent_sentence_links_after_wire_receipt() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Links" body cons paragraph cons sentence sentence cons link "https://example.org/docs?q=one&lang=en#part" text "Reference" cons text " / " cons link "mailto:author@example.org" text "Mail" nil nil nil"#;
    with_input(&compiled, source, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let document = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let raw = portable::to_value(&document, profile.registry(), &mut codec, &mut budget())
            .map_err(err)?;
        let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
        let mut received_admission = SourceAdmission::default();
        let mut receiver =
            FoundationCodec::new(profile.registry(), &empty, &mut received_admission)
                .map_err(err)?;
        let received = portable::from_value(
            &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
            profile.registry(),
            &mut receiver,
            &mut budget(),
        )
        .map_err(err)?;
        retention::assert_doc_retention(&document, &received)?;
        let options = RenderOptions {
            parallel: ParallelMode::Rows,
        };
        let mut original = None;
        for doc in [&document, &received] {
            let prepared = prepare_article_with_foreign(
                doc,
                &options,
                profile.registry(),
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            let mut calls = Vec::new();
            let rendered = render_article_with_foreign(
                &prepared,
                &mut |slot, embed, b| {
                    calls.push(embed);
                    let sentence = nepl3_suite::adapters::document::sentence::lower(
                        slot,
                        slot.schema(),
                        &[],
                        profile.registry(),
                        &mut receiver,
                        b,
                    )
                    .map_err(err)?;
                    nepl3_suite::adapters::sentence::html::render(
                        &sentence,
                        profile.registry(),
                        b,
                        &mut SourceAdmission::default(),
                    )
                    .map(|v| v.into_markup())
                    .map_err(err)
                },
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(calls.len(), 2); // Title and body have separate Sentence owners.
            assert_eq!(
                rendered.foreign.iter().map(|p| p.embed).collect::<Vec<_>>(),
                calls
            );
            assert!(rendered.foreign.iter().all(|p| p.elements > 0));
            for duplicate in [true, false] {
                use nepl3_markup::html::{
                    HtmlError, HtmlFragment, HtmlPolicy, HtmlRequest, HtmlSlot, HtmlTag,
                };
                let mut calls = 0;
                let result = render_article_with_foreign(
                    &prepared,
                    &mut |_, _, _| {
                        calls += 1;
                        Ok::<_, String>(HtmlRequest {
                            fragment: HtmlFragment {
                                root: 0,
                                nodes: vec![HtmlNode::Element {
                                    tag: if duplicate { HtmlTag::Span } else { HtmlTag::A },
                                    attributes: vec![if duplicate {
                                        HtmlAttribute::Id {
                                            value: "shared-id".into(),
                                        }
                                    } else {
                                        HtmlAttribute::Href {
                                            value: HtmlHref::Fragment {
                                                id: "missing-id".into(),
                                            },
                                        }
                                    }],
                                    children: vec![],
                                }],
                            },
                            slot: HtmlSlot::Phrasing,
                            policy: HtmlPolicy { classes: vec![] },
                        })
                    },
                    &mut budget(),
                );
                assert_eq!(calls, 2); // Each part is locally valid; assembly must reject it.
                match result {
                    Err(nepl3_doc_html::ForeignRenderError::Render(
                        nepl3_doc_html::RenderError::Markup(error),
                    )) => {
                        assert!(matches!(
                            (duplicate, error),
                            (true, HtmlError::DuplicateId(_))
                                | (false, HtmlError::MissingFragment(_))
                        ));
                    }
                    _ => return Err("invalid Article namespace accepted".into()),
                }
            }
            let markup = &rendered.fragment.markup;
            let hrefs = markup
                .fragment
                .nodes
                .iter()
                .flat_map(|node| match node {
                    HtmlNode::Element { attributes, .. } => attributes.as_slice(),
                    _ => &[],
                })
                .filter_map(|attr| match attr {
                    HtmlAttribute::Href {
                        value: HtmlHref::External { uri },
                    } => Some(uri.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                hrefs,
                [
                    "https://example.org/docs?q=one&lang=en#part",
                    "mailto:author@example.org"
                ]
            );
            let html = serialize(
                &validate(&markup.fragment, markup.slot, &markup.policy, &mut budget())
                    .map_err(err)?,
                &mut budget(),
            )
            .map_err(err)?;
            assert!(html.contains("q=one&amp;lang=en#part"));
            // A complete block Article cannot occupy a phrasing Sentence slot.
            assert!(matches!(
                render_article_with_foreign(
                    &prepared,
                    &mut |_, _, _| Ok::<_, String>(markup.clone()),
                    &mut budget(),
                ),
                Err(nepl3_doc_html::ForeignRenderError::Render(
                    nepl3_doc_html::RenderError::Markup(_)
                ))
            ));
            for (limits, reason) in [
                (
                    nepl3_core::budget::Limits {
                        work: 0,
                        ..budget().limits()
                    },
                    nepl3_core::budget::StopReason::WorkLimit,
                ),
                (
                    nepl3_core::budget::Limits {
                        allocation_units: 0,
                        ..budget().limits()
                    },
                    nepl3_core::budget::StopReason::AllocationLimit,
                ),
            ] {
                let mut limited = nepl3_core::budget::Budget::new(limits);
                let mut called = false;
                assert!(matches!(render_article_with_foreign(
                    &prepared,
                    &mut |_, _, _| {
                        called = true;
                        Err::<nepl3_markup::html::HtmlRequest, _>("unexpected callback")
                    },
                    &mut limited,
                ), Err(nepl3_doc_html::ForeignRenderError::Render(
                    nepl3_doc_html::RenderError::Stopped(actual)
                )) if actual == reason));
                assert!(!called);
                assert_eq!(limited.poll(), Err(reason));
            }
            if let Some(prior) = &original {
                assert_eq!(prior, &rendered.fragment);
            } else {
                original = Some(rendered.fragment);
            }
            let mut cancelled = budget();
            cancelled.cancel();
            let mut called = false;
            assert!(
                render_article_with_foreign(
                    &prepared,
                    &mut |_, _, _| {
                        called = true;
                        Err::<nepl3_markup::html::HtmlRequest, _>("unexpected callback")
                    },
                    &mut cancelled
                )
                .is_err()
            );
            assert!(!called);
        }
        Ok(())
    })
}
