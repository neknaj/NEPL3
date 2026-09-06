//! Typed requests, results, provider suspension and transactional reader facts.
use crate::plan::{CharClass, ReaderId};
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    budget::{StopReason, Usage},
    diagnostic::{Diagnostic, Event, Report, TraceOverflow},
    origin::{Mapping, Origin},
    source::{Digest, SourceRef, SourceSnapshot, Span},
    syntax::EnvironmentEntry,
    value::{NdfValue, OperationRef, SchemaRef, TypedValue},
    view::{PresentationClass, ViewBundle},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expectation {
    Literal(String),
    ScalarClass(CharClass),
    EndOfInput,
    TokenBoundary,
    Provider {
        operation: OperationRef,
        arguments: TypedValue,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReaderFact {
    Capture {
        name: String,
        span: Span,
    },
    Presentation {
        class: PresentationClass,
        span: Span,
    },
    Relation {
        schema: SchemaRef,
        kind: String,
        from: Span,
        to: Span,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderContext {
    pub schema: SchemaRef,
    pub category: String,
    pub mode: String,
    pub environment: EnvironmentEntry,
    pub origins: Vec<Origin>,
}
#[derive(Clone, Copy, Debug)]
pub struct ReadRequest<'a> {
    pub snapshot: &'a SourceSnapshot,
    pub start: u64,
    pub limit: u64,
    pub final_input: bool,
    pub context: &'a crate::context::CheckedReaderContext<'a>,
    pub state: &'a NdfValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedReadRequest {
    pub snapshot: SourceRef,
    pub sources: Vec<SourceSnapshot>,
    pub start: u64,
    pub limit: u64,
    pub final_input: bool,
    pub context: ReaderContext,
    pub state: NdfValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransformRequest {
    pub value: NdfValue,
    pub span: Span,
    pub view: ViewBundle,
    pub context: ReaderContext,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependentRequest {
    pub first: NdfValue,
    pub end: u64,
    pub request: OwnedReadRequest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderCall {
    Read {
        session_id: String,
        call_id: u64,
        depth_base: u64,
        operation: OperationRef,
        request: OwnedReadRequest,
    },
    Transform {
        session_id: String,
        call_id: u64,
        depth_base: u64,
        operation: OperationRef,
        request: TransformRequest,
    },
    Dependent {
        session_id: String,
        call_id: u64,
        depth_base: u64,
        operation: OperationRef,
        request: DependentRequest,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransformReply {
    pub value: NdfValue,
    pub view: ViewBundle,
    pub facts: Vec<ReaderFact>,
    pub sources: Vec<SourceSnapshot>,
    pub source_maps: Vec<Mapping>,
    pub report: Report,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderCheckpoint {
    pub cursor: u64,
    pub state: NdfValue,
    pub view: ViewBundle,
    pub facts: Vec<ReaderFact>,
    pub diagnostics: Vec<Diagnostic>,
    pub events: Vec<Event>,
    pub trace_overflow: Option<TraceOverflow>,
    pub sources: Vec<SourceSnapshot>,
    pub source_maps: Vec<Mapping>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FramePhase {
    Enter,
    Seq {
        next: u64,
        values: Vec<NdfValue>,
    },
    Choice {
        next: u64,
        furthest: u64,
        expected: Vec<Expectation>,
    },
    Repeat {
        count: u64,
        iteration_start: u64,
        values: Vec<NdfValue>,
    },
    AwaitChild,
    Then {
        first: NdfValue,
        end: u64,
    },
    Provider {
        call_id: u64,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderFrame {
    pub expression: ReaderId,
    pub start: u64,
    pub checkpoint: ReaderCheckpoint,
    pub phase: FramePhase,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderContinuation {
    pub session_id: String,
    pub depth_base: u64,
    pub plan_schema: SchemaRef,
    pub plan_digest: Digest,
    pub request: OwnedReadRequest,
    pub frames: Vec<ReaderFrame>,
    pub current: ReaderCheckpoint,
    pub pending: ProviderCall,
    pub usage: Usage,
    pub report: Report,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadReply {
    Matched {
        value: NdfValue,
        end: u64,
        new_state: NdfValue,
        view: ViewBundle,
        facts: Vec<ReaderFact>,
        sources: Vec<SourceSnapshot>,
        source_maps: Vec<Mapping>,
        report: Report,
    },
    NoMatch {
        expected: Vec<Expectation>,
        furthest: u64,
        sources: Vec<SourceSnapshot>,
        source_maps: Vec<Mapping>,
        report: Report,
    },
    NeedMore {
        expected: Vec<Expectation>,
        sources: Vec<SourceSnapshot>,
        source_maps: Vec<Mapping>,
        report: Report,
    },
    Failed {
        diagnostic: Diagnostic,
        recovery: Option<Span>,
        sources: Vec<SourceSnapshot>,
        source_maps: Vec<Mapping>,
        report: Report,
    },
    Stopped {
        reason: StopReason,
        sources: Vec<SourceSnapshot>,
        source_maps: Vec<Mapping>,
        report: Report,
    },
    Await {
        call: Box<ProviderCall>,
        continuation: Box<ReaderContinuation>,
        report: Report,
    },
}
