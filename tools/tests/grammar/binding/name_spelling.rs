use super::*;
#[path = "name_spelling/provider.rs"]
mod provider;
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
                        start: 0,
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
                        after: "",
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
                        start: 0,
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
                            assert_eq!(trace.trace().rows().len(), 1);
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
                        after: "",
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
                let terminal = b
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
