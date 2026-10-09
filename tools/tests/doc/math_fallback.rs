//! Real missing-module fallback in one cumulative Doc output operation.
use super::*;
use nepl3_doc_core::{check::Category, lower};
use nepl3_doc_html::{ParallelMode, RenderOptions, guests};
use nepl3_markup::html::{HtmlFragment, HtmlNode, HtmlPolicy, HtmlRequest, HtmlSlot};
use nepl3_tools::doc::math::{
    self, MathDisplayHost,
    display::{
        Preference,
        generation::{
            Setup,
            composite::{CompletionPolicy, FallbackReason, Representation},
        },
        process::Config,
    },
    occurrence,
};
use nepl3_wire::foundation::FoundationCodec;
use std::{path::Path, time::Duration};
#[test]
#[ignore = "requires pinned Node and fixed Math packages"]
fn real_unavailable_fallback_preserves_context_through_bundle() -> Result<(), String> {
    let node = std::env::var("NEPL3_TEST_NODE").unwrap_or_else(|_| "node".into());
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let url = std::process::Command::new(&node)
        .args([
            "-p",
            "require('node:url').pathToFileURL(process.argv[1]+require('node:path').sep).href",
        ])
        .arg(tools.join("audit/math/node_modules"))
        .output()
        .map_err(err)?;
    assert!(url.status.success());
    let url = String::from_utf8(url.stdout).map_err(err)?;
    let missing = format!("{}missing/", url.trim());
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
    let c = compiled()?;
    with_input(
        &c,
        "article en \"Fallback\" body cons code Math 1 cons display Math frac 1 0 cons code Math 2 cons display Math add x y nil",
        "Article",
        |tree, profile, _, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let doc = lower::document(
                tree.syntax(),
                &c.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let mut output = budget();
            let prepared =
                guests::prepare(&doc, &options, profile.registry(), &mut codec, &mut output)
                    .map_err(err)?;
            let mut calls = 0;
            let mut codes = Vec::new();
            let mut original = Vec::new();
            let closed = math::document::render(
                &prepared,
                2,
                &mut |context, b| {
                    let selected = if context.ordinal() == 3 {
                        Config {
                            modules_url: &missing,
                            ..config
                        }
                    } else {
                        config
                    };
                    let scope = format!("nepl-math-fallback-{}", context.ordinal());
                    let mut host = MathDisplayHost {
                        registry: profile.registry(),
                        math_surface: &c.others[0].schema,
                        sentence_surface: Some(&c.others[3].schema),
                        doc_surface: Some(&c.doc.package.schema),
                        codec: &mut codec,
                    };
                    let generated = occurrence::generate(
                        context,
                        &mut host,
                        Preference::KaTeXPreferred,
                        Setup {
                            controls: super::math::request_controls(),
                            request_cap: 4096,
                            config: selected,
                            assets: &assets,
                            scope: &scope,
                        },
                        |request, config, b| {
                            calls += 1;
                            super::math_process::owned_driver(request, config, b)
                        },
                        b,
                    )
                    .map_err(err)?;
                    let occurrence::Composition::Ready(composed) = generated
                        .into_composite_with_policy(CompletionPolicy::OrdinaryMathmlFallback, b)
                        .map_err(err)?
                    else {
                        return Err("unexpected Deferred".into());
                    };
                    original.push(composed.math().math().syntax.value.nodes.as_ptr());
                    Ok(composed)
                },
                &mut |context, _| {
                    codes.push(context.ordinal());
                    // Code is a deliberately minimal valid test renderer; only its
                    // placement/ordinal coexistence with the real Math pipeline matters.
                    Ok::<_, String>(HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Text {
                                text: "code fixture".into(),
                            }],
                        },
                        slot: HtmlSlot::Phrasing,
                        policy: HtmlPolicy { classes: vec![] },
                    })
                },
                &mut output,
            )
            .map_err(err)?;
            assert_eq!(calls, 2);
            assert_eq!(codes, vec![0, 2]);
            assert_eq!(closed.math().len(), 2);
            assert_eq!(closed.math()[0].context().ordinal(), 1);
            assert_eq!(closed.math()[1].context().ordinal(), 3);
            assert_eq!(closed.math()[0].representation(), Representation::Dual);
            assert!(closed.math()[0].fallback().is_none());
            let fallback = &closed.math()[1];
            assert_eq!(fallback.representation(), Representation::MathML);
            let Some(FallbackReason::Unavailable(Some(cause))) = fallback.fallback() else {
                return Err("fallback cause".into());
            };
            assert_eq!(cause.code(), "module-file");
            assert!(cause.module().is_some());
            assert_eq!(
                fallback
                    .selected_config()
                    .ok_or("selected config")?
                    .modules_url,
                missing
            );
            assert_eq!(fallback.controls(), Some(super::math::request_controls()));
            assert_eq!(fallback.termination_failure(), Some(false));
            assert!(fallback.observations().is_some());
            assert!(fallback.generated().is_none());
            assert!(fallback.stylesheet().is_empty());
            assert!(fallback.assets().is_none());
            for (i, imported) in closed.math().iter().enumerate() {
                assert_eq!(imported.source().syntax.value.nodes.as_ptr(), original[i]);
                let placement = imported.placement().ok_or("placement")?;
                assert!(
                    imported
                        .source()
                        .node_roots
                        .iter()
                        .all(|r| *r >= placement.first_element)
                );
            }
            let before = output.usage();
            let bundle = closed.materialize_bundle(&mut output).map_err(err)?;
            assert!(output.usage().work > before.work);
            assert!(core::ptr::eq(bundle.source(), &closed));
            assert!(bundle.source().math()[1].fallback().is_some());
            assert!(bundle.html().contains("<math"));
            assert_eq!(calls, 2);
            output.cancel();
            assert!(closed.materialize_bundle(&mut output).is_err());
            assert_eq!(output.poll(), Err(StopReason::Cancelled));
            Ok(())
        },
    )
}
