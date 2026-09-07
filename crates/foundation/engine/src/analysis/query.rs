//! Queries consume the completed resolutions, never repeat name lookup.
use super::{AnalysisKey, BindingAccessError, BoundBindingReply};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    facts::{EntityId, OccurrenceId, OccurrenceRole, ReferenceResolution},
    source::{SourceAdmission, SourceError, SourceRef, SourceSnapshot, Span},
};
mod run;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReferenceOptions {
    pub include_definitions: bool,
    pub include_imports: bool,
    pub include_exports: bool,
    pub include_ambiguous: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryKind {
    Definition,
    References(ReferenceOptions),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryRequest {
    pub key: AnalysisKey,
    pub source: SourceRef,
    pub offset: u64,
    pub kind: QueryKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuerySelection {
    pub occurrence: OccurrenceId,
    pub span: Span,
    pub resolution: ReferenceResolution,
    pub open_input: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionLocation {
    pub uri: String,
    pub range: Span,
    pub selection: Option<Span>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionTarget {
    pub entity: EntityId,
    pub location: Option<DefinitionLocation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceLocation {
    pub occurrence: OccurrenceId,
    pub role: OccurrenceRole,
    pub uri: String,
    pub span: Span,
    pub ambiguous: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityReferences {
    pub entity: EntityId,
    pub locations: Vec<ReferenceLocation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryError {
    Access(BindingAccessError),
    Source(SourceError),
    Facts,
}
impl From<StopReason> for QueryError {
    fn from(r: StopReason) -> Self {
        Self::Access(BindingAccessError::Stopped(r))
    }
}
impl From<SourceError> for QueryError {
    fn from(e: SourceError) -> Self {
        Self::Source(e)
    }
}
impl From<BindingAccessError> for QueryError {
    fn from(e: BindingAccessError) -> Self {
        Self::Access(e)
    }
}
impl QueryError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Access(BindingAccessError::Stopped(r))
            | Self::Source(SourceError::Stopped(r)) => Some(*r),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryOutcome {
    Definition {
        selection: Option<QuerySelection>,
        targets: Vec<DefinitionTarget>,
    },
    References {
        selection: Option<QuerySelection>,
        groups: Vec<EntityReferences>,
    },
    Invalid(QueryError),
    Stopped(StopReason),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryReply {
    pub key: AnalysisKey,
    pub outcome: QueryOutcome,
    pub report: Report,
    pub sources: Vec<SourceSnapshot>,
}

/// The query is atomic and produces no new diagnostics/events. On failure its
/// typed outcome and consumed Usage remain available; candidate locations are
/// not published. Existing analysis diagnostics remain in the binding reply.
pub fn query(
    binding: &BoundBindingReply,
    request: &QueryRequest,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> QueryReply {
    let mut sources = Vec::new();
    let outcome = match budget.with_depth(|budget| {
        let result = run::run(binding, request, &mut sources, budget, admission)?;
        run::sort_sources(&mut sources, budget)?;
        Ok::<_, QueryError>(result)
    }) {
        Ok(v) => v,
        Err(e) => {
            sources.clear();
            match e.stop_reason() {
                Some(r) => QueryOutcome::Stopped(r),
                None => QueryOutcome::Invalid(e),
            }
        }
    };
    QueryReply {
        key: request.key,
        outcome,
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
        sources,
    }
}
