//! Mode declarations and native tokenization results. Array order is semantic.
use crate::{builtin::BuiltinReader, model::*};
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    budget::StopReason,
    diagnostic::{Diagnostic, Report},
    origin::Mapping,
    source::{SourceRef, SourceSnapshot, Span},
    value::{KindRef, NdfValue},
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
/// Source/context remain borrowed while suspended; the initial state borrow ends after `read`.
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
        continuation: Box<ReaderContinuation>,
    },
    Reserve {
        request: ReservationRequest,
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
