use super::*;
#[test]
fn disabled_capture_preserves_usage_after_inline_frontier_savings() -> Result<(), String> {
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
                    expected(10, (2223, 6, 2), 4, 21, (5819, 4), 0, 0)
                } else {
                    expected(10, (2340, 6, 2), 5, 21, (6369, 4), 0, 0)
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
                    expected(10, (2155, 6, 2), 4, 21, (5819, 4), 0, 0)
                } else {
                    expected(10, (2272, 6, 2), 5, 21, (6369, 4), 0, 0)
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
        assert_eq!(
            b.usage(),
            expected(20, (8854, 36, 6), 8, 98, (22039, 22), 2, 1)
        );
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
        assert_eq!(
            short.usage(),
            expected(20, (8789, 36, 6), 8, 98, (22039, 22), 2, 1)
        );
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
        assert_eq!(b.usage(), expected(8, (2041, 6, 2), 5, 17, (5844, 2), 0, 0));
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
        assert_eq!(
            short.usage(),
            expected(8, (1973, 6, 2), 5, 17, (5276, 2), 0, 0)
        );
        Ok(())
    })
}

// Measured on main23fab043 before capture changes, including Work-stop boundaries.
// AllocationUnits contains native layout sizes; the fixed baseline is explicitly 64-bit.
// Inline validation removes one heap tuple per scalar validation here:
// lambda: 3 token payloads + 1 Expr leaf; recovered lambda: 2 token payloads.
// custom: (6 tokens + 2 Expr leaves) checked at analyze entry and FactsRequestView
// issue, plus 3 metadata records with 2 independently validated scalar fields.
// Both normal and one-short Work runs reach all of these validations. Every
// other historical Usage field stays exact; this is not a broad tolerance.
// Each shared binding-input table pays 3 for the first source lookup,
// identity and geometry; later spans pay one pointer probe and one range check.
// The work tuple is (historical Work, span count, independent table count).
// The explicit 2*spans+tables addition preserves every other Usage field.
// Lambda and recovered lambda each validate 3 Direct origins at the
// entry tree and final facts boundaries (6). RecoveryMissing supplies the
// tokenless third Direct origin in the recovered fixture.
// The custom fixture validates 6 Direct origins at six boundaries (36):
// analyze-entry tree; request-view tree and existing facts; post-host
// existing facts; combined delta; final facts. Its host adds no origins.
// Both normal and one-short paths reach the same origin validations.
fn expected(
    source_bytes: u64,
    work: (u64, u64, u64),
    depth: u64,
    nodes: u64,
    allocation_units: (u64, u64),
    diagnostics: u64,
    events: u64,
) -> nepl3_core::budget::Usage {
    nepl3_core::budget::Usage {
        source_bytes,
        work: work.0 + work.1 * 2 + work.2,
        depth,
        nodes,
        allocation_units: allocation_units.0
            - allocation_units.1
                * core::mem::size_of::<(
                    &nepl3_core::schema::TypeDescriptor,
                    &nepl3_core::value::NdfValue,
                    u64,
                )>() as u64,
        output_bytes: 0,
        diagnostics,
        events,
    }
}
