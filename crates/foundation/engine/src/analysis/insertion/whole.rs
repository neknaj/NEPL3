//! Compose exact checked insertion evidence with strict candidate consumption.
//! This is syntax evidence only: Binding, freshness and edit authority stay separate.
use super::checked::CheckedInsertion;
use crate::parse::whole::{self, WholeInput, WholeInputError};
use nepl3_core::{budget::Budget, diagnostic::Report};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WholeInsertionError {
    LimitsMismatch,
    Input(WholeInputError),
}

/// Both proofs refer to the checked insertion's actual immutable candidate.
/// Callers cannot attach consumption evidence from another parse execution.
pub struct WholeCheckedInsertion<'a, 'i, 'p> {
    checked: &'a CheckedInsertion<'i, 'p>,
    input: WholeInput<'a, 'p>,
    report: Report,
}
impl<'a, 'i, 'p> WholeCheckedInsertion<'a, 'i, 'p> {
    pub fn checked(&self) -> &'a CheckedInsertion<'i, 'p> {
        self.checked
    }
    pub fn input(&self) -> &WholeInput<'a, 'p> {
        &self.input
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}

/// Continue the caller's cumulative Budget without parsing or admitting sources.
pub fn check<'a, 'i, 'p>(
    checked: &'a CheckedInsertion<'i, 'p>,
    budget: &mut Budget,
) -> Result<WholeCheckedInsertion<'a, 'i, 'p>, WholeInsertionError> {
    if budget.limits() != checked.limits() {
        return Err(WholeInsertionError::LimitsMismatch);
    }
    let input = whole::check(checked.candidate(), budget).map_err(WholeInsertionError::Input)?;
    Ok(WholeCheckedInsertion {
        checked,
        input,
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
    })
}
