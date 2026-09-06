//! Explicit runtime state for prefix parsing; incomplete arenas have no invented root.
use crate::{
    package::{EntryContext, ReadSpecId},
    recovery::{BundleRecovery, ParseTree},
    selection::{NodeSelection, ShapeSelection},
};
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    budget::StopReason,
    diagnostic::Report,
    origin::{Mapping, Origin},
    source::{SourceRef, SourceSnapshot},
    syntax::{EnvironmentEntry, FieldValue, NodeRef, SyntaxNode},
    value::NdfValue,
    view::Token,
};
use nepl3_reader::{
    context::CheckedReaderContext,
    model::{Expectation, ProviderCall, ReaderFact},
    tokenizer::{ReservationRequest, TokenizationContinuation, TokenizationScope},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageReaderState {
    pub alias: String,
    pub state: NdfValue,
}
pub struct LanguageEnvironment<'a> {
    pub alias: String,
    pub context: CheckedReaderContext<'a>,
}
pub struct ParseRequest<'a> {
    pub snapshot: &'a SourceSnapshot,
    pub start: u64,
    pub limit: u64,
    pub final_input: bool,
    pub entry: &'a EntryContext,
    pub states: &'a [LanguageReaderState],
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageEnvironmentRef {
    pub alias: String,
    pub environment: nepl3_core::syntax::EnvironmentRef,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedParseRequest {
    pub snapshot: SourceRef,
    pub start: u64,
    pub limit: u64,
    pub final_input: bool,
    pub entry: EntryContext,
    pub states: Vec<LanguageReaderState>,
    pub environments: Vec<LanguageEnvironmentRef>,
    pub environment_entries: Vec<EnvironmentEntry>,
    pub origins: Vec<Origin>,
    pub sources: Vec<SourceSnapshot>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParseArena {
    pub path: Vec<crate::recovery::ForeignStep>,
    pub sources: Vec<SourceSnapshot>,
    pub nodes: Vec<SyntaxNode>,
    pub origins: Vec<Origin>,
    pub tokens: Vec<Token>,
    pub source_maps: Vec<Mapping>,
    pub environments: Vec<EnvironmentEntry>,
    pub selections: Vec<NodeSelection>,
    pub root: Option<NodeRef>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseFrame {
    pub entry: EntryContext,
    pub read: Option<ReadSpecId>,
    pub selection: Option<ShapeSelection>,
    pub node: Option<NodeRef>,
    pub arity: u64,
    pub children: Vec<FieldValue>,
    pub next_child: u64,
    pub arena: u64,
    pub foreign: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderFactBatch {
    pub path: Vec<crate::recovery::ForeignStep>,
    pub node: Option<NodeRef>,
    pub entry: EntryContext,
    pub facts: Vec<ReaderFact>,
    pub trivia: Vec<nepl3_core::view::Trivia>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseProgress {
    pub request: OwnedParseRequest,
    pub scope: TokenizationScope,
    pub cursor: u64,
    pub states: Vec<LanguageReaderState>,
    pub arenas: Vec<ParseArena>,
    pub frames: Vec<ParseFrame>,
    pub facts: Vec<ReaderFactBatch>,
    pub recovery: Vec<BundleRecovery>,
    pub contexts: Vec<crate::selection::BundleContext>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseContinuation {
    pub usage: nepl3_core::budget::Usage,
    pub depth_base: u64,
    pub session_id: String,
    pub progress: ParseProgress,
    pub tokenizer: Box<TokenizationContinuation>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum ParseOutcome {
    Complete {
        tree: ParseTree,
        cursor: u64,
        states: Vec<LanguageReaderState>,
        facts: Vec<ReaderFactBatch>,
    },
    Recovered {
        tree: ParseTree,
        cursor: u64,
        states: Vec<LanguageReaderState>,
        facts: Vec<ReaderFactBatch>,
    },
    NeedMore {
        expected: Vec<Expectation>,
        progress: ParseProgress,
    },
    Stopped {
        reason: StopReason,
        progress: Option<ParseProgress>,
    },
    Await {
        call: Box<ProviderCall>,
        continuation: Box<ParseContinuation>,
    },
    Reserve {
        request: ReservationRequest,
        continuation: Box<ParseContinuation>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub struct ParseReply {
    pub outcome: ParseOutcome,
    pub report: Report,
    pub sources: Vec<SourceSnapshot>,
    pub source_maps: Vec<Mapping>,
}
