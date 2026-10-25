use super::*;
use crate::doc::{
    annotations::{alt, article},
    source::{compiled, err, with_input_route},
};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::check::Category;
use nepl3_doc_html::{
    assets::{SvgInput, SvgMode},
    namespace::assets as namespace_assets,
};
use nepl3_markup::html::*;
use nepl3_wire::foundation::FoundationCodec;
const SVG: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const SOURCE: &str = r#"article en Sentence sentence cons document anchor title text "Title" nil body
cons image asset "logo" none Sentence "Alt" some Sentence sentence cons document anchor cap text "Caption" nil
cons paragraph cons parallel cons variant en Sentence sentence cons document anchor enlabel text "English" nil cons variant ja Sentence sentence cons document anchor jalabel text "Japanese" nil nil
cons code Math frac 3 4 cons sentence cons math Math frac 1 2 nil
cons sentence cons image asset "logo" none Sentence "Inline alt" nil
cons Sentence sentence cons document ref cap text "Tail" nil nil nil"#;

fn compose(source: &str, mode: ParallelMode, mismatch: u8) -> Result<Vec<usize>, String> {
    let compiled = compiled()?;
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
        let options = RenderOptions { parallel: mode };
        let plan = article::display::plan(&selection, &options, b).map_err(err)?;
        let blocked = render_planned(
            &plan,
            &mut host,
            &mut |_, _, _, _| Err::<HtmlRequest, String>("unexpected asset-free callback".into()),
            b,
        );
        assert!(matches!(blocked, Err(Error::Document { .. })));
        let projected = alt::prepare(&root, &mut host, b).map_err(err)?;
        let svg = [SvgInput {
            id: "logo",
            svg: SVG,
        }];
        let assets = [namespace_assets::MemberAssets {
            member: MemberId(0),
            inputs: &svg,
            resolved_alt: &projected.resolved,
        }];
        let (_, ids) = namespace::resolve_with(
            plan.selection(),
            plan.adoption(),
            &mut host,
            b,
            |checked, members, host, b| {
                let prepared = namespace_assets::prepare(
                    checked,
                    plan.options(),
                    &assets,
                    SvgMode::Embedded,
                    host.registry,
                    host.codec,
                    b,
                )
                .map_err(err)?;
                let mut session = Session::for_plan::<(), String>(&plan, b).map_err(err)?;
                if mismatch == 1 {
                    session.selected.as_mut().ok_or("selected")?.pop();
                }
                let mut other = |view: &DocEmbedView<'_>,
                                 _: EmbedRef,
                                 _: &mut FoundationCodec<'_>,
                                 _: &mut Budget| {
                    assert!(matches!(view.kind, EmbedKind::Code | EmbedKind::InlineMath));
                    Ok::<_, String>(HtmlRequest {
                        slot: HtmlSlot::Phrasing,
                        policy: HtmlPolicy { classes: vec![] },
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Text {
                                text: "OTHER".into(),
                            }],
                        },
                    })
                };
                let mut adapter = |view: &DocEmbedView<'_>, embed: EmbedRef, b: &mut Budget| {
                    session.render_guest(&prepared, members, host, &mut other, view, embed, b)
                };
                let rendered = display::render_article(&prepared, MemberId(0), &mut adapter, b)
                    .map_err(err)?;
                if mismatch == 2 {
                    session.next_member = 1;
                }
                let (rendered, records) = session
                    .finish::<(), String>(rendered, members, b)
                    .map_err(err)?;
                let checked_html = nepl3_markup::html::validate(
                    &rendered.fragment.markup.fragment,
                    rendered.fragment.markup.slot,
                    &rendered.fragment.markup.policy,
                    b,
                )
                .map_err(err)?;
                let html = nepl3_markup::html::serialize(&checked_html, b).map_err(err)?;
                assert!(html.contains("alt=\"Alt\"") && html.contains("alt=\"Inline alt\""));
                assert!(html.contains("Caption"));
                assert_eq!(rendered.foreign.len(), records.len() + 2);
                for record in &records {
                    let actual = &rendered.foreign[record.placement];
                    assert_eq!(actual.embed, selection.owners()[record.owner.0].embed());
                    assert!(!record.origins.is_empty());
                    assert!(record.origins.iter().all(|origin| origin.element
                        < rendered.fragment.markup.fragment.nodes.len() as u64));
                }
                Ok::<_, String>(records.iter().map(|r| r.occurrence.0).collect::<Vec<_>>())
            },
        )
        .map_err(err)?;
        Ok(ids)
    })
}
#[test]
fn role_session_consumes_non_alt_gaps_and_keeps_caption_and_foreign_placements()
-> Result<(), String> {
    assert_eq!(compose(SOURCE, ParallelMode::Rows, 0)?, vec![0, 2, 3, 4, 6]);
    assert_eq!(
        compose(SOURCE, ParallelMode::Columns, 0)?,
        vec![0, 2, 3, 4, 6]
    );
    assert_eq!(
        compose(
            SOURCE,
            ParallelMode::Single {
                language: "en".into(),
                fallbacks: vec![]
            },
            0
        )?,
        vec![0, 2, 3, 6]
    );
    assert_eq!(
        compose(
            SOURCE,
            ParallelMode::Single {
                language: "fr".into(),
                fallbacks: vec!["ja".into()]
            },
            0
        )?,
        vec![0, 2, 4, 6]
    );
    assert!(compose(SOURCE, ParallelMode::Rows, 1).is_err());
    let hidden = SOURCE.replace("ref cap", "ref jalabel");
    compose(&hidden, ParallelMode::Rows, 0)?;
    let failure = compose(
        &hidden,
        ParallelMode::Single {
            language: "en".into(),
            fallbacks: vec![],
        },
        0,
    )
    .err()
    .ok_or("hidden target was emitted")?;
    assert!(failure.contains("MissingFragment"), "{failure}");
    for mode in [ParallelMode::Rows, ParallelMode::Columns] {
        let failure = compose(SOURCE, mode, 2)
            .err()
            .ok_or("unconsumed member accepted")?;
        assert!(failure.contains("Selection"), "{failure}");
    }
    let trailing = SOURCE.replace(
        "cons Sentence sentence cons document ref cap text \"Tail\" nil ",
        "",
    );
    assert_eq!(
        compose(
            &trailing,
            ParallelMode::Single {
                language: "en".into(),
                fallbacks: vec![]
            },
            0
        )?,
        vec![0, 2, 3]
    );
    Ok(())
}

