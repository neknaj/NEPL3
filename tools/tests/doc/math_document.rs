//! Real producer/Doc-import interoperability, not a production artifact adapter.
use super::*;
use nepl3_doc_core::{
    check::Category,
    lower,
    model::{BlockRef, DocKind},
};
use nepl3_doc_html::{ParallelMode, RenderOptions, guests};
use nepl3_markup::html::{self, HtmlAttribute, HtmlNode, HtmlTag};
use nepl3_markup::mathml::{Attribute as MathAttribute, Display};
use nepl3_tools::doc::math::occurrence::{self, Composition};
use nepl3_tools::doc::math::{
    MathDisplayHost,
    display::{
        Preference, TexPreparation,
        generation::{Setup, composite::Representation},
        process::Config,
    },
};
use std::{path::Path, time::Duration};

#[test]
#[ignore = "requires pinned Node and fixed Math packages"]
fn real_math_composites_import_into_their_doc_occurrences() -> Result<(), String> {
    let node = std::env::var("NEPL3_TEST_NODE").unwrap_or_else(|_| "node".into());
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = tools.join("audit/math/node_modules");
    let url = std::process::Command::new(&node)
        .args([
            "-p",
            "require('node:url').pathToFileURL(process.argv[1]+require('node:path').sep).href",
        ])
        .arg(&root)
        .output()
        .map_err(err)?;
    assert!(url.status.success());
    let url = String::from_utf8(url.stdout).map_err(err)?;
    let bridge = tools.join("math/katex/node/stdio.mjs");
    let config = Config {
        node: Path::new(&node),
        bridge: &bridge,
        modules_url: url.trim(),
        input_cap: 4096,
        output_cap: 1000000,
        timeout: Duration::from_secs(10),
    };
    let assets = super::math_process::fixed_assets()?;
    let compiled = nepl3_tools::doc::source::compiled_with_sentence_forms(&[
        nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "doc",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "closed Doc import mapping regression",
        },
        nepl3_grammar_core::compile::package::ForeignForm {
            kind: "InlineMath",
            category: "Inline",
            spelling: "math",
            field: "syntax",
            alias: "Math",
            guest_category: "Expr",
            origin_reason: "closed Doc nested Math regression",
        },
    ])?;
    let input = r#"article en "Composite" body cons paragraph cons sentence cons math Math add x y nil nil cons display Math sqrt x cons display Math label x Sentence sentence cons ruby text "字" text "じ" cons math label y Sentence "inner" cons doc anchor target math Math label z Sentence "doc inner" cons doc ref target text "go" nil nil"#;
    let cleanup_fixture = super::math_bundle::cleanup_failure_bridge(&bridge)?;
    for native in [false, true] {
        // The portable-parser route deliberately injects failure metadata into
        // an actual producer reply. Native parsing retains the ordinary route.
        // Neither path is claimed to induce a real worker cleanup failure.
        let config = if native {
            config
        } else {
            Config {
                bridge: cleanup_fixture.path(),
                ..config
            }
        };
        with_input_route(
            native,
            &compiled,
            input,
            "Article",
            |tree, profile, b, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                let mut document = lower::document(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let repeated = document
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
                    .ok_or("display")? as u64;
                let body = document
                    .value
                    .nodes
                    .iter_mut()
                    .find_map(|n| match &mut n.kind {
                        DocKind::Body { blocks } => Some(blocks),
                        _ => None,
                    })
                    .ok_or("body")?;
                body.push(BlockRef(repeated));
                let options = RenderOptions {
                    parallel: ParallelMode::Rows,
                };
                let prepared =
                    guests::prepare(&document, &options, profile.registry(), &mut codec, b)
                        .map_err(err)?;
                let mut records = Vec::new();
                let mut driver_calls = 0;
                let rendered = guests::render_with_context(
                    &prepared,
                    &mut |context, b| {
                        assert!(core::ptr::eq(context.document(), &document));
                        let scope = format!("nepl-math-doc-{}", context.ordinal());
                        let mut host = MathDisplayHost {
                            registry: profile.registry(),
                            math_surface: &compiled.others[0].schema,
                            sentence_surface: Some(&compiled.others[3].schema),
                            doc_surface: Some(&compiled.doc.package.schema),
                            codec: &mut codec,
                        };
                        let generated = occurrence::generate(
                            context,
                            &mut host,
                            Preference::KaTeXPreferred,
                            Setup {
                                controls: super::math::request_controls(),
                                request_cap: 4096,
                                config,
                                assets: &assets,
                                scope: &scope,
                            },
                            |request, config, b| {
                                driver_calls += 1;
                                super::math_process::owned_driver(request, config, b)
                            },
                            b,
                        )
                        .map_err(err)?;
                        drop(scope);
                        let Composition::Ready(composed) =
                            generated.into_composite(b).map_err(err)?
                        else {
                            return Err("deferred generation is not import success".to_owned());
                        };
                        assert_eq!(composed.context().node(), composed.math().doc_node());
                        // Test-only transfer: this clone/record collection is intentionally
                        // outside a production accounting claim. A real adapter must move
                        // or meter markup and retain/remap these records transactionally.
                        let request = composed.math().math().markup.clone();
                        records.push(composed);
                        Ok::<_, String>(request)
                    },
                    b,
                )
                .map_err(err)?;
                // The closed production import path moves markup and metadata.
                // Snapshots below are test expectations only, not transport.
                let mut source_buffers = Vec::new();
                let mut closed_calls = 0;
                let mut supervisor =
                    nepl3_tools::doc::math::display::process::supervisor::Supervisor::default();
                let mut closed_budget = budget();
                let closed = nepl3_tools::doc::math::document::render(
                    &prepared,
                    4,
                    &mut |context, b| {
                        let scope = format!("nepl-math-doc-{}", context.ordinal());
                        let mut host = MathDisplayHost {
                            registry: profile.registry(),
                            math_surface: &compiled.others[0].schema,
                            sentence_surface: Some(&compiled.others[3].schema),
                            doc_surface: Some(&compiled.doc.package.schema),
                            codec: &mut codec,
                        };
                        let generated = occurrence::generate(
                            context,
                            &mut host,
                            Preference::KaTeXPreferred,
                            Setup {
                                controls: super::math::request_controls(),
                                request_cap: 4096,
                                config,
                                assets: &assets,
                                scope: &scope,
                            },
                            |request, config, b| {
                                closed_calls += 1;
                                supervisor.run(request, config, b, &mut || {
                                    std::thread::sleep(Duration::from_millis(2));
                                    nepl3_tools::doc::math::display::process::supervisor::RunControl::Continue
                                }).map_err(err)
                            },
                            b,
                        )
                        .map_err(err)?;
                        let Composition::Ready(composed) =
                            generated.into_composite(b).map_err(err)?
                        else {
                            return Err("closed import received deferred".to_owned());
                        };
                        source_buffers.push((
                            composed.math().math().syntax.value.nodes.as_ptr(),
                            composed.math().math().annotations.as_ptr(),
                            composed.math().selected_scope().as_ptr(),
                            composed.math().stylesheet().as_ptr(),
                        ));
                        Ok(composed)
                    },
                    &mut |_, _| Err::<html::HtmlRequest, String>("unexpected Code".into()),
                    &mut closed_budget,
                )
                .map_err(err)?;
                assert!(!supervisor.has_pending_cleanup());
                let termination = closed.check_math_termination(&mut closed_budget);
                if native {
                    termination.map_err(err)?;
                } else {
                    assert!(matches!(
                        termination,
                        Err(nepl3_tools::doc::math::document::TerminationError::Failed {
                            occurrence: 0
                        })
                    ));
                }
                super::math_bundle::check_bundle(&closed)?;
                super::math_bundle::check_html_parser(&closed, &node)?;
                let bundle = closed.materialize_bundle(&mut budget()).map_err(err)?;
                assert!(core::ptr::eq(bundle.source(), &closed));
                for imported in bundle.source().math() {
                    if imported.assets().is_some() {
                        assert_eq!(imported.termination_failure(), Some(!native));
                        if !native {
                            let observation = imported
                                .observations()
                                .ok_or("cleanup fixture observations")?;
                            assert_eq!(observation.diagnostics.len(), 1);
                            assert_eq!(
                                observation.diagnostics[0].level,
                                nepl3_tools::doc::math::display::process::reply::Level::Warn
                            );
                            assert_eq!(
                                observation.diagnostics[0].text,
                                "bundle preserved cleanup diagnostic"
                            );
                            assert_eq!(
                                observation.diagnostic_bytes,
                                "bundle preserved cleanup diagnostic".len() as u64 + 1
                            );
                            assert!(observation.reply_bytes > 0);
                        }
                    } else {
                        assert_eq!(imported.termination_failure(), None);
                        assert!(imported.observations().is_none());
                    }
                }
                assert!(core::ptr::eq(closed.prepared(), &prepared));
                assert_eq!(closed_calls, 3);
                assert_eq!(closed.rendered().fragment, rendered.fragment);
                assert_eq!(closed.rendered().foreign, rendered.foreign);
                assert_eq!(closed.math().len(), records.len());
                for (i, (imported, expected)) in closed.math().iter().zip(&records).enumerate() {
                    let placement = imported.placement().ok_or("import placement")?;
                    assert_eq!(placement, rendered.foreign[i]);
                    assert!(core::ptr::eq(imported.context().document(), &document));
                    assert_eq!(imported.context().node(), expected.context().node());
                    assert_eq!(imported.context().ordinal(), expected.context().ordinal());
                    assert_eq!(
                        imported.source().syntax.value.nodes.as_ptr(),
                        source_buffers[i].0
                    );
                    assert_eq!(imported.source().annotations.as_ptr(), source_buffers[i].1);
                    assert_eq!(imported.scope().as_ptr(), source_buffers[i].2);
                    assert_eq!(imported.stylesheet().as_ptr(), source_buffers[i].3);
                    assert_eq!(
                        imported.source().node_roots,
                        expected
                            .math()
                            .math()
                            .node_roots
                            .iter()
                            .map(|n| n + placement.first_element)
                            .collect::<Vec<_>>()
                    );
                    assert!(
                        imported
                            .source()
                            .annotation_roots
                            .iter()
                            .map(|r| (r.node, r.markup))
                            .eq(expected
                                .math()
                                .math()
                                .annotation_roots
                                .iter()
                                .map(|r| (r.node, r.markup + placement.first_element)))
                    );
                    let mut expected_annotations = super::math_process::annotation_mappings(
                        &expected.math().math().annotations,
                    );
                    shift_mapping_values(&mut expected_annotations, placement.first_element)?;
                    assert_eq!(
                        super::math_process::annotation_mappings(&imported.source().annotations),
                        expected_annotations
                    );
                    assert_eq!(imported.scope(), expected.math().selected_scope());
                    assert_eq!(imported.stylesheet(), expected.math().stylesheet());
                    assert_eq!(imported.display(), expected.math().display());
                    assert_eq!(imported.tex(), expected.math().tex());
                    assert_eq!(imported.representation(), expected.math().representation());
                    assert_eq!(imported.selected_config(), Some(config));
                    assert_eq!(imported.controls(), Some(super::math::request_controls()));
                    assert_eq!(
                        imported.termination_failure(),
                        expected.math().termination_failure()
                    );
                    if let Some(old) = expected.math().generated() {
                        let new = imported.generated().ok_or("generated range")?;
                        assert_eq!(new.first, old.first + placement.first_element);
                        assert_eq!(new.elements, old.elements);
                        assert_eq!(new.visual_root, old.visual_root + placement.first_element);
                        assert_eq!(
                            new.accessible_root,
                            old.accessible_root + placement.first_element
                        );
                        assert!(core::ptr::eq(
                            imported.assets().ok_or("resources")?,
                            &assets
                        ));
                    } else {
                        assert!(imported.generated().is_none());
                        assert!(imported.assets().is_none());
                    }
                }
                let duplicate = nepl3_tools::doc::math::document::render(
                    &prepared,
                    4,
                    &mut |context, b| {
                        let mut host = MathDisplayHost {
                            registry: profile.registry(),
                            math_surface: &compiled.others[0].schema,
                            sentence_surface: Some(&compiled.others[3].schema),
                            doc_surface: Some(&compiled.doc.package.schema),
                            codec: &mut codec,
                        };
                        let generated = occurrence::generate(
                            context,
                            &mut host,
                            Preference::KaTeXPreferred,
                            Setup {
                                controls: super::math::request_controls(),
                                request_cap: 4096,
                                config,
                                assets: &assets,
                                scope: "nepl-math-duplicate",
                            },
                            super::math_process::owned_driver,
                            b,
                        )
                        .map_err(err)?;
                        let Composition::Ready(composed) =
                            generated.into_composite(b).map_err(err)?
                        else {
                            return Err("duplicate fixture deferred".to_owned());
                        };
                        Ok(composed)
                    },
                    &mut |_, _| Err::<html::HtmlRequest, String>("unexpected Code".into()),
                    &mut budget(),
                );
                assert!(matches!(
                    duplicate,
                    Err(nepl3_tools::doc::math::document::Error::DuplicateScope {
                        previous: 0,
                        current: 1
                    })
                ));
                // Bound failures keep their actual Doc owner, while parent stops
                // outrank either preparation or driver errors.
                for mode in 0..4 {
                    let mut operation = budget();
                    let mut calls = 0;
                    let result = guests::render_with_context(
                        &prepared,
                        &mut |context, b| {
                            let original_node = context.node();
                            let original_ordinal = context.ordinal();
                            if mode == 0 {
                                b.cancel();
                            }
                            if mode == 3 {
                                b.charge(Resource::Work, b.limits().work - b.usage().work)
                                    .map_err(err)?;
                            }
                            let mut host = MathDisplayHost {
                                registry: profile.registry(),
                                math_surface: &compiled.others[0].schema,
                                sentence_surface: Some(&compiled.others[3].schema),
                                doc_surface: Some(&compiled.doc.package.schema),
                                codec: &mut codec,
                            };
                            let result = occurrence::generate(
                                context,
                                &mut host,
                                Preference::KaTeXPreferred,
                                Setup {
                                    controls: super::math::request_controls(),
                                    request_cap: 4096,
                                    config,
                                    assets: &assets,
                                    scope: "nepl-math-failure",
                                },
                                |_, _, b| {
                                    calls += 1;
                                    if mode == 2 {
                                        b.cancel();
                                    }
                                    Err::<
                                        nepl3_tools::doc::math::display::process::reply::Reply<'_>,
                                        _,
                                    >(
                                        "intentional driver failure"
                                    )
                                },
                                b,
                            );
                            if mode == 1 {
                                let result = result.map_err(err)?;
                                assert!(core::ptr::eq(result.context().document(), &document));
                                assert_eq!(result.context().node(), original_node);
                                let Composition::Deferred(result) =
                                    result.into_composite(b).map_err(err)?
                                else {
                                    return Err("driver failure promoted".into());
                                };
                                assert!(core::ptr::eq(result.context().document(), &document));
                                assert_eq!(result.context().ordinal(), original_ordinal);
                                assert_eq!(
                                    result.generation().prepared().doc_node(),
                                    original_node
                                );
                                assert!(matches!(result.generation().attempt(), nepl3_tools::doc::math::display::generation::Attempt::DriverFailed("intentional driver failure")));
                            } else {
                                let reason = if mode == 3 {
                                    StopReason::WorkLimit
                                } else {
                                    StopReason::Cancelled
                                };
                                assert!(
                                    matches!(result, Err(occurrence::Error::Stopped(s)) if s == reason)
                                );
                            }
                            Err::<html::HtmlRequest, String>("checked failure".into())
                        },
                        &mut operation,
                    );
                    if mode == 1 {
                        assert!(
                            matches!(result, Err(nepl3_doc_html::ForeignRenderError::Foreign(e)) if e == "checked failure")
                        );
                    } else {
                        let reason = if mode == 3 {
                            StopReason::WorkLimit
                        } else {
                            StopReason::Cancelled
                        };
                        assert!(
                            matches!(result, Err(nepl3_doc_html::ForeignRenderError::Render(nepl3_doc_html::RenderError::Stopped(s))) if s == reason)
                        );
                    }
                    assert_eq!(calls, usize::from(mode == 1 || mode == 2));
                }
                assert_eq!(driver_calls, 3);
                assert_eq!(records.len(), 4);
                assert_eq!(records[1].context().node(), records[3].context().node());
                assert_eq!(
                    records[1].context().reference(),
                    records[3].context().reference()
                );
                assert_ne!(
                    records[1].math().selected_scope(),
                    records[3].math().selected_scope()
                );
                assert_eq!(rendered.foreign.len(), records.len());
                for (i, (bound, placement)) in records.iter().zip(&rendered.foreign).enumerate() {
                    let context = bound.context();
                    let composed = bound.math();
                    assert_eq!(context.ordinal(), i as u64);
                    assert_eq!(context.document_digest(), rendered.fragment.document_digest);
                    assert_eq!(context.reference(), placement.embed);
                    assert_eq!(composed.selected_scope(), format!("nepl-math-doc-{i}"));
                    assert_eq!(
                        placement.elements,
                        composed.math().markup.fragment.nodes.len() as u64
                    );
                    let final_nodes = &rendered.fragment.markup.fragment.nodes;
                    let imported_root =
                        placement.first_element + composed.math().markup.fragment.root;
                    let parents: Vec<_> = final_nodes
                        .iter()
                        .enumerate()
                        .filter(|(_, node)| children(node).contains(&imported_root))
                        .collect();
                    assert_eq!(parents.len(), 1);
                    let (parent_index, parent) = parents[0];
                    let parent_owner = rendered
                        .fragment
                        .origins
                        .iter()
                        .find(|o| o.element == parent_index as u64)
                        .ok_or("parent owner")?
                        .node;
                    let expected_display = if i == 0 {
                        Display::Inline
                    } else {
                        Display::Block
                    };
                    assert_eq!(composed.display(), expected_display);
                    if i == 0 {
                        assert!(
                            matches!(&document.value.nodes[parent_owner as usize].kind, DocKind::Sentence { inlines } if inlines.iter().any(|n| n.0 == context.node()))
                        );
                    } else {
                        assert!(matches!(
                            parent,
                            HtmlNode::Element {
                                tag: HtmlTag::Div,
                                ..
                            }
                        ));
                        assert_eq!(parent_index as u64 + 1, placement.first_element);
                        assert_eq!(parent_owner, context.node());
                    }
                    let math_root = match composed.generated() {
                        Some(range) => children(
                            &composed.math().markup.fragment.nodes[range.accessible_root as usize],
                        )[0],
                        None => composed.math().markup.fragment.root,
                    } + placement.first_element;
                    let HtmlNode::MathElement { attributes, .. } = &final_nodes[math_root as usize]
                    else {
                        return Err("MathML root".into());
                    };
                    assert!(attributes.contains(&MathAttribute::Display(expected_display)));
                    let mut paths = vec![(rendered.fragment.markup.fragment.root, false)];
                    let mut found = 0;
                    while let Some((index, hidden)) = paths.pop() {
                        let node = &final_nodes[index as usize];
                        let hidden = hidden || aria_hidden(node);
                        if index == math_root {
                            assert!(!hidden);
                            found += 1;
                        }
                        paths.extend(children(node).iter().map(|n| (*n, hidden)));
                    }
                    assert_eq!(found, 1);
                    if i == 1 || i == 3 {
                        assert!(
                            composed
                                .math()
                                .markup
                                .fragment
                                .nodes
                                .iter()
                                .any(|n| matches!(n, HtmlNode::SvgElement { .. }))
                        );
                    }
                    for (j, mut expected) in composed
                        .math()
                        .markup
                        .fragment
                        .nodes
                        .clone()
                        .into_iter()
                        .enumerate()
                    {
                        match &mut expected {
                            HtmlNode::Element { children, .. }
                            | HtmlNode::MathElement { children, .. }
                            | HtmlNode::SvgElement { children, .. } => {
                                for child in children {
                                    *child += placement.first_element;
                                }
                            }
                            HtmlNode::Text { .. } => {}
                        }
                        let index = placement.first_element + j as u64;
                        assert_eq!(
                            rendered.fragment.markup.fragment.nodes[index as usize],
                            expected
                        );
                        assert!(
                            rendered
                                .fragment
                                .origins
                                .iter()
                                .any(|o| o.element == index && o.node == context.node())
                        );
                    }
                    if i != 2 {
                        assert_eq!(composed.representation(), Representation::Dual);
                        assert!(core::ptr::eq(composed.assets().ok_or("assets")?, &assets));
                        assert!(composed.stylesheet().contains(&format!(
                            ".nepl-math-doc-{i} .nepl-math-doc-{i}-accessible"
                        )));
                    } else {
                        assert_eq!(composed.representation(), Representation::MathML);
                        assert!(matches!(
                            composed.tex(),
                            TexPreparation::Unsupported {
                                reason: nepl3_math_tex::Unsupported::ForeignAnnotation,
                                ..
                            }
                        ));
                        assert!(composed.assets().is_none());
                        assert!(composed.stylesheet().is_empty());
                        assert!(!composed.math().annotation_roots.is_empty());
                    }
                }
                let request = &rendered.fragment.markup;
                let checked = html::validate(&request.fragment, request.slot, &request.policy, b)
                    .map_err(err)?;
                let html = html::serialize(&checked, b).map_err(err)?;
                // Pinned KaTeX retains an inner hidden visual wrapper as well as
                // our outer one; verify all typed attributes instead of assuming
                // exactly one hidden attribute per Math occurrence.
                assert_eq!(
                    html.matches("aria-hidden=\"true\"").count(),
                    request
                        .fragment
                        .nodes
                        .iter()
                        .filter(|n| aria_hidden(n))
                        .count()
                );
                assert_eq!(html.matches("<math ").count(), 6);
                assert!(html.contains("<svg "));
                assert!(html.contains("<path "));
                // Sentence's established Ruby contract uses nepl-ruby/base/reading
                // spans rather than HTML ruby/rt elements.
                let ruby = request
                    .fragment
                    .nodes
                    .iter()
                    .position(|n| has_class(n, "nepl-ruby"))
                    .ok_or("ruby layout")?;
                let [base, reading] = children(&request.fragment.nodes[ruby]) else {
                    return Err("ruby children".into());
                };
                assert!(has_class(
                    &request.fragment.nodes[*base as usize],
                    "nepl-base"
                ));
                assert!(has_class(
                    &request.fragment.nodes[*reading as usize],
                    "nepl-reading"
                ));
                assert!(children(&request.fragment.nodes[*base as usize]).iter().any(|n| matches!(&request.fragment.nodes[*n as usize], HtmlNode::Text { text } if text == "字")));
                assert!(children(&request.fragment.nodes[*reading as usize]).iter().any(|n| matches!(&request.fragment.nodes[*n as usize], HtmlNode::Text { text } if text == "じ")));
                assert!(html.contains('字'));
                assert!(html.contains('じ'));
                assert!(!html.contains(" style="));
                assert!(!html.contains("<script"));
                Ok(())
            },
        )?;
    }
    with_input(
        &compiled,
        "article en \"Code\" body cons code Math x nil",
        "Article",
        |tree, profile, b, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let document = lower::document(
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
            let prepared = guests::prepare(&document, &options, profile.registry(), &mut codec, b)
                .map_err(err)?;
            let mut calls = 0;
            let result = guests::render_with_context(
                &prepared,
                &mut |context, b| {
                    assert_eq!(context.embed().kind, nepl3_doc_core::model::EmbedKind::Code);
                    let code_node = context.node();
                    let mut host = MathDisplayHost {
                        registry: profile.registry(),
                        math_surface: &compiled.others[0].schema,
                        sentence_surface: Some(&compiled.others[3].schema),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    let result = occurrence::generate(
                        context,
                        &mut host,
                        Preference::KaTeXPreferred,
                        Setup {
                            controls: super::math::request_controls(),
                            request_cap: 4096,
                            config,
                            assets: &assets,
                            scope: "nepl-math-code-reject",
                        },
                        |_, _, _| {
                            calls += 1;
                            Err::<nepl3_tools::doc::math::display::process::reply::Reply<'_>, _>(
                                "unexpected driver",
                            )
                        },
                        b,
                    );
                    assert!(matches!(
                        result,
                        Err(occurrence::Error::Prepare(
                            nepl3_tools::doc::math::Error::Node(n)
                        )) if n == code_node
                    ));
                    Err::<html::HtmlRequest, String>("non-Math context rejected".into())
                },
                b,
            );
            assert_eq!(calls, 0);
            assert!(
                matches!(result, Err(nepl3_doc_html::ForeignRenderError::Foreign(e)) if e == "non-Math context rejected")
            );
            Ok(())
        },
    )?;
    Ok(())
}

