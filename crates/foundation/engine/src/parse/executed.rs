//! Execution provenance is an owned native proof, separate from raw ParseTree
//! transport. Only these actual session calls can construct it.
use super::*;
use crate::recovery::ParseTree;
use alloc::vec::Vec;
use nepl3_core::{
    budget::Budget,
    diagnostic::Report,
    origin::Mapping,
    source::{SourceAdmission, SourceReservation, SourceSnapshot, SourceStore},
};
use nepl3_reader::runtime::ProviderReply;

/// Outcome of an actual completed prefix parse, including recovery.
/// This is independent of whole-input consumption and semantic validity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionKind {
    Complete,
    Recovered,
}

pub struct ExecutedParse {
    kind: ExecutionKind,
    tree: ParseTree,
    cursor: u64,
    states: Vec<LanguageReaderState>,
    facts: Vec<ReaderFactBatch>,
    report: Report,
    sources: Vec<SourceSnapshot>,
    source_maps: Vec<Mapping>,
}
impl ExecutedParse {
    pub fn kind(&self) -> ExecutionKind {
        self.kind
    }
    pub fn tree(&self) -> &ParseTree {
        &self.tree
    }
    pub fn cursor(&self) -> u64 {
        self.cursor
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
    pub fn reader_facts(&self) -> &[ReaderFactBatch] {
        &self.facts
    }
    /// Sources retained by the actual parse reply, including provider output.
    /// This is the reply closure, not the seed store, the recursive syntax-bundle
    /// closure, a host-current snapshot set, or evidence of editing authority.
    /// Borrowing does not allocate or consume the execution proof.
    pub fn sources(&self) -> &[SourceSnapshot] {
        &self.sources
    }
    /// Source mappings retained by the same actual parse reply. These remain
    /// unchanged when the proof is consumed with `into_reply`.
    pub fn source_maps(&self) -> &[Mapping] {
        &self.source_maps
    }
    /// Taking raw data consumes the execution proof. There is intentionally no
    /// inverse constructor from ParseReply, ParseTree or a decoded wire value.
    pub fn into_reply(self) -> ParseReply {
        ParseReply {
            outcome: match self.kind {
                ExecutionKind::Complete => ParseOutcome::Complete {
                    tree: self.tree,
                    cursor: self.cursor,
                    states: self.states,
                    facts: self.facts,
                },
                ExecutionKind::Recovered => ParseOutcome::Recovered {
                    tree: self.tree,
                    cursor: self.cursor,
                    states: self.states,
                    facts: self.facts,
                },
            },
            report: self.report,
            sources: self.sources,
            source_maps: self.source_maps,
        }
    }
}
/// Continue contains immutable syntax from actual Complete or Recovered execution.
/// Break retains other replies unchanged, with no extra allocation.
pub type ParseExecution = core::ops::ControlFlow<ParseReply, ExecutedParse>;
fn executed(reply: ParseReply) -> ParseExecution {
    let ParseReply {
        outcome,
        report,
        sources,
        source_maps,
    } = reply;
    match outcome {
        ParseOutcome::Complete {
            tree,
            cursor,
            states,
            facts,
        } => ParseExecution::Continue(ExecutedParse {
            kind: ExecutionKind::Complete,
            tree,
            cursor,
            states,
            facts,
            report,
            sources,
            source_maps,
        }),
        ParseOutcome::Recovered {
            tree,
            cursor,
            states,
            facts,
        } => ParseExecution::Continue(ExecutedParse {
            kind: ExecutionKind::Recovered,
            tree,
            cursor,
            states,
            facts,
            report,
            sources,
            source_maps,
        }),
        outcome => ParseExecution::Break(ParseReply {
            outcome,
            report,
            sources,
            source_maps,
        }),
    }
}
impl ParseSession<'_> {
    pub fn read_executed(
        &mut self,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseExecution, ParseError> {
        self.read(request, sources, budget, admission).map(executed)
    }
    pub fn resume_executed(
        &mut self,
        echo: &ParseContinuation,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseExecution, ParseError> {
        self.resume(echo, reply, sources, budget, admission)
            .map(executed)
    }
    pub fn reserve_executed(
        &mut self,
        echo: &ParseContinuation,
        reservation: &SourceReservation,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseExecution, ParseError> {
        self.reserve(echo, reservation, sources, budget, admission)
            .map(executed)
    }
    pub fn resume_head_executed(
        &mut self,
        echo: &HeadContinuation,
        reply: crate::head::HeadReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseExecution, ParseError> {
        self.resume_head(echo, reply, sources, budget, admission)
            .map(executed)
    }
}

/// Preserves synchronous callback errors alongside the ordinary parse result.
pub struct ParseHostExecution {
    pub execution: ParseExecution,
    pub host_error: Option<ParseError>,
}
impl ParseSession<'_> {
    pub fn read_executed_with_host(
        &mut self,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        host: &mut impl ParseHost,
    ) -> Result<ParseHostExecution, ParseError> {
        self.read_with_host(request, sources, budget, admission, host)
            .map(|value| ParseHostExecution {
                execution: executed(value.reply),
                host_error: value.host_error,
            })
    }
    pub fn continue_input_executed(
        &mut self,
        echo: &ParseProgress,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseExecution, ParseError> {
        self.continue_input(echo, request, sources, budget, admission)
            .map(executed)
    }
}
