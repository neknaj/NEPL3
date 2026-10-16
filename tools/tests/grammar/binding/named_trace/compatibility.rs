use super::*;
#[test]
fn disabled_capture_preserves_usage_after_inline_frontier_savings() -> Result<(), String> {
    let lambda_owner_lookup = namespace_lookup_work("test.binding-execution");
    let lambda_lookup = 4 * lambda_owner_lookup;
    let recovered_lookup = 3 * lambda_owner_lookup;
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
                    expected(10, (2223, 6, 2, lambda_lookup), 4, 21, (5819, 4, 0), 0, 0)
                } else {
                    expected(10, (2340, 6, 2, lambda_lookup), 5, 21, (6369, 4, 0), 0, 0)
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
                    expected(10, (2155, 6, 2, lambda_lookup), 4, 21, (5819, 4, 0), 0, 0)
                } else {
                    expected(10, (2272, 6, 2, lambda_lookup), 5, 21, (6369, 4, 0), 0, 0)
                }
            );
        }
        Ok(())
    })?;
    let custom_lookup = (4 + 12) * namespace_lookup_work("test.binding-custom")
        + 2 * lookup_work(
            "nepl3.engine",
            &["nepl3.foundation", "nepl3.reader", "nepl3.engine"],
        )
        + diagnostic_payload_lookup_work();
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
            expected(
                20,
                (8854, 36, 6, custom_lookup),
                8,
                98,
                (22039, 22, 1),
                2,
                1
            )
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
            expected(
                20,
                (8789, 36, 6, custom_lookup),
                8,
                98,
                (22039, 22, 1),
                2,
                1
            )
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
        assert_eq!(
            b.usage(),
            expected(8, (2041, 6, 2, recovered_lookup), 5, 17, (5844, 2, 0), 0, 0)
        );
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
            expected(8, (1973, 6, 2, recovered_lookup), 5, 17, (5276, 2, 0), 0, 0)
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
// The work tuple is (historical Work, span count, independent table count,
// newly metered owner-schema searches).
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
    work: (u64, u64, u64, u64),
    depth: u64,
    nodes: u64,
    allocation_units: (u64, u64, u64),
    diagnostics: u64,
    events: u64,
) -> nepl3_core::budget::Usage {
    nepl3_core::budget::Usage {
        source_bytes,
        // Each public binding entry validates one native syntax bundle,
        // adding the checked-local-source-set cold probe. Facts validation
        // remains separate and does not call that native bundle helper.
        work: work.0 + work.1 * 2 + work.2 + work.3 + 1,
        depth,
        nodes,
        allocation_units: allocation_units.0
            - allocation_units.1
                * core::mem::size_of::<(
                    &nepl3_core::schema::TypeDescriptor,
                    &nepl3_core::value::NdfValue,
                    u64,
                )>() as u64
            // Custom host request validation reuses the same one-source set:
            // one snapshot handle and one source-index slot are not rebuilt.
            - allocation_units.2 * (core::mem::size_of::<nepl3_core::source::SourceSnapshot>()
                + core::mem::size_of::<usize>()) as u64,
        output_bytes: 0,
        diagnostics,
        events,
    }
}

// bootstrap::catalog registers foundation, reader, engine, grammar, then the
// fixture language. These independent formulas charge one scan entry, each
// candidate's two package names and revision comparison, and exact identity.
// Ordinary, traced and recovered lambda each validate their final namespace
// once (294 Work). The custom fixture validates its namespace at request issue,
// post-host existing facts, combined delta and final facts (4 * 273), and checks
// engine metadata for the emitted diagnostic and event (2 * 157). The built-in
// UndefinedName diagnostic validates only arguments. Structural validation now
// charges the engine schema and argument definition for it and both custom
// metadata payloads, independently of the two outer metadata owner searches.
// View/Token owner admission additionally validates three token kinds at the
// ordinary/traced lambda entry, two at recovered lambda entry, and six tokens
// at each of analyze entry and FactsRequestView issue for the custom fixture.
// Thus lambda has 1 namespace + 3 token searches; recovered has 1 + 2; custom
// has 4 namespace + 12 token searches, in addition to its two metadata owners.
// Full and one-short runs reach all of these searches; other counters and the
// subsequent stop prefixes remain exact rather than allowing a tolerance.
fn namespace_lookup_work(owner: &str) -> u64 {
    lookup_work(
        owner,
        &[
            "nepl3.foundation",
            "nepl3.reader",
            "nepl3.engine",
            "nepl3.grammar",
            owner,
        ],
    )
}
fn lookup_work(owner: &str, candidates: &[&str]) -> u64 {
    1 + candidates
        .iter()
        .map(|candidate| (candidate.len() + owner.len()) as u64 + 9)
        .sum::<u64>()
        + owner.len() as u64
        + 41
}

// Three BindingDiagnosticArguments payloads are validated: built-in
// UndefinedName, CustomNote, and CustomVisit. Each exact engine schema search
// costs 157 (derived by lookup_work). Binary definition lookup visits
// ParseProfile, ExpectedReadReply, CandidateError, BindingOptions,
// BindingBundleScope, BindingFailure, BindingDiagnosticArguments. Their
// compared UTF-8 pair counts are respectively 1, 1, 1, 8, 8, 8, 26;
// each comparison charges one entry plus three per visited byte pair.
// Total added Work is 3 * (157 + 166) = 969. Text children do not use
// Named resolution, so no definition-cache probe/insert belongs here.
// All three validations precede the final facts boundary in both full and
// one-short runs. This independently derived charge preserves the historical
// non-Work counters and exact stopping prefix.
fn diagnostic_payload_lookup_work() -> u64 {
    let schema = lookup_work(
        "nepl3.engine",
        &["nepl3.foundation", "nepl3.reader", "nepl3.engine"],
    );
    let definition = [1u64, 1, 1, 8, 8, 8, 26]
        .into_iter()
        .map(|pairs| 1 + 3 * pairs)
        .sum::<u64>();
    3 * (schema + definition)
}
