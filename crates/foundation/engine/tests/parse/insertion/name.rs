use super::*;
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_engine::{
    analysis::{
        insertion::name::{self, NameSpelling},
        probe::{
            candidates,
            read::{self, ReadOutcome},
        },
    },
    package::{Binding, NameSelector, ReadSpec, ReadSpecId},
};
#[derive(Clone, Copy)]
enum ResultKind {
    Good,
    LocalOnly,
    Unresolved,
    Head,
    Payload,
    Spelling,
    Recovery,
}
#[test]
fn explicit_name_spelling_checks_actual_text_payload_and_excludes_affixes() -> TestResult {
    for (old_text, text, before, spelling, after, expected) in [
        ("let x", false, " ", "x", "", ResultKind::Good),
        ("let x", false, "y ", "x", "", ResultKind::Unresolved),
        ("let あ", false, " ", "あ", "", ResultKind::Good),
        ("let x", false, " ", "x", " tail", ResultKind::LocalOnly),
        ("let x tail", false, " ", "x", "", ResultKind::LocalOnly),
        ("let x", false, " ", "y", "", ResultKind::Payload),
        ("let x", false, " ", "@", "", ResultKind::Recovery),
        ("let x", false, " x ", "bad", "", ResultKind::Head),
        ("let x", false, " ", "x", "y", ResultKind::Head),
        ("let x", false, " ", "", "x", ResultKind::Spelling),
        (
            "let \"a b\"",
            true,
            " ",
            "\"a\\u{20}b\"",
            "",
            ResultKind::Good,
        ),
        (
            "let \"a b\"",
            true,
            " ",
            "\"wrong\"",
            "",
            ResultKind::Payload,
        ),
    ] {
        retained::with_edited_package(
            old_text,
            |p| {
                p.forms[0].fields[1].read = ReadSpecId(0);
                p.bindings[1] = Binding::Reference {
                    namespace: "Value".into(),
                    name: NameSelector::Field("body".into()),
                };
                if text && let ReadSpec::Builtin { reader, .. } = &mut p.reads[0] {
                    *reader = nepl3_reader::builtin::BuiltinReader::Text;
                }
            },
            |profile, environments, sources, source, entry| {
                let old_limit = if old_text == "let x tail" {
                    5
                } else {
                    old_text.len() as u64
                };
                let mut b = budget();
                let mut ledger = SourceAdmission::default();
                let states = [LanguageReaderState {
                    alias: "Host".into(),
                    state: NdfValue::Unit,
                }];
                let mut session = RetainedParseSession::new(
                    "name-old".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: source,
                        start: 0,
                        limit: old_limit,
                        final_input: true,
                        entry,
                        states: &states,
                    },
                    sources,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let old = drive(&mut session, "old", &mut b, &mut ledger)?;
                let empty = SourceStore::default();
                let prepared = {
                    let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
                        .map_err(|e| format!("{e:?}"))?;
                    analysis::prepare(
                        "name-old",
                        old.execution().tree(),
                        BindingOptions,
                        b.limits(),
                        profile,
                        &mut codec,
                        &mut b,
                    )
                    .map_err(|e| format!("{e:?}"))?
                };
                let bound = prepared
                    .probe_missing_reference(&mut b, &mut ledger)
                    .map_err(|e| format!("{e:?}"))?;
                let request = ExpectedReadRequest {
                    key: prepared.key(),
                    source: source.reference(),
                    offset: old_limit,
                };
                let correlation = read::correlate(&bound, &prepared, &request, &mut b, &mut ledger)
                    .map_err(|e| format!("{e:?}"))?;
                let ReadOutcome::Hit(read) = correlation.outcome() else {
                    return Err("read hit".into());
                };
                let candidates = candidates::names(
                    &bound,
                    &candidates::ProbeCandidateRequest {
                        key: prepared.key(),
                        source: &request.source,
                        offset: request.offset,
                        prefix: "",
                    },
                    &mut b,
                    &mut ledger,
                )
                .map_err(|e| format!("{e:?}"))?;
                let limits = b.limits();
                let other_candidates = candidates::names(
                    &bound,
                    &candidates::ProbeCandidateRequest {
                        key: prepared.key(),
                        source: &request.source,
                        offset: 0,
                        prefix: "",
                    },
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default(),
                )
                .map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    name::select(&other_candidates, read, 0, &mut Budget::new(limits)),
                    Err(name::NameError::ChoiceUnavailable)
                ));
                let mut cancelled_execution = Budget::new(limits);
                cancelled_execution.cancel();
                let stopped_bound = prepared
                    .probe_missing_reference(
                        &mut cancelled_execution,
                        &mut SourceAdmission::default(),
                    )
                    .map_err(|e| format!("{e:?}"))?;
                let stopped_candidates = candidates::names(
                    &stopped_bound,
                    &candidates::ProbeCandidateRequest {
                        key: prepared.key(),
                        source: &request.source,
                        offset: request.offset,
                        prefix: "",
                    },
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default(),
                )
                .map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    name::select(&stopped_candidates, read, 0, &mut Budget::new(limits)),
                    Err(name::NameError::ChoiceUnavailable)
                ));
                let mut mismatch_limits = limits;
                mismatch_limits.work = 0;
                mismatch_limits.depth = 0;
                let mut mismatched = Budget::new(mismatch_limits);
                mismatched.cancel();
                assert!(matches!(
                    name::select(&candidates, read, 0, &mut mismatched),
                    Err(name::NameError::Insertion(InsertionError::Access(
                        nepl3_engine::analysis::BindingAccessError::LimitsMismatch
                    )))
                ));
                assert_eq!(mismatched.usage().work, 0);
                let mut cancelled_select = Budget::new(limits);
                cancelled_select.cancel();
                assert!(matches!(
                    name::select(&candidates, read, 0, &mut cancelled_select),
                    Err(name::NameError::Stopped(StopReason::Cancelled))
                ));
                let choice =
                    name::select(&candidates, read, 0, &mut b).map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    name::prepare(
                        InsertionInput {
                            parsed: &old,
                            prepared: &prepared
                        },
                        &request,
                        choice,
                        NameSpelling {
                            before,
                            spelling,
                            after
                        },
                        &mut cancelled_select,
                        &mut ledger
                    ),
                    Err(name::NameError::Stopped(StopReason::Cancelled))
                ));

                let choice =
                    name::select(&candidates, read, 0, &mut b).map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    name::prepare(
                        InsertionInput {
                            parsed: &old,
                            prepared: &prepared
                        },
                        &request,
                        choice,
                        NameSpelling {
                            before,
                            spelling,
                            after
                        },
                        &mut mismatched,
                        &mut ledger
                    ),
                    Err(name::NameError::Insertion(InsertionError::Access(
                        nepl3_engine::analysis::BindingAccessError::LimitsMismatch
                    )))
                ));
                assert_eq!(mismatched.usage().work, 0);

                assert!(matches!(
                    name::select(&candidates, read, usize::MAX, &mut Budget::new(limits)),
                    Err(name::NameError::ChoiceUnavailable)
                ));
                let second_bound = prepared
                    .probe_missing_reference(
                        &mut Budget::new(limits),
                        &mut SourceAdmission::default(),
                    )
                    .map_err(|e| format!("{e:?}"))?;
                let second_read = read::correlate(
                    &second_bound,
                    &prepared,
                    &request,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default(),
                )
                .map_err(|e| format!("{e:?}"))?;
                let ReadOutcome::Hit(second_read) = second_read.outcome() else {
                    return Err("second hit".into());
                };
                assert!(matches!(
                    name::select(&candidates, second_read, 0, &mut Budget::new(limits)),
                    Err(name::NameError::ProofMismatch)
                ));
                for change in 0..5 {
                    let mut changed = request.clone();
                    match change {
                        0 => changed.offset += 1,
                        1 => changed.source.revision += 1,
                        2 => changed.source.digest.0[0] ^= 1,
                        3 => changed.source.source_id.0.push_str("-wrong"),
                        _ => changed.key.request_digest.0[0] ^= 1,
                    }
                    let choice =
                        name::select(&candidates, read, 0, &mut b).map_err(|e| format!("{e:?}"))?;
                    let source_usage = b.usage().source_bytes;
                    let rejected = name::prepare(
                        InsertionInput {
                            parsed: &old,
                            prepared: &prepared,
                        },
                        &changed,
                        choice,
                        NameSpelling {
                            before,
                            spelling,
                            after,
                        },
                        &mut b,
                        &mut ledger,
                    );
                    assert!(matches!(
                        rejected,
                        Err(name::NameError::RequestMismatch | name::NameError::ProofMismatch)
                    ));
                    assert_eq!(b.usage().source_bytes, source_usage);
                }
                let selected =
                    name::select(&candidates, read, 0, &mut b).map_err(|e| format!("{e:?}"))?;
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
                        after,
                    },
                    &mut b,
                    &mut ledger,
                );
                if matches!(expected, ResultKind::Spelling) {
                    assert!(matches!(proposal, Err(name::NameError::Spelling)));
                    return Ok(());
                }
                let proposal = proposal.map_err(|e| format!("{e:?}"))?;
                let draft = proposal.draft();
                let mut session = RetainedParseSession::new(
                    "name-new".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: draft.snapshot(),
                        start: 0,
                        limit: draft.limit(),
                        final_input: true,
                        entry,
                        states: &states,
                    },
                    draft.sources(),
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let candidate = drive(&mut session, "new", &mut b, &mut ledger)?;
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
                    .map_err(|e| format!("{e:?}"))?;
                let result = name::check(
                    InsertionInput {
                        parsed: &old,
                        prepared: &prepared,
                    },
                    &candidate,
                    &proposal,
                    &request,
                    "name-new",
                    &mut codec,
                    &mut b,
                );
                match expected {
                    ResultKind::Good | ResultKind::LocalOnly | ResultKind::Unresolved => {
                        let checked = result.map_err(|e| format!("{e:?}"))?;
                        assert_eq!(checked.checked().path(), read.expected().path);
                        assert_eq!(checked.report().usage, b.usage());
                        // The before-affix extends the existing declaration x to xy.
                        // Correct spelling and complete parsing do not prove name resolution.
                        if matches!(expected, ResultKind::Unresolved) {
                            assert_eq!(candidate.seed().request().snapshot.text(), "let xy x");
                            nepl3_engine::analysis::insertion::whole::check(
                                checked.checked(),
                                &mut b,
                            )
                            .map_err(|e| format!("{e:?}"))?;
                            let reply = nepl3_engine::analysis::insertion::binding::execute(
                                checked.checked(),
                                "name-new",
                                &mut codec,
                                &mut b,
                            )
                            .map_err(|e| format!("{e:?}"))?;
                            assert!(matches!(
                                reply.reply().outcome,
                                nepl3_engine::binding::BindingOutcome::Complete { .. }
                            ));
                            let actual = reply
                                .for_source(
                                    &reply.key(),
                                    &candidate.seed().request().snapshot.reference(),
                                    &mut b,
                                )
                                .map_err(|e| format!("{e:?}"))?;
                            let references: Vec<_> = actual
                                .facts()
                                .occurrences
                                .iter()
                                .filter(|o| o.role == nepl3_core::facts::OccurrenceRole::Reference)
                                .collect();
                            assert_eq!(references.len(), 1);
                            let reference = references[0];
                            assert_eq!(reference.name, "x");
                            assert_eq!(Some(&reference.span), checked.checked().head());
                            assert_eq!(
                                reference.resolution,
                                nepl3_core::facts::ReferenceResolution::Unresolved("x".into())
                            );
                            assert_eq!(actual.facts().entities.len(), 1);
                            assert_eq!(actual.facts().entities[0].name, "xy");
                        }

                        if matches!(expected, ResultKind::LocalOnly) {
                            assert!(matches!(nepl3_engine::analysis::insertion::whole::check(checked.checked(), &mut b), Err(nepl3_engine::analysis::insertion::whole::WholeInsertionError::Input(nepl3_engine::parse::whole::WholeInputError::PartialRange | nepl3_engine::parse::whole::WholeInputError::Unconsumed { .. }))));
                        }

                        assert!(core::ptr::eq(
                            checked.choice().candidate(),
                            proposal.choice().candidate()
                        ));
                    }
                    ResultKind::Head => assert!(matches!(result, Err(name::CheckError::Head))),
                    ResultKind::Payload => {
                        assert!(matches!(result, Err(name::CheckError::Payload)))
                    }
                    ResultKind::Recovery => assert!(matches!(
                        result,
                        Err(name::CheckError::Checked(
                            nepl3_engine::analysis::insertion::checked::CheckError::Insertion(
                                InsertionError::RecoveryTarget
                            )
                        ))
                    )),
                    ResultKind::Spelling => return Err("handled empty spelling".into()),
                }
                if old_text == "let x" && spelling == "x" && before == " " && after.is_empty() {
                    let original_report = bound.reply().report.clone();
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &empty, &mut admission)
                            .map_err(|e| format!("{e:?}"))?;
                    let mut measured = Budget::new(limits);
                    name::check(
                        InsertionInput {
                            parsed: &old,
                            prepared: &prepared,
                        },
                        &candidate,
                        &proposal,
                        &request,
                        "name-new",
                        &mut codec,
                        &mut measured,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                    let usage = measured.usage();
                    let mut cancelled = Budget::new(limits);
                    cancelled.cancel();
                    assert!(matches!(
                        name::check(
                            InsertionInput {
                                parsed: &old,
                                prepared: &prepared
                            },
                            &candidate,
                            &proposal,
                            &request,
                            "name-new",
                            &mut codec,
                            &mut cancelled
                        ),
                        Err(name::CheckError::Name(name::NameError::Stopped(
                            StopReason::Cancelled
                        )))
                    ));

                    for extra in [0, 1] {
                        let mut admission = SourceAdmission::default();
                        let mut codec =
                            FoundationCodec::new(profile.registry(), &empty, &mut admission)
                                .map_err(|e| format!("{e:?}"))?;
                        let mut depth = Budget::new(limits);
                        let result =
                            depth.with_depth_at_least(limits.depth - usage.depth + extra, |b| {
                                name::check(
                                    InsertionInput {
                                        parsed: &old,
                                        prepared: &prepared,
                                    },
                                    &candidate,
                                    &proposal,
                                    &request,
                                    "name-new",
                                    &mut codec,
                                    b,
                                )
                            });
                        if extra == 0 {
                            result.map_err(|e| format!("{e:?}"))?;
                            assert_eq!(depth.poll(), Ok(()));
                        } else {
                            assert!(result.is_err());
                            assert_eq!(depth.poll(), Err(StopReason::DepthLimit));
                        }
                    }

                    for (resource, total, used, reason) in [
                        (
                            Resource::Work,
                            limits.work,
                            usage.work,
                            StopReason::WorkLimit,
                        ),
                        (
                            Resource::AllocationUnits,
                            limits.allocation_units,
                            usage.allocation_units,
                            StopReason::AllocationLimit,
                        ),
                        (
                            Resource::Nodes,
                            limits.nodes,
                            usage.nodes,
                            StopReason::NodeLimit,
                        ),
                        (
                            Resource::SourceBytes,
                            limits.source_bytes,
                            usage.source_bytes,
                            StopReason::SourceLimit,
                        ),
                        (
                            Resource::OutputBytes,
                            limits.output_bytes,
                            usage.output_bytes,
                            StopReason::OutputLimit,
                        ),
                    ] {
                        assert!(used > 0, "resource must be exercised: {resource:?}");
                        let mut admission = SourceAdmission::default();
                        let mut codec =
                            FoundationCodec::new(profile.registry(), &empty, &mut admission)
                                .map_err(|e| format!("{e:?}"))?;
                        let mut limited = Budget::new(limits);
                        limited
                            .charge(resource, total - used + 1)
                            .map_err(|e| format!("{e:?}"))?;
                        let failed = name::check(
                            InsertionInput {
                                parsed: &old,
                                prepared: &prepared,
                            },
                            &candidate,
                            &proposal,
                            &request,
                            "name-new",
                            &mut codec,
                            &mut limited,
                        );
                        if resource == Resource::Work {
                            assert!(matches!(
                                failed,
                                Err(name::CheckError::Stopped(StopReason::WorkLimit))
                            ));
                        } else {
                            assert!(failed.is_err());
                        }
                        assert_eq!(limited.poll(), Err(reason));
                        assert_eq!(bound.reply().report, original_report);
                    }
                    let mut changed = limits;
                    changed.work = 0;
                    changed.depth = 0;
                    let mut cancelled = Budget::new(changed);
                    cancelled.cancel();
                    let failed = name::check(
                        InsertionInput {
                            parsed: &old,
                            prepared: &prepared,
                        },
                        &candidate,
                        &proposal,
                        &request,
                        "name-new",
                        &mut codec,
                        &mut cancelled,
                    );
                    assert!(matches!(
                        failed,
                        Err(name::CheckError::Name(name::NameError::Insertion(
                            InsertionError::Access(
                                nepl3_engine::analysis::BindingAccessError::LimitsMismatch
                            )
                        )))
                    ));
                    assert_eq!(cancelled.usage().work, 0);
                }
                if old_text == "let x" && spelling == "x" && before == " " && after.is_empty() {
                    let mut duplicate_parser = RetainedParseSession::new(
                        "independent-old".into(),
                        profile,
                        environments,
                        ParseRequest {
                            snapshot: source,
                            start: 0,
                            limit: old_limit,
                            final_input: true,
                            entry,
                            states: &states,
                        },
                        sources,
                        &mut b,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                    let duplicate = drive(
                        &mut duplicate_parser,
                        "independent-old",
                        &mut b,
                        &mut ledger,
                    )?;
                    let selected =
                        name::select(&candidates, read, 0, &mut b).map_err(|e| format!("{e:?}"))?;
                    assert!(matches!(
                        name::prepare(
                            InsertionInput {
                                parsed: &duplicate,
                                prepared: &prepared
                            },
                            &request,
                            selected,
                            NameSpelling {
                                before,
                                spelling,
                                after
                            },
                            &mut b,
                            &mut ledger
                        ),
                        Err(name::NameError::ProofMismatch)
                    ));
                }
                assert_eq!(source.text(), old_text);
                assert_eq!(sources.snapshots().len(), 1);
                Ok(())
            },
        )?;
    }
    Ok(())
}

