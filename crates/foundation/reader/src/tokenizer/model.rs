//! Mode declarations and native tokenization results. Array order is semantic.
use crate::{builtin::BuiltinReader, model::*};
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    budget::{StopReason, Usage},
    diagnostic::{Diagnostic, Report},
    origin::Mapping,
    source::{Digest, SourceRef, SourceSnapshot, Span},
    value::{KindRef, NdfValue, SchemaRef},
    view::{Token, Trivia},
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenReader {
    Builtin(BuiltinReader),
    Rule(String),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkipRule {
    pub reader: TokenReader,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TakeRule {
    pub reader: TokenReader,
    pub kind: KindRef,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderMode {
    pub name: String,
    pub skip: Vec<SkipRule>,
    pub take: Vec<TakeRule>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenTarget {
    Mode,
    Builtin {
        reader: BuiltinReader,
        token_kind: KindRef,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenizationPhase {
    Skip { next: u64 },
    Take { next: u64 },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenizationWait {
    Reservation {
        request: ReservationRequest,
    },
    Provider {
        continuation: Box<ReaderContinuation>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenizationScope {
    pub operation_id: String,
    pub profile_digest: Digest,
    pub snapshot: SourceRef,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenizationContinuation {
    pub scope: TokenizationScope,
    pub session_id: String,
    pub reader_schema: SchemaRef,
    pub reader_plan_digest: Digest,
    pub configuration_digest: Digest,
    pub request: OwnedReadRequest,
    pub mode: String,
    pub target: TokenTarget,
    pub phase: TokenizationPhase,
    pub current: ReaderCheckpoint,
    pub trivia: Vec<Trivia>,
    pub expected: Vec<Expectation>,
    pub furthest: u64,
    pub pending: TokenizationWait,
    pub depth_base: u64,
    pub usage: Usage,
    pub report: Report,
}
/// An explicit parsing operation may carry its accepted collector across language tokenizers.
pub struct ScopedTokenizationRequest<'source, 'state> {
    pub scope: &'source TokenizationScope,
    pub target: TokenTarget,
    pub input: TokenizationRequest<'source, 'state>,
}
/// Borrowed input for one call; suspension returns an owned continuation.
#[derive(Clone, Copy)]
pub struct TokenizationRequest<'source, 'state> {
    pub snapshot: &'source SourceSnapshot,
    pub start: u64,
    pub limit: u64,
    pub final_input: bool,
    pub context: &'source crate::context::CheckedReaderContext<'source>,
    pub state: &'state NdfValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReservationRequest {
    pub session_id: String,
    pub request_id: u64,
    pub snapshot: SourceRef,
    pub start: u64,
    pub limit: u64,
}
#[derive(Debug, Eq, PartialEq)]
pub enum TokenizationOutcome {
    Token(Token),
    End,
    NoMatch {
        expected: Vec<Expectation>,
        furthest: u64,
    },
    NeedMore {
        expected: Vec<Expectation>,
    },
    Failed {
        diagnostic: Diagnostic,
        recovery: Option<Span>,
    },
    Stopped {
        reason: StopReason,
    },
    Await {
        call: Box<ProviderCall>,
        continuation: Box<TokenizationContinuation>,
    },
    Reserve {
        request: ReservationRequest,
        continuation: Box<TokenizationContinuation>,
    },
}
#[derive(Debug, Eq, PartialEq)]
pub struct TokenizationReply {
    pub outcome: TokenizationOutcome,
    pub cursor: u64,
    pub new_state: Option<NdfValue>,
    pub trivia: Vec<Trivia>,
    pub facts: Vec<ReaderFact>,
    pub sources: Vec<SourceSnapshot>,
    pub source_maps: Vec<Mapping>,
    pub report: Report,
}
