use super::*;

type HostError = planned::Error<nepl3_wire::WireError, String>;
#[derive(Clone, Copy, Eq, PartialEq)]
enum Attack {
    Normal,
    MissingPlan,
    SwappedPlans,
    ClonedRoot,
    ClonedOptions,
    CancelCallback,
    WrongCode,
}
#[allow(clippy::too_many_arguments)]
fn capture_error(
    c: &Compiled,
    set: &PageSet,
    codes: &[code::PreparedPageCode],
    options: &RenderOptions,
    inputs: &[planned::RootAssets<'_>],
    attack: Attack,
    limits: nepl3_core::budget::Limits,
) -> Result<(HostError, usize, Budget), String> {
    let mut b = Budget::new(limits);
    b.charge(Resource::Work, 7).map_err(err)?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut host = host(c, &mut codec)?;
    let selections = set
        .pages
        .iter()
        .map(|p| article::collect_structural(&p.document, &mut host, &mut b).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let mut plans = selections
        .iter()
        .map(|s| article::display::plan(s, options, &mut b).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    if attack == Attack::MissingPlan {
        plans.pop();
    }
    if attack == Attack::SwappedPlans {
        plans.swap(0, 1);
    }
    let cloned_set = set.clone();
    let cloned_options = options.clone();
    let mut calls = 0;
    let result = planned::render(
        if attack == Attack::ClonedRoot {
            &cloned_set
        } else {
            set
        },
        if attack == Attack::ClonedOptions {
            &cloned_options
        } else {
            options
        },
        &plans,
        inputs,
        &mut host,
        &mut |page, view, embed, codec, b| {
            calls += 1;
            if attack == Attack::CancelCallback {
                b.cancel();
                return Err("callback error must not hide cancellation".into());
            }
            if attack == Attack::WrongCode && view.kind == EmbedKind::Code {
                return codes[1 - usize::try_from(page).map_err(err)?]
                    .render_with_codec(view, embed, codec, b);
            }
            // Controlled display callback for rejection tests only. Actual Code
            // and Math adapters are exercised by the positive test above.
            Ok(html::HtmlRequest {
                slot: html::HtmlSlot::Phrasing,
                policy: html::HtmlPolicy { classes: vec![] },
                fragment: html::HtmlFragment {
                    root: 0,
                    nodes: vec![HtmlNode::Text {
                        text: "callback".into(),
                    }],
                },
            })
        },
        &mut b,
    );
    let error = match result {
        Ok(_) => return Err("unexpected complete result".into()),
        Err(error) => error,
    };
    Ok((error, calls, b))
}
#[test]
fn planned_pages_svg_rejects_scope_and_root_asset_misbinding_before_callbacks() -> Result<(), String>
{
    let c = compiled()?;
    let (set, codes) = fixture(&c, ONE, TWO, true)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
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
    for attack in [
        Attack::MissingPlan,
        Attack::SwappedPlans,
        Attack::ClonedRoot,
        Attack::ClonedOptions,
    ] {
        let (error, calls, _) = capture_error(
            &c,
            &set,
            &codes,
            &options,
            &inputs,
            attack,
            crate::doc::source::budget().limits(),
        )?;
        assert_eq!(calls, 0);
        assert!(match attack {
            Attack::MissingPlan => matches!(error, planned::Error::PlanCount),
            Attack::ClonedOptions => matches!(error, planned::Error::OptionScope { page: 0 }),
            _ => matches!(error, planned::Error::RootScope { page: 0 }),
        });
    }
    for (inputs, duplicate) in [
        (
            vec![planned::RootAssets {
                page: 2,
                inputs: &a,
            }],
            false,
        ),
        (
            vec![
                planned::RootAssets {
                    page: 0,
                    inputs: &a,
                },
                planned::RootAssets {
                    page: 0,
                    inputs: &bb,
                },
            ],
            true,
        ),
    ] {
        let (error, calls, _) = capture_error(
            &c,
            &set,
            &codes,
            &options,
            &inputs,
            Attack::Normal,
            crate::doc::source::budget().limits(),
        )?;
        assert_eq!(calls, 0);
        assert!(if duplicate {
            matches!(error, planned::Error::DuplicateInputPage(0))
        } else {
            matches!(error, planned::Error::InvalidInputPage(2))
        });
    }
    let (error, calls, _) = capture_error(
        &c,
        &set,
        &codes,
        &options,
        &inputs[..1],
        Attack::Normal,
        crate::doc::source::budget().limits(),
    )?;
    assert_eq!(calls, 0);
    let planned::Error::Page { page, error } = error else {
        return Err("page-owned missing SVG".into());
    };
    assert_eq!(page, 1);
    let article::render::svg::Error::Preparation { location, error } = *error else {
        return Err("owned preparation".into());
    };
    assert_eq!(
        location.member,
        Some(nepl3_doc_core::labels::namespace::MemberId(0))
    );
    assert!(matches!(
        *error,
        article::render::svg::AssetFailure::Missing
    ));
    Ok(())
}
#[test]
fn planned_pages_svg_validates_hidden_assets_labels_and_emitted_targets() -> Result<(), String> {
    let c = compiled()?;
    let options = RenderOptions {
        parallel: ParallelMode::Single {
            language: "en".into(),
            fallbacks: vec![],
        },
    };
    let a = [SvgInput { id: "logo", svg: A }];
    let inputs = [
        planned::RootAssets {
            page: 0,
            inputs: &a,
        },
        planned::RootAssets {
            page: 1,
            inputs: &a,
        },
    ];
    let hidden_asset=TWO.replace("cons code Math",r#"cons parallel cons variant en Sentence "Shown" cons variant ja Sentence sentence cons document image asset "logo" none "Hidden" nil nil cons code Math"#);
    let (set, codes) = fixture(&c, ONE, &hidden_asset, true)?;
    let (error, calls, _) = capture_error(
        &c,
        &set,
        &codes,
        &options,
        &inputs,
        Attack::Normal,
        crate::doc::source::budget().limits(),
    )?;
    assert_eq!(calls, 0);
    let planned::Error::Page { page, error } = error else {
        return Err("hidden asset page".into());
    };
    assert_eq!(page, 1);
    let article::render::svg::Error::Preparation { location, error } = *error else {
        return Err("hidden asset preparation".into());
    };
    assert!(matches!(
        *error,
        article::render::svg::AssetFailure::Missing
    ));
    assert!(matches!(
        location.origin,
        Some(article::namespace::MemberOrigin::SentenceInline { .. })
    ));
    let duplicate = ONE.replace("anchor jalabel", "anchor enlabel");
    let (set, codes) = fixture(&c, &duplicate, TWO, true)?;
    let (error, calls, _) = capture_error(
        &c,
        &set,
        &codes,
        &options,
        &inputs,
        Attack::Normal,
        crate::doc::source::budget().limits(),
    )?;
    assert_eq!(calls, 0);
    let planned::Error::Page { page, error } = error else {
        return Err("hidden duplicate page".into());
    };
    assert_eq!(page, 0);
    let article::render::svg::Error::Resolve(error) = *error else {
        return Err("hidden namespace".into());
    };
    assert!(matches!(*error, article::namespace::Error::Label(_)));
    let hidden_target=TWO.replace("anchor guest text \"Target\"","anchor visible text \"Visible\"").replace("cons code Math",r#"cons parallel cons variant en Sentence "Shown" cons variant ja Sentence sentence cons document anchor guest text "Hidden" nil nil cons code Math"#);
    let (set, codes) = fixture(&c, ONE, &hidden_target, true)?;
    let (error, calls, _) = capture_error(
        &c,
        &set,
        &codes,
        &options,
        &inputs,
        Attack::Normal,
        crate::doc::source::budget().limits(),
    )?;
    assert!(calls > 0);
    assert!(
        matches!(error,planned::Error::Render(error) if matches!(*error,nepl3_doc_html::namespace::pages::RenderError::MissingAnchor {page:0,target:1,..}))
    );
    Ok(())
}
#[test]
fn planned_pages_svg_keeps_stops_and_owned_diagnostics_after_scope_exit() -> Result<(), String> {
    let c = compiled()?;
    let (set, codes) = fixture(&c, ONE, TWO, true)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let a = [SvgInput { id: "logo", svg: A }];
    let inputs = [
        planned::RootAssets {
            page: 0,
            inputs: &a,
        },
        planned::RootAssets {
            page: 1,
            inputs: &a,
        },
    ];
    let (error, calls, b) = capture_error(
        &c,
        &set,
        &codes,
        &options,
        &inputs,
        Attack::CancelCallback,
        crate::doc::source::budget().limits(),
    )?;
    assert_eq!(calls, 1);
    assert!(matches!(
        error,
        planned::Error::Stopped(StopReason::Cancelled)
    ));
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    let (error, calls, _) = capture_error(
        &c,
        &set,
        &codes,
        &options,
        &inputs,
        Attack::WrongCode,
        crate::doc::source::budget().limits(),
    )?;
    assert_eq!(calls, 1);
    // A different page may lack this contextual EmbedRef entirely, or bind
    // that number to a different closed input. Both reject before rendering.
    let planned::Error::Render(error) = error else {
        return Err("page callback error".into());
    };
    let nepl3_doc_html::namespace::pages::RenderError::Page {
        page: 0,
        error: article::render::Error::Adapter(message),
    } = *error
    else {
        return Err("owned adapter error".into());
    };
    assert!(
        message == "CodeInputMissing" || message == "CodeInputMismatch",
        "{message}"
    );
    let duplicate = ONE.replace("anchor jalabel", "anchor enlabel");
    let (set, codes) = fixture(&c, &duplicate, TWO, true)?;
    let (error, calls, full) = capture_error(
        &c,
        &set,
        &codes,
        &options,
        &inputs,
        Attack::Normal,
        crate::doc::source::budget().limits(),
    )?;
    assert_eq!(calls, 0);
    assert!(matches!(error, planned::Error::Page { page: 0, .. }));
    // Error owns its diagnostic after capture_error drops every temporary
    // resolution/member/namespace, while allocation exhaustion remains sticky.
    assert!(format!("{error:?}").contains("Label"));
    let mut limits = full.limits();
    limits.allocation_units = full.usage().allocation_units - 1;
    let (error, _, mut stopped) =
        capture_error(&c, &set, &codes, &options, &inputs, Attack::Normal, limits)?;
    assert!(matches!(
        error,
        planned::Error::Stopped(StopReason::AllocationLimit)
    ));
    let usage = stopped.usage();
    assert_eq!(
        stopped.charge(Resource::Work, 1),
        Err(StopReason::AllocationLimit)
    );
    assert_eq!(stopped.usage(), usage);
    Ok(())
}

#[test]
fn planned_pages_svg_owns_nonzero_page_routing_failures() -> Result<(), String> {
    let c = compiled()?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let a = [SvgInput { id: "logo", svg: A }];
    let inputs = [
        planned::RootAssets {
            page: 0,
            inputs: &a,
        },
        planned::RootAssets {
            page: 1,
            inputs: &a,
        },
    ];
    for missing_page in [true, false] {
        let two = if missing_page {
            TWO.replace(
                "link relative \"../one.nepld\" some \"cap\"",
                "link page \"missing\" some \"cap\"",
            )
        } else {
            TWO.replace("some \"cap\"", "some \"absent\"")
        };
        let (set, codes) = fixture(&c, ONE, &two, true)?;
        let (error, calls, _) = capture_error(
            &c,
            &set,
            &codes,
            &options,
            &inputs,
            Attack::Normal,
            crate::doc::source::budget().limits(),
        )?;
        assert_eq!(calls, 0);
        let planned::Error::Routing(error) = error else {
            return Err("owned routing failure".into());
        };
        let origin = if missing_page {
            let planned::RoutingFailure::Destination {
                member,
                origin,
                error,
            } = *error
            else {
                return Err("destination failure".into());
            };
            assert_eq!(member, nepl3_doc_core::labels::namespace::MemberId(3));
            assert!(matches!(
                *error,
                nepl3_doc_core::pages::PageError::MissingPage { page: 1, .. }
            ));
            origin.ok_or("guest origin")?
        } else {
            let planned::RoutingFailure::MissingFragment {
                site,
                target,
                origin,
            } = *error
            else {
                return Err("semantic fragment failure".into());
            };
            // The retained Inline Link has its Text label at node0 and
            // the Link itself at node1 in the typed postorder arena.
            assert_eq!(
                (site.page, site.member, site.node, target),
                (1, nepl3_doc_core::labels::namespace::MemberId(3), 1, 0)
            );
            origin
        };
        assert!(matches!(
            origin,
            article::namespace::MemberOrigin::SentenceInline {
                sentence: article::namespace::SentenceOccurrenceId(1),
                owner: nepl3_suite::adapters::sentence::document_guests::OwnerId(1),
                ..
            }
        ));
    }
    Ok(())
}
