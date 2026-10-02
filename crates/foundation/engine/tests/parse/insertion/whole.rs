use super::*;
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_engine::{
    analysis::insertion::{checked, draft, whole},
    parse::whole::WholeInputError,
};

#[test]
fn whole_checked_insertion_rejects_recovery_and_unconsumed_candidate_bytes() -> TestResult {
    for (original_text, replacement, expected) in [
        ("", "let x x", None),
        ("", "let あ あ", None),
        // Syntax consumption must not be mistaken for resolved Binding.
        ("", "let x y", None),
        ("", "let", Some(WholeInputError::Recovered)),
        (
            "",
            "let x x ",
            Some(WholeInputError::Unconsumed {
                cursor: 7,
                limit: 8,
            }),
        ),
        (
            "",
            "let x x tail",
            Some(WholeInputError::Unconsumed {
                cursor: 7,
                limit: 12,
            }),
        ),
        (" tail", "let x x", Some(WholeInputError::PartialRange)),
    ] {
        super::super::retained::with_context(
            original_text,
            |profile, environments, sources, source, entry| {
                let states = [LanguageReaderState {
                    alias: "Host".into(),
                    state: NdfValue::Unit,
                }];
                let mut b = budget();
                let mut ledger = SourceAdmission::default();
                let mut original_session = RetainedParseSession::new(
                    "whole-old".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: source,
                        start: 0,
                        limit: 0,
                        final_input: true,
                        entry,
                        states: &states,
                    },
                    sources,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let RetainedParseExecution::Continue(original) = original_session
                    .read(&mut b, &mut ledger)
                    .map_err(|e| format!("{e:?}"))?
                else {
                    return Err("original parse".into());
                };
                let empty = SourceStore::default();
                let prepared = {
                    let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
                        .map_err(|e| format!("{e:?}"))?;
                    analysis::prepare(
                        "whole-old",
                        original.execution().tree(),
                        BindingOptions,
                        b.limits(),
                        profile,
                        &mut codec,
                        &mut b,
                    )
                    .map_err(|e| format!("{e:?}"))?
                };
                let query = ExpectedReadRequest {
                    key: prepared.key(),
                    source: source.reference(),
                    offset: 0,
                };
                let draft = draft::prepare(
                    InsertionInput {
                        parsed: &original,
                        prepared: &prepared,
                    },
                    &query,
                    replacement,
                    &mut b,
                    &mut ledger,
                )
                .map_err(|e| format!("{e:?}"))?;
                let mut candidate_session = RetainedParseSession::new(
                    "whole-new".into(),
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
                let RetainedParseExecution::Continue(candidate) = candidate_session
                    .read(&mut b, &mut ledger)
                    .map_err(|e| format!("{e:?}"))?
                else {
                    return Err("candidate parse".into());
                };
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
                    .map_err(|e| format!("{e:?}"))?;
                let checked = checked::check(
                    InsertionInput {
                        parsed: &original,
                        prepared: &prepared,
                    },
                    &candidate,
                    &draft,
                    &query,
                    "whole-new",
                    &mut codec,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let before = b.usage();
                match (whole::check(&checked, &mut b), expected) {
                    (Ok(proof), None) => {
                        assert!(core::ptr::eq(proof.checked(), &checked));
                        assert!(core::ptr::eq(proof.input().parsed(), checked.candidate()));
                        assert!(core::ptr::eq(proof.checked().edit(), draft.edit()));
                        assert_eq!(proof.report().usage, b.usage());
                    }
                    (Err(actual), Some(expected)) => assert_eq!(
                        actual,
                        whole::WholeInsertionError::Input(expected),
                        "{replacement}"
                    ),
                    _ => {
                        return Err(
                            format!("unexpected whole insertion result: {replacement}").into()
                        );
                    }
                }
                assert_eq!(b.usage().work, before.work + 1);
                assert_eq!(b.usage().source_bytes, before.source_bytes);
                assert_eq!(b.usage().allocation_units, before.allocation_units);
                let mut changed_limits = b.limits();
                changed_limits.work -= 1;
                let mut changed = Budget::new(changed_limits);
                assert!(matches!(
                    whole::check(&checked, &mut changed),
                    Err(whole::WholeInsertionError::LimitsMismatch)
                ));
                assert_eq!(changed.usage().work, 0);
                let mut exhausted = Budget::new(b.limits());
                exhausted
                    .charge(Resource::Work, b.limits().work)
                    .map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    whole::check(&checked, &mut exhausted),
                    Err(whole::WholeInsertionError::Input(WholeInputError::Stopped(
                        StopReason::WorkLimit
                    )))
                ));
                let mut cancelled = Budget::new(b.limits());
                cancelled.cancel();
                assert!(matches!(
                    whole::check(&checked, &mut cancelled),
                    Err(whole::WholeInsertionError::Input(WholeInputError::Stopped(
                        StopReason::Cancelled
                    )))
                ));
                assert_eq!(source.text(), original_text);
                assert_eq!(sources.snapshots().len(), 1);
                Ok(())
            },
        )?;
    }
    Ok(())
}
