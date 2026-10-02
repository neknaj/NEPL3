//! Native evidence for one direct inserted occurrence in two actual executions.
//! It does not apply an edit, prove binding correctness, or consume the whole input.
use super::{AnalysisKey, BindingAccessError, PreparedBindingRequest, expected::*};
use crate::parse::RetainedParse;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, StopReason},
    diagnostic::Report,
    source::{SourceAdmission, SourceError, Span, TextEdit},
};
pub mod draft;
mod identity;
mod occurrence;

pub struct InsertionInput<'a, 'tree, 'p> {
    pub parsed: &'a RetainedParse<'p>,
    pub prepared: &'a PreparedBindingRequest<'tree, 'p>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum InsertionError {
    Access(BindingAccessError),
    Expected(ExpectedReadError),
    Source(SourceError),
    Stopped(StopReason),
    ProofMismatch,
    EditMismatch,
    ContextMismatch,
    NoMissing,
    OccurrenceMismatch,
    RecoveryTarget,
    UnsupportedOrigin,
}
impl From<StopReason> for InsertionError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
impl From<SourceError> for InsertionError {
    fn from(value: SourceError) -> Self {
        match value {
            SourceError::Stopped(reason) => Self::Stopped(reason),
            value => Self::Source(value),
        }
    }
}
pub struct InsertionObservation<'a, 'p> {
    original: &'a RetainedParse<'p>,
    candidate: &'a RetainedParse<'p>,
    original_key: AnalysisKey,
    candidate_key: AnalysisKey,
    path: Vec<ExpectedReadStep>,
    cover: Span,
    report: Report,
}
impl<'a, 'p> InsertionObservation<'a, 'p> {
    pub fn original(&self) -> &'a RetainedParse<'p> {
        self.original
    }
    pub fn candidate(&self) -> &'a RetainedParse<'p> {
        self.candidate
    }
    pub fn keys(&self) -> (AnalysisKey, AnalysisKey) {
        (self.original_key, self.candidate_key)
    }
    pub fn path(&self) -> &[ExpectedReadStep] {
        &self.path
    }
    pub fn cover(&self) -> &Span {
        &self.cover
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
/// The entire positive-length direct target cover must lie inside the insertion.
/// Other recovered occurrences or unconsumed input remain outside this guarantee.
pub fn observe<'a, 'tree, 'p>(
    original: InsertionInput<'a, 'tree, 'p>,
    candidate: InsertionInput<'a, 'tree, 'p>,
    request: &ExpectedReadRequest,
    edit: &TextEdit,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<InsertionObservation<'a, 'p>, InsertionError> {
    if b.limits() != original.prepared.limits || b.limits() != candidate.prepared.limits {
        return Err(InsertionError::Access(BindingAccessError::LimitsMismatch));
    }
    let (path, cover) = b.with_depth(|b| {
        identity::check(&original, &candidate, request, edit, b, admission)?;
        let expected = expected_read(original.prepared, request, b, admission);
        let wanted = match expected.outcome {
            ExpectedReadOutcome::Complete(Some(value)) => value,
            ExpectedReadOutcome::Complete(None) => return Err(InsertionError::NoMissing),
            ExpectedReadOutcome::Invalid(error) => return Err(InsertionError::Expected(error)),
            ExpectedReadOutcome::Stopped(reason) => return Err(InsertionError::Stopped(reason)),
        };
        let cover = occurrence::check(
            original.prepared,
            candidate.prepared,
            &wanted,
            candidate.parsed,
            edit,
            b,
        )?;
        Ok((wanted.path, cover))
    })?;
    Ok(InsertionObservation {
        original: original.parsed,
        candidate: candidate.parsed,
        original_key: original.prepared.key(),
        candidate_key: candidate.prepared.key(),
        path,
        cover,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
