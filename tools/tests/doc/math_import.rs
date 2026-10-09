//! Default-profile negative/Code tests require neither Node nor KaTeX assets.
use super::*;
use nepl3_doc_core::{check::Category, lower};
use nepl3_doc_html::{ParallelMode, RenderOptions, guests};
use nepl3_markup::html::{HtmlFragment, HtmlNode, HtmlPolicy, HtmlRequest, HtmlSlot};
use nepl3_tools::doc::math::{document, occurrence};

#[test]
fn closed_math_import_preserves_limits_stops_and_adapter_errors() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "article en \"Math\" body cons display Math x nil",
        "Article",
        |tree, profile, b, _| {
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
            let prepared =
                guests::prepare(&doc, &options, profile.registry(), &mut codec, b).map_err(err)?;
            for mode in 0..5 {
                let mut limits = budget().limits();
                if mode == 2 {
                    limits.work = 0;
                }
                let mut operation = Budget::new(limits);
                if mode == 1 {
                    operation.cancel();
                }
                let mut called = 0;
                let result = document::render(
                    &prepared,
                    if mode == 0 { 0 } else { 1 },
                    &mut |_, b| {
                        called += 1;
                        if mode == 3 {
                            b.cancel();
                        }
                        Err::<occurrence::Composed<'_, '_, '_>, String>("adapter failure".into())
                    },
                    &mut |_, _| Err::<HtmlRequest, String>("unexpected Code".into()),
                    &mut operation,
                );
                match mode {
                    0 => assert!(matches!(
                        result,
                        Err(document::Error::OccurrenceLimit { max: 0 })
                    )),
                    1 | 3 => assert!(matches!(
                        result,
                        Err(document::Error::Stopped(StopReason::Cancelled))
                    )),
                    2 => assert!(matches!(
                        result,
                        Err(document::Error::Stopped(StopReason::WorkLimit))
                    )),
                    _ => assert!(
                        matches!(result, Err(document::Error::Adapter(e)) if e == "adapter failure")
                    ),
                }
                assert_eq!(called, usize::from(mode >= 3));
            }
            Ok(())
        },
    )
}
#[test]
fn closed_math_import_keeps_code_ordinals_and_reserves_math_classes() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "article en \"Code\" body cons code Math x nil",
        "Article",
        |tree, profile, b, _| {
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
            let prepared =
                guests::prepare(&doc, &options, profile.registry(), &mut codec, b).map_err(err)?;
            for reserved in [false, true] {
                let mut math_calls = 0;
                let mut code_calls = 0;
                let result = document::render(
                    &prepared,
                    0,
                    &mut |_, _| {
                        math_calls += 1;
                        Err::<occurrence::Composed<'_, '_, '_>, String>("unexpected Math".into())
                    },
                    &mut |context, _| {
                        code_calls += 1;
                        assert_eq!(context.ordinal(), 0);
                        assert!(core::ptr::eq(context.document(), &doc));
                        // Caller-owned fixture text; the renderer meters its own import.
                        Ok::<_, String>(HtmlRequest {
                            fragment: HtmlFragment {
                                root: 0,
                                nodes: vec![HtmlNode::Text { text: "x".into() }],
                            },
                            slot: HtmlSlot::Phrasing,
                            policy: HtmlPolicy {
                                classes: if reserved {
                                    vec!["nepl-math-injected".into()]
                                } else {
                                    vec![]
                                },
                            },
                        })
                    },
                    &mut budget(),
                );
                assert_eq!(math_calls, 0);
                assert_eq!(code_calls, 1);
                if reserved {
                    assert!(matches!(result, Err(document::Error::ReservedClass)));
                } else {
                    let result = result.map_err(err)?;
                    assert!(result.math().is_empty());
                    assert_eq!(result.rendered().foreign.len(), 1);
                    assert!(core::ptr::eq(result.prepared(), &prepared));
                }
            }
            Ok(())
        },
    )
}

