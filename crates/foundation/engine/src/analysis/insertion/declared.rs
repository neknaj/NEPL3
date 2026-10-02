//! Bridge declared spellings to explicit drafts, then check the actual head.
use super::*;
use crate::analysis::alternatives::{
    DeclaredAlternativesOutcome, DeclaredReadAlternatives, declared_alternatives,
};
use alloc::string::String;
use nepl3_core::{budget::Resource, value_codec::FoundationValueCodec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredChoice {
    Form(u64),
    ListCons,
    ListNil,
}
#[derive(Debug)]
pub enum DraftError {
    Insertion(InsertionError),
    Selection(ExpectedReadError),
    Stopped(StopReason),
    ChoiceUnavailable,
}
impl From<StopReason> for DraftError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct DeclaredDraft {
    choice: DeclaredChoice,
    head: (u64, u64),
    draft: draft::InsertionDraft,
}
impl DeclaredDraft {
    pub fn choice(&self) -> DeclaredChoice {
        self.choice
    }
    pub fn head_range(&self) -> (u64, u64) {
        self.head
    }
    pub fn draft(&self) -> &draft::InsertionDraft {
        &self.draft
    }
}
/// Affixes are explicit input text, not inferred whitespace or trusted syntax.
/// The selected spelling must later be the actual target head, not merely occur
/// somewhere in a syntactically accepted insertion.
pub fn prepare(
    original: InsertionInput<'_, '_, '_>,
    request: &ExpectedReadRequest,
    choice: DeclaredChoice,
    before: &str,
    after: &str,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<DeclaredDraft, DraftError> {
    if b.limits() != original.prepared.limits {
        return Err(DraftError::Insertion(InsertionError::Access(
            BindingAccessError::LimitsMismatch,
        )));
    }
    b.with_depth(|b| {
        b.charge(Resource::Work, 1)?;
        if !core::ptr::eq(
            original.parsed.execution().tree(),
            original.prepared.tree.tree(),
        ) || !core::ptr::eq(original.parsed.seed().profile(), original.prepared.profile)
        {
            return Err(DraftError::Insertion(InsertionError::ProofMismatch));
        }
        let reply = declared_alternatives(original.prepared, request, b, admission);
        let set = match reply.outcome {
            DeclaredAlternativesOutcome::Complete(Some(set)) => set,
            DeclaredAlternativesOutcome::Complete(None) => {
                return Err(DraftError::Insertion(InsertionError::NoMissing));
            }
            DeclaredAlternativesOutcome::Invalid(error) => {
                return Err(DraftError::Selection(error));
            }
            DeclaredAlternativesOutcome::Stopped(reason) => {
                return Err(DraftError::Stopped(reason));
            }
        };
        let word = match (choice, &set.alternatives) {
            (DeclaredChoice::Form(index), DeclaredReadAlternatives::Category { forms, .. }) => {
                let mut selected = None;
                for form in forms {
                    b.charge(Resource::Work, 1)?;
                    if form.index == index {
                        selected = Some(form.spelling.as_str());
                        break;
                    }
                }
                selected.ok_or(DraftError::ChoiceUnavailable)?
            }
            (DeclaredChoice::ListCons, DeclaredReadAlternatives::List) => "cons",
            (DeclaredChoice::ListNil, DeclaredReadAlternatives::List) => "nil",
            _ => return Err(DraftError::ChoiceUnavailable),
        };
        let length = before
            .len()
            .checked_add(word.len())
            .and_then(|n| n.checked_add(after.len()))
            .ok_or_else(|| DraftError::Stopped(b.stop(StopReason::AllocationLimit)))?;
        let start = request
            .offset
            .checked_add(before.len() as u64)
            .ok_or_else(|| DraftError::Stopped(b.stop(StopReason::SourceLimit)))?;
        let end = start
            .checked_add(word.len() as u64)
            .ok_or_else(|| DraftError::Stopped(b.stop(StopReason::SourceLimit)))?;
        b.charge(Resource::Work, length as u64)?;
        b.charge(Resource::AllocationUnits, length as u64)?;
        let mut replacement = String::with_capacity(length);
        replacement.push_str(before);
        replacement.push_str(word);
        replacement.push_str(after);
        let draft = draft::prepare(original, request, &replacement, b, admission)
            .map_err(DraftError::Insertion)?;
        Ok(DeclaredDraft {
            choice,
            head: (start, end),
            draft,
        })
    })
}

#[derive(Debug)]
pub enum CheckError<E> {
    Check(checked::CheckError<E>),
    Stopped(StopReason),
    ShapeMismatch,
    HeadMismatch,
}
impl<E> From<StopReason> for CheckError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct CheckedDeclaredInsertion<'a, 'p> {
    choice: DeclaredChoice,
    checked: checked::CheckedInsertion<'a, 'p>,
    report: Report,
}
impl<'a, 'p> CheckedDeclaredInsertion<'a, 'p> {
    pub fn choice(&self) -> DeclaredChoice {
        self.choice
    }
    pub fn checked(&self) -> &checked::CheckedInsertion<'a, 'p> {
        &self.checked
    }
    pub fn edit(&self) -> &'a TextEdit {
        self.checked.edit()
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
pub fn check<'a, 'tree, 'p, C: FoundationValueCodec>(
    original: InsertionInput<'a, 'tree, 'p>,
    candidate: &'a RetainedParse<'p>,
    selected: &'a DeclaredDraft,
    request: &ExpectedReadRequest,
    candidate_analysis_id: &str,
    codec: &mut C,
    b: &mut Budget,
) -> Result<CheckedDeclaredInsertion<'a, 'p>, CheckError<C::Error>> {
    if b.limits() != original.prepared.limits {
        return Err(CheckError::Check(checked::CheckError::Insertion(
            InsertionError::Access(BindingAccessError::LimitsMismatch),
        )));
    }
    let checked = b.with_depth(|b| {
        let checked = checked::check(
            original,
            candidate,
            &selected.draft,
            request,
            candidate_analysis_id,
            codec,
            b,
        )
        .map_err(CheckError::Check)?;
        b.charge(Resource::Work, 1)?;
        let matches = match (selected.choice, checked.shape()) {
            (DeclaredChoice::Form(a), InsertedShape::Form(c)) => a == c,
            (DeclaredChoice::ListCons, InsertedShape::List { cons: true, .. })
            | (DeclaredChoice::ListNil, InsertedShape::List { cons: false, .. }) => true,
            _ => false,
        };
        if !matches {
            return Err(CheckError::ShapeMismatch);
        }
        let head = checked.head().ok_or(CheckError::HeadMismatch)?;
        if head
            .snapshot_ref()
            .compare_with_budget(selected.draft.snapshot().identity(), b)?
            != core::cmp::Ordering::Equal
        {
            return Err(CheckError::HeadMismatch);
        }
        b.charge(Resource::Work, 3)?;
        if head.start() != selected.head.0 || head.end() != selected.head.1 {
            return Err(CheckError::HeadMismatch);
        }
        Ok(checked)
    })?;
    Ok(CheckedDeclaredInsertion {
        choice: selected.choice,
        checked,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
