use super::*;
use nepl3_engine::parse::whole::{self, WholeInputError};

#[test]
fn whole_input_requires_complete_final_full_snapshot_consumption() -> TestResult {
    for (text, start, limit, final_input, expected) in [
        ("let x x", 0, 7, true, None),
        ("let あ あ", 0, 11, true, None),
        ("let x", 0, 5, true, Some(WholeInputError::Recovered)),
        (
            "let x x tail",
            0,
            12,
            true,
            Some(WholeInputError::Unconsumed {
                cursor: 7,
                limit: 12,
            }),
        ),
        (
            "let x x ",
            0,
            8,
            true,
            Some(WholeInputError::Unconsumed {
                cursor: 7,
                limit: 8,
            }),
        ),
        (
            "let x x tail",
            0,
            7,
            true,
            Some(WholeInputError::PartialRange),
        ),
        ("  let x x", 2, 9, true, Some(WholeInputError::PartialRange)),
        (
            "let x x tail",
            0,
            12,
            false,
            Some(WholeInputError::OpenInput),
        ),
    ] {
        with_context(text, |profile, environments, sources, source, entry| {
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Unit,
            }];
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let mut session = RetainedParseSession::new(
                "whole-input".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start,
                    limit,
                    final_input,
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
                return Err(format!("native execution absent: {text}").into());
            };
            let before = b.usage();
            match (whole::check(&parsed, &mut b), expected) {
                (Ok(whole), None) => assert!(core::ptr::eq(whole.parsed(), &parsed)),
                (Err(actual), Some(expected)) => assert_eq!(actual, expected, "{text}"),
                _ => return Err(format!("whole-input mismatch: {text}").into()),
            }
            assert_eq!(b.usage().work, before.work + 1);
            assert_eq!(b.usage().source_bytes, before.source_bytes);
            let mut limits = b.limits();
            limits.work = 0;
            assert!(matches!(
                whole::check(&parsed, &mut nepl3_core::budget::Budget::new(limits)),
                Err(WholeInputError::Stopped(
                    nepl3_core::budget::StopReason::WorkLimit
                ))
            ));
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                whole::check(&parsed, &mut cancelled),
                Err(WholeInputError::Stopped(
                    nepl3_core::budget::StopReason::Cancelled
                ))
            ));
            Ok(())
        })?;
    }
    Ok(())
}