/// The actual package assets are admitted, but MathML-only must never invoke Node.
#[test]
#[ignore = "requires fixed Math packages"]
fn closed_import_checks_native_owners_and_mixed_guest_ordinals() -> Result<(), String> {
    use nepl3_tools::doc::math::{
        MathDisplayHost,
        display::{Preference, generation::Setup, process::Config},
        occurrence::Composition,
    };
    use std::{path::Path, time::Duration};
    let assets = super::math_process::fixed_assets()?;
    let config = Config {
        node: Path::new("unused-node"),
        bridge: Path::new("unused-bridge"),
        modules_url: "file:///unused/",
        input_cap: 4096,
        output_cap: 1000000,
        timeout: Duration::from_secs(10),
    };
    let compiled = compiled()?;
    with_input(
        &compiled,
        "article en \"Mixed\" body cons code Math x cons display Math x cons code Math y cons display Math y nil",
        "Article",
        |tree, profile, b, _| {
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
            let blocks = doc
                .value
                .nodes
                .iter_mut()
                .find_map(|n| match &mut n.kind {
                    nepl3_doc_core::model::DocKind::Body { blocks } => Some(blocks),
                    _ => None,
                })
                .ok_or("body")?;
            // Repeat exactly the same node/embed at two different guest ordinals.
            blocks.push(blocks[1]);
            let other_doc = doc.clone();
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let other_options = options.clone();
            let prepared =
                guests::prepare(&doc, &options, profile.registry(), &mut codec, b).map_err(err)?;
            let wrong_doc =
                guests::prepare(&other_doc, &options, profile.registry(), &mut codec, b)
                    .map_err(err)?;
            let wrong_options =
                guests::prepare(&doc, &other_options, profile.registry(), &mut codec, b)
                    .map_err(err)?;
            let mut baseline = None;
            for mode in 0..12 {
                let original = match mode {
                    1 => &wrong_doc,
                    2 => &wrong_options,
                    _ => &prepared,
                };
                let mut records = Vec::new();
                // Prepare genuine bound outputs independently so the test can deliberately
                // substitute an equal-content owner or another occurrence. This input
                // preparation is not counted as cumulative production rendering.
                guests::render_with_context(
                    original,
                    &mut |context, b| {
                        if context.embed().kind == nepl3_doc_core::model::EmbedKind::Code {
                            return Ok::<_, String>(code_fixture());
                        }
                        let mut host = MathDisplayHost {
                            registry: profile.registry(),
                            math_surface: &compiled.others[0].schema,
                            sentence_surface: Some(&compiled.others[3].schema),
                            doc_surface: Some(&compiled.doc.package.schema),
                            codec: &mut codec,
                        };
                        let generated =
                            occurrence::generate(
                                context,
                                &mut host,
                                Preference::MathMLOnly,
                                Setup {
                                    controls: super::math::request_controls(),
                                    request_cap: 4096,
                                    config,
                                    assets: &assets,
                                    scope: "same-inert-scope",
                                },
                                |_, _, _| {
                                    Err::<
                                        nepl3_tools::doc::math::display::process::reply::Reply<'_>,
                                        _,
                                    >("unexpected Node")
                                },
                                b,
                            )
                            .map_err(err)?;
                        let Composition::Ready(composed) =
                            generated.into_composite(b).map_err(err)?
                        else {
                            return Err("MathML-only deferred".into());
                        };
                        let fixture = composed.math().math().markup.clone();
                        records.push(composed);
                        Ok(fixture)
                    },
                    &mut budget(),
                )
                .map_err(err)?;
                assert_eq!(records.len(), 3);
                assert_eq!(records[0].context().node(), records[2].context().node());
                assert_eq!(
                    records[0].context().reference(),
                    records[2].context().reference()
                );
                assert_ne!(
                    records[0].context().ordinal(),
                    records[2].context().ordinal()
                );
                if mode != 3 {
                    records.reverse();
                }
                let mut code_ordinals = Vec::new();
                let mut limits = budget().limits();
                if mode >= 4 {
                    let usage: nepl3_core::budget::Usage = baseline.ok_or("baseline")?;
                    limits.work = usage.work - u64::from(mode == 5);
                    limits.allocation_units = usage.allocation_units - u64::from(mode == 6);
                    limits.nodes = usage.nodes - u64::from(mode == 7);
                    // This stage builds typed markup and performs no serialization.
                    assert_eq!(usage.output_bytes, 0);
                    limits.output_bytes = 0;
                    if mode >= 10 {
                        limits.depth = usage.depth + 5 - u64::from(mode == 11);
                    }
                }
                let mut operation = Budget::new(limits);
                let mut render = |operation: &mut Budget| {
                    document::render(
                        &prepared,
                        3,
                        &mut |_, b| {
                            if mode == 8 {
                                b.charge(
                                    Resource::AllocationUnits,
                                    b.limits().allocation_units - b.usage().allocation_units,
                                )
                                .map_err(err)?;
                            }
                            if mode == 9 {
                                b.cancel();
                            }
                            records
                                .pop()
                                .ok_or_else(|| "missing prepared Math".to_owned())
                        },
                        &mut |context, _| {
                            code_ordinals.push(context.ordinal());
                            Ok(code_fixture())
                        },
                        operation,
                    )
                };
                let result = if mode >= 10 {
                    operation.with_depth_at_least(5, &mut render)
                } else {
                    render(&mut operation)
                };
                if mode == 0 {
                    baseline = Some(operation.usage());
                }
                if matches!(mode, 5..=9 | 11) {
                    let expected = match mode {
                        5 => StopReason::WorkLimit,
                        6 => StopReason::AllocationLimit,
                        7 => StopReason::NodeLimit,
                        8 => StopReason::AllocationLimit,
                        9 => StopReason::Cancelled,
                        _ => StopReason::DepthLimit,
                    };
                    assert!(matches!(result, Err(document::Error::Stopped(s)) if s == expected));
                    assert_eq!(operation.poll(), Err(expected));
                    // A one-short Work limit reaches final metadata remapping;
                    // all guest callbacks completed before this terminal stop.
                    if mode == 5 {
                        assert!(records.is_empty());
                        assert_eq!(code_ordinals, [0, 2]);
                    }
                    if mode == 8 {
                        assert_eq!(records.len(), 2);
                        assert_eq!(code_ordinals, [0]);
                    }
                } else if matches!(mode, 1..=3) {
                    assert!(matches!(result, Err(document::Error::Association)));
                    assert_eq!(code_ordinals, [0]);
                } else {
                    let result = result.map_err(err)?;
                    assert_eq!(code_ordinals, [0, 2]);
                    if mode == 0 {
                        super::math_bundle::check_bundle(&result)?;
                    }
                    assert_eq!(result.rendered().foreign.len(), 5);
                    for (item, ordinal) in result.math().iter().zip([1, 3, 4]) {
                        assert_eq!(item.context().ordinal(), ordinal);
                        assert_eq!(
                            item.placement(),
                            Some(result.rendered().foreign[ordinal as usize])
                        );
                        let placement = item.placement().ok_or("placement")?;
                        assert!(
                            item.source()
                                .node_roots
                                .iter()
                                .all(|n| *n >= placement.first_element
                                    && *n < placement.first_element + placement.elements)
                        );
                        // Equal scopes on inert MathML-only outputs are harmless.
                        assert!(item.assets().is_none());
                    }
                }
            }
            Ok(())
        },
    )
}
fn code_fixture() -> HtmlRequest {
    HtmlRequest {
        fragment: HtmlFragment {
            root: 0,
            nodes: vec![HtmlNode::Text {
                text: "code".into(),
            }],
        },
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes: vec![] },
    }
}
