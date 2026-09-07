use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    lexical::{language::well_formed, name, name_continue, name_start},
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        ..Limits::default()
    })
}
#[test]
fn whole_name_uses_pinned_unicode_spelling_and_excludes_trivia() -> Result<(), StopReason> {
    // U+105C0 is in Unicode 16.0; U+1E6C0 is outside the pinned table.
    for text in ["_", "日本語", "a\u{301}", "\u{105c0}", "name_42"] {
        assert!(name(text, &mut budget())?, "{text}");
    }
    for text in ["", "1name", "\u{301}a", "\u{1e6c0}", "a b", "name\n", "a-b"] {
        assert!(!name(text, &mut budget())?, "{text}");
    }
    assert!(name_start('_'));
    assert!(!name_start('1'));
    assert!(name_continue('1'));
    assert!(name_continue('\u{301}'));
    Ok(())
}
#[test]
fn whole_language_tag_preserves_abnf_scope() -> Result<(), StopReason> {
    // Syntactic validity deliberately does not require registry lookup, nor
    // uniqueness of variant or extension singleton subtags.
    for text in [
        "ja",
        "EN-latn-us",
        "x-a",
        "i-klingon",
        "SGN-be-FR",
        "zh-cmn-Hans-CN",
        "de-1901-1901",
        "en-a-ab-a-cd",
        "abc-def-ghi-jkl-Latn-123-abcde-x-a",
    ] {
        assert!(well_formed(text, &mut budget())?, "{text}");
    }
    for text in [
        "",
        "x",
        "en-",
        "en-a",
        "en-abcdefghi",
        "a",
        "en--US",
        "en-x",
        "en日",
        "en US",
    ] {
        assert!(!well_formed(text, &mut budget())?, "{text}");
    }
    Ok(())
}
#[test]
fn whole_lexemes_stop_without_allocating_or_losing_cancellation() {
    let long = "a".repeat(100_000);
    let mut name_budget = Budget::new(Limits {
        work: 1,
        ..Limits::default()
    });
    assert_eq!(name(&long, &mut name_budget), Err(StopReason::WorkLimit));
    assert_eq!(name_budget.usage().work, 1);
    assert_eq!(name_budget.usage().allocation_units, 0);
    let mut lang_budget = Budget::new(Limits {
        work: 1,
        ..Limits::default()
    });
    assert_eq!(
        well_formed(&long, &mut lang_budget),
        Err(StopReason::WorkLimit)
    );
    assert_eq!(lang_budget.usage().work, 0);
    assert_eq!(lang_budget.usage().allocation_units, 0);
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(name("", &mut cancelled), Err(StopReason::Cancelled));
    assert_eq!(well_formed("", &mut cancelled), Err(StopReason::Cancelled));
}
