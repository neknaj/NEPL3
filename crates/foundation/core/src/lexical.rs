//! Shared lexical predicates, without tokenization or platform dependencies.
//! Name uses Unicode 16.0 XID tables, with the NEPL underscore-start extension.
//! Lang checks RFC 5646 section 2.1 grammar, not language registry validity.
use crate::budget::{Budget, Resource, StopReason};
pub mod language;

/// Constant-cost character predicate shared with the Name reader and CharClass.
pub fn name_start(ch: char) -> bool {
    ch == '_' || unicode_ident::is_xid_start(ch)
}
/// Constant-cost character predicate; no normalization or case folding occurs.
pub fn name_continue(ch: char) -> bool {
    unicode_ident::is_xid_continue(ch)
}
/// Checks the complete spelling of one Name, excluding delimiters and trivia.
/// Reserved form heads remain the responsibility of the selected language.
pub fn name(input: &str, budget: &mut Budget) -> Result<bool, StopReason> {
    budget.charge(Resource::Work, 1)?;
    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return Ok(false);
    };
    if !name_start(first) {
        return Ok(false);
    }
    for ch in chars {
        budget.charge(Resource::Work, 1)?;
        if !name_continue(ch) {
            return Ok(false);
        }
    }
    Ok(true)
}
