use super::*;
use nepl3_core::source::{Digest, TextEdit};
use nepl3_engine::{
    analysis::{BindingOptions, expected::ExpectedReadRequest, insertion::*},
    portable::analysis,
};

struct Case<'a> {
    old: &'a str,
    new: &'a str,
    at: u64,
    inserted: &'a str,
    limit: Option<u64>,
    expected: Result<(u64, u64), InsertionError>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Mutation {
    None,
    StaleKey,
    WrongPrepared,
    WrongDigest,
    Revision,
    Uri,
    Work,
    Allocation,
    Depth,
    Cancelled,
}
fn check(case: Case<'_>) -> TestResult {
    check_with(case, Mutation::None)
}
fn check_with(case: Case<'_>, mutation: Mutation) -> TestResult {
    super::retained::with_context(case.old, |profile, environments, sources, source, entry| {
        let states = [LanguageReaderState {
            alias: "Host".into(),
            state: NdfValue::Unit,
        }];
        let next = SourceSnapshot::new(
            SourceId("input".into()),
            if mutation == Mutation::Revision { 2 } else { 1 },
            if mutation == Mutation::Uri {
                "memory:other".into()
            } else {
                "memory:input".into()
            },
            case.new.as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut next_sources = SourceStore::default();
        next_sources
            .insert(next.clone())
            .map_err(|e| format!("{e:?}"))?;
        let limit = case.limit.unwrap_or(case.old.len() as u64);
        let mut old_session = RetainedParseSession::new(
            "insertion-old".into(),
            profile,
            environments,
            ParseRequest {
                snapshot: source,
                start: 0,
                limit,
                final_input: true,
                entry,
                states: &states,
            },
            sources,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let RetainedParseExecution::Continue(old) = old_session
            .read(&mut budget(), &mut SourceAdmission::default())
            .map_err(|e| format!("{e:?}"))?
        else {
            return Err("original terminal syntax".into());
        };
        let mut new_session = RetainedParseSession::new(
            "insertion-new".into(),
            profile,
            environments,
            ParseRequest {
                snapshot: &next,
                start: 0,
                limit: limit + case.inserted.len() as u64,
                final_input: true,
                entry,
                states: &states,
            },
            &next_sources,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let RetainedParseExecution::Continue(new) = new_session
            .read(&mut budget(), &mut SourceAdmission::default())
            .map_err(|e| format!("{e:?}"))?
        else {
            return Err("candidate terminal syntax".into());
        };
        if case.old.is_empty() && case.new == "@" {
            use nepl3_engine::recovery::RecoveryKind;
            assert!(
                new.execution()
                    .tree()
                    .recovery
                    .iter()
                    .flat_map(|v| &v.entries)
                    .any(|entry| matches!(
                        entry.kind,
                        RecoveryKind::Unexpected { .. } | RecoveryKind::Unparsed { .. }
                    ))
            );
            let tree = new.execution().tree();
            let root = &tree.bundle.nodes[tree.bundle.root.0 as usize];
            assert!(
                root.cover
                    .as_ref()
                    .is_some_and(|span| span.start() < span.end())
            );
        }
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
        let mut limits = budget().limits();
        match mutation {
            Mutation::Work => limits.work = 0,
            Mutation::Allocation => limits.allocation_units = 0,
            Mutation::Depth => limits.depth = 0,
            _ => {}
        }
        let old_prepared = analysis::prepare(
            "insertion-old",
            old.execution().tree(),
            BindingOptions,
            limits,
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let new_prepared = analysis::prepare(
            "insertion-new",
            new.execution().tree(),
            BindingOptions,
            limits,
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut request = ExpectedReadRequest {
            key: old_prepared.key(),
            source: source.reference(),
            offset: case.at,
        };
        let mut edit = TextEdit {
            span: source
                .span(case.at, case.at)
                .map_err(|e| format!("{e:?}"))?,
            expected_digest: Digest::of(b""),
            replacement: case.inserted.into(),
        };
        if mutation == Mutation::StaleKey {
            request.key.request_digest = Digest::of(b"stale");
        }
        if mutation == Mutation::WrongDigest {
            edit.expected_digest = Digest::of(b"wrong");
        }
        let mut b = nepl3_core::budget::Budget::new(limits);
        if mutation == Mutation::Cancelled {
            b.cancel();
        }
        let result = observe(
            InsertionInput {
                parsed: &old,
                prepared: if mutation == Mutation::WrongPrepared {
                    &new_prepared
                } else {
                    &old_prepared
                },
            },
            InsertionInput {
                parsed: &new,
                prepared: &new_prepared,
            },
            &request,
            &edit,
            &mut b,
            &mut SourceAdmission::default(),
        );
        let actual = result.map(|proof| {
            assert!(core::ptr::eq(proof.original(), &old));
            assert!(core::ptr::eq(proof.candidate(), &new));
            assert_eq!(proof.report().usage, b.usage());
            (proof.cover().start(), proof.cover().end())
        });
        assert_eq!(actual, case.expected, "{} -> {}", case.old, case.new);
        Ok(())
    })
}

#[test]
fn direct_inserted_occurrence_can_complete_or_leave_new_missing_children() -> TestResult {
    for case in [
        Case {
            old: "let x",
            new: "let x x",
            at: 5,
            inserted: " x",
            limit: None,
            expected: Ok((6, 7)),
        },
        Case {
            old: "",
            new: "let",
            at: 0,
            inserted: "let",
            limit: None,
            expected: Ok((0, 3)),
        },
        Case {
            old: "let x",
            new: "let x let",
            at: 5,
            inserted: " let",
            limit: None,
            expected: Ok((6, 9)),
        },
        Case {
            old: "let",
            new: "let x",
            at: 3,
            inserted: " x",
            limit: None,
            expected: Ok((4, 5)),
        },
    ] {
        check(case)?;
    }
    Ok(())
}
#[test]
fn direct_observation_rejects_changed_bytes_joined_heads_and_recovery_targets() -> TestResult {
    for case in [
        Case {
            old: "let x",
            new: "let y x",
            at: 5,
            inserted: " x",
            limit: None,
            expected: Err(InsertionError::EditMismatch),
        },
        Case {
            old: "let x",
            new: "let x",
            at: 5,
            inserted: "",
            limit: None,
            expected: Err(InsertionError::EditMismatch),
        },
        Case {
            old: "let x",
            new: "let x ",
            at: 5,
            inserted: " ",
            limit: None,
            expected: Err(InsertionError::RecoveryTarget),
        },
        Case {
            old: "",
            new: "@",
            at: 0,
            inserted: "@",
            limit: None,
            expected: Err(InsertionError::RecoveryTarget),
        },
        Case {
            old: "let",
            new: "letx",
            at: 3,
            inserted: "x",
            limit: None,
            expected: Err(InsertionError::OccurrenceMismatch),
        },
    ] {
        check(case)?;
    }
    Ok(())
}
#[test]
fn direct_observation_checks_snapshot_bytes_outside_the_parse_limit() -> TestResult {
    check(Case {
        old: "letBAD",
        new: "let xBAD",
        at: 3,
        inserted: " x",
        limit: Some(3),
        expected: Ok((4, 5)),
    })?;
    check(Case {
        old: "letBAD",
        new: "let xOTHER",
        at: 3,
        inserted: " x",
        limit: Some(3),
        expected: Err(InsertionError::EditMismatch),
    })
}

#[test]
fn insertion_identity_gates_and_stops_never_publish_an_observation() -> TestResult {
    use nepl3_core::budget::StopReason;
    use nepl3_engine::analysis::BindingAccessError;
    for (mutation, expected) in [
        (
            Mutation::StaleKey,
            InsertionError::Access(BindingAccessError::StaleAnalysis),
        ),
        (Mutation::WrongPrepared, InsertionError::ProofMismatch),
        (Mutation::WrongDigest, InsertionError::EditMismatch),
        (Mutation::Revision, InsertionError::EditMismatch),
        (Mutation::Uri, InsertionError::EditMismatch),
        (
            Mutation::Work,
            InsertionError::Stopped(StopReason::WorkLimit),
        ),
        (
            Mutation::Allocation,
            InsertionError::Stopped(StopReason::AllocationLimit),
        ),
        (
            Mutation::Depth,
            InsertionError::Stopped(StopReason::DepthLimit),
        ),
        (
            Mutation::Cancelled,
            InsertionError::Stopped(StopReason::Cancelled),
        ),
    ] {
        check_with(
            Case {
                old: "let x",
                new: "let x x",
                at: 5,
                inserted: " x",
                limit: None,
                expected: Err(expected),
            },
            mutation,
        )?;
    }
    Ok(())
}

#[test]
fn insertion_compares_actual_initial_states_with_a_finite_depth_budget() -> TestResult {
    use nepl3_core::{budget::StopReason, schema::TypeDescriptor};
    for (changed, depth) in [(false, 1000), (true, 1000), (false, 8)] {
        super::retained::with_state_context(
            "let x",
            TypeDescriptor::NdfValue,
            |profile, environments, sources, source, entry| {
                let mut state = NdfValue::Unit;
                for _ in 0..40 {
                    state = NdfValue::Some(Box::new(state));
                }
                let old_states = [LanguageReaderState {
                    alias: "Host".into(),
                    state: state.clone(),
                }];
                let new_states = [LanguageReaderState {
                    alias: "Host".into(),
                    state: if changed { NdfValue::Bool(true) } else { state },
                }];
                let next = SourceSnapshot::new(
                    SourceId("input".into()),
                    1,
                    "memory:input".into(),
                    b"let x x".to_vec(),
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?;
                let mut next_sources = SourceStore::default();
                next_sources
                    .insert(next.clone())
                    .map_err(|e| format!("{e:?}"))?;
                let mut old_session = RetainedParseSession::new(
                    "state-old".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: source,
                        start: 0,
                        limit: 5,
                        final_input: true,
                        entry,
                        states: &old_states,
                    },
                    sources,
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?;
                let RetainedParseExecution::Continue(old) = old_session
                    .read(&mut budget(), &mut SourceAdmission::default())
                    .map_err(|e| format!("{e:?}"))?
                else {
                    return Err("old".into());
                };
                let mut new_session = RetainedParseSession::new(
                    "state-new".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: &next,
                        start: 0,
                        limit: 7,
                        final_input: true,
                        entry,
                        states: &new_states,
                    },
                    &next_sources,
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?;
                let RetainedParseExecution::Continue(new) = new_session
                    .read(&mut budget(), &mut SourceAdmission::default())
                    .map_err(|e| format!("{e:?}"))?
                else {
                    return Err("new".into());
                };
                let mut limits = budget().limits();
                limits.depth = depth;
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(|e| format!("{e:?}"))?;
                let a = analysis::prepare(
                    "state-old",
                    old.execution().tree(),
                    BindingOptions,
                    limits,
                    profile,
                    &mut codec,
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?;
                let c = analysis::prepare(
                    "state-new",
                    new.execution().tree(),
                    BindingOptions,
                    limits,
                    profile,
                    &mut codec,
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?;
                let mut b = nepl3_core::budget::Budget::new(limits);
                let result = observe(
                    InsertionInput {
                        parsed: &old,
                        prepared: &a,
                    },
                    InsertionInput {
                        parsed: &new,
                        prepared: &c,
                    },
                    &ExpectedReadRequest {
                        key: a.key(),
                        source: source.reference(),
                        offset: 5,
                    },
                    &TextEdit {
                        span: source.span(5, 5).map_err(|e| format!("{e:?}"))?,
                        expected_digest: Digest::of(b""),
                        replacement: " x".into(),
                    },
                    &mut b,
                    &mut SourceAdmission::default(),
                );
                if changed {
                    assert!(matches!(result, Err(InsertionError::ContextMismatch)));
                } else if depth == 8 {
                    assert!(matches!(
                        result,
                        Err(InsertionError::Stopped(StopReason::DepthLimit))
                    ));
                } else {
                    assert!(result.is_ok());
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn insertion_before_unexpected_suffix_requires_an_actual_missing_occurrence() -> TestResult {
    check(Case {
        old: "let 123",
        new: "let foo 123",
        at: 4,
        inserted: "foo ",
        limit: None,
        expected: Err(InsertionError::NoMissing),
    })?;
    check(Case {
        old: "let 123",
        new: "let f123",
        at: 4,
        inserted: "f",
        limit: None,
        expected: Err(InsertionError::NoMissing),
    })
}

#[path = "insertion/draft.rs"]
mod draft;

#[path = "insertion/checked.rs"]
mod checked;

#[path = "insertion/declared.rs"]
mod declared;

#[path = "insertion/binding.rs"]
mod binding;

#[path = "insertion/whole.rs"]
mod whole;

#[path = "insertion/name.rs"]
mod name;