fn children(node: &HtmlNode) -> &[u64] {
    match node {
        HtmlNode::Element { children, .. }
        | HtmlNode::MathElement { children, .. }
        | HtmlNode::SvgElement { children, .. } => children,
        HtmlNode::Text { .. } => &[],
    }
}
fn aria_hidden(node: &HtmlNode) -> bool {
    matches!(node, HtmlNode::Element { attributes, .. } if attributes.contains(&HtmlAttribute::AriaHidden { value: true }))
}

fn has_class(node: &HtmlNode, class: &str) -> bool {
    matches!(node, HtmlNode::Element { attributes, .. } if attributes.iter().any(|a| matches!(a, HtmlAttribute::Class { values } if values.iter().any(|value| value == class))))
}

fn shift_mapping_values(value: &mut serde_json::Value, offset: u64) -> Result<(), String> {
    match value {
        serde_json::Value::Object(fields) => {
            for (name, value) in fields {
                if name == "roots" {
                    let roots = value.as_array_mut().ok_or("mapping roots array")?;
                    for root in roots {
                        let n = root.as_u64().ok_or("mapping root index")?;
                        *root = serde_json::json!(n.checked_add(offset).ok_or("mapping overflow")?);
                    }
                } else if name == "origins" || name == "annotationRoots" {
                    let pairs = value.as_array_mut().ok_or("mapping pairs array")?;
                    for pair in pairs {
                        let pair = pair.as_array_mut().ok_or("mapping pair")?;
                        if pair.len() != 2 {
                            return Err("mapping pair length".into());
                        }
                        let n = pair[1].as_u64().ok_or("mapping target index")?;
                        pair[1] =
                            serde_json::json!(n.checked_add(offset).ok_or("mapping overflow")?);
                    }
                } else {
                    shift_mapping_values(value, offset)?;
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                shift_mapping_values(value, offset)?;
            }
        }
        _ => {}
    }
    Ok(())
}

struct HoldingBridge(std::path::PathBuf);
impl HoldingBridge {
    fn new() -> Result<Self, String> {
        use std::io::Write;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(err)?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("nepl-doc-held-{}-{stamp}.cjs", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(err)?;
        let fixture = Self(path);
        // It cannot finish before interruption, even if Node runs before the first poll.
        file.write_all(b"process.stdin.resume();setInterval(()=>{},1000);")
            .map_err(err)?;
        Ok(fixture)
    }
}
impl Drop for HoldingBridge {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
#[ignore = "requires pinned Node and fixed Math packages"]
fn supervised_native_mixed_document_keeps_occurrences_and_resources() -> Result<(), String> {
    use nepl3_tools::doc::math::display::process::supervisor::{RunControl, Supervisor};
    let node = std::env::var("NEPL3_TEST_NODE").unwrap_or_else(|_| "node".into());
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = tools.join("audit/math/node_modules");
    let url = std::process::Command::new(&node)
        .args([
            "-p",
            "require('node:url').pathToFileURL(process.argv[1]+require('node:path').sep).href",
        ])
        .arg(&root)
        .output()
        .map_err(err)?;
    assert!(url.status.success());
    let url = String::from_utf8(url.stdout).map_err(err)?;
    let bridge = tools.join("math/katex/node/stdio.mjs");
    let config = Config {
        node: Path::new(&node),
        bridge: &bridge,
        modules_url: url.trim(),
        input_cap: 4096,
        output_cap: 1000000,
        timeout: Duration::from_secs(10),
    };
    let holding = HoldingBridge::new()?;
    let assets = super::math_process::fixed_assets()?;
    let compiled = compiled()?;
    let input = r#"article en "Mixed native" body cons code Math x cons paragraph cons sentence cons math Math add x y nil nil cons display Math sqrt y nil"#;
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
        let repeated = doc
            .value
            .nodes
            .iter()
            .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
            .ok_or("display")? as u64;
        let body = doc
            .value
            .nodes
            .iter_mut()
            .find_map(|n| match &mut n.kind {
                DocKind::Body { blocks } => Some(blocks),
                _ => None,
            })
            .ok_or("body")?;
        body.push(BlockRef(repeated));
        let options = RenderOptions {
            parallel: ParallelMode::Rows,
        };
        let prepared =
            guests::prepare(&doc, &options, profile.registry(), &mut codec, b).map_err(err)?;
        for mode in 0..3 {
            let config = if mode == 0 {
                config
            } else {
                Config {
                    bridge: &holding.0,
                    ..config
                }
            };
            // Each independent document attempt has one execution-to-bundle ledger.
            let mut operation = budget();
            let mut supervisor = Supervisor::default();
            let mut math_ordinals = Vec::new();
            let mut code_ordinals = Vec::new();
            let result = nepl3_tools::doc::math::document::render(
                &prepared,
                3,
                &mut |context, b| {
                    math_ordinals.push(context.ordinal());
                    let scope = format!("nepl-math-native-{}", context.ordinal());
                    let mut host = MathDisplayHost {
                        registry: profile.registry(),
                        math_surface: &compiled.others[0].schema,
                        sentence_surface: Some(&compiled.others[3].schema),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    let generation = occurrence::generate(
                        context,
                        &mut host,
                        Preference::KaTeXPreferred,
                        Setup {
                            controls: super::math::request_controls(),
                            request_cap: 4096,
                            config,
                            assets: &assets,
                            scope: &scope,
                        },
                        |request, config, b| {
                            supervisor.run(request, config, b, &mut || {
                                std::thread::sleep(Duration::from_millis(2));
                                match mode {
                                    1 => RunControl::Abort,
                                    2 => RunControl::CancelParent,
                                    _ => RunControl::Continue,
                                }
                            })
                        },
                        b,
                    )
                    .map_err(err)?;
                    let Composition::Ready(composed) = generation.into_composite(b).map_err(err)?
                    else {
                        return Err("native attempt was not composable".to_owned());
                    };
                    assert_eq!(composed.math().termination_failure(), Some(false));
                    Ok(composed)
                },
                &mut |context, _| {
                    code_ordinals.push(context.ordinal());
                    // Test-selected Code presentation, not execution or the CLI highlighter.
                    Ok::<_, String>(html::HtmlRequest {
                        fragment: html::HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Text { text: "x".into() }],
                        },
                        slot: html::HtmlSlot::Phrasing,
                        policy: html::HtmlPolicy { classes: vec![] },
                    })
                },
                &mut operation,
            );
            assert_eq!(code_ordinals, [0]);
            if mode != 0 {
                assert_eq!(math_ordinals, [1]);
                match result {
                    Err(nepl3_tools::doc::math::document::Error::Adapter(_)) if mode == 1 => {}
                    Err(nepl3_tools::doc::math::document::Error::Stopped(
                        StopReason::Cancelled,
                    )) if mode == 2 => {}
                    _ => return Err("interrupted Doc returned an unexpected result".into()),
                }
                assert!(supervisor.has_pending_cleanup());
                let usage = operation.usage();
                let until = std::time::Instant::now() + Duration::from_secs(10);
                while supervisor.has_pending_cleanup() {
                    let reclaimed = supervisor
                        .poll_cleanup()
                        .ok_or("missing interrupted Doc cleanup")?;
                    assert_eq!(
                        reclaimed.parent_stop,
                        (mode == 2).then_some(StopReason::Cancelled)
                    );
                    if std::time::Instant::now() >= until {
                        return Err("interrupted Doc cleanup watchdog".into());
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert_eq!(operation.usage(), usage);
                assert_eq!(
                    operation.poll(),
                    if mode == 2 {
                        Err(StopReason::Cancelled)
                    } else {
                        Ok(())
                    }
                );
                continue;
            }
            let closed = result.map_err(err)?;
            assert_eq!(math_ordinals, [1, 2, 3]);
            assert!(!supervisor.has_pending_cleanup());
            assert_eq!(closed.rendered().foreign.len(), 4);
            assert_eq!(closed.math().len(), 3);
            for (i, imported) in closed.math().iter().enumerate() {
                assert_eq!(imported.context().ordinal(), i as u64 + 1);
                assert_eq!(imported.placement(), Some(closed.rendered().foreign[i + 1]));
                assert_eq!(imported.representation(), Representation::Dual);
                assert_eq!(imported.selected_config(), Some(config));
                assert_eq!(imported.termination_failure(), Some(false));
            }
            assert_eq!(
                closed.math()[1].context().node(),
                closed.math()[2].context().node()
            );
            assert_ne!(closed.math()[1].scope(), closed.math()[2].scope());
            closed.check_math_termination(&mut operation).map_err(err)?;
            let bundle = closed.materialize_bundle(&mut operation).map_err(err)?;
            assert!(bundle.html().contains("assets/katex/katex.min.css"));
            assert!(!bundle.html().contains("<script"));
            assert_eq!(bundle.katex_assets().len(), 62);
            assert!(!bundle.math_stylesheet().is_empty());
        }
        Ok(())
    })
}
