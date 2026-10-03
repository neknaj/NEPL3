use super::*;
#[test]
fn disabled_capture_preserves_measured_prechange_usage() -> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "lambda x x", |tree, profile, _, _| {
        for mode in [0, 1] {
            let mut b = budget();
            if mode == 0 {
                let r = analyze(
                    "baseline",
                    tree,
                    profile,
                    &mut b,
                    &mut SourceAdmission::default(),
                );
                assert!(matches!(r.outcome, BindingOutcome::Complete(_)));
            } else {
                let r = nepl3_engine::binding::trace::analyze(
                    "baseline",
                    tree,
                    profile,
                    None,
                    &mut b,
                    &mut SourceAdmission::default(),
                );
                assert!(r.complete().is_some());
            }
            assert_eq!(
                b.usage(),
                if mode == 0 {
                    expected(10, 2223, 4, 21, 5819, 0, 0)
                } else {
                    expected(10, 2340, 5, 21, 6369, 0, 0)
                }
            );
            let mut limits = budget().limits();
            limits.work = b.usage().work - 1;
            let mut short = Budget::new(limits);
            if mode == 0 {
                let r = analyze(
                    "baseline",
                    tree,
                    profile,
                    &mut short,
                    &mut SourceAdmission::default(),
                );
                assert!(matches!(
                    r.outcome,
                    BindingOutcome::Stopped {
                        reason: StopReason::WorkLimit,
                        ..
                    }
                ));
            } else {
                let r = nepl3_engine::binding::trace::analyze(
                    "baseline",
                    tree,
                    profile,
                    None,
                    &mut short,
                    &mut SourceAdmission::default(),
                );
                assert!(matches!(
                    r.reply().outcome,
                    BindingOutcome::Stopped {
                        reason: StopReason::WorkLimit,
                        ..
                    }
                ));
            }
            assert_eq!(
                short.usage(),
                if mode == 0 {
                    expected(10, 2155, 4, 21, 5819, 0, 0)
                } else {
                    expected(10, 2272, 5, 21, 6369, 0, 0)
                }
            );
        }
        Ok(())
    })?;
    let compiled = custom::compiled()?;
    with_input(&compiled, "early x z custom x x", |tree, profile, _, _| {
        let mut b = budget();
        let mut host = custom::query_host(true, false);
        let r = analyze_with_host(
            "baseline",
            tree,
            profile,
            &mut host,
            &mut b,
            &mut SourceAdmission::default(),
        );
        assert!(matches!(r.outcome, BindingOutcome::Complete(_)));
        assert_eq!(b.usage(), expected(20, 8854, 8, 98, 22039, 2, 1));
        let mut limits = budget().limits();
        limits.work = b.usage().work - 1;
        let mut short = Budget::new(limits);
        let mut host = custom::query_host(true, false);
        let r = analyze_with_host(
            "baseline",
            tree,
            profile,
            &mut host,
            &mut short,
            &mut SourceAdmission::default(),
        );
        assert!(matches!(
            r.outcome,
            BindingOutcome::Stopped {
                reason: StopReason::WorkLimit,
                ..
            }
        ));
        assert_eq!(short.usage(), expected(20, 8789, 8, 98, 22039, 2, 1));
        Ok(())
    })?;
    let compiled = missing_probe::named_lambda()?;
    with_source_profile(&compiled, None, "lambda x", |source, profile, b, a| {
        let parsed = parse_any_with_aux(source, &[], &[], profile, b, a)?;
        let ParseCompletion::Break(ParseReply {
            outcome: ParseOutcome::Recovered { tree, .. },
            ..
        }) = parsed
        else {
            return Err("recovered".into());
        };
        let checked = tree.validate(profile, b, a).map_err(err)?;
        let mut b = budget();
        let r = nepl3_engine::binding::probe::first_missing_reference(
            "baseline",
            &checked,
            profile,
            &mut b,
            &mut SourceAdmission::default(),
        );
        assert!(matches!(
            r.outcome,
            nepl3_engine::binding::probe::ProbeOutcome::Hit(_)
        ));
        assert_eq!(b.usage(), expected(8, 2041, 5, 17, 5844, 0, 0));
        let mut limits = budget().limits();
        limits.work = b.usage().work - 1;
        let mut short = Budget::new(limits);
        let r = nepl3_engine::binding::probe::first_missing_reference(
            "baseline",
            &checked,
            profile,
            &mut short,
            &mut SourceAdmission::default(),
        );
        assert!(matches!(
            r.outcome,
            nepl3_engine::binding::probe::ProbeOutcome::Stopped(StopReason::WorkLimit)
        ));
        assert_eq!(short.usage(), expected(8, 1973, 5, 17, 5276, 0, 0));
        Ok(())
    })
}

// Measured on main23fab043 before capture changes, including Work-stop boundaries.
// AllocationUnits contains native layout sizes; the fixed baseline is explicitly 64-bit.
fn expected(
    source_bytes: u64,
    work: u64,
    depth: u64,
    nodes: u64,
    allocation_units: u64,
    diagnostics: u64,
    events: u64,
) -> nepl3_core::budget::Usage {
    nepl3_core::budget::Usage {
        source_bytes,
        work,
        depth,
        nodes,
        allocation_units,
        output_bytes: 0,
        diagnostics,
        events,
    }
}
