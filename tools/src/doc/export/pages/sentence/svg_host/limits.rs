use super::*;

#[test]
fn planned_pages_svg_operation_limits_are_exact_in_all_display_modes() -> Result<(), String> {
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
            let result = (|| -> Result<Vec<html::HtmlRequest>, String> {
                b.charge(Resource::Work, 19).map_err(err)?;
                b.with_depth(|b| {
                    Ok::<_, StopReason>((|| -> Result<Vec<html::HtmlRequest>, String> {
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
                        let output = planned::render(
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
                        Ok(output
                            .pages
                            .fragments
                            .into_iter()
                            .map(|f| f.fragment.markup)
                            .collect())
                    })())
                })
                .map_err(err)?
            })();
            (result, b)
        };
        let (baseline, full) = execute(crate::doc::source::budget().limits());
        let baseline = baseline?;
        let used = full.usage();
        for dimension in 0..6 {
            for short in [false, true] {
                let mut limits = full.limits();
                let less = u64::from(short);
                let expected = match dimension {
                    0 => {
                        limits.work = used.work - less;
                        StopReason::WorkLimit
                    }
                    1 => {
                        limits.allocation_units = used.allocation_units - less;
                        StopReason::AllocationLimit
                    }
                    2 => {
                        limits.nodes = used.nodes - less;
                        StopReason::NodeLimit
                    }
                    3 => {
                        limits.depth = used.depth - less;
                        StopReason::DepthLimit
                    }
                    4 => {
                        limits.output_bytes = used.output_bytes - less;
                        StopReason::OutputLimit
                    }
                    _ => {
                        limits.source_bytes = used.source_bytes - less;
                        StopReason::SourceLimit
                    }
                };
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
