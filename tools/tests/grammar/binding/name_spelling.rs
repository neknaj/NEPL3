use super::*;
#[path = "name_spelling/provider.rs"]
mod provider;
#[path = "name_spelling/reader.rs"]
mod reader;
#[path = "name_spelling/sources.rs"]
mod sources;
use nepl3_engine::analysis::insertion::quality::{self, QualityOutcome};
use nepl3_engine::{
    analysis::{
        BindingOptions,
        expected::ExpectedReadRequest,
        insertion::{
            InsertionInput,
            name::{self, NameSpelling},
        },
        probe::{
            candidates,
            read::{self, ReadOutcome},
        },
    },
    portable::analysis as keyed,
};

#[test]
fn name_spelling_preserves_ambiguity_and_supports_same_source_foreign_paths() -> Result<(), String>
{
    for (input, spelling, index, ambiguous, foreign, custom_update, before) in [
        ("guest guest lambda inner", "inner", 0, false, true, false),
        (
            "let outer 1 guest lambda inner",
            "inner",
            0,
            false,
            true,
            false,
        ),
        (
            "recursive cons define a 1 cons define a 2 nil lambda x",
            "a",
            1,
            true,
            false,
            false,
        ),
        ("let outer 1 lambda inner", "outer", 1, false, false, false),
        (
            "recursive cons define a 1 nil lambda x",
            "a",
            1,
            false,
            false,
            false,
        ),
        (
            "repeat cons define a 1 nil lambda x",
            "a",
            1,
            false,
            false,
            false,
        ),
        ("let holder self lambda y", "self", 2, false, false, false),
        (
            "let outer missing lambda inner",
            "inner",
            0,
            false,
            false,
            false,
        ),
        (
            "let outer open lambda inner",
            "inner",
            0,
            false,
            false,
            false,
        ),
        ("tail lambda x", "x", 0, false, false, false),
        (
            "lambda trailingSource",
            "trailingSource",
            0,
            false,
            false,
            false,
        ),
        ("lambda parseError", "parseError", 0, false, false, false),
        (
            "lambda parseWarning",
            "parseWarning",
            0,
            false,
            false,
            false,
        ),
        (
            "lambda parseInformation",
            "parseInformation",
            0,
            false,
            false,
            false,
        ),
        ("lambda parseHint", "parseHint", 0, false, false, false),
        ("lambda parseMixed", "parseMixed", 0, false, false, false),
        ("let other x lambda x", "x", 0, false, false, true),
        ("let another x lambda x", "x", 0, false, false, true),
        ("lambda q", "q", 0, false, false, false),
        ("apply lambda x", "x", 0, false, false, false),
        ("lambda x", "x", 0, false, false, true),
        ("let x 1 lambda x", "x", 0, false, false, true),
        ("let x 1 let x 2 lambda x", "x", 0, false, false, true),
    ]
    .into_iter()
    .map(|(i, s, n, a, f, c)| (i, s, n, a, f, c, " "))
    .chain([("lambda x", "x", 0, false, false, false, "y ")])
    {
        let mut compiled = if custom_update {
            super::missing_probe::named_lambda_from(super::custom::compiled()?)?
        } else {
            super::missing_probe::named_lambda()?
        };
        if input == "let holder self lambda y" {
            use nepl3_engine::package::{Binding, BindingId, NameSelector};
            let leaf = compiled
                .package
                .leaves
                .iter()
                .find(|v| matches!(v.payload, nepl3_core::schema::TypeDescriptor::Text))
                .ok_or("Text leaf")?;
            compiled.package.bindings[leaf.binding.0 as usize] = Binding::Export {
                namespace: "Value".into(),
                name: NameSelector::SelfValue,
            };
            let form = compiled
                .package
                .forms
                .iter()
                .find(|f| f.spelling == "let")
                .ok_or("Let")?;
            let import = BindingId(compiled.package.bindings.len() as u64);
            compiled
                .package
                .bindings
                .push(Binding::Import("init".into()));
            let Binding::Group(actions) = &mut compiled.package.bindings[form.binding.0 as usize]
            else {
                return Err("Let group".into());
            };
            *actions.first_mut().ok_or("init action")? = import;
        }
        if custom_update {
            use nepl3_engine::package::Binding;
            let custom_id = compiled
                .package
                .bindings
                .iter()
                .position(|v| matches!(v, Binding::Custom(_)))
                .ok_or("custom action")?;
            let lambda = compiled
                .package
                .forms
                .iter()
                .find(|f| f.spelling == "lambda")
                .ok_or("lambda")?;
            let Binding::Scope(actions) = &mut compiled.package.bindings[lambda.binding.0 as usize]
            else {
                return Err("scope".into());
            };
            if input == "let x 1 let x 2 lambda x" {
                if actions.is_empty() {
                    return Err("lambda actions".into());
                }
                actions.remove(0); // Keep the two Let declarations visible without a third shadow.
            }
            actions.push(nepl3_engine::package::BindingId(custom_id as u64));
        }
        if input == "let other x lambda x" || input == "let another x lambda x" {
            // Keep the early Reference and later Custom action in the same scope;
            // the fixture must not request forbidden ancestor-scope write authority.
            for binding in &mut compiled.package.bindings {
                if let nepl3_engine::package::Binding::Scope(actions) = binding {
                    let actions = core::mem::take(actions);
                    *binding = nepl3_engine::package::Binding::Group(actions);
                }
            }
        }
        if input == "let outer open lambda inner" || input == "let another x lambda x" {
            compiled.package.namespaces[0].policy = nepl3_engine::package::NamespacePolicy::Open;
        }
        with_source_profile(&compiled, None, input, |source, profile, b, ledger| {
            with_source_profile(&compiled, None, input, |_, alternate_profile, _, _| {
                let registry = profile.registry();
                let package = profile.language("B", b).map_err(err)?;
                let foundation = registry
                    .selected("nepl3.foundation", 1)
                    .ok_or("foundation")?;
                let mut store = SourceStore::default();
                store.insert(source.clone()).map_err(err)?;
                let environment = Environment {
                    bindings: vec![],
                    resources: vec![],
                };
                let raw = ReaderContext {
                    schema: package.schema.clone(),
                    category: "Expr".into(),
                    mode: "Code".into(),
                    origins: vec![],
                    environment: EnvironmentEntry {
                        id: 0,
                        digest: environment_digest(&environment, foundation, registry, b)
                            .map_err(err)?,
                        value: environment,
                    },
                };
                let environments = {
                    let mut codec = FoundationCodec::new(registry, &store, ledger).map_err(err)?;
                    let proof = raw.check(&mut codec, &store, registry, b).map_err(err)?;
                    ParseEnvironmentSet::prepare(
                        profile,
                        &[EnvironmentInput {
                            alias: "B",
                            context: &proof,
                        }],
                        &store,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?
                };
                let entry = profile.entry("B", None, b).map_err(err)?;
                let states = [LanguageReaderState {
                    alias: "B".into(),
                    state: NdfValue::Unit,
                }];
                let mut parser = RetainedParseSession::new(
                    "name-foreign-old".into(),
                    profile,
                    &environments,
                    ParseRequest {
                        snapshot: source,
                        start: if input == "tail lambda x" { 5 } else { 0 },
                        limit: input.len() as u64,
                        final_input: true,
                        entry: &entry,
                        states: &states,
                    },
                    &store,
                    b,
                )
                .map_err(err)?;
                let old = drive(&mut parser, registry, "old", b, ledger)?;
                let empty = SourceStore::default();
                let prepared = {
                    let mut codec = FoundationCodec::new(registry, &empty, ledger).map_err(err)?;
                    keyed::prepare(
                        "name-foreign-old",
                        old.execution().tree(),
                        BindingOptions,
                        b.limits(),
                        profile,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?
                };
                let probe_profile = if input == "lambda q" {
                    alternate_profile
                } else {
                    profile
                };
                let probe_prepared = {
                    let mut codec = FoundationCodec::new(probe_profile.registry(), &empty, ledger)
                        .map_err(err)?;
                    keyed::prepare(
                        "name-foreign-old",
                        old.execution().tree(),
                        BindingOptions,
                        b.limits(),
                        probe_profile,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?
                };
                assert_eq!(probe_prepared.key(), prepared.key());
                let original_named = probe_prepared
                    .probe_missing_reference_with_births(b, ledger)
                    .map_err(err)?;
                let bound = original_named.probe();
                let request = ExpectedReadRequest {
                    key: prepared.key(),
                    source: source.reference(),
                    offset: input.len() as u64,
                };
                let correlation =
                    read::correlate(bound, &prepared, &request, b, ledger).map_err(err)?;
                let ReadOutcome::Hit(read) = correlation.outcome() else {
                    return Err("read correspondence".into());
                };
                let candidates = candidates::names(
                    bound,
                    &candidates::ProbeCandidateRequest {
                        key: prepared.key(),
                        source: &request.source,
                        offset: request.offset,
                        prefix: "",
                    },
                    b,
                    ledger,
                )
                .map_err(err)?;
                let selected = name::select(&candidates, read, index, b).map_err(err)?;
                assert_eq!(selected.candidate().name, spelling);
                assert_eq!(
                    matches!(
                        selected.candidate().resolution,
                        ReferenceResolution::Ambiguous(_)
                    ),
                    ambiguous
                );
                let original_resolution = selected.candidate().resolution.clone();
                let proposal = name::prepare(
                    InsertionInput {
                        parsed: &old,
                        prepared: &prepared,
                    },
                    &request,
                    selected,
                    NameSpelling {
                        before,
                        spelling,
                        after: if input == "lambda trailingSource" {
                            " trailing"
                        } else {
                            ""
                        },
                    },
                    b,
                    ledger,
                )
                .map_err(err)?;
                let draft = proposal.draft();
                let mut parser = RetainedParseSession::new(
                    "name-foreign-new".into(),
                    profile,
                    &environments,
                    ParseRequest {
                        snapshot: draft.snapshot(),
                        start: if input == "tail lambda x" { 5 } else { 0 },
                        limit: draft.limit(),
                        final_input: true,
                        entry: &entry,
                        states: &states,
                    },
                    draft.sources(),
                    b,
                )
                .map_err(err)?;
                let candidate = drive(&mut parser, registry, "new", b, ledger)?;
                {
                    let mut codec = FoundationCodec::new(registry, &empty, ledger).map_err(err)?;
                    let checked = name::check(
                        InsertionInput {
                            parsed: &old,
                            prepared: &prepared,
                        },
                        &candidate,
                        &proposal,
                        &request,
                        "name-foreign-new",
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    assert_eq!(checked.choice().candidate().resolution, original_resolution);
                    {
                        use nepl3_core::value_codec::FoundationValueCodec;
                        use nepl3_engine::analysis::insertion::reference::{
                            self, ReferenceOutcome,
                        };
                        let fresh = keyed::prepare(
                            "name-foreign-new",
                            candidate.execution().tree(),
                            BindingOptions,
                            b.limits(),
                            profile,
                            &mut codec,
                            b,
                        )
                        .map_err(err)?;
                        let trace = fresh
                            .trace_references(b, codec.source_admission())
                            .map_err(err)?;
                        let named = fresh
                            .trace_named(b, codec.source_admission())
                            .map_err(err)?;
                        match reference::correlate(&checked, &fresh, named.references(), b)
                            .map_err(err)?
                        {
                            ReferenceOutcome::Unique(matched) => {
                                assert!(
                                    !input.starts_with("repeat"),
                                    "repeated execution must stop before a unique Reference proof"
                                );
                                assert!(core::ptr::eq(matched.trace(), named.references()));
                                use nepl3_engine::analysis::insertion::declaration::{
                                    self, DeclarationOutcome, UnprovenReason,
                                };
                                let unrelated = fresh
                                    .trace_named(b, codec.source_admission())
                                    .map_err(err)?;
                                assert!(matches!(
                                    declaration::correlate(
                                        &matched,
                                        &original_named,
                                        &unrelated,
                                        &prepared,
                                        &fresh,
                                        b
                                    ),
                                    Err(declaration::DeclarationError::ProofMismatch)
                                ));
                                let unrelated_old = prepared
                                    .probe_missing_reference_with_births(
                                        b,
                                        codec.source_admission(),
                                    )
                                    .map_err(err)?;
                                assert!(matches!(
                                    declaration::correlate(
                                        &matched,
                                        &unrelated_old,
                                        &named,
                                        &prepared,
                                        &fresh,
                                        b
                                    ),
                                    Err(declaration::DeclarationError::ProofMismatch)
                                ));
                                if input == "lambda q" {
                                    assert!(!core::ptr::eq(
                                        original_named.profile(),
                                        checked.checked().original().seed().profile()
                                    ));
                                    assert!(core::ptr::eq(
                                        original_named.probe(),
                                        checked.choice().read().reply()
                                    ));

                                    // All checked dependencies still use the original profile; only the
                                    // actual issuing probe used a separately resolved equal-key profile.
                                    assert!(matches!(
                                        declaration::correlate(
                                            &matched,
                                            &original_named,
                                            &named,
                                            &prepared,
                                            &fresh,
                                            b
                                        ),
                                        Err(declaration::DeclarationError::ProofMismatch)
                                    ));
                                } else {
                                    let result = declaration::correlate(
                                        &matched,
                                        &original_named,
                                        &named,
                                        &prepared,
                                        &fresh,
                                        b,
                                    )
                                    .map_err(err)?;
                                    if before == "y " {
                                        assert!(matches!(
                                            result,
                                            DeclarationOutcome::Unproven(
                                                UnprovenReason::FinalUnresolved
                                            )
                                        ));
                                    } else if ambiguous {
                                        assert!(matches!(
                                            result,
                                            DeclarationOutcome::Unproven(
                                                UnprovenReason::OriginalAmbiguous
                                            )
                                        ));
                                    } else {
                                        match result {
                                            DeclarationOutcome::Same(proof) => {
                                                use nepl3_core::budget::{Resource, StopReason};
                                                use nepl3_core::diagnostic::Severity;
                                                let outcome = quality::check(&proof, b);
                                                match input {
                                                    "tail lambda x" => assert!(matches!(outcome, Err(quality::QualityError::Whole(nepl3_engine::parse::whole::WholeInputError::PartialRange)))),
                                                    "lambda trailingSource" => assert!(matches!(outcome, Err(quality::QualityError::Whole(nepl3_engine::parse::whole::WholeInputError::Unconsumed {cursor, limit})) if cursor < limit)),
                                                    "lambda parseError" | "lambda parseMixed" => {
                                                        let QualityOutcome::Rejected(quality::Rejection::Diagnostic {stage: quality::DiagnosticStage::Parse, diagnostic}) = outcome.map_err(err)? else { return Err("parse Error accepted".into()); };
                                                        assert_eq!(diagnostic.code, "QualityParseFixture");
                                                        assert_eq!(diagnostic.severity, Severity::Error);
                                                    }
                                                    "let outer missing lambda inner" => {
                                                        let QualityOutcome::Rejected(quality::Rejection::Diagnostic {stage: quality::DiagnosticStage::Binding, diagnostic}) = outcome.map_err(err)? else {return Err("binding Error accepted".into());};
                                                        assert_eq!(diagnostic.code, "UndefinedName");
                                                    }
                                                    "let outer open lambda inner" => assert!(matches!(outcome.map_err(err)?, QualityOutcome::Rejected(quality::Rejection::OpenInput {..}))),
                                                    _ => {
                                                        let QualityOutcome::Suitable(suitable) = outcome.map_err(err)? else {return Err(format!("strict suitable {input}"));};
                                                        assert!(core::ptr::eq(suitable.declaration(), &proof));
                                                        sources::verify(&suitable, b)?;
                                                        assert!(core::ptr::eq(suitable.whole().parsed(), checked.checked().candidate()));
                                                        let parse_report = checked.checked().candidate().execution().report();
                                                        let parse_report_before = parse_report.clone();
                                                        let binding_report = named.references().trace().reply().report.clone();
                                                        if input.starts_with("lambda parse") {
                                                            let expected = match input { "lambda parseWarning" => Severity::Warning, "lambda parseInformation" => Severity::Information, "lambda parseHint" => Severity::Hint, _ => return Err("unexpected suitable parse diagnostic".into()) };
                                                            assert!(!parse_report.diagnostics.is_empty());
                                                            assert!(parse_report.diagnostics.iter().all(|d|d.severity == expected));
                                                        }
                                                        let (analysis, _) = named.references().trace().complete().ok_or("complete")?;
                                                        let rows = (parse_report.diagnostics.len() + binding_report.diagnostics.len() + analysis.facts().occurrences.len()) as u64;
                                                        let mut measured = Budget::new(b.limits());
                                                        assert!(matches!(quality::check(&proof, &mut measured).map_err(err)?, QualityOutcome::Suitable(_)));
                                                        assert_eq!(measured.usage().work, 130 + rows);
                                                        assert_eq!(measured.usage().nodes, rows);
                                                        assert_eq!(measured.usage().diagnostics, 0);
                                                        assert_eq!(measured.usage().events, 0);
                                                        assert_eq!(measured.usage().allocation_units, 0);
                                                        for (resource, cap, cost, reason) in [(Resource::Work,b.limits().work,130+rows,StopReason::WorkLimit),(Resource::Nodes,b.limits().nodes,rows,StopReason::NodeLimit)] {
                                                            let mut exact=Budget::new(b.limits()); exact.charge(resource,cap-cost).map_err(err)?;
                                                            assert!(matches!(quality::check(&proof,&mut exact).map_err(err)?,QualityOutcome::Suitable(_)));
                                                            let mut stopped=Budget::new(b.limits()); stopped.charge(resource,cap-(cost-1)).map_err(err)?;
                                                            let Err(error)=quality::check(&proof,&mut stopped) else{return Err("late quality stop".into());};
                                                            assert_eq!(error.stop_reason(),Some(reason));
                                                        }
                                                        let mut cancelled=Budget::new(b.limits()); cancelled.cancel();
                                                        let Err(error)=quality::check(&proof,&mut cancelled) else{return Err("cancelled quality".into());};
                                                        assert_eq!(error.stop_reason(), Some(StopReason::Cancelled));
                                                        let mut limits=b.limits(); limits.work-=1;
                                                        let mut other=Budget::new(limits); other.cancel();
                                                        assert!(matches!(quality::check(&proof,&mut other),Err(quality::QualityError::LimitsMismatch)));
                                                        assert_eq!(other.usage(), Budget::new(limits).usage());
                                                        assert_eq!(named.references().trace().reply().report,binding_report);
                                                        assert_eq!(checked.checked().candidate().execution().report(), &parse_report_before);
                                                    }
                                                }
                                                if input == "lambda parseMixed" {
                                                    let diagnostics = &checked
                                                        .checked()
                                                        .candidate()
                                                        .execution()
                                                        .report()
                                                        .diagnostics;
                                                    assert_eq!(
                                                        diagnostics[0].severity,
                                                        Severity::Warning
                                                    );
                                                    assert_eq!(
                                                        diagnostics[1].severity,
                                                        Severity::Error
                                                    );
                                                    let mut stopped = Budget::new(b.limits());
                                                    stopped
                                                        .charge(
                                                            Resource::Work,
                                                            b.limits().work - 130,
                                                        )
                                                        .map_err(err)?;
                                                    let Err(error) =
                                                        quality::check(&proof, &mut stopped)
                                                    else {
                                                        return Err("mixed late stop".into());
                                                    };
                                                    assert_eq!(
                                                        error.stop_reason(),
                                                        Some(StopReason::WorkLimit)
                                                    );
                                                    assert_eq!(stopped.usage().nodes, 1);
                                                    let mut exact = Budget::new(b.limits());
                                                    exact
                                                        .charge(
                                                            Resource::Work,
                                                            b.limits().work - 131,
                                                        )
                                                        .map_err(err)?;
                                                    exact
                                                        .charge(
                                                            Resource::Nodes,
                                                            b.limits().nodes - 2,
                                                        )
                                                        .map_err(err)?;
                                                    let QualityOutcome::Rejected(
                                                        quality::Rejection::Diagnostic {
                                                            diagnostic,
                                                            ..
                                                        },
                                                    ) = quality::check(&proof, &mut exact)
                                                        .map_err(err)?
                                                    else {
                                                        return Err("mixed Error rejection".into());
                                                    };
                                                    assert!(core::ptr::eq(
                                                        diagnostic,
                                                        &diagnostics[1]
                                                    ));
                                                    assert_eq!(exact.usage().work, b.limits().work);
                                                    assert_eq!(
                                                        exact.usage().nodes,
                                                        b.limits().nodes
                                                    );
                                                }
                                                if input == "let outer open lambda inner" {
                                                    assert!(
                                                        checked
                                                            .checked()
                                                            .candidate()
                                                            .execution()
                                                            .report()
                                                            .diagnostics
                                                            .is_empty()
                                                    );
                                                    assert!(
                                                        named
                                                            .references()
                                                            .trace()
                                                            .reply()
                                                            .report
                                                            .diagnostics
                                                            .is_empty()
                                                    );
                                                    let mut exact = Budget::new(b.limits());
                                                    exact
                                                        .charge(
                                                            Resource::Work,
                                                            b.limits().work - 130,
                                                        )
                                                        .map_err(err)?;
                                                    exact
                                                        .charge(
                                                            Resource::Nodes,
                                                            b.limits().nodes - 1,
                                                        )
                                                        .map_err(err)?;
                                                    assert!(matches!(
                                                        quality::check(&proof, &mut exact)
                                                            .map_err(err)?,
                                                        QualityOutcome::Rejected(
                                                            quality::Rejection::OpenInput { .. }
                                                        )
                                                    ));
                                                    assert_eq!(exact.usage().work, b.limits().work);
                                                    assert_eq!(
                                                        exact.usage().nodes,
                                                        b.limits().nodes
                                                    );
                                                    let mut stopped = Budget::new(b.limits());
                                                    stopped
                                                        .charge(Resource::Nodes, b.limits().nodes)
                                                        .map_err(err)?;
                                                    let Err(error) =
                                                        quality::check(&proof, &mut stopped)
                                                    else {
                                                        return Err("open ledger stop".into());
                                                    };
                                                    assert_eq!(
                                                        error.stop_reason(),
                                                        Some(StopReason::NodeLimit)
                                                    );
                                                }

                                                assert!(core::ptr::eq(
                                                    proof.original(),
                                                    &original_named
                                                ));
                                                assert!(core::ptr::eq(proof.candidate(), &named));
                                                assert_eq!(proof.original_entity().name, spelling);
                                                assert_eq!(proof.candidate_entity().name, spelling);
                                                if input == "let holder self lambda y" {
                                                    assert_eq!(
                                                        proof.original_birth().owner,
                                                        proof.original_birth().name_target
                                                    );
                                                    assert_eq!(
                                                        proof.candidate_birth().owner,
                                                        proof.candidate_birth().name_target
                                                    );
                                                    assert_eq!(proof.original_birth().kind, nepl3_engine::binding::trace::birth::BirthKind::Export);
                                                }

                                                if input == "guest guest lambda inner" {
                                                    use nepl3_core::budget::{
                                                        Resource, StopReason,
                                                    };
                                                    with_source_profile(&compiled, None, input, |_, other_profile, other_budget, other_admission| {
                                                let mut other_codec = FoundationCodec::new(other_profile.registry(), &empty, other_admission).map_err(err)?;
                                                let other = keyed::prepare("name-foreign-old", old.execution().tree(), BindingOptions, b.limits(), other_profile, &mut other_codec, other_budget).map_err(err)?;
                                                assert_eq!(other.key(), prepared.key());
                                                assert!(matches!(declaration::correlate(&matched, &original_named, &named, &other, &fresh, other_budget), Err(declaration::DeclarationError::ProofMismatch)));
                                                Ok(())
                                            })?;
                                                    let mut measured = Budget::new(b.limits());
                                                    assert!(matches!(
                                                        declaration::correlate(
                                                            &matched,
                                                            &original_named,
                                                            &named,
                                                            &prepared,
                                                            &fresh,
                                                            &mut measured
                                                        )
                                                        .map_err(err)?,
                                                        DeclarationOutcome::Same(_)
                                                    ));
                                                    let usage = measured.usage();
                                                    for (resource, cap, used, expected) in [
                                                        (
                                                            Resource::Work,
                                                            b.limits().work,
                                                            usage.work,
                                                            StopReason::WorkLimit,
                                                        ),
                                                        (
                                                            Resource::Nodes,
                                                            b.limits().nodes,
                                                            usage.nodes,
                                                            StopReason::NodeLimit,
                                                        ),
                                                        (
                                                            Resource::AllocationUnits,
                                                            b.limits().allocation_units,
                                                            usage.allocation_units,
                                                            StopReason::AllocationLimit,
                                                        ),
                                                    ] {
                                                        assert!(used > 2);
                                                        for available in [0, used / 2, used - 1] {
                                                            let mut stopped =
                                                                Budget::new(b.limits());
                                                            stopped
                                                                .charge(resource, cap - available)
                                                                .map_err(err)?;
                                                            let Err(error) = declaration::correlate(
                                                                &matched,
                                                                &original_named,
                                                                &named,
                                                                &prepared,
                                                                &fresh,
                                                                &mut stopped,
                                                            ) else {
                                                                return Err("declaration budget accepted incomplete scan".into());
                                                            };
                                                            assert_eq!(
                                                                error.stop_reason(),
                                                                Some(expected)
                                                            );
                                                        }
                                                    }
                                                    let mut cancelled = Budget::new(b.limits());
                                                    cancelled.cancel();
                                                    let Err(error) = declaration::correlate(
                                                        &matched,
                                                        &original_named,
                                                        &named,
                                                        &prepared,
                                                        &fresh,
                                                        &mut cancelled,
                                                    ) else {
                                                        return Err(
                                                            "declaration cancellation".into()
                                                        );
                                                    };
                                                    assert_eq!(
                                                        error.stop_reason(),
                                                        Some(StopReason::Cancelled)
                                                    );
                                                }
                                            }
                                            DeclarationOutcome::Unproven(reason) => {
                                                return Err(format!(
                                                    "declaration correspondence {input}: {reason:?}"
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                            ReferenceOutcome::Invalid(_)
                                if input == "apply lambda x" || custom_update => {}
                            ReferenceOutcome::Multiple if input.starts_with("repeat") => {}
                            _ => return Err("named trace borrowed Reference correspondence".into()),
                        }
                        match reference::correlate(&checked, &fresh, &trace, b).map_err(err)? {
                            ReferenceOutcome::Unique(matched) => {
                                assert!(
                                    !input.starts_with("repeat"),
                                    "repeated execution must stop before a unique Reference proof"
                                );
                                assert_eq!(matched.final_reference().occurrence.name, spelling);
                                assert_eq!(
                                    matches!(
                                        matched.final_reference().occurrence.resolution,
                                        ReferenceResolution::Ambiguous(_)
                                    ),
                                    ambiguous
                                );
                                assert_eq!(matched.issuance().owner.path.is_empty(), !foreign);
                                if input == "guest guest lambda inner" {
                                    assert_eq!(matched.issuance().owner.path.len(), 2);
                                    assert_eq!(
                                        matched.issuance().name_target.path,
                                        matched.issuance().owner.path
                                    );
                                }
                                assert!(core::ptr::eq(matched.trace(), &trace));
                            }
                            ReferenceOutcome::Invalid(_)
                                if input == "apply lambda x" || custom_update => {}
                            ReferenceOutcome::Multiple if input.starts_with("repeat") => {}
                            _ => return Err("candidate Reference correspondence".into()),
                        }
                        if custom_update {
                            assert_eq!(
                                trace.trace().rows().len(),
                                if input == "let other x lambda x"
                                    || input == "let another x lambda x"
                                {
                                    2
                                } else {
                                    1
                                }
                            );
                            assert!(matches!(
                                reference::correlate(&checked, &fresh, &trace, b).map_err(err)?,
                                ReferenceOutcome::Invalid(
                                    nepl3_engine::binding::BindingError::MissingProvider
                                )
                            ));
                            let mut a = super::custom::query_host(false, false);
                            let mut c = super::custom::query_host(true, false);
                            let first = fresh
                                .trace_references_with_host(&mut a, b, codec.source_admission())
                                .map_err(err)?;
                            let second = fresh
                                .trace_references_with_host(&mut c, b, codec.source_admission())
                                .map_err(err)?;
                            assert_eq!(first.key(), second.key());
                            for (bound, updated) in [(&first, false), (&second, true)] {
                                let ReferenceOutcome::Unique(matched) =
                                    reference::correlate(&checked, &fresh, bound, b)
                                        .map_err(err)?
                                else {
                                    return Err(format!(
                                        "custom correspondence {input}: {:?}",
                                        bound.trace().reply().outcome
                                    ));
                                };
                                assert!(core::ptr::eq(matched.trace(), bound));
                                assert_eq!(
                                    matched.final_reference().occurrence.resolution
                                        == ReferenceResolution::Resolved(EntityId(100)),
                                    updated
                                );
                            }
                            for updated in [false, true] {
                                use nepl3_engine::analysis::insertion::declaration::{
                                    self, DeclarationOutcome, UnprovenReason,
                                };
                                let mut host = super::custom::query_host(updated, false);
                                let named = fresh
                                    .trace_named_with_host(&mut host, b, codec.source_admission())
                                    .map_err(err)?;
                                let ReferenceOutcome::Unique(matched) =
                                    reference::correlate(&checked, &fresh, named.references(), b)
                                        .map_err(err)?
                                else {
                                    return Err("named Custom correspondence".into());
                                };
                                let result = declaration::correlate(
                                    &matched,
                                    &original_named,
                                    &named,
                                    &prepared,
                                    &fresh,
                                    b,
                                )
                                .map_err(err)?;
                                if updated {
                                    assert!(matches!(
                                        result,
                                        DeclarationOutcome::Unproven(
                                            UnprovenReason::UntracedEntity
                                        )
                                    ));
                                } else {
                                    assert!(matches!(result, DeclarationOutcome::Same(_)));
                                }
                            }
                            if input == "let x 1 lambda x" || input == "let x 1 let x 2 lambda x" {
                                use nepl3_engine::analysis::insertion::declaration::{
                                    self, DeclarationOutcome, UnprovenReason,
                                };
                                let mut host = provider::RetargetBuiltIn(
                                    super::custom::query_host(true, false),
                                );
                                let named = fresh
                                    .trace_named_with_host(&mut host, b, codec.source_admission())
                                    .map_err(err)?;
                                let ReferenceOutcome::Unique(matched) =
                                    reference::correlate(&checked, &fresh, named.references(), b)
                                        .map_err(err)?
                                else {
                                    return Err("built-in retarget Reference".into());
                                };
                                assert_eq!(
                                    matched.final_reference().occurrence.resolution,
                                    ReferenceResolution::Resolved(EntityId(0))
                                );
                                assert_eq!(
                                    checked.choice().candidate().resolution,
                                    ReferenceResolution::Resolved(EntityId(1))
                                );
                                if input == "let x 1 let x 2 lambda x" {
                                    use nepl3_engine::binding::trace::birth::BirthLookup;
                                    let BirthLookup::Born {
                                        birth: old_birth, ..
                                    } = original_named
                                        .entity_birth(&checked.checked().keys().0, EntityId(1), b)
                                        .map_err(err)?
                                    else {
                                        return Err("original birth".into());
                                    };
                                    let BirthLookup::Born {
                                        birth: new_birth, ..
                                    } = named
                                        .entity_birth(&checked.checked().keys().1, EntityId(0), b)
                                        .map_err(err)?
                                    else {
                                        return Err("candidate birth".into());
                                    };
                                    assert_eq!(old_birth.binding, new_birth.binding);
                                    assert_eq!(old_birth.kind, new_birth.kind);
                                    assert_ne!(old_birth.owner, new_birth.owner);
                                }
                                assert!(matches!(
                                    declaration::correlate(
                                        &matched,
                                        &original_named,
                                        &named,
                                        &prepared,
                                        &fresh,
                                        b
                                    )
                                    .map_err(err)?,
                                    DeclarationOutcome::Unproven(reason) if reason == if input == "let x 1 lambda x" { UnprovenReason::Action } else { UnprovenReason::Context }
                                ));
                            }
                            if input == "lambda x" {
                                use nepl3_core::diagnostic::Severity;
                                use nepl3_engine::analysis::insertion::declaration::{
                                    self, DeclarationOutcome,
                                };
                                for severity in [
                                    Severity::Warning,
                                    Severity::Information,
                                    Severity::Hint,
                                    Severity::Error,
                                ] {
                                    let mut host = provider::ReportDiagnostic {
                                        inner: super::custom::query_host(false, false),
                                        severity,
                                    };
                                    let named = fresh
                                        .trace_named_with_host(
                                            &mut host,
                                            b,
                                            codec.source_admission(),
                                        )
                                        .map_err(err)?;
                                    let ReferenceOutcome::Unique(matched) = reference::correlate(
                                        &checked,
                                        &fresh,
                                        named.references(),
                                        b,
                                    )
                                    .map_err(err)?
                                    else {
                                        return Err(format!(
                                            "diagnostic Reference {:?}",
                                            named.references().trace().reply().outcome
                                        ));
                                    };
                                    let DeclarationOutcome::Same(same) = declaration::correlate(
                                        &matched,
                                        &original_named,
                                        &named,
                                        &prepared,
                                        &fresh,
                                        b,
                                    )
                                    .map_err(err)?
                                    else {
                                        return Err("diagnostic declaration".into());
                                    };
                                    assert!(
                                        named
                                            .references()
                                            .trace()
                                            .reply()
                                            .report
                                            .diagnostics
                                            .iter()
                                            .any(|d| d.code == "QualityFixture"
                                                && d.severity == severity)
                                    );
                                    match quality::check(&same, b).map_err(err)? {
                                        QualityOutcome::Rejected(
                                            quality::Rejection::Diagnostic {
                                                stage: quality::DiagnosticStage::Binding,
                                                diagnostic,
                                            },
                                        ) if severity == Severity::Error => {
                                            assert_eq!(diagnostic.code, "QualityFixture")
                                        }
                                        QualityOutcome::Suitable(strict)
                                            if severity != Severity::Error =>
                                        {
                                            sources::verify(&strict, b)?;
                                            use nepl3_engine::analysis::insertion::sources::{
                                                Side, Stage, collect,
                                            };
                                            let inventory = collect(&strict, b).map_err(err)?;
                                            let added: Vec<_> = inventory
                                                .entries()
                                                .iter()
                                                .filter(|row| {
                                                    row.source.identity().source.0
                                                        == "candidate-diagnostic-only"
                                                })
                                                .collect();
                                            assert_eq!(added.len(), 1);
                                            assert_eq!(
                                                (added[0].side, added[0].stage),
                                                (Side::Candidate, Stage::BindingReply)
                                            );
                                        }
                                        _ => return Err("strict diagnostic threshold".into()),
                                    }
                                }
                                for kind in [
                                    provider::ReferenceFixture::Unresolved,
                                    provider::ReferenceFixture::Ambiguous,
                                    provider::ReferenceFixture::Deferred,
                                ] {
                                    let mut host = provider::ExtraReference {
                                        inner: super::custom::query_host(false, false),
                                        kind,
                                    };
                                    let named = fresh
                                        .trace_named_with_host(
                                            &mut host,
                                            b,
                                            codec.source_admission(),
                                        )
                                        .map_err(err)?;
                                    let ReferenceOutcome::Unique(matched) = reference::correlate(
                                        &checked,
                                        &fresh,
                                        named.references(),
                                        b,
                                    )
                                    .map_err(err)?
                                    else {
                                        return Err("extra Reference correspondence".into());
                                    };
                                    let DeclarationOutcome::Same(same) = declaration::correlate(
                                        &matched,
                                        &original_named,
                                        &named,
                                        &prepared,
                                        &fresh,
                                        b,
                                    )
                                    .map_err(err)?
                                    else {
                                        return Err("extra declaration correspondence".into());
                                    };
                                    assert_eq!(named.references().trace().rows().len(), 1);
                                    let QualityOutcome::Rejected(quality::Rejection::Reference {
                                        occurrence,
                                    }) = quality::check(&same, b).map_err(err)?
                                    else {
                                        return Err("extra Custom Reference quality".into());
                                    };
                                    assert_eq!(occurrence.id, OccurrenceId(201));
                                    match kind {
                                        provider::ReferenceFixture::Unresolved => {
                                            assert!(matches!(
                                                occurrence.resolution,
                                                ReferenceResolution::Unresolved(_)
                                            ))
                                        }
                                        provider::ReferenceFixture::Ambiguous => assert!(matches!(
                                            occurrence.resolution,
                                            ReferenceResolution::Ambiguous(_)
                                        )),
                                        provider::ReferenceFixture::Deferred => assert!(matches!(
                                            occurrence.resolution,
                                            ReferenceResolution::Deferred(_)
                                        )),
                                    }
                                    let (analysis, _) =
                                        named.references().trace().complete().ok_or("complete")?;
                                    assert_eq!(
                                        analysis.facts().occurrences.last().ok_or("last")?.id,
                                        OccurrenceId(201)
                                    );
                                    assert!(analysis.facts().occurrences.iter().any(|o| o.role
                                        == OccurrenceRole::Reference
                                        && matches!(
                                            o.resolution,
                                            ReferenceResolution::Resolved(_)
                                        )));
                                    let rows = (checked
                                        .checked()
                                        .candidate()
                                        .execution()
                                        .report()
                                        .diagnostics
                                        .len()
                                        + named
                                            .references()
                                            .trace()
                                            .reply()
                                            .report
                                            .diagnostics
                                            .len()
                                        + analysis.facts().occurrences.len())
                                        as u64;
                                    use nepl3_core::budget::{Resource, StopReason};
                                    for (resource, cap, cost, reason) in [
                                        (
                                            Resource::Work,
                                            b.limits().work,
                                            130 + rows,
                                            StopReason::WorkLimit,
                                        ),
                                        (
                                            Resource::Nodes,
                                            b.limits().nodes,
                                            rows,
                                            StopReason::NodeLimit,
                                        ),
                                    ] {
                                        let mut exact = Budget::new(b.limits());
                                        exact.charge(resource, cap - cost).map_err(err)?;
                                        assert!(matches!(
                                            quality::check(&same, &mut exact).map_err(err)?,
                                            QualityOutcome::Rejected(
                                                quality::Rejection::Reference { .. }
                                            )
                                        ));
                                        let mut stopped = Budget::new(b.limits());
                                        stopped.charge(resource, cap - (cost - 1)).map_err(err)?;
                                        let Err(error) = quality::check(&same, &mut stopped) else {
                                            return Err("late Custom Reference stop".into());
                                        };
                                        assert_eq!(error.stop_reason(), Some(reason));
                                    }
                                }
                            }
                            if input == "let other x lambda x" || input == "let another x lambda x"
                            {
                                use nepl3_engine::analysis::insertion::declaration::{
                                    self, DeclarationOutcome,
                                };
                                let mut host = provider::RetargetBuiltIn(
                                    super::custom::query_host(true, false),
                                );
                                let named = fresh
                                    .trace_named_with_host(&mut host, b, codec.source_admission())
                                    .map_err(err)?;
                                let ReferenceOutcome::Unique(matched) =
                                    reference::correlate(&checked, &fresh, named.references(), b)
                                        .map_err(err)?
                                else {
                                    return Err("cleanup Reference".into());
                                };
                                let DeclarationOutcome::Same(same) = declaration::correlate(
                                    &matched,
                                    &original_named,
                                    &named,
                                    &prepared,
                                    &fresh,
                                    b,
                                )
                                .map_err(err)?
                                else {
                                    return Err("cleanup declaration".into());
                                };
                                let (analysis, _) =
                                    named.references().trace().complete().ok_or("complete")?;
                                assert!(
                                    analysis
                                        .facts()
                                        .occurrences
                                        .iter()
                                        .filter(|o| o.role == OccurrenceRole::Reference)
                                        .all(|o| matches!(
                                            o.resolution,
                                            ReferenceResolution::Resolved(EntityId(1))
                                        ))
                                );
                                assert!(analysis.result().open_inputs.is_empty());
                                match quality::check(&same, b).map_err(err)? {
                                    QualityOutcome::Rejected(quality::Rejection::Diagnostic {
                                        stage: quality::DiagnosticStage::Binding,
                                        diagnostic,
                                    }) if input == "let other x lambda x" => {
                                        assert_eq!(diagnostic.code, "UndefinedName")
                                    }
                                    QualityOutcome::Suitable(_)
                                        if input == "let another x lambda x" =>
                                    {
                                        assert!(
                                            !first
                                                .trace()
                                                .complete()
                                                .ok_or("old complete")?
                                                .0
                                                .result()
                                                .open_inputs
                                                .is_empty()
                                        )
                                    }
                                    _ => {
                                        return Err(
                                            "retained diagnostic or open-input cleanup".into()
                                        );
                                    }
                                }
                            }
                            let first_again =
                                first.final_reference(&first.key(), 0, b).map_err(err)?;
                            assert_ne!(
                                first_again.occurrence.resolution,
                                ReferenceResolution::Resolved(EntityId(100))
                            );
                        }
                    }

                    if input == "apply lambda x" {
                        assert!(matches!(
                        nepl3_engine::analysis::insertion::whole::check(checked.checked(), b),
                        Err(
                            nepl3_engine::analysis::insertion::whole::WholeInsertionError::Input(
                                nepl3_engine::parse::whole::WholeInputError::Recovered
                            )
                        )
                    ));
                    }

                    assert_eq!(
                        checked.checked().path().iter().any(|s| matches!(
                            s,
                            nepl3_engine::analysis::expected::ExpectedReadStep::Foreign { .. }
                        )),
                        foreign
                    );
                }
                // Identical bytes from another private draft do not confer parser provenance.
                let selected = name::select(&candidates, read, index, b).map_err(err)?;
                let second = name::prepare(
                    InsertionInput {
                        parsed: &old,
                        prepared: &prepared,
                    },
                    &request,
                    selected,
                    NameSpelling {
                        before,
                        spelling,
                        after: if input == "lambda trailingSource" {
                            " trailing"
                        } else {
                            ""
                        },
                    },
                    b,
                    ledger,
                )
                .map_err(err)?;
                assert_eq!(second.draft().snapshot().text(), draft.snapshot().text());
                let mut codec = FoundationCodec::new(registry, &empty, ledger).map_err(err)?;
                assert!(matches!(
                    name::check(
                        InsertionInput {
                            parsed: &old,
                            prepared: &prepared
                        },
                        &candidate,
                        &second,
                        &request,
                        "name-foreign-new",
                        &mut codec,
                        b
                    ),
                    Err(name::CheckError::Checked(
                        nepl3_engine::analysis::insertion::checked::CheckError::DraftMismatch
                    ))
                ));
                assert_eq!(source.text(), input);
                Ok(())
            })
        })?;
    }
    Ok(())
}

fn drive<'a>(
    parser: &mut RetainedParseSession<'a>,
    registry: &nepl3_core::schema::SchemaRegistry,
    tag: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<nepl3_engine::parse::RetainedParse<'a>, String> {
    let mut result = parser.read(b, a).map_err(err)?;
    let mut reservations = 0;
    loop {
        match result {
            RetainedParseExecution::Continue(parsed) => return Ok(parsed),
            RetainedParseExecution::Break(ParseReply {
                outcome: ParseOutcome::Await { call, continuation },
                ..
            }) => {
                let ProviderCall::Read {
                    operation,
                    request,
                    depth_base,
                    ..
                } = call.as_ref()
                else {
                    return Err("read provider".into());
                };
                let mut declared = SourceStore::default();
                for source in &request.sources {
                    declared.insert(source.clone()).map_err(err)?;
                }
                let snapshot = declared
                    .resolve(&request.snapshot)
                    .ok_or("provider source")?;
                let mut terminal = b
                    .with_depth_at_least(*depth_base, |b| {
                        let mut codec = FoundationCodec::new(registry, &declared, a)
                            .map_err(|_| nepl3_reader::runtime::ReaderError::Context)?;
                        let checked = request
                            .context
                            .check(&mut codec, &declared, registry, b)
                            .map_err(|_| nepl3_reader::runtime::ReaderError::Context)?;
                        nepl3_reader::builtin::provider::read(
                            operation,
                            ReadRequest {
                                snapshot,
                                start: request.start,
                                limit: request.limit,
                                final_input: request.final_input,
                                context: &checked,
                                state: &request.state,
                            },
                            registry,
                            &declared,
                            b,
                            a,
                        )
                    })
                    .map_err(err)?;
                reader::decorate(&mut terminal, snapshot, registry, b)?;
                result = parser
                    .resume(&continuation, ProviderReply::Read(Box::new(terminal)), b, a)
                    .map_err(err)?;
            }
            RetainedParseExecution::Break(ParseReply {
                outcome: ParseOutcome::Reserve { continuation, .. },
                ..
            }) => {
                reservations += 1;
                let reservation = nepl3_core::source::SourceReservation {
                    source_id: SourceId(format!("name-{tag}-{reservations}")),
                    revision: 0,
                    uri: format!("memory:name-{tag}-{reservations}"),
                };
                result = parser
                    .reserve(&continuation, &reservation, b, a)
                    .map_err(err)?;
            }
            RetainedParseExecution::Break(reply) => return Err(err(reply)),
        }
    }
}

#[test]
fn terminal_foreign_name_fields_remain_rejected_by_package_contract() -> Result<(), String> {
    let mut compiled = execution()?;
    let guest = compiled
        .package
        .forms
        .iter()
        .find(|v| v.spelling == "guest")
        .ok_or("guest")?;
    compiled.package.bindings[guest.binding.0 as usize] =
        nepl3_engine::package::Binding::Reference {
            namespace: "Value".into(),
            name: nepl3_engine::package::NameSelector::Field("value".into()),
        };
    // The existing Guest schema already accepts Foreign. Failure is the name-read contract.
    assert!(matches!(
        compiled.package.check(&compiled.registry, &mut budget()),
        Err(nepl3_engine::package::PackageError::InvalidBinding)
    ));
    Ok(())
}
