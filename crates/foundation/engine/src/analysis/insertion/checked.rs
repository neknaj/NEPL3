//! Native checked edits borrow their proposal and both actual executions.
//! This proves one syntax occurrence, not binding, authority, or full consumption.
use super::*;
use crate::portable::{PortableError, analysis};
use nepl3_core::{budget::Resource, value_codec::FoundationValueCodec};

#[derive(Debug)]
pub enum CheckError<E> {
    Insertion(InsertionError),
    Preparation(PortableError<E>),
    Stopped(StopReason),
    DraftMismatch,
}
impl<E> From<StopReason> for CheckError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

pub struct CheckedInsertion<'a, 'p> {
    original: &'a RetainedParse<'p>,
    candidate: &'a RetainedParse<'p>,
    draft: &'a draft::InsertionDraft,
    keys: (AnalysisKey, AnalysisKey),
    shape: InsertedShape,
    head: Option<Span>,
    limits: nepl3_core::budget::Limits,
    report: Report,
}
impl<'a, 'p> CheckedInsertion<'a, 'p> {
    pub fn original(&self) -> &'a RetainedParse<'p> {
        self.original
    }
    pub fn candidate(&self) -> &'a RetainedParse<'p> {
        self.candidate
    }
    pub fn edit(&self) -> &'a TextEdit {
        self.draft.edit()
    }
    pub fn keys(&self) -> (AnalysisKey, AnalysisKey) {
        self.keys
    }
    pub fn shape(&self) -> InsertedShape {
        self.shape
    }
    pub fn head(&self) -> Option<&Span> {
        self.head.as_ref()
    }
    pub fn limits(&self) -> nepl3_core::budget::Limits {
        self.limits
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
/// The caller retains the draft while driving the real parser, including awaits.
/// Only a resulting RetainedParse can enter this check. The selected foundation
/// codec supplies the same operation's admission ledger; its preparation and
/// the observation use the caller's cumulative Budget.
pub fn check<'a, 'tree, 'p, C: FoundationValueCodec>(
    original: InsertionInput<'a, 'tree, 'p>,
    candidate: &'a RetainedParse<'p>,
    draft: &'a draft::InsertionDraft,
    request: &ExpectedReadRequest,
    candidate_analysis_id: &str,
    codec: &mut C,
    b: &mut Budget,
) -> Result<CheckedInsertion<'a, 'p>, CheckError<C::Error>> {
    if b.limits() != original.prepared.limits {
        return Err(CheckError::Insertion(InsertionError::Access(
            BindingAccessError::LimitsMismatch,
        )));
    }
    let (keys, shape, head) = b.with_depth(|b| {
        b.charge(Resource::Work, 1)?;
        if !core::ptr::eq(
            original.parsed.execution().tree(),
            original.prepared.tree.tree(),
        ) || !core::ptr::eq(original.parsed.seed().profile(), original.prepared.profile)
        {
            return Err(CheckError::Insertion(InsertionError::ProofMismatch));
        }
        if request.key != original.prepared.key() || draft.original_key() != request.key {
            return Err(CheckError::Insertion(InsertionError::Access(
                BindingAccessError::StaleAnalysis,
            )));
        }
        if !core::ptr::eq(candidate.seed().sources(), draft.sources())
            || candidate.seed().request().limit != draft.limit()
        {
            return Err(CheckError::DraftMismatch);
        }
        let source = candidate.seed().request().snapshot;
        if source
            .identity()
            .compare_with_budget(draft.snapshot().identity(), b)?
            != core::cmp::Ordering::Equal
        {
            return Err(CheckError::DraftMismatch);
        }
        b.charge(
            Resource::Work,
            (source.uri().len() as u64)
                .saturating_add(draft.snapshot().uri().len() as u64)
                .saturating_add(1),
        )?;
        if source.uri() != draft.snapshot().uri() {
            return Err(CheckError::DraftMismatch);
        }
        if !core::ptr::eq(original.parsed.seed().profile(), candidate.seed().profile())
            || !core::ptr::eq(
                original.parsed.seed().environments(),
                candidate.seed().environments(),
            )
        {
            return Err(CheckError::Insertion(InsertionError::ContextMismatch));
        }
        let prepared = analysis::prepare(
            candidate_analysis_id,
            candidate.execution().tree(),
            original.prepared.options,
            b.limits(),
            candidate.seed().profile(),
            codec,
            b,
        )
        .map_err(CheckError::Preparation)?;
        let observed = observe(
            InsertionInput {
                parsed: original.parsed,
                prepared: original.prepared,
            },
            InsertionInput {
                parsed: candidate,
                prepared: &prepared,
            },
            request,
            draft.edit(),
            b,
            codec.source_admission(),
        )
        .map_err(CheckError::Insertion)?;
        Ok((observed.keys(), observed.shape, observed.head))
    })?;
    Ok(CheckedInsertion {
        original: original.parsed,
        candidate,
        draft,
        keys,
        shape,
        head,
        limits: b.limits(),
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
