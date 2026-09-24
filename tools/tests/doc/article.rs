use super::*;
use nepl3_doc_html::{
    ParallelMode, RenderOptions, prepare_article_with_foreign, render_article_with_foreign,
};
use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode, serialize, validate};

#[test]
fn article_requires_resolution_for_block_guests_and_assets() -> Result<(), String> {
    let compiled = compiled()?;
    for block in [
        "display Math add 1 2",
        "code Math add 1 2",
        "circuit sentence sentence nil Circuit design nil Main nil",
        "image asset \"logo\" none sentence \"alt\" none",
    ] {
        // Independent parser fixtures exercise each unsupported requirement.
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
