//! Opt-in strict candidate screening, without workspace freshness or edit authority.
use super::declaration::SameDeclaration;
use crate::parse::whole::{self, WholeInput, WholeInputError};
use crate::{analysis::BindingAccessError, binding::BindingOutcome};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::{Diagnostic, Report, Severity},
    facts::{Occurrence, OccurrenceId, OccurrenceRole, ReferenceResolution},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticStage {
    Parse,
    Binding,
}
#[derive(Debug)]
pub enum QualityError {
    LimitsMismatch,
    Whole(WholeInputError),
    Access(BindingAccessError),
    Incomplete,
    Stopped(StopReason),
}
impl From<StopReason> for QualityError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
pub enum Rejection<'a> {
    Diagnostic {
        stage: DiagnosticStage,
        diagnostic: &'a Diagnostic,
    },
    OpenInput {
        occurrence: OccurrenceId,
    },
    Reference {
        occurrence: &'a Occurrence,
    },
}
/// A fixed conservative policy: whole input, no Errors, no open inputs, and
/// every final Reference resolved. Warnings and other non-error diagnostics remain.
pub struct StrictNameInsertion<'a, 'i, 'tree, 'p> {
    declaration: &'a SameDeclaration<'i, 'tree, 'p>,
    whole: WholeInput<'a, 'p>,
    report: Report,
}
impl<'a, 'i, 'tree, 'p> StrictNameInsertion<'a, 'i, 'tree, 'p> {
    pub fn declaration(&self) -> &'a SameDeclaration<'i, 'tree, 'p> {
        self.declaration
    }
    pub fn whole(&self) -> &WholeInput<'a, 'p> {
        &self.whole
    }
    /// Usage-only report for this screening query; permitted diagnostics remain in the retained executions.
    pub fn report(&self) -> &Report {
        &self.report
    }
}
pub enum QualityOutcome<'a, 'i, 'tree, 'p> {
    Suitable(StrictNameInsertion<'a, 'i, 'tree, 'p>),
    Rejected(Rejection<'a>),
}
fn diagnostic<'a>(
    report: &'a Report,
    stage: DiagnosticStage,
    b: &mut Budget,
) -> Result<Option<Rejection<'a>>, QualityError> {
    for diagnostic in &report.diagnostics {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        if diagnostic.severity == Severity::Error {
            return Ok(Some(Rejection::Diagnostic { stage, diagnostic }));
        }
    }
    Ok(None)
}
/// Continue the cumulative Budget as a caller contract; matching Limits alone
/// cannot prove budget-object continuity. This query emits no new diagnostics.
pub fn check<'a, 'i, 'tree, 'p>(
    declaration: &'a SameDeclaration<'i, 'tree, 'p>,
    b: &mut Budget,
) -> Result<QualityOutcome<'a, 'i, 'tree, 'p>, QualityError> {
    let checked = declaration.reference().insertion().checked();
    if b.limits() != checked.limits() {
        return Err(QualityError::LimitsMismatch);
    }
    let whole = whole::check(checked.candidate(), b).map_err(QualityError::Whole)?;
    let native = declaration
        .candidate()
        .references()
        .for_key(&checked.keys().1, b)
        .map_err(QualityError::Access)?;
    let BindingOutcome::Complete(analysis) = &native.reply().outcome else {
        return Err(QualityError::Incomplete);
    };
    if let Some(reason) = diagnostic(
        checked.candidate().execution().report(),
        DiagnosticStage::Parse,
        b,
    )? {
        return Ok(QualityOutcome::Rejected(reason));
    }
    if let Some(reason) = diagnostic(&native.reply().report, DiagnosticStage::Binding, b)? {
        return Ok(QualityOutcome::Rejected(reason));
    }
    // Custom resolution updates already maintain this native ledger. Do not
    // reconstruct or repair it from guessed lookup results in this operation.
    b.charge(Resource::Work, 1)?;
    if let Some(occurrence) = analysis.result().open_inputs.first() {
        b.charge(Resource::Nodes, 1)?;
        return Ok(QualityOutcome::Rejected(Rejection::OpenInput {
            occurrence: *occurrence,
        }));
    }
    // Include Custom-created references; builtin trace rows are only a subset.
    for occurrence in &analysis.facts().occurrences {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        if occurrence.role == OccurrenceRole::Reference
            && !matches!(occurrence.resolution, ReferenceResolution::Resolved(_))
        {
            return Ok(QualityOutcome::Rejected(Rejection::Reference {
                occurrence,
            }));
        }
    }
    Ok(QualityOutcome::Suitable(StrictNameInsertion {
        declaration,
        whole,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    }))
}
impl QualityError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(s)
            | Self::Access(BindingAccessError::Stopped(s))
            | Self::Whole(WholeInputError::Stopped(s)) => Some(*s),
            _ => None,
        }
    }
}
