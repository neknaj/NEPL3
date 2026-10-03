use super::*;
use nepl3_engine::{
    binding::{
        self,
        probe::{ProbeOutcome, first_missing_reference},
    },
    package::{Binding, NameSelector, NamespacePolicy, ReadSpecId},
};

#[test]
fn probe_captures_only_an_executed_missing_reference_field() -> TestResult {
    for global in [false, true] {
        for (input, hit, blocked) in [
            ("let x", true, false),
            ("let あ", true, false),
            ("let x y", false, false),
            ("", false, true),
            ("let", false, true),
            ("let @", false, true),
        ] {
            retained::with_edited_package(
                input,
                |package| {
                    if global {
                        package.namespaces[0].policy = NamespacePolicy::Global;
                    }
                    package.forms[0].fields[1].read = ReadSpecId(0);
                    package.bindings[1] = Binding::Reference {
                        namespace: "Value".into(),
                        name: NameSelector::Field("body".into()),
                    };
                },
                |profile, environments, sources, source, entry| {
                    let states = [LanguageReaderState {
                        alias: "Host".into(),
                        state: NdfValue::Unit,
                    }];
                    let mut b = budget();
                    let mut admission = SourceAdmission::default();
                    let mut session = RetainedParseSession::new(
                        "probe".into(),
                        profile,
                        environments,
                        ParseRequest {
                            snapshot: source,
                            start: 0,
                            limit: input.len() as u64,
                            final_input: true,
                            entry,
                            states: &states,
                        },
                        sources,
                        &mut b,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                    let RetainedParseExecution::Continue(parsed) = session
                        .read(&mut b, &mut admission)
                        .map_err(|e| format!("{e:?}"))?
                    else {
                        return Err("parse".into());
                    };
                    let tree = parsed
                        .execution()
                        .tree()
                        .validate(profile, &mut b, &mut admission)
                        .map_err(|e| format!("{e:?}"))?;
                    if input == "let @" {
                        recovery_tokens_are_not_names(parsed.execution().tree(), profile)?;
                    }
                    let reply =
                        first_missing_reference("probe", &tree, profile, &mut b, &mut admission);
                    assert_eq!(reply.report.usage, b.usage());
                    if hit && !global {
                        stopped_capture(&tree, profile)?;
                    }
                    match &reply.outcome {
                        ProbeOutcome::Hit(found) if hit => {
                            assert_eq!(found.site().anchor.start(), input.len() as u64);
                            assert_eq!(found.site().anchor.end(), input.len() as u64);
                            assert_eq!(found.site().anchor.snapshot_ref(), source.identity());
                            assert_eq!(found.site().binding.0, 1);
                            assert!(core::ptr::eq(found.tree(), parsed.execution().tree()));
                            assert_eq!(found.site().stage != found.site().namespace_stage, global);
                            assert!(found.site().execution_step > 0);
                            assert_eq!(found.namespace().ok_or("namespace")?.name, "Value");
                            assert_eq!(
                                found.stages()[found.site().namespace_stage.0 as usize]
                                    .introduced
                                    .len(),
                                1
                            );
                            assert_eq!(found.sources().len(), 1);
                            assert!(matches!(
                                binding::analyze("strict", &tree, profile, &mut b, &mut admission)
                                    .outcome,
                                binding::BindingOutcome::Invalid {
                                    error: binding::BindingError::RecoveredTree,
                                    ..
                                }
                            ));
                        }
                        ProbeOutcome::NoHit if !hit && !blocked => {}
                        ProbeOutcome::Blocked(_) if blocked => {}
                        other => return Err(format!("{input:?}: {other:?}").into()),
                    }
                    Ok(())
                },
            )?;
        }
    }
    Ok(())
}

fn recovery_tokens_are_not_names(
    raw: &nepl3_engine::recovery::ParseTree,
    profile: &ResolvedParseProfile<'_>,
) -> TestResult {
    use nepl3_core::syntax::TokenRef;
    use nepl3_engine::recovery::RecoveryKind;
    for unexpected in [false, true] {
        let mut raw = raw.clone();
        let entry = raw.recovery[0].entries[0].clone();
        let RecoveryKind::Unparsed { span, .. } = entry.kind else {
            return Err("unparsed fixture".into());
        };
        let mut token = raw.bundle.tokens[0].clone();
        token.head = span.clone();
        token.payload = NdfValue::Text("@".into());
        let token_ref = TokenRef(raw.bundle.tokens.len() as u64);
        raw.bundle.tokens.push(token);
        let node = &mut raw.bundle.nodes[entry.node.0 as usize];
        node.token = Some(token_ref);
        node.head = Some(span);
        if unexpected {
            node.kind = "RecoveryUnexpected".into();
            raw.recovery[0].entries[0].kind = RecoveryKind::Unexpected { token: token_ref };
        }
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let checked = raw
            .validate(profile, &mut b, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
        assert!(matches!(
            first_missing_reference("token-recovery", &checked, profile, &mut b, &mut admission)
                .outcome,
            ProbeOutcome::Blocked(binding::BindingError::RecoveredTree)
        ));
    }
    Ok(())
}
fn stopped_capture(
    tree: &nepl3_engine::tree::ValidatedParseTree<'_>,
    profile: &ResolvedParseProfile<'_>,
) -> TestResult {
    use nepl3_core::budget::{Budget, Resource, StopReason};
    let mut measured = budget();
    let mut admission = SourceAdmission::default();
    assert!(matches!(
        first_missing_reference("cost", tree, profile, &mut measured, &mut admission).outcome,
        ProbeOutcome::Hit(_)
    ));
    let limits = measured.limits();
    for (resource, cost, limit, reason) in [
        (
            Resource::Work,
            measured.usage().work,
            limits.work,
            StopReason::WorkLimit,
        ),
        (
            Resource::AllocationUnits,
            measured.usage().allocation_units,
            limits.allocation_units,
            StopReason::AllocationLimit,
        ),
    ] {
        for available in [0, cost - 1] {
            let mut b = Budget::new(limits);
            let mut a = SourceAdmission::default();
            b.charge(resource, limit - available)
                .map_err(|e| format!("{e:?}"))?;
            assert!(
                matches!(first_missing_reference("cost", tree, profile, &mut b, &mut a).outcome, ProbeOutcome::Stopped(actual) if actual == reason)
            );
        }
    }
    let mut shallow = limits;
    shallow.depth = 0;
    let mut b = Budget::new(shallow);
    assert!(matches!(
        first_missing_reference(
            "cost",
            tree,
            profile,
            &mut b,
            &mut SourceAdmission::default()
        )
        .outcome,
        ProbeOutcome::Stopped(StopReason::DepthLimit)
    ));
    let mut b = Budget::new(limits);
    let mut changed_profile = profile.profile().clone();
    changed_profile.id.push_str("-different");
    let package = profile
        .language("Host", &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let packages = [package];
    let changed = changed_profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            profile.registry(),
            &mut b,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_ne!(changed.digest(), profile.digest());
    assert!(matches!(
        first_missing_reference(
            "cost",
            tree,
            &changed,
            &mut b,
            &mut SourceAdmission::default()
        )
        .outcome,
        ProbeOutcome::Blocked(binding::BindingError::Tree(
            nepl3_engine::tree::TreeError::Selection
        ))
    ));
    let mut b = Budget::new(limits);
    b.cancel();
    assert!(matches!(
        first_missing_reference(
            "cost",
            tree,
            profile,
            &mut b,
            &mut SourceAdmission::default()
        )
        .outcome,
        ProbeOutcome::Stopped(StopReason::Cancelled)
    ));
    Ok(())
}

fn ordered_case(
    input: &str,
    custom: bool,
    edit: impl FnOnce(&mut nepl3_engine::package::LanguagePackage),
    verify: impl FnOnce(nepl3_engine::binding::probe::ProbeReply) -> TestResult,
) -> TestResult {
    let exercise = |profile: &ResolvedParseProfile<'_>,
                    environments: &ParseEnvironmentSet<'_>,
                    sources: &SourceStore,
                    source: &SourceSnapshot,
                    entry: &nepl3_engine::package::EntryContext| {
        let states = [LanguageReaderState {
            alias: "Host".into(),
            state: NdfValue::Unit,
        }];
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let mut session = RetainedParseSession::new(
            "ordered".into(),
            profile,
            environments,
            ParseRequest {
                snapshot: source,
                start: 0,
                limit: input.len() as u64,
                final_input: true,
                entry,
                states: &states,
            },
            sources,
            &mut b,
        )
        .map_err(|e| format!("{e:?}"))?;
        let RetainedParseExecution::Continue(parsed) =
            session.read(&mut b, &mut a).map_err(|e| format!("{e:?}"))?
        else {
            return Err("parse".into());
        };
        let checked = parsed
            .execution()
            .tree()
            .validate(profile, &mut b, &mut a)
            .map_err(|e| format!("{e:?}"))?;
        verify(first_missing_reference(
            "ordered", &checked, profile, &mut b, &mut a,
        ))
    };
    if custom {
        retained::with_custom_edited_package(input, edit, exercise)
    } else {
        retained::with_edited_package(input, edit, exercise)
    }
}
#[test]
fn first_runtime_action_is_not_a_node_wide_or_complete_binding_claim() -> TestResult {
    use nepl3_engine::package::BindingId;
    for (actions, expected) in [
        (vec![BindingId(1), BindingId(0)], 0),
        (vec![BindingId(0), BindingId(1), BindingId(1)], 1),
    ] {
        ordered_case(
            "let x",
            false,
            |p| {
                p.forms[0].fields[1].read = ReadSpecId(0);
                p.bindings[1] = Binding::Reference {
                    namespace: "Value".into(),
                    name: NameSelector::Field("body".into()),
                };
                p.bindings[2] = Binding::Group(actions);
            },
            |reply| {
                let ProbeOutcome::Hit(hit) = reply.outcome else {
                    return Err("hit".into());
                };
                assert_eq!(
                    hit.stages()[hit.site().namespace_stage.0 as usize]
                        .introduced
                        .len(),
                    expected
                );
                assert_eq!(hit.site().execution_step, 2 + expected as u64);
                Ok(())
            },
        )?;
    }
    ordered_case(
        "let x",
        false,
        |p| {
            p.forms[0].fields[1].read = ReadSpecId(0);
            p.bindings[1] = Binding::Reference {
                namespace: "Value".into(),
                name: NameSelector::Field("body".into()),
            };
            p.bindings[2] = Binding::Group(vec![BindingId(0), BindingId(0), BindingId(1)]);
        },
        |reply| {
            let ProbeOutcome::Hit(hit) = reply.outcome else {
                return Err("repeated prefix hit".into());
            };
            let last = &hit.stages()[hit.site().namespace_stage.0 as usize];
            let prior = &hit.stages()[last.previous.ok_or("prior bind")?.0 as usize];
            assert_eq!(last.introduced.len(), 1);
            assert_eq!(prior.introduced.len(), 1);
            assert_ne!(last.introduced[0], prior.introduced[0]);
            assert_eq!(hit.site().execution_step, 4);
            Ok(())
        },
    )?;
    ordered_case(
        "let x",
        false,
        |p| {
            p.forms[0].fields[1].read = ReadSpecId(0);
            p.bindings[2] = Binding::None;
        },
        |reply| {
            assert!(matches!(reply.outcome, ProbeOutcome::NoHit));
            Ok(())
        },
    )?;
    ordered_case(
        "let",
        false,
        |p| {
            p.bindings[0] = Binding::Export {
                namespace: "Value".into(),
                name: NameSelector::Field("name".into()),
            }
        },
        |reply| {
            assert!(matches!(
                reply.outcome,
                ProbeOutcome::Blocked(binding::BindingError::RecoveredTree)
            ));
            Ok(())
        },
    )
}
#[test]
fn custom_is_blocked_only_when_executed_and_never_called_by_the_probe() -> TestResult {
    use nepl3_engine::package::BindingId;
    for before in [false, true] {
        ordered_case(
            "let x",
            true,
            |p| {
                p.forms[0].fields[1].read = ReadSpecId(0);
                p.bindings[1] = Binding::Reference {
                    namespace: "Value".into(),
                    name: NameSelector::Field("body".into()),
                };
                let custom = p.bindings[2].clone();
                let id = BindingId(p.bindings.len() as u64);
                p.bindings.push(custom);
                p.bindings[2] = Binding::Group(if before {
                    vec![id, BindingId(0), BindingId(1)]
                } else {
                    vec![BindingId(0), BindingId(1), id]
                });
            },
            |reply| {
                if before {
                    assert!(matches!(
                        reply.outcome,
                        ProbeOutcome::Blocked(binding::BindingError::MissingProvider)
                    ));
                } else {
                    assert!(matches!(reply.outcome, ProbeOutcome::Hit(_)));
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}
