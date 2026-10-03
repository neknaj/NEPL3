//! Explicit name spelling checked against an actual inserted semantic Text token.
use super::*;
use crate::analysis::{
    completion::NameCandidate,
    probe::{
        candidates::{ProbeCandidateOutcome, ProbeCandidates},
        read::ReadCorrespondence,
    },
};
use alloc::string::String;
use nepl3_core::{
    budget::Resource, syntax::FieldValue, value::NdfValue, value_codec::FoundationValueCodec,
};

#[derive(Debug)]
pub enum NameError {
    Insertion(InsertionError),
    Stopped(StopReason),
    ChoiceUnavailable,
    ProofMismatch,
    RequestMismatch,
    Spelling,
}
impl From<StopReason> for NameError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
pub struct NameChoice<'a, 'tree> {
    candidate: &'a NameCandidate,
    read: &'a ReadCorrespondence<'a, 'tree>,
}
impl<'a, 'tree> NameChoice<'a, 'tree> {
    pub fn candidate(&self) -> &'a NameCandidate {
        self.candidate
    }
    pub fn read(&self) -> &'a ReadCorrespondence<'a, 'tree> {
        self.read
    }
}
pub fn select<'a, 'tree>(
    candidates: &'a ProbeCandidates<'a, 'tree>,
    read: &'a ReadCorrespondence<'a, 'tree>,
    index: usize,
    b: &mut Budget,
) -> Result<NameChoice<'a, 'tree>, NameError> {
    if b.limits() != read.limits() {
        return Err(NameError::Insertion(InsertionError::Access(
            BindingAccessError::LimitsMismatch,
        )));
    }
    b.charge(Resource::Work, 129)?;
    if candidates.key() != read.reply().key() {
        return Err(NameError::ProofMismatch);
    }
    let ProbeCandidateOutcome::Hit { hit, candidates } = candidates.outcome() else {
        return Err(NameError::ChoiceUnavailable);
    };
    if !core::ptr::eq(*hit, read.hit()) {
        return Err(NameError::ProofMismatch);
    }
    let candidate = candidates.get(index).ok_or(NameError::ChoiceUnavailable)?;
    Ok(NameChoice { candidate, read })
}
pub struct NameSpelling<'a> {
    pub before: &'a str,
    pub spelling: &'a str,
    pub after: &'a str,
}
pub struct NameDraft<'a, 'tree> {
    choice: NameChoice<'a, 'tree>,
    head: (u64, u64),
    draft: draft::InsertionDraft,
}
impl<'a, 'tree> NameDraft<'a, 'tree> {
    pub fn choice(&self) -> &NameChoice<'a, 'tree> {
        &self.choice
    }
    pub fn head_range(&self) -> (u64, u64) {
        self.head
    }
    pub fn draft(&self) -> &draft::InsertionDraft {
        &self.draft
    }
}
fn request_matches(
    choice: &NameChoice<'_, '_>,
    original: &InsertionInput<'_, '_, '_>,
    request: &ExpectedReadRequest,
    b: &mut Budget,
) -> Result<(), NameError> {
    b.charge(Resource::Work, 257)?;
    if choice.read.reply().key() != request.key
        || original.prepared.key() != request.key
        || !core::ptr::eq(choice.read.hit().tree(), original.parsed.execution().tree())
    {
        return Err(NameError::ProofMismatch);
    }
    let anchor = &choice.read.hit().site().anchor;
    let identity = anchor.snapshot_ref();
    b.charge(
        Resource::Work,
        (identity.source.0.len() as u64)
            .saturating_add(request.source.source_id.0.len() as u64)
            .saturating_add(56),
    )?;
    if identity.source != request.source.source_id
        || identity.revision != request.source.revision
        || identity.digest != request.source.digest
        || anchor.start() != request.offset
        || anchor.end() != request.offset
    {
        return Err(NameError::RequestMismatch);
    }
    Ok(())
}
/// All affixes and source spelling are explicit. Semantic names are not encoded here.
pub fn prepare<'a, 'tree>(
    original: InsertionInput<'_, '_, '_>,
    request: &ExpectedReadRequest,
    choice: NameChoice<'a, 'tree>,
    surface: NameSpelling<'_>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<NameDraft<'a, 'tree>, NameError> {
    if b.limits() != original.prepared.limits || b.limits() != choice.read.limits() {
        return Err(NameError::Insertion(InsertionError::Access(
            BindingAccessError::LimitsMismatch,
        )));
    }
    b.with_depth(|b| {
        request_matches(&choice, &original, request, b)?;
        let NameSpelling {
            before,
            spelling,
            after,
        } = surface;
        if spelling.is_empty() {
            return Err(NameError::Spelling);
        }
        let length = before
            .len()
            .checked_add(spelling.len())
            .and_then(|n| n.checked_add(after.len()))
            .ok_or_else(|| NameError::Stopped(b.stop(StopReason::AllocationLimit)))?;
        let start = request
            .offset
            .checked_add(before.len() as u64)
            .ok_or_else(|| NameError::Stopped(b.stop(StopReason::SourceLimit)))?;
        let end = start
            .checked_add(spelling.len() as u64)
            .ok_or_else(|| NameError::Stopped(b.stop(StopReason::SourceLimit)))?;
        b.charge(Resource::Work, length as u64)?;
        b.charge(Resource::AllocationUnits, length as u64)?;
        let mut replacement = String::with_capacity(length);
        replacement.push_str(before);
        replacement.push_str(spelling);
        replacement.push_str(after);
        let draft = draft::prepare(original, request, &replacement, b, admission)
            .map_err(NameError::Insertion)?;
        Ok(NameDraft {
            choice,
            head: (start, end),
            draft,
        })
    })
}
#[derive(Debug)]
pub enum CheckError<E> {
    Name(NameError),
    Checked(checked::CheckError<E>),
    Stopped(StopReason),
    Target,
    Payload,
    Head,
}
impl<E> From<StopReason> for CheckError<E> {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
pub struct CheckedNameInsertion<'a, 'tree, 'p> {
    choice: &'a NameChoice<'a, 'tree>,
    checked: checked::CheckedInsertion<'a, 'p>,
    report: Report,
}
impl<'a, 'tree, 'p> CheckedNameInsertion<'a, 'tree, 'p> {
    pub fn choice(&self) -> &'a NameChoice<'a, 'tree> {
        self.choice
    }
    pub fn checked(&self) -> &checked::CheckedInsertion<'a, 'p> {
        &self.checked
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
/// Local token/payload evidence only. Whole input and final resolution stay separate.
pub fn check<'a, 'tree, 'p, C: FoundationValueCodec>(
    original: InsertionInput<'a, '_, 'p>,
    candidate: &'a RetainedParse<'p>,
    draft: &'a NameDraft<'a, 'tree>,
    request: &ExpectedReadRequest,
    candidate_analysis_id: &str,
    codec: &mut C,
    b: &mut Budget,
) -> Result<CheckedNameInsertion<'a, 'tree, 'p>, CheckError<C::Error>> {
    if b.limits() != original.prepared.limits || b.limits() != draft.choice.read.limits() {
        return Err(CheckError::Name(NameError::Insertion(
            InsertionError::Access(BindingAccessError::LimitsMismatch),
        )));
    }
    request_matches(&draft.choice, &original, request, b).map_err(CheckError::Name)?;
    let checked = checked::check(
        original,
        candidate,
        &draft.draft,
        request,
        candidate_analysis_id,
        codec,
        b,
    )
    .map_err(CheckError::Checked)?;
    b.with_depth(|b| {
        b.charge(Resource::Work, checked.path().len() as u64 + 1)?;
        if checked.path() != draft.choice.read.expected().path {
            return Err(CheckError::Target);
        }
        let tree = candidate.execution().tree();
        let mut bundle = &tree.bundle;
        let mut target = bundle.root;
        for (depth, step) in checked.path().iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            b.charge(Resource::Nodes, 1)?;
            b.observe_depth(depth as u64 + 1)?;
            let node = bundle
                .nodes
                .get(usize::try_from(target.0).map_err(|_| CheckError::Target)?)
                .ok_or(CheckError::Target)?;
            match *step {
                ExpectedReadStep::Child { field } => {
                    let Some(FieldValue::Child(child)) = node
                        .fields
                        .get(usize::try_from(field).map_err(|_| CheckError::Target)?)
                    else {
                        return Err(CheckError::Target);
                    };
                    target = *child;
                }
                ExpectedReadStep::Foreign { field } => {
                    let Some(FieldValue::Foreign(value)) = node
                        .fields
                        .get(usize::try_from(field).map_err(|_| CheckError::Target)?)
                    else {
                        return Err(CheckError::Target);
                    };
                    bundle = &value.bundle;
                    target = bundle.root;
                }
            }
        }
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        b.observe_depth(checked.path().len() as u64 + 1)?;
        let node = bundle
            .nodes
            .get(usize::try_from(target.0).map_err(|_| CheckError::Target)?)
            .ok_or(CheckError::Target)?;
        let token = bundle
            .tokens
            .get(
                usize::try_from(node.token.ok_or(CheckError::Target)?.0)
                    .map_err(|_| CheckError::Target)?,
            )
            .ok_or(CheckError::Target)?;
        let head = checked.head().ok_or(CheckError::Head)?;
        for actual in [head, &token.head] {
            if actual
                .snapshot_ref()
                .compare_with_budget(draft.draft.snapshot().identity(), b)?
                != core::cmp::Ordering::Equal
                || actual.start() != draft.head.0
                || actual.end() != draft.head.1
            {
                return Err(CheckError::Head);
            }
        }
        let NdfValue::Text(actual) = &token.payload else {
            return Err(CheckError::Payload);
        };
        b.charge(
            Resource::Work,
            (actual.len() as u64)
                .saturating_add(draft.choice.candidate.name.len() as u64)
                .saturating_add(1),
        )?;
        if actual != &draft.choice.candidate.name {
            return Err(CheckError::Payload);
        }
        Ok(())
    })?;
    Ok(CheckedNameInsertion {
        choice: &draft.choice,
        checked,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
