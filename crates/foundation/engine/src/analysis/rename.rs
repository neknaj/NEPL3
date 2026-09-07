//! Rename candidates are private transactions until a complete reparse and
//! binding execution preserve syntax and every affected resolution.
use super::{AnalysisKey, BindingAccessError, BoundBindingReply, PreparedBindingRequest};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    facts::{EntityId, FactSet, ReferenceResolution},
    origin::{MappingKind, Origin, OriginError},
    source::{
        Digest, SourceAdmission, SourceError, SourceRef, SourceSnapshot, SourceStore, Span,
        TextEdit,
    },
};
mod mapping;
mod owner;
mod prepare;
mod shape;
mod verify;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameRequest {
    pub key: AnalysisKey,
    pub source: SourceRef,
    pub offset: u64,
    pub new_name: String,
    /// Host-authorized editable snapshots, never inferred from a URI or a
    /// generated value's SourceRef. Initial decoding does not authorize these.
    pub writable: Vec<SourceRef>,
}
impl RenameRequest {
    /// Structural identity checks only. A decoded writable list is not a grant.
    pub fn validate_identity(&self, b: &mut Budget) -> Result<(), SourceError> {
        b.charge(Resource::Work, self.source.source_id.0.len() as u64 + 1)?;
        if self.source.source_id.0.is_empty() {
            return Err(SourceError::Locator);
        }
        for (index, source) in self.writable.iter().enumerate() {
            b.charge(Resource::Work, source.source_id.0.len() as u64 + 1)?;
            if source.source_id.0.is_empty() {
                return Err(SourceError::Locator);
            }
            for prior in &self.writable[..index] {
                b.charge(
                    Resource::Work,
                    (source.source_id.0.len() + prior.source_id.0.len()) as u64 + 42,
                )?;
                if source.source_id == prior.source_id {
                    return Err(SourceError::IdentityConflict);
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenameError {
    Stopped(StopReason),
    Access(BindingAccessError),
    Source(SourceError),
    Origin(OriginError),
    NoOccurrence,
    Unresolved,
    Ambiguous,
    Deferred,
    NoLocation,
    InvalidName,
    NotWritable,
    RenameNotInvertible,
    RequestMismatch,
    ShapeChanged,
    ResolutionChanged,
    Collision,
}
impl From<StopReason> for RenameError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<BindingAccessError> for RenameError {
    fn from(v: BindingAccessError) -> Self {
        Self::Access(v)
    }
}
impl From<SourceError> for RenameError {
    fn from(v: SourceError) -> Self {
        Self::Source(v)
    }
}
impl From<OriginError> for RenameError {
    fn from(v: OriginError) -> Self {
        Self::Origin(v)
    }
}
impl RenameError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(r)
            | Self::Access(BindingAccessError::Stopped(r))
            | Self::Source(SourceError::Stopped(r))
            | Self::Origin(OriginError::Stopped(r))
            | Self::Origin(OriginError::Source(SourceError::Stopped(r))) => Some(*r),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenameOutcome {
    Complete {
        new_key: AnalysisKey,
        edits: Vec<TextEdit>,
    },
    Invalid(RenameError),
    Stopped(StopReason),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameReply {
    pub key: AnalysisKey,
    pub outcome: RenameOutcome,
    pub report: Report,
    pub sources: Vec<SourceSnapshot>,
}

/// A speculative branch owns its source admission and exclusively borrows the
/// operation Budget. Dropping it discards candidates, never consumed resources.
/// No edits accessor exists: only `accept` can publish the atomic transaction.
pub struct RenameDraft<'a, 'tree, 'profile, 'budget> {
    original: &'a PreparedBindingRequest<'tree, 'profile>,
    binding: &'a BoundBindingReply,
    request: &'a RenameRequest,
    entity: EntityId,
    edits: Vec<TextEdit>,
    candidate_edits: Vec<TextEdit>,
    sources: SourceStore,
    admission: SourceAdmission,
    budget: &'budget mut Budget,
}
impl RenameDraft<'_, '_, '_, '_> {
    /// Ends a failed reparse without exposing any transaction. The caller keeps
    /// the parser's own typed reply/report; this envelope preserves the rename
    /// failure and the resource usage consumed before rejection.
    pub fn reject(self, error: RenameError) -> RenameReply {
        RenameReply {
            key: self.request.key,
            outcome: match error.stop_reason() {
                Some(r) => RenameOutcome::Stopped(r),
                None => RenameOutcome::Invalid(error),
            },
            report: Report {
                usage: self.budget.usage(),
                ..Report::default()
            },
            sources: Vec::new(),
        }
    }
    /// The host runs the ordinary parser and binding preparation/execution with
    /// these exact candidate snapshots and the same operation resources. The
    /// closure may return owned trees/replies; it cannot retain this ledger.
    pub fn with_operation<T>(
        &mut self,
        f: impl FnOnce(&SourceStore, &mut Budget, &mut SourceAdmission) -> T,
    ) -> T {
        f(&self.sources, self.budget, &mut self.admission)
    }
    pub fn accept(
        mut self,
        parsed: &crate::parse::CompletedParse,
        prepared: &PreparedBindingRequest<'_, '_>,
        binding: &BoundBindingReply,
    ) -> RenameReply {
        let result = if core::ptr::eq(parsed.tree(), prepared.tree.tree()) {
            verify::verify(&mut self, prepared, binding)
        } else {
            Err(RenameError::RequestMismatch)
        };
        let mut sources = Vec::new();
        let outcome = match result.and_then(|()| {
            for edit in &self.edits {
                let source = self
                    .sources
                    .get_ref(edit.span.snapshot_ref())
                    .ok_or(SourceError::MissingSnapshot)?;
                let mut known = false;
                for old in &sources {
                    let old: &SourceSnapshot = old;
                    same_source(old.identity(), source.identity(), self.budget)?;
                    known |= old.identity() == source.identity();
                }
                if !known {
                    push(
                        &mut sources,
                        source.clone_with_budget(self.budget)?,
                        self.budget,
                    )?;
                }
            }
            Ok::<_, RenameError>(())
        }) {
            Ok(()) => RenameOutcome::Complete {
                new_key: prepared.key(),
                edits: self.edits,
            },
            Err(error) => {
                sources.clear();
                match error.stop_reason() {
                    Some(reason) => RenameOutcome::Stopped(reason),
                    None => RenameOutcome::Invalid(error),
                }
            }
        };
        RenameReply {
            key: self.request.key,
            outcome,
            report: Report {
                usage: self.budget.usage(),
                ..Report::default()
            },
            sources,
        }
    }
}

pub fn prepare<'a, 'tree, 'profile, 'budget>(
    parsed: &crate::parse::CompletedParse,
    original: &'a PreparedBindingRequest<'tree, 'profile>,
    binding: &'a BoundBindingReply,
    request: &'a RenameRequest,
    budget: &'budget mut Budget,
) -> Result<RenameDraft<'a, 'tree, 'profile, 'budget>, RenameError> {
    budget.poll()?;
    if !core::ptr::eq(parsed.tree(), original.tree.tree()) {
        return Err(RenameError::RequestMismatch);
    }
    prepare::prepare(original, binding, request, budget)
}
fn push<T>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), RenameError> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    v.push(item);
    Ok(())
}
fn text(v: &str, b: &mut Budget) -> Result<String, RenameError> {
    b.charge(Resource::Work, v.len() as u64)?;
    b.charge(Resource::AllocationUnits, v.len() as u64)?;
    Ok(v.into())
}
fn same_source(
    a: &nepl3_core::source::SnapshotId,
    c: &nepl3_core::source::SnapshotId,
    b: &mut Budget,
) -> Result<bool, RenameError> {
    b.charge(
        Resource::Work,
        (a.source.0.len() + c.source.0.len()) as u64 + 42,
    )?;
    Ok(a == c)
}
fn copy_span(span: &Span, b: &mut Budget) -> Result<Span, RenameError> {
    b.charge(
        Resource::Work,
        span.snapshot_ref().source.0.len() as u64 + 42,
    )?;
    b.charge(
        Resource::AllocationUnits,
        span.snapshot_ref().source.0.len() as u64,
    )?;
    Ok(span.clone())
}
