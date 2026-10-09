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
                    expected(
                        10,
                        2223 + 2 * schema_kind_work("test.binding-execution", 3)
                            + syntax_lookup_work(0)
                            + schema_kind_work("test.binding-execution", 1),
                        4,
                        21,
                        (5819, 4),
                        0,
                        0,
                    )
                } else {
                    expected(
                        10,
                        2340 + 2 * schema_kind_work("test.binding-execution", 3)
                            + syntax_lookup_work(0)
                            + schema_kind_work("test.binding-execution", 1),
                        5,
                        21,
                        (6369, 4),
                        0,
                        0,
                    )
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
                    expected(
                        10,
                        2155 + 2 * schema_kind_work("test.binding-execution", 3)
                            + syntax_lookup_work(0)
                            + schema_kind_work("test.binding-execution", 1),
                        4,
                        21,
                        (5819, 4),
                        0,
                        0,
                    )
                } else {
                    expected(
                        10,
                        2272 + 2 * schema_kind_work("test.binding-execution", 3)
                            + syntax_lookup_work(0)
                            + schema_kind_work("test.binding-execution", 1),
                        5,
                        21,
                        (6369, 4),
                        0,
                        0,
                    )
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
            expected(
                20,
                8854 + metadata_lookup_work()
                    + 2 * schema_kind_work("test.binding-custom", 12)
                    + syntax_lookup_work(1)
                    + schema_kind_work("test.binding-custom", 4),
                8,
                98,
                (22039, 22),
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
                8789 + metadata_lookup_work()
                    + 2 * schema_kind_work("test.binding-custom", 12)
                    + syntax_lookup_work(1)
                    + schema_kind_work("test.binding-custom", 4),
                8,
                98,
                (22039, 22),
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
            expected(
                8,
                2041 + 2 * schema_kind_work("test.binding-execution", 2)
                    + syntax_lookup_work(2)
                    + schema_kind_work("test.binding-execution", 1),
                5,
                17,
                (5844, 2),
                0,
                0
            )
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
            expected(
                8,
                1973 + 2 * schema_kind_work("test.binding-execution", 2)
                    + syntax_lookup_work(2)
                    + schema_kind_work("test.binding-execution", 1),
                5,
                17,
                (5276, 2),
                0,
                0
            )
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
fn expected(
    source_bytes: u64,
    work: u64,
    depth: u64,
    nodes: u64,
    allocation_units: (u64, u64),
    diagnostics: u64,
    events: u64,
) -> nepl3_core::budget::Usage {
    nepl3_core::budget::Usage {
        source_bytes,
        work,
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

// The built-in UndefinedName diagnostic, custom CustomNote diagnostic and
// CustomVisit event each validate borrowed
// BindingDiagnosticArguments once, before the final one-short boundary. The
// bootstrap catalog registers foundation, reader, engine in that order. Engine
// definitions are sorted; these are its binary probes for this record name.
// This independent fee sum updates only the newly metered lookups, not a golden
// replacement of the measured total or any other historical Usage field.
fn metadata_lookup_work() -> u64 {
    let package = "nepl3.engine";
    let kind = "BindingDiagnosticArguments";
    let schema_search = 1 + ["nepl3.foundation", "nepl3.reader", "nepl3.engine"]
        .iter()
        .map(|candidate| (candidate.len() + package.len() + 9) as u64)
        .sum::<u64>();
    let exact_identity = (package.len() + 41) as u64;
    let type_search = [
        "ParseProfile",
        "ExpectedReadReply",
        "CandidateError",
        "BindingOptions",
        "BindingBundleScope",
        "BindingFailure",
        "BindingDiagnosticArguments",
    ]
    .iter()
    .map(|candidate| (candidate.len() + kind.len() + 1) as u64)
    .sum::<u64>();
    // CustomNote and CustomVisit additionally admit their metadata-owner
    // schema through the shared metadata validators. The built-in UndefinedName
    // path admits its selected engine owner before constructing known metadata,
    // then validates arguments. This owner selection visits the same three
    // catalog entries without the exact-reference comparison used by metadata.
    3 * (schema_search + exact_identity + type_search)
        + 2 * (schema_search + exact_identity)
        + schema_search
}

// Token-kind and selected-kind admission each perform one exact schema
// lookup. These fixtures have empty token views and Unit/Text payloads. Normal/recovered
// lambda validation visits three/two token kinds once. The custom-host fixture
// visits six tokens at analysis entry and again at FactsRequestView admission.
// The same number of non-recovery nodes reach same_kind; RecoveryMissing
// takes the dedicated recovery branch and contributes no selected-kind lookup.
// Thus historical call sites add twice this fee, while syntax_lookup_work
// separately counts SyntaxBundle descriptor/name admission. All lookups occur
// before the historical final one-short boundary.
// Fact namespace admission also uses exact identity. Normal analysis (including
// trace-disabled capture) and the recovery probe each validate their one
// namespace at the final facts boundary. Custom performs four such validations:
// request.issue, existing facts before the returned delta, delta's shared View,
// and final facts. All precede the final historical one-short boundary.
// Bootstrap registers the four standard descriptors before its language schema.
// Derive the new catalog/identity fees independently; retain every other
// historical Usage field and the final one-short stopping boundary.
fn schema_kind_work(package: &str, count: u64) -> u64 {
    let catalog = [
        "nepl3.foundation",
        "nepl3.reader",
        "nepl3.engine",
        "nepl3.grammar",
        package,
    ];
    count
        * (1 + catalog
            .iter()
            .map(|name| (name.len() + package.len() + 9) as u64)
            .sum::<u64>()
            + package.len() as u64
            + 41)
}

// Syntax now meters exact descriptor and normalized named-type admission.
// These probe paths come from the generated 24/26-type execution/custom
// descriptors, not from fitting the measured total. Registry registration sorts
// names. Normal lambda has Form:Lambda, Builtin:Name, Leaf:Name. Custom has
// Early, Leaf:Name, Builtin:Name, Custom, Builtin:Name, Leaf:Name, and is validated
// at analysis entry and FactsRequestView issuance. Recovery replaces the final
// node with engine/RecoveryMissing (160 normalized engine types). All these
// admissions precede the same final one-short stop as the historical baseline.
fn syntax_lookup_work(case: u8) -> u64 {
    let probes = |kind: &str, names: &[&str]| -> u64 {
        names
            .iter()
            .map(|name| (name.len() + kind.len() + 1) as u64)
            .sum()
    };
    let lambda = probes(
        "Form:Lambda",
        &["Form:Recursive", "Form:Early", "Form:Lambda"],
    );
    let name = probes(
        "Builtin:Name",
        &[
            "Form:Recursive",
            "Form:Early",
            "BuiltinToken:Text",
            "Builtin:Text",
            "Builtin:Name",
        ],
    );
    match case {
        0 => {
            schema_kind_work("test.binding-execution", 3)
                + lambda
                + name
                + probes("Leaf:Name", &["Form:Recursive", "Leaf:Name"])
        }
        1 => {
            2 * (schema_kind_work("test.binding-custom", 6)
                + probes(
                    "Form:Early",
                    &[
                        "Form:LetText",
                        "Form:CustomDecl",
                        "Form:Imported",
                        "Form:Early",
                    ],
                )
                + 2 * probes("Leaf:Name", &["Form:LetText", "Leaf:Name"])
                + 2 * probes(
                    "Builtin:Name",
                    &[
                        "Form:LetText",
                        "Form:CustomDecl",
                        "BuiltinToken:Text",
                        "Builtin:Text",
                        "Builtin:Name",
                    ],
                )
                + probes(
                    "Form:Custom",
                    &[
                        "Form:LetText",
                        "Form:CustomDecl",
                        "BuiltinToken:Text",
                        "Form:Custom",
                    ],
                ))
        }
        _ => {
            let package = "nepl3.engine";
            let engine = 1
                + ["nepl3.foundation", "nepl3.reader", "nepl3.engine"]
                    .iter()
                    .map(|candidate| (candidate.len() + package.len() + 9) as u64)
                    .sum::<u64>()
                + package.len() as u64
                + 41;
            schema_kind_work("test.binding-execution", 2)
                + lambda
                + name
                + engine
                + probes(
                    "RecoveryMissing",
                    &[
                        "ParseProfile",
                        "RegionCapability",
                        "ProviderRequirement",
                        "RecoveryEntry",
                        "RecoveryUnexpected",
                        "RecoveryPlan",
                        "RecoveryMissing",
                    ],
                )
        }
    }
}
