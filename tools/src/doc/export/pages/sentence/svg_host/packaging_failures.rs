use super::*;

use crate::doc::export::pages::svg as package;

type HostError = package::Error<nepl3_wire::WireError, String>;
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
    let result = package::render_embedded(
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
fn packaged_svg_rejects_collisions_before_returning_completion() -> Result<(), String> {
    let c = compiled()?;
    let one = "article en \"One\" body cons image asset \"logo\" none \"First\" none nil";
    let two = "article en \"Two\" body cons image asset \"logo\" none \"Second\" none nil";
    let (baseline, codes) = fixture(&c, one, two, false)?;
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
    let css = format!(
        "first/assets/doc-{}.css",
        crate::doc::export::digest_hex(Digest::of(b"first/index.html"))
    );
    for route in [
        css.as_str(),
        "manifest.json",
        "first/index.html",
        "first/assets",
    ] {
        let mut set = baseline.clone();
        set.files.push(nepl3_doc_core::pages::PageFile {
            registration: nepl3_doc_core::pages::PageRegistration {
                id: "file".into(),
                source: "data.bin".into(),
                route: route.into(),
            },
            content: nepl3_doc_core::pages::FileBytes(vec![1, 2, 3]),
        });
        let (error, _, b) = capture_error(
            &c,
            &set,
            &codes,
            &options,
            &inputs,
            Attack::Normal,
            crate::doc::source::budget().limits(),
        )?;
        assert!(!matches!(error, HostError::Stopped(_)), "{error:?}");
        assert_eq!(b.poll(), Ok(()));
    }
    let mut set = baseline;
    set.pages[0].registration.route = "first.txt".into();
    let (error, _, _) = capture_error(
        &c,
        &set,
        &codes,
        &options,
        &inputs,
        Attack::Normal,
        crate::doc::source::budget().limits(),
    )?;
    assert!(matches!(error, HostError::Packaging(_)));
    Ok(())
}

#[test]
fn packaged_svg_cancellation_and_scope_failures_cannot_return_files() -> Result<(), String> {
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
        Attack::WrongCode,
        Attack::CancelCallback,
    ] {
        let (error, calls, b) = capture_error(
            &c,
            &set,
            &codes,
            &options,
            &inputs,
            attack,
            crate::doc::source::budget().limits(),
        )?;
        if attack == Attack::CancelCallback {
            assert!(calls > 0);
            assert!(matches!(error, HostError::Stopped(StopReason::Cancelled)));
            assert_eq!(b.poll(), Err(StopReason::Cancelled));
        } else {
            assert!(matches!(error, HostError::Render(_)));
        }
    }
    Ok(())
}
