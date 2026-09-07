//! The head operation receives only explicit windows and completed syntax.
//! This projection is intentionally not a SyntaxBundle or a reader request.
use crate::{package::EntryContext, selection::HeadShape};
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    budget::{StopReason, Usage},
    diagnostic::Severity,
    source::{Digest, SourceRef},
    syntax::EnvironmentRef,
    value::{KindRef, NdfScalar, NdfValue, OperationRef, SchemaRef, TypedValue},
};

/// Bytes for one exact, scalar-aligned range of a larger immutable snapshot.
/// The whole snapshot digest is metadata; it cannot be recomputed from a window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedSpan {
    pub source: SourceRef,
    pub start: u64,
    pub end: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceWindow {
    pub span: ProjectedSpan,
    pub bytes: Vec<u8>,
}
/// Accepted semantic values, including compound values, belong to read information.
/// Their contents never grant source lookup, admission or environment authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedToken {
    pub kind: KindRef,
    pub head: ProjectedSpan,
    pub payload: NdfValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedHead {
    pub token: ProjectedToken,
    pub window: SourceWindow,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedNodeRef(pub u64);
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectedFieldValue {
    Atom(NdfScalar),
    Child(ProjectedNodeRef),
    Children(Vec<ProjectedNodeRef>),
    /// Environment is opaque metadata local to the original guest bundle;
    /// it is not a lookup key into a projection-wide environment table.
    Foreign {
        schema: SchemaRef,
        category: String,
        root: ProjectedNodeRef,
        environment: EnvironmentRef,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedNode {
    pub schema: SchemaRef,
    pub kind: String,
    pub head: Option<ProjectedSpan>,
    pub cover: Option<ProjectedSpan>,
    pub token: Option<ProjectedToken>,
    pub fields: Vec<ProjectedFieldValue>,
}
/// Node references use this projection's flat arena, including foreign children.
/// There are no environment resource bytes, source snapshots or origin tables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedSyntax {
    pub nodes: Vec<ProjectedNode>,
    pub roots: Vec<ProjectedNodeRef>,
    pub windows: Vec<SourceWindow>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadCallIdentity {
    pub session_id: String,
    pub call_id: u64,
    pub operation: OperationRef,
    pub profile_digest: Digest,
    pub execution_digest: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeadRequest {
    Shape,
    ChildContext {
        shape: Box<HeadShape>,
        index: u64,
        completed: ProjectedSyntax,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadCall {
    pub identity: HeadCallIdentity,
    pub depth_base: u64,
    pub entry: EntryContext,
    pub environment: EnvironmentRef,
    pub head: ProjectedHead,
    pub request: HeadRequest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeadOutcome {
    Shape {
        shape: Option<Box<HeadShape>>,
    },
    ChildContext {
        context: EntryContext,
    },
    Failed {
        diagnostic: Box<ProjectedDiagnostic>,
    },
    Stopped {
        reason: StopReason,
    },
}
/// This metadata operation cannot add sources. Report spans must be contained
/// in the originating call's explicit windows, including related/fix locations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadReply {
    pub identity: HeadCallIdentity,
    pub outcome: HeadOutcome,
    pub report: ProjectedReport,
}

/// These report positions are snapshot-absolute byte claims restricted to saved windows.
/// The host resolves
/// them against its saved projection. They are not unchecked core Spans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedRelated {
    pub span: Option<ProjectedSpan>,
    pub code: String,
    pub arguments: TypedValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedEdit {
    pub span: ProjectedSpan,
    pub expected_digest: Digest,
    pub replacement: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedFix {
    pub id: String,
    pub edits: Vec<ProjectedEdit>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedDiagnostic {
    pub schema: SchemaRef,
    pub code: String,
    pub severity: Severity,
    pub stage: String,
    pub arguments: TypedValue,
    pub primary: Option<ProjectedSpan>,
    pub related: Vec<ProjectedRelated>,
    pub fixes: Vec<ProjectedFix>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedEvent {
    pub schema: SchemaRef,
    pub kind: String,
    pub operation_path: Vec<u64>,
    pub span: Option<ProjectedSpan>,
    pub payload: TypedValue,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectedReport {
    pub diagnostics: Vec<ProjectedDiagnostic>,
    pub events: Vec<ProjectedEvent>,
    pub trace_overflow: Option<nepl3_core::diagnostic::TraceOverflow>,
    pub usage: Usage,
}