fn drive<'a>(
    session: &mut RetainedParseSession<'a>,
    tag: &str,
    b: &mut Budget,
    ledger: &mut SourceAdmission,
) -> Result<nepl3_engine::parse::RetainedParse<'a>, String> {
    let mut reply = session.read(b, ledger).map_err(|e| format!("{e:?}"))?;
    let mut count = 0;
    loop {
        match reply {
            RetainedParseExecution::Continue(parsed) => return Ok(parsed),
            RetainedParseExecution::Break(ParseReply {
                outcome: ParseOutcome::Reserve { continuation, .. },
                ..
            }) => {
                count += 1;
                let reservation = nepl3_core::source::SourceReservation {
                    source_id: SourceId(format!("name-{tag}-{count}")),
                    revision: 0,
                    uri: format!("memory:name-{tag}-{count}"),
                };
                reply = session
                    .reserve(&continuation, &reservation, b, ledger)
                    .map_err(|e| format!("{e:?}"))?;
            }
            RetainedParseExecution::Break(reply) => return Err(format!("parse reply {reply:?}")),
        }
    }
}

#[test]
fn non_text_name_reader_is_rejected_before_proof_construction() -> TestResult {
    let (mut package, registry) = support::fixture()?;
    let integer_kind = registry
        .kind_id(&package.schema, "Token:Integer")
        .map_err(|e| format!("{e:?}"))?;
    let mut numeric = package.reads[0].clone();
    let ReadSpec::Builtin {
        reader, token_kind, ..
    } = &mut numeric
    else {
        return Err("builtin fixture".into());
    };
    *reader = nepl3_reader::builtin::BuiltinReader::Nat;
    token_kind.local_kind = integer_kind;
    package.forms[0].fields[1].read = ReadSpecId(package.reads.len() as u64);
    package.reads.push(numeric);
    package.bindings[1] = Binding::Reference {
        namespace: "Value".into(),
        name: NameSelector::Field("body".into()),
    };
    assert!(matches!(
        package.check(&registry, &mut budget()),
        Err(nepl3_engine::package::PackageError::InvalidBinding)
    ));
    Ok(())
}