#[test]
fn role_session_planned_pipeline_uses_cumulative_limits_and_owned_failures() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en Sentence "Title" body cons paragraph cons parallel cons variant en Sentence sentence cons document anchor english text "English" nil cons variant ja Sentence sentence cons document anchor japanese text "Japanese" nil nil cons Sentence sentence cons document ref english text "Tail" nil nil nil"#;
    with_input_route(
        true,
        &compiled,
        source,
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
            let run = |b: &mut Budget, language: &str| -> Result<(), String> {
                b.charge(Resource::Work, 7).map_err(err)?;
                b.charge(Resource::AllocationUnits, 11).map_err(err)?;
                b.charge(Resource::SourceBytes, 13).map_err(err)?;
                b.with_depth_at_least(3, |b| {
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
                                language: language.into(),
                                fallbacks: vec![],
                            },
                        };
                        let plan = article::display::plan(&selection, &options, b).map_err(err)?;
                        let result = render_planned(
                            &plan,
                            &mut host,
                            &mut |_, _, _, _| {
                                Err::<HtmlRequest, String>("unexpected callback".into())
                            },
                            b,
                        );
                        // The error is owned and can be formatted after the operation's
                        // checked namespace and preparation storage have been dropped.
                        result.map(|_| ()).map_err(err)
                    })())
                })
                .map_err(err)?
            };
            let mut baseline = crate::doc::source::budget();
            run(&mut baseline, "en")?;
            let usage = baseline.usage();
            assert_eq!(baseline.current_depth(), 0);
            for dimension in 0..5 {
                let expected = [
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
                        0 => limits.work = usage.work - less,
                        1 => limits.allocation_units = usage.allocation_units - less,
                        2 => limits.source_bytes = usage.source_bytes - less,
                        3 => limits.depth = usage.depth - less,
                        _ => limits.nodes = usage.nodes - less,
                    }
                    let mut b = Budget::new(limits);
                    let result = run(&mut b, "en");
                    if short {
                        assert!(result.is_err());
                        assert_eq!(b.poll(), Err(expected));
                    } else {
                        result?;
                    }
                    assert_eq!(b.current_depth(), 0);
                }
            }
            let failure = run(&mut crate::doc::source::budget(), "ja")
                .err()
                .ok_or("hidden anchor output accepted")?;
            assert!(failure.contains("MissingFragment"), "{failure}");
            let mut cancelled = crate::doc::source::budget();
            cancelled.cancel();
            assert!(run(&mut cancelled, "en").is_err());
            assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
            Ok(())
        },
    )
}
