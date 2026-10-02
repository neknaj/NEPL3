//! A private source proposal, not an accepted completion or a workspace edit.
use super::*;
use nepl3_core::{
    budget::Resource,
    source::{Digest, SourceSnapshot, SourceStore},
};

pub struct InsertionDraft {
    key: AnalysisKey,
    edit: TextEdit,
    sources: SourceStore,
    snapshot: SourceSnapshot,
    limit: u64,
    report: Report,
}
impl InsertionDraft {
    pub fn original_key(&self) -> AnalysisKey {
        self.key
    }
    pub fn edit(&self) -> &TextEdit {
        &self.edit
    }
    pub fn sources(&self) -> &SourceStore {
        &self.sources
    }
    pub fn snapshot(&self) -> &SourceSnapshot {
        &self.snapshot
    }
    pub fn limit(&self) -> u64 {
        self.limit
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
/// Copy the retained store and apply one explicit insertion to the private copy.
/// The host must reparse using the original seed and validate it with `observe`.
/// No token separator, spelling, name, or editing authority is inferred here.
pub fn prepare(
    original: InsertionInput<'_, '_, '_>,
    request: &ExpectedReadRequest,
    replacement: &str,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<InsertionDraft, InsertionError> {
    if b.limits() != original.prepared.limits {
        return Err(InsertionError::Access(BindingAccessError::LimitsMismatch));
    }
    b.with_depth(|b| {
        b.charge(Resource::Work, 1)?;
        if !core::ptr::eq(
            original.parsed.execution().tree(),
            original.prepared.tree.tree(),
        ) || !core::ptr::eq(original.parsed.seed().profile(), original.prepared.profile)
        {
            return Err(InsertionError::ProofMismatch);
        }
        if request.key != original.prepared.key() {
            return Err(InsertionError::Access(BindingAccessError::StaleAnalysis));
        }
        let seed = original.parsed.seed();
        let input = seed.request();
        b.charge(
            Resource::Work,
            (request.source.source_id.0.len() as u64)
                .saturating_add(replacement.len() as u64)
                .saturating_add(42),
        )?;
        if request.source.source_id != input.snapshot.identity().source
            || request.source.revision != input.snapshot.identity().revision
            || request.source.digest != input.snapshot.identity().digest
            || request.offset < input.start
            || request.offset > input.limit
            || replacement.is_empty()
        {
            return Err(InsertionError::EditMismatch);
        }
        input.snapshot.check_range(request.offset, request.offset)?;
        let expected = expected_read(original.prepared, request, b, admission);
        match expected.outcome {
            ExpectedReadOutcome::Complete(Some(_)) => {}
            ExpectedReadOutcome::Complete(None) => return Err(InsertionError::NoMissing),
            ExpectedReadOutcome::Invalid(error) => return Err(InsertionError::Expected(error)),
            ExpectedReadOutcome::Stopped(reason) => return Err(InsertionError::Stopped(reason)),
        }
        let limit = input
            .limit
            .checked_add(replacement.len() as u64)
            .ok_or(InsertionError::EditMismatch)?;
        b.charge(
            Resource::AllocationUnits,
            (replacement.len() as u64).saturating_add(core::mem::size_of::<TextEdit>() as u64),
        )?;
        let edit = TextEdit {
            span: input
                .snapshot
                .span_with_budget(request.offset, request.offset, b)?,
            expected_digest: Digest::of(b""),
            replacement: replacement.into(),
        };
        let mut sources = SourceStore::default();
        for source in seed.sources().snapshots() {
            b.charge(Resource::Work, 1)?;
            admission.admit_existing(source, b)?;
            sources.insert_with_budget(source.clone_with_budget(b)?, b)?;
        }
        let next = sources.apply(core::slice::from_ref(&edit), b, admission)?;
        let [identity] = next.as_slice() else {
            return Err(InsertionError::EditMismatch);
        };
        let selected = sources
            .get_revision_with_budget(&identity.source, identity.revision, b)?
            .ok_or(SourceError::MissingSnapshot)?;
        if selected.identity().compare_with_budget(identity, b)? != core::cmp::Ordering::Equal {
            return Err(InsertionError::EditMismatch);
        }
        let snapshot = selected.clone_with_budget(b)?;
        Ok(InsertionDraft {
            key: request.key,
            edit,
            sources,
            snapshot,
            limit,
            report: Report {
                usage: b.usage(),
                ..Report::default()
            },
        })
    })
}
