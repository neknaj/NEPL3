use super::*;
use crate::doc::export::pages::svg as package;
use std::collections::BTreeMap;

#[test]
fn external_packaged_svg_operation_limits_include_shell_and_receipt() -> Result<(), String> {
    let c = compiled()?;
    let (set, codes) = fixture(&c, ONE, TWO, true)?;
    let a = [SvgInput { id: "logo", svg: A }];
    let bb = [SvgInput { id: "logo", svg: B }];
    let inputs = [
        planned::RootAssets {
            page: 0,
            inputs: &a,
        },
        planned::RootAssets {
            page: 1,
            inputs: &bb,
        },
    ];
    for parallel in [
        ParallelMode::Rows,
        ParallelMode::Columns,
        ParallelMode::Single {
            language: "fr".into(),
            fallbacks: vec!["ja".into()],
        },
    ] {
        let options = RenderOptions { parallel };
        let execute = |limits| {
            let mut b = Budget::new(limits);
            let result = (|| -> Result<BTreeMap<String, Vec<u8>>, String> {
                b.charge(Resource::Work, 19).map_err(err)?;
                b.with_depth(|b| {
                    Ok::<_, StopReason>((|| -> Result<BTreeMap<String, Vec<u8>>, String> {
                        let store = SourceStore::default();
                        let mut admission = SourceAdmission::default();
                        let mut codec =
                            FoundationCodec::new(&c.doc.registry, &store, &mut admission)
                                .map_err(err)?;
                        let mut host = host(&c, &mut codec)?;
                        let selections = set
                            .pages
                            .iter()
                            .map(|p| {
                                article::collect_structural(&p.document, &mut host, b).map_err(err)
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let plans = selections
                            .iter()
                            .map(|s| article::display::plan(s, &options, b).map_err(err))
                            .collect::<Result<Vec<_>, _>>()?;
                        let mut maths = [Vec::new(), Vec::new()];
                        let output = package::render_external(
                            &set,
                            &options,
                            &plans,
                            &inputs,
                            &mut host,
                            &mut |page, view, embed, codec, b| {
                                let page = usize::try_from(page).map_err(err)?;
                                if view.kind == EmbedKind::Code {
                                    codes[page].render_with_codec(view, embed, codec, b)
                                } else {
                                    crate::doc::export::math::DisplayHost {
                                        compiled: &c,
                                        registry: &c.doc.registry,
                                        codec,
                                        preference: crate::doc::export::MathRenderer::MathMLOnly,
                                    }
                                    .render(
                                        view,
                                        embed,
                                        &mut maths[page],
                                        b,
                                    )
                                }
                            },
                            b,
                        )
                        .map_err(err)?;
                        Ok(output.files)
                    })())
                })
                .map_err(err)?
            })();
            (result, b)
        };
        let (baseline, full) = execute(crate::doc::source::budget().limits());
        let baseline = baseline?;
        for dimension in 0..6 {
            let amount = |u: nepl3_core::budget::Usage| match dimension {
                0 => u.work,
                1 => u.allocation_units,
                2 => u.nodes,
                3 => u.depth,
                4 => u.output_bytes,
                _ => u.source_bytes,
            };
            let select = |limits: &mut nepl3_core::budget::Limits, n| match dimension {
                0 => limits.work = n,
                1 => limits.allocation_units = n,
                2 => limits.nodes = n,
                3 => limits.depth = n,
                4 => limits.output_bytes = n,
                _ => limits.source_bytes = n,
            };
            // The receipt records the selected cap. Reach the exact fixed point
            // after its decimal representation shrinks from the default cap.
            let mut cap = amount(full.usage());
            let mut settled = false;
            for _ in 0..32 {
                let mut limits = full.limits();
                select(&mut limits, cap);
                let (result, measured) = execute(limits);
                assert_eq!(result?, baseline);
                let next = amount(measured.usage());
                assert!(next <= cap);
                if next == cap {
                    settled = true;
                    break;
                }
                cap = next;
            }
            assert!(settled, "receipt budget fixed point");
            let expected = match dimension {
                0 => StopReason::WorkLimit,
                1 => StopReason::AllocationLimit,
                2 => StopReason::NodeLimit,
                3 => StopReason::DepthLimit,
                4 => StopReason::OutputLimit,
                _ => StopReason::SourceLimit,
            };
            for short in [false, true] {
                let mut limits = full.limits();
                select(&mut limits, cap - u64::from(short));
                let (result, mut b) = execute(limits);
                if short {
                    assert!(result.is_err(), "dimension {dimension}");
                    assert_eq!(b.poll(), Err(expected));
                    let before = b.usage();
                    assert_eq!(b.charge(Resource::Work, 1), Err(expected));
                    assert_eq!(b.usage(), before);
                } else {
                    assert_eq!(result?, baseline);
                }
            }
        }
    }
    Ok(())
}
