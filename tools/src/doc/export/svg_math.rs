use super::*;
use nepl3_doc_core::model::{BlockRef, DocKind, EmbedKind};
use nepl3_doc_html::{
    ForeignRenderError, RenderError,
    assets::{self, AssetError, SvgInput, SvgMode},
};
const SVG: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='10pt' height='10pt' viewBox='0 0 10 10'><path d='M0 0L10 0L5 10Z' fill='none' stroke='#000'/></svg>";

#[test]
fn svg_math_native_policies_shared_occurrences_and_stops() -> Result<(), String> {
    use nepl3_core::budget::{Budget, StopReason};
    let compiled = compiled()?;
    for (case, input) in [
        (
            0,
            "article en \"Code\" body cons image asset \"a\" none \"image\" none cons code Math frac 1 0 nil",
        ),
        (
            1,
            "article en \"Math\" body cons image asset \"a\" none \"image\" none cons display Math frac 1 0 nil",
        ),
        (
            2,
            "article en \"Link\" body cons image asset \"a\" none \"image\" none cons paragraph cons sentence cons link external \"https://example.test/\" text \"x\" nil nil nil",
        ),
        (
            3,
            "article en \"Alt\" body cons image asset \"a\" none sentence cons math Math 7 nil none nil",
        ),
    ] {
        with_input_route(true, &compiled, input, "Article", |tree, profile, b, _| {
            let registry = profile.registry();
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
            let mut doc = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                registry,
                b,
                &mut codec,
            )
            .map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let inputs = [SvgInput { id: "a", svg: SVG }];
            if case == 2 {
                assert!(matches!(
                    assets::prepare_svg_guests(
                        &doc,
                        &options,
                        &inputs,
                        SvgMode::Embedded,
                        registry,
                        &mut codec,
                        b
                    ),
                    Err(AssetError::Unsupported)
                ));
                return Ok(());
            }
            if case == 3 {
                assert!(matches!(
                    assets::prepare_svg_guests(
                        &doc,
                        &options,
                        &inputs,
                        SvgMode::Embedded,
                        registry,
                        &mut codec,
                        b
                    ),
                    Err(AssetError::Alt)
                ));
                return Ok(());
            }
            assert!(matches!(
                assets::prepare_svg(
                    &doc,
                    &options,
                    &inputs,
                    SvgMode::Embedded,
                    registry,
                    &mut codec,
                    b
                ),
                Err(AssetError::Unsupported)
            ));
            let old_code = assets::prepare_svg_code(
                &doc,
                &options,
                &inputs,
                SvgMode::Embedded,
                registry,
                &mut codec,
                b,
            );
            if case == 0 {
                old_code.map_err(err)?;
            } else {
                assert!(matches!(old_code, Err(AssetError::Unsupported)));
            }
            if case == 1 {
                // Reuse the exact same Math and image nodes, not merely equal source text.
                let image = doc
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::Image { .. }))
                    .ok_or("image")?;
                let display = doc
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
                    .ok_or("display")?;
                let blocks = doc
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
                blocks.extend([BlockRef(image as u64), BlockRef(display as u64)]);
            }
            if case == 1 {
                let mut wrong = doc.clone();
                for node in &mut wrong.value.nodes {
                    if let DocKind::Image { asset, .. } = &mut node.kind {
                        asset.digest = Some(nepl3_core::source::Digest::of(b"wrong SVG"));
                    }
                }
                assert!(matches!(
                    assets::prepare_svg_guests(
                        &wrong,
                        &options,
                        &inputs,
                        SvgMode::Embedded,
                        registry,
                        &mut codec,
                        b
                    ),
                    Err(AssetError::Digest)
                ));
                let mut unsupported = doc.clone();
                let title = match unsupported.value.root {
                    nepl3_doc_core::model::DocRoot::Article(root) => {
                        match unsupported.value.nodes[root.0 as usize].kind {
                            DocKind::Article { title, .. } => title,
                            _ => return Err("article node".into()),
                        }
                    }
                    _ => return Err("article".into()),
                };
                let index = unsupported
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
                    .ok_or("display")?;
                let syntax = match unsupported.value.nodes[index].kind {
                    DocKind::DisplayMath { syntax } => syntax,
                    _ => return Err("display".into()),
                };
                unsupported.value.embeds[syntax.0 as usize].kind = EmbedKind::CircuitFigure;
                unsupported.value.nodes[index].kind = DocKind::CircuitFigure {
                    caption: title,
                    syntax,
                };
                assert!(matches!(
                    assets::prepare_svg_guests(
                        &unsupported,
                        &options,
                        &inputs,
                        SvgMode::Embedded,
                        registry,
                        &mut codec,
                        b
                    ),
                    Err(AssetError::Unsupported)
                ));
            }
            let modes = [SvgMode::Embedded, SvgMode::External];
            for mode in modes {
                for policy in [math::Renderer::MathmlOnly, math::Renderer::KatexPreferred] {
                    let mut limits = budget().limits();
                    limits.diagnostics = 0;
                    let mut measured = Budget::new(limits);
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                    let prepared = assets::prepare_svg_guests(
                        &doc,
                        &options,
                        &inputs,
                        mode,
                        registry,
                        &mut codec,
                        &mut measured,
                    )
                    .map_err(err)?;
                    let prepared_usage = measured.usage();
                    let mut report = math::Report::new(policy);
                    let mut ordinal = 0u64;
                    let result = assets::render_svg_guests(
                        &prepared,
                        &mut |guest, embed, b| {
                            let index = ordinal;
                            ordinal += 1;
                            if guest.kind == EmbedKind::Code {
                                return code::render(guest, tree, profile, b);
                            }
                            let mut host = crate::doc::math::MathDisplayHost {
                                registry,
                                math_surface: &compiled.others[0].schema,
                                sentence_surface: Some(&compiled.others[3].schema),
                                doc_surface: Some(&compiled.doc.package.schema),
                                codec: &mut codec,
                            };
                            report.render(guest, embed, index, &mut host, b)
                        },
                        &mut measured,
                    );
                    assert!(measured.usage().work >= prepared_usage.work);
                    assert!(measured.usage().source_bytes >= prepared_usage.source_bytes);
                    if case == 1 && policy == math::Renderer::KatexPreferred {
                        assert!(matches!(
                            result,
                            Err(ForeignRenderError::Render(RenderError::Stopped(
                                StopReason::DiagnosticLimit
                            )))
                        ));
                        assert_eq!(measured.poll(), Err(StopReason::DiagnosticLimit));
                    } else {
                        let rendered = result.map_err(err)?;
                        if case == 1 {
                            assert_eq!(rendered.foreign.len(), 2);
                            assert_eq!(rendered.foreign[0].embed, rendered.foreign[1].embed);
                            assert_ne!(
                                rendered.foreign[0].first_element,
                                rendered.foreign[1].first_element
                            );
                            assert_eq!(report.occurrences.len(), 2);
                            let markup = &rendered.fragment.markup;
                            let checked = nepl3_markup::html::validate(
                                &markup.fragment,
                                markup.slot,
                                &markup.policy,
                                &mut measured,
                            )
                            .map_err(err)?;
                            let html = nepl3_markup::html::serialize(&checked, &mut measured)
                                .map_err(err)?;
                            // Two shared block occurrences each keep their full-size disclosure.
                            assert_eq!(html.matches("<img ").count(), 4);
                            assert_eq!(html.matches("<math").count(), 2);
                        }
                    }
                }
            }
            // Stops during guest work cannot be turned into SVG/Math success.
            let mut measured = budget();
            let prepared = assets::prepare_svg_guests(
                &doc,
                &options,
                &inputs,
                SvgMode::Embedded,
                registry,
                &mut codec,
                &mut measured,
            )
            .map_err(err)?;
            let before = measured.usage();
            let result = assets::render_svg_guests(
                &prepared,
                &mut |_, _, b| {
                    b.cancel();
                    Err::<nepl3_markup::html::HtmlRequest, _>("renderer unavailable")
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
            assert!(measured.usage().work >= before.work);
            Ok(())
        })?;
    }
    Ok(())
}
