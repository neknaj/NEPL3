use super::*;
use nepl3_doc_core::{check::Category, model::*};
use nepl3_doc_html::{
    ParallelMode, RenderOptions,
    assets::{SvgInput, SvgMode},
};
use nepl3_markup::html::*;
use nepl3_tools::doc::annotations::{
    SentenceAnnotationRenderer,
    article::{self, render::svg},
};
const SVG: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const SOURCE: &str = r#"article en Sentence sentence cons document anchor title text "Title" nil body
cons image asset "logo" none Sentence "{[漢/かん]/note}" some Sentence sentence cons document anchor cap text "Caption" nil
cons paragraph cons parallel cons variant en Sentence sentence cons document anchor enlabel text "English" nil cons variant ja Sentence sentence cons document anchor jalabel text "Japanese" nil nil
cons code Math frac 3 4 cons sentence cons math Math frac 1 2 nil
cons sentence cons image asset "logo" none Sentence "Inline & <alt>" nil
cons Sentence sentence cons document ref cap text "Tail" nil nil nil"#;
fn other(
    view: &DocEmbedView<'_>,
    _: EmbedRef,
    _: &mut FoundationCodec<'_>,
    _: &mut Budget,
) -> Result<HtmlRequest, String> {
    assert!(matches!(view.kind, EmbedKind::Code | EmbedKind::InlineMath));
    Ok(HtmlRequest {
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes: vec![] },
        fragment: HtmlFragment {
            root: 0,
            nodes: vec![HtmlNode::Text {
                text: "OTHER".into(),
            }],
        },
    })
}
#[test]
fn planned_svg_host_retains_actual_alt_owners_and_places_selected_occurrences() -> Result<(), String>
{
    let compiled = compiled()?;
    for (parallel, ids) in [
        (ParallelMode::Rows, vec![0, 2, 3, 4, 6]),
        (ParallelMode::Columns, vec![0, 2, 3, 4, 6]),
        (
            ParallelMode::Single {
                language: "en".into(),
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
        let options = RenderOptions { parallel };
        with_input_route(true, &compiled, SOURCE, "Article", |tree, profile, b, _| {
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
            let plan = article::display::plan(&selection, &options, b).map_err(err)?;
            let inputs = [SvgInput {
                id: "logo",
                svg: SVG,
            }];
            for mode in [SvgMode::Embedded, SvgMode::External] {
                let source_before = b.usage().source_bytes;
                let result =
                    svg::render(&plan, &inputs, mode, &mut host, &mut other, b).map_err(err)?;
                assert_eq!(b.usage().source_bytes - source_before, SVG.len() as u64);
                assert!(std::ptr::eq(result.output.resolution.input(), &selection));
                assert_eq!(result.alt_projection.sentences.len(), 2);
                assert!(
                    result
                        .alt_projection
                        .sentences
                        .iter()
                        .all(|(_, syntax)| !syntax.sources.is_empty()
                            && !syntax.locations.is_empty())
                );
                assert_eq!(
                    result
                        .alt_projection
                        .resolved
                        .iter()
                        .map(|text| text.text.as_str())
                        .collect::<Vec<_>>(),
                    vec!["漢", "Inline & <alt>"]
                );
                assert_eq!(
                    result
                        .output
                        .sentences
                        .iter()
                        .map(|record| record.occurrence.0)
                        .collect::<Vec<_>>(),
                    ids
                );
                let markup = &result.output.article.fragment.markup;
                let checked =
                    nepl3_markup::html::validate(&markup.fragment, markup.slot, &markup.policy, b)
                        .map_err(err)?;
                let html = nepl3_markup::html::serialize(&checked, b).map_err(err)?;
                assert!(
                    html.contains("alt=\"漢\"")
                        && html.contains("alt=\"Inline &amp; &lt;alt&gt;\"")
                );
                assert!(!html.contains("かん") && !html.contains(">note<"));
                assert!(html.contains("Caption") && html.contains("Tail"));
                assert_eq!(result.output.article.foreign.len(), ids.len() + 2);
                for record in &result.output.sentences {
                    assert_eq!(
                        result.output.article.foreign[record.placement].embed,
                        selection.owners()[record.owner.0].embed()
                    );
                    assert!(
                        record
                            .origins
                            .iter()
                            .all(|origin| origin.element < markup.fragment.nodes.len() as u64)
                    );
                }
                if mode == SvgMode::External {
                    assert!(html.contains("src=\"assets/"));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn planned_svg_host_root_inputs_do_not_resolve_visible_or_hidden_guest_assets() -> Result<(), String>
{
    use nepl3_doc_core::labels::namespace::MemberId;
    let compiled = compiled()?;
    for source in [
        r#"article en "Title" body cons image asset "logo" none "root" none cons paragraph cons Sentence sentence cons document image asset "logo" none "guest" nil nil nil"#,
        r#"article en "Title" body cons image asset "logo" none "root" none cons paragraph cons parallel cons variant en Sentence "visible" cons variant ja Sentence sentence cons document image asset "logo" none "guest" nil nil nil nil"#,
    ] {
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
            let options = RenderOptions {
                parallel: ParallelMode::Single {
                    language: "en".into(),
                    fallbacks: vec![],
                },
            };
            let plan = article::display::plan(&selection, &options, b).map_err(err)?;
            let mut callbacks = 0;
            let failure = match svg::render(
                &plan,
                &[SvgInput {
                    id: "logo",
                    svg: SVG,
                }],
                SvgMode::Embedded,
                &mut host,
                &mut |_, _, _, _| {
                    callbacks += 1;
                    Err::<HtmlRequest, String>("unexpected callback".into())
                },
                b,
            ) {
                Ok(_) => return Err("root input crossed namespace owner".into()),
                Err(error) => error,
            };
            assert_eq!(callbacks, 0);
            let svg::Error::Preparation { location, error } = failure else {
                return Err(err(failure));
            };
            assert_eq!(location.member, Some(MemberId(1)));
            assert!(matches!(
                location.origin,
                Some(article::namespace::MemberOrigin::SentenceInline { .. })
            ));
            assert!(matches!(*error, svg::AssetFailure::Missing));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn planned_svg_host_failures_are_typed_owned_and_root_located() -> Result<(), String> {
    use nepl3_doc_core::labels::namespace::MemberId;
    let compiled = compiled()?;
    let source =
        r#"article en Sentence "Title" body cons image asset "logo" none Sentence "Alt" none nil"#;
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
        let options = RenderOptions {
            parallel: ParallelMode::Rows,
        };
        let plan = article::display::plan(&selection, &options, b).map_err(err)?;
        for case in 0..4 {
            let inputs = match case {
                0 => vec![],
                1 => vec![
                    SvgInput {
                        id: "logo",
                        svg: SVG,
                    },
                    SvgInput {
                        id: "logo",
                        svg: SVG,
                    },
                ],
                2 => vec![SvgInput {
                    id: "unused",
                    svg: SVG,
                }],
                _ => vec![SvgInput {
                    id: "logo",
                    svg: "<svg><script/></svg>",
                }],
            };
            let failure =
                match svg::render(&plan, &inputs, SvgMode::Embedded, &mut host, &mut other, b) {
                    Ok(_) => return Err("bad input accepted".into()),
                    Err(error) => error,
                };
            let svg::Error::Preparation { location, error } = failure else {
                return Err(err(failure));
            };
            assert_eq!(
                location,
                svg::Location {
                    member: Some(MemberId(0)),
                    origin: Some(article::namespace::MemberOrigin::Article)
                }
            );
            assert!(matches!(
                (case, *error),
                (0, svg::AssetFailure::Missing)
                    | (1, svg::AssetFailure::Duplicate)
                    | (2, svg::AssetFailure::Unused)
                    | (3, svg::AssetFailure::InvalidSvg)
            ));
        }
        Ok(())
    })
}

#[test]
fn planned_svg_host_preserves_guest_error_location_and_rejects_unresolved_alt() -> Result<(), String>
{
    use nepl3_doc_core::labels::namespace::MemberId;
    use nepl3_tools::doc::annotations::{alt, document};
    let compiled = compiled()?;
    for (source, alt_error) in [
        (
            r#"article en "Title" body cons image asset "logo" none "root" none cons paragraph cons parallel cons variant en Sentence "visible" cons variant ja Sentence sentence cons document link external "javascript:alert(1)" text "bad" nil nil nil nil"#,
            false,
        ),
        (
            r#"article en Sentence "Title" body cons image asset "logo" none Sentence sentence cons math frac 1 2 nil none nil"#,
            true,
        ),
    ] {
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
            let options = RenderOptions {
                parallel: ParallelMode::Single {
                    language: "en".into(),
                    fallbacks: vec![],
                },
            };
            let plan = article::display::plan(&selection, &options, b).map_err(err)?;
            let failure = match svg::render(
                &plan,
                &[SvgInput {
                    id: "logo",
                    svg: SVG,
                }],
                SvgMode::Embedded,
                &mut host,
                &mut other,
                b,
            ) {
                Ok(_) => return Err("unresolved content accepted".into()),
                Err(error) => error,
            };
            if alt_error {
                assert!(
                    matches!(failure, svg::Error::Alt(error) if matches!(*error, alt::Error::Text(_)))
                );
            } else {
                let svg::Error::Preparation { location, error } = failure else {
                    return Err(err(failure));
                };
                assert_eq!(location.member, Some(MemberId(1)));
                assert!(matches!(
                    location.origin,
                    Some(article::namespace::MemberOrigin::SentenceInline { .. })
                ));
                assert!(matches!(
                    *error,
                    svg::AssetFailure::Document(document::Error::InvalidExternalUri { .. })
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn planned_svg_host_checks_digest_and_stop_precedence_after_callbacks() -> Result<(), String> {
    let compiled = compiled()?;
    with_input_route(
        true,
        &compiled,
        SOURCE,
        "Article",
        |tree, profile, initial, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let mut root = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                initial,
                &mut codec,
            )
            .map_err(err)?;
            for wrong_digest in [false, true] {
                for node in &mut root.value.nodes {
                    if let DocKind::Image { asset, .. } | DocKind::InlineImage { asset, .. } =
                        &mut node.kind
                    {
                        asset.digest = wrong_digest.then_some(Digest::of(b"wrong"));
                    }
                }
                for callback_succeeds in [false, true] {
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &store, &mut admission)
                            .map_err(err)?;
                    let mut host = SentenceAnnotationRenderer {
                        registry: profile.registry(),
                        surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                        math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    let mut b = budget();
                    let selection =
                        article::collect_structural(&root, &mut host, &mut b).map_err(err)?;
                    let options = RenderOptions {
                        parallel: ParallelMode::Rows,
                    };
                    let plan = article::display::plan(&selection, &options, &mut b).map_err(err)?;
                    let failure = match svg::render(
                        &plan,
                        &[SvgInput {
                            id: "logo",
                            svg: SVG,
                        }],
                        SvgMode::Embedded,
                        &mut host,
                        &mut |view, embed, codec, b| {
                            b.cancel();
                            if callback_succeeds {
                                other(view, embed, codec, b)
                            } else {
                                Err("adapter failure after cancel".into())
                            }
                        },
                        &mut b,
                    ) {
                        Ok(_) => return Err("invalid operation succeeded".into()),
                        Err(error) => error,
                    };
                    if wrong_digest {
                        assert!(
                            matches!(failure, svg::Error::Preparation { error, .. } if matches!(*error, svg::AssetFailure::Digest))
                        );
                        assert!(b.poll().is_ok());
                    } else {
                        assert!(matches!(
                            failure,
                            svg::Error::Stopped(StopReason::Cancelled)
                        ));
                        assert_eq!(b.poll(), Err(StopReason::Cancelled));
                    }
                }
            }
            Ok(())
        },
    )
}

#[test]
fn planned_svg_host_shared_owner_stays_visible_and_sources_are_retained() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en Sentence "{[漢/かん]/note}" body cons image asset "logo" none Sentence "Retained" none cons paragraph cons Sentence "Tail" nil nil"#;
    with_input_route(true, &compiled, source, "Article", |tree, profile, b, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let mut root = nepl3_suite::adapters::sentence::literal::lower(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        let DocRoot::Article(article_root) = root.value.root else {
            return Err("root".into());
        };
        let DocKind::Article { title, .. } = root.value.nodes[article_root.0 as usize].kind else {
            return Err("article".into());
        };
        let mut previous_alt = None;
        for node in &mut root.value.nodes {
            if let DocKind::Image { alt, .. } = &mut node.kind {
                previous_alt = Some(*alt);
                *alt = title;
            }
        }
        let previous_alt = previous_alt.ok_or("alt")?;
        for node in &mut root.value.nodes {
            if let DocKind::Paragraph { items } = &mut node.kind {
                items.push(FlowRef(previous_alt.0));
                items.push(FlowRef(title.0));
            }
        }
        let mut host = SentenceAnnotationRenderer {
            registry: profile.registry(),
            surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
            math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
            doc_surface: Some(&compiled.doc.package.schema),
            codec: &mut codec,
        };
        let selection = article::collect_structural(&root, &mut host, b).map_err(err)?;
        assert_eq!(selection.sentences().len(), 5);
        assert_eq!(selection.owners().len(), 3);
        let options = RenderOptions {
            parallel: ParallelMode::Rows,
        };
        let plan = article::display::plan(&selection, &options, b).map_err(err)?;
        let result = svg::render(
            &plan,
            &[SvgInput {
                id: "logo",
                svg: SVG,
            }],
            SvgMode::Embedded,
            &mut host,
            &mut other,
            b,
        )
        .map_err(err)?;
        assert_eq!(
            result
                .output
                .sentences
                .iter()
                .map(|r| r.occurrence.0)
                .collect::<Vec<_>>(),
            vec![0, 2, 3, 4]
        );
        assert_eq!(
            result.output.sentences[0].owner,
            result.output.sentences[3].owner
        );
        assert_eq!(result.alt_projection.sentences.len(), 1);
        assert_eq!(result.alt_projection.resolved[0].text, "漢");
        assert!(!result.alt_projection.sentences[0].1.sources.is_empty());
        let m = &result.output.article.fragment.markup;
        let checked =
            nepl3_markup::html::validate(&m.fragment, m.slot, &m.policy, b).map_err(err)?;
        let html = nepl3_markup::html::serialize(&checked, b).map_err(err)?;
        assert!(html.contains("alt=\"漢\"") && html.contains("かん") && html.contains("Retained"));
        Ok(())
    })
}

#[test]
fn planned_svg_host_composed_budget_limits_are_exact_and_sticky() -> Result<(), String> {
    let compiled = compiled()?;
    with_input_route(
        true,
        &compiled,
        SOURCE,
        "Article",
        |tree, profile, initial, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let root = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                initial,
                &mut codec,
            )
            .map_err(err)?;
            let run = |b: &mut Budget| -> Result<(), String> {
                b.charge(Resource::Work, 11).map_err(err)?;
                b.charge(Resource::AllocationUnits, 17).map_err(err)?;
                b.charge(Resource::SourceBytes, 23).map_err(err)?;
                b.with_depth_at_least(5, |b| {
                    Ok::<_, StopReason>((|| {
                        let mut admission = SourceAdmission::default();
                        let mut codec =
                            FoundationCodec::new(profile.registry(), &store, &mut admission)
                                .map_err(err)?;
                        let mut host = SentenceAnnotationRenderer {
                            registry: profile.registry(),
                            surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                            math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                            doc_surface: Some(&compiled.doc.package.schema),
                            codec: &mut codec,
                        };
                        let selection =
                            article::collect_structural(&root, &mut host, b).map_err(err)?;
                        let options = RenderOptions {
                            parallel: ParallelMode::Single {
                                language: "en".into(),
                                fallbacks: vec![],
                            },
                        };
                        let plan = article::display::plan(&selection, &options, b).map_err(err)?;
                        svg::render(
                            &plan,
                            &[SvgInput {
                                id: "logo",
                                svg: SVG,
                            }],
                            SvgMode::Embedded,
                            &mut host,
                            &mut other,
                            b,
                        )
                        .map(|_| ())
                        .map_err(err)
                    })())
                })
                .map_err(err)?
            };
            let mut baseline = budget();
            run(&mut baseline)?;
            let used = baseline.usage();
            assert_eq!(baseline.current_depth(), 0);
            for dimension in 0..5 {
                let reason = [
                    StopReason::WorkLimit,
                    StopReason::AllocationLimit,
                    StopReason::SourceLimit,
                    StopReason::DepthLimit,
                    StopReason::NodeLimit,
                ][dimension];
                for short in [false, true] {
                    let less = u64::from(short);
                    let mut limits = baseline.limits();
                    match dimension {
                        0 => limits.work = used.work - less,
                        1 => limits.allocation_units = used.allocation_units - less,
                        2 => limits.source_bytes = used.source_bytes - less,
                        3 => limits.depth = used.depth - less,
                        _ => limits.nodes = used.nodes - less,
                    }
                    let mut b = Budget::new(limits);
                    let result = run(&mut b);
                    if short {
                        assert!(result.is_err());
                        assert_eq!(b.poll(), Err(reason));
                    } else {
                        result?;
                    }
                    assert_eq!(b.current_depth(), 0);
                }
            }
            Ok(())
        },
    )
}
