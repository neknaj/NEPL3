use super::*;
use nepl3_engine::analysis::insertion::{
    checked,
    declared::{self, DeclaredChoice},
};

#[derive(Clone, Copy, Debug)]
enum Expected {
    Accepted,
    Shape,
    Head,
    Unavailable,
    Lexical,
}
struct Case<'a> {
    old: &'a str,
    list: bool,
    choice: DeclaredChoice,
    before: &'a str,
    after: &'a str,
    expected: Expected,
}
fn run(case: Case<'_>) -> TestResult {
    let exercise = |profile: &ResolvedParseProfile<'_>,
                    environments: &ParseEnvironmentSet<'_>,
                    sources: &SourceStore,
                    source: &SourceSnapshot,
                    entry: &nepl3_engine::package::EntryContext| {
        let mut b = budget();
        let mut ledger = SourceAdmission::default();
        let states = [LanguageReaderState {
            alias: "Host".into(),
            state: NdfValue::Unit,
        }];
        let mut session = RetainedParseSession::new(
            "declared-old".into(),
            profile,
            environments,
            ParseRequest {
                snapshot: source,
                start: 0,
                limit: case.old.len() as u64,
                final_input: true,
                entry,
                states: &states,
            },
            sources,
            &mut b,
        )
        .map_err(|e| format!("{e:?}"))?;
        let RetainedParseExecution::Continue(old) = session
            .read(&mut b, &mut ledger)
            .map_err(|e| format!("{e:?}"))?
        else {
            return Err("old proof".into());
        };
        let empty = SourceStore::default();
        let prepared = {
            let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
                .map_err(|e| format!("{e:?}"))?;
            analysis::prepare(
                "declared-old",
                old.execution().tree(),
                BindingOptions,
                b.limits(),
                profile,
                &mut codec,
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?
        };
        let request = ExpectedReadRequest {
            key: prepared.key(),
            source: source.reference(),
            offset: case.old.len() as u64,
        };
        let selected = declared::prepare(
            InsertionInput {
                parsed: &old,
                prepared: &prepared,
            },
            &request,
            case.choice,
            case.before,
            case.after,
            &mut b,
            &mut ledger,
        );
        if matches!(case.expected, Expected::Unavailable) {
            assert!(matches!(
                selected,
                Err(declared::DraftError::ChoiceUnavailable)
            ));
            assert_eq!(sources.snapshots().len(), 1);
            return Ok(());
        }
        let selected = selected.map_err(|e| format!("{e:?}"))?;
        assert_eq!(selected.choice(), case.choice);
        assert_eq!(
            selected.head_range().0,
            case.old.len() as u64 + case.before.len() as u64
        );
        let draft = selected.draft();
        let mut session = RetainedParseSession::new(
            "declared-new".into(),
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
        let RetainedParseExecution::Continue(candidate) = session
            .read(&mut b, &mut ledger)
            .map_err(|e| format!("{e:?}"))?
        else {
            return Err("candidate proof".into());
        };
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
            .map_err(|e| format!("{e:?}"))?;
        // Generic syntax validity alone cannot establish the selected declaration.
        let generic = checked::check(
            InsertionInput {
                parsed: &old,
                prepared: &prepared,
            },
            &candidate,
            draft,
            &request,
            "generic",
            &mut codec,
            &mut b,
        );
        if matches!(case.expected, Expected::Lexical) {
            assert!(matches!(
                generic,
                Err(checked::CheckError::Insertion(
                    InsertionError::RecoveryTarget
                ))
            ));
        } else {
            generic.map_err(|e| {
                format!(
                    "generic before={:?} after={:?}: {e:?}",
                    case.before, case.after
                )
            })?;
        }
        let result = declared::check(
            InsertionInput {
                parsed: &old,
                prepared: &prepared,
            },
            &candidate,
            &selected,
            &request,
            "declared",
            &mut codec,
            &mut b,
        );
        match case.expected {
            Expected::Accepted => {
                let result = result.map_err(|e| format!("{e:?}"))?;
                assert_eq!(result.choice(), case.choice);
                assert!(core::ptr::eq(result.edit(), draft.edit()));
                let head = result.checked().head().ok_or("head")?;
                assert_eq!((head.start(), head.end()), selected.head_range());
                assert_eq!(result.report().usage, b.usage());
                // Keep the effective limits unchanged and measure with fresh
                // admissions, so the stop occurs after generic checking.
                use nepl3_core::budget::{Budget, Resource, StopReason};
                let limits = b.limits();
                let mut measured = Budget::new(limits);
                let mut measured_ledger = SourceAdmission::default();
                let mut measured_codec =
                    FoundationCodec::new(profile.registry(), &empty, &mut measured_ledger)
                        .map_err(|e| format!("{e:?}"))?;
                checked::check(
                    InsertionInput {
                        parsed: &old,
                        prepared: &prepared,
                    },
                    &candidate,
                    draft,
                    &request,
                    "boundary",
                    &mut measured_codec,
                    &mut measured,
                )
                .map_err(|e| format!("{e:?}"))?;
                let generic_work = measured.usage().work;
                let mut whole = Budget::new(limits);
                let mut whole_ledger = SourceAdmission::default();
                let mut whole_codec =
                    FoundationCodec::new(profile.registry(), &empty, &mut whole_ledger)
                        .map_err(|e| format!("{e:?}"))?;
                declared::check(
                    InsertionInput {
                        parsed: &old,
                        prepared: &prepared,
                    },
                    &candidate,
                    &selected,
                    &request,
                    "boundary",
                    &mut whole_codec,
                    &mut whole,
                )
                .map_err(|e| format!("{e:?}"))?;
                assert!(whole.usage().work > generic_work);
                for available in [generic_work, whole.usage().work - 1] {
                    let mut stopped = Budget::new(limits);
                    stopped
                        .charge(Resource::Work, limits.work - available)
                        .map_err(|e| format!("{e:?}"))?;
                    let mut stopped_ledger = SourceAdmission::default();
                    let mut stopped_codec =
                        FoundationCodec::new(profile.registry(), &empty, &mut stopped_ledger)
                            .map_err(|e| format!("{e:?}"))?;
                    let result = declared::check(
                        InsertionInput {
                            parsed: &old,
                            prepared: &prepared,
                        },
                        &candidate,
                        &selected,
                        &request,
                        "boundary",
                        &mut stopped_codec,
                        &mut stopped,
                    );
                    assert!(matches!(
                        result,
                        Err(declared::CheckError::Stopped(StopReason::WorkLimit))
                    ));
                }
            }
            Expected::Shape => assert!(matches!(result, Err(declared::CheckError::ShapeMismatch))),
            Expected::Head => assert!(matches!(result, Err(declared::CheckError::HeadMismatch))),
            Expected::Lexical => assert!(matches!(
                result,
                Err(declared::CheckError::Check(checked::CheckError::Insertion(
                    InsertionError::RecoveryTarget
                )))
            )),
            Expected::Unavailable => return Err("unexpected accepted choice".into()),
        }
        assert_eq!(sources.snapshots().len(), 1);
        assert_eq!(source.text(), case.old);
        Ok(())
    };
    if case.list {
        super::super::retained::with_list_context(case.old, exercise)
    } else {
        super::super::retained::with_context(case.old, exercise)
    }
}
#[test]
fn selected_form_and_exact_spelling_head_are_checked() -> TestResult {
    for (before, after, expected) in [
        ("", " x x", Expected::Accepted),
        ("\u{3000}", " x x", Expected::Lexical),
        ("#あ\n", " x x", Expected::Accepted),
        ("", "", Expected::Accepted),
        ("z ", " x x", Expected::Shape),
        ("let a a ", " x x", Expected::Head),
    ] {
        run(Case {
            old: "",
            list: false,
            choice: DeclaredChoice::Form(0),
            before,
            after,
            expected,
        })?;
    }
    Ok(())
}
#[test]
fn declared_list_choices_and_wrong_read_classes_are_separate() -> TestResult {
    for (choice, after) in [
        (DeclaredChoice::ListNil, ""),
        (DeclaredChoice::ListCons, " x nil"),
    ] {
        run(Case {
            old: "let x",
            list: true,
            choice,
            before: " ",
            after,
            expected: Expected::Accepted,
        })?;
    }
    for (choice, before, after, expected) in [
        (DeclaredChoice::ListCons, " nil ", " x nil", Expected::Shape),
        (DeclaredChoice::ListNil, " cons x nil ", "", Expected::Shape),
        (DeclaredChoice::ListNil, " nil ", "", Expected::Head),
        (
            DeclaredChoice::ListCons,
            " cons x nil ",
            " x nil",
            Expected::Head,
        ),
        (DeclaredChoice::ListNil, " #あ\n", "", Expected::Accepted),
    ] {
        run(Case {
            old: "let x",
            list: true,
            choice,
            before,
            after,
            expected,
        })?;
    }
    for (old, list, choice) in [
        ("", false, DeclaredChoice::Form(99)),
        ("", false, DeclaredChoice::ListNil),
        ("let", false, DeclaredChoice::Form(0)),
        ("let x", true, DeclaredChoice::Form(0)),
    ] {
        run(Case {
            old,
            list,
            choice,
            before: " ",
            after: "",
            expected: Expected::Unavailable,
        })?;
    }
    Ok(())
}
