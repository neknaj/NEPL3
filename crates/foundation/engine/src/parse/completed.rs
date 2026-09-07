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

pub struct CompletedParse {
    tree: ParseTree,
    cursor: u64,
    states: Vec<LanguageReaderState>,
    facts: Vec<ReaderFactBatch>,
    report: Report,
    sources: Vec<SourceSnapshot>,
    source_maps: Vec<Mapping>,
}
impl CompletedParse {
    pub fn tree(&self) -> &ParseTree {
        &self.tree
    }
    pub fn cursor(&self) -> u64 {
        self.cursor
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
    /// Taking raw data consumes the execution proof. There is intentionally no
    /// inverse constructor from ParseReply, ParseTree or a decoded wire value.
    pub fn into_reply(self) -> ParseReply {
        ParseReply {
            outcome: ParseOutcome::Complete {
                tree: self.tree,
                cursor: self.cursor,
                states: self.states,
                facts: self.facts,
            },
            report: self.report,
            sources: self.sources,
            source_maps: self.source_maps,
        }
    }
}
/// Continue contains an immutable completed result. Break retains the ordinary
/// wait/recovery/stop reply, with no extra allocation that could lose its report.
pub type ParseCompletion = core::ops::ControlFlow<ParseReply, CompletedParse>;
fn completed(reply: ParseReply) -> ParseCompletion {
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
        } => ParseCompletion::Continue(CompletedParse {
            tree,
            cursor,
            states,
            facts,
            report,
            sources,
            source_maps,
        }),
        outcome => ParseCompletion::Break(ParseReply {
            outcome,
            report,
            sources,
            source_maps,
        }),
    }
}
impl ParseSession<'_> {
    pub fn read_completed(
        &mut self,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseCompletion, ParseError> {
        self.read(request, sources, budget, admission)
            .map(completed)
    }
    pub fn resume_completed(
        &mut self,
        echo: &ParseContinuation,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseCompletion, ParseError> {
        self.resume(echo, reply, sources, budget, admission)
            .map(completed)
    }
    pub fn reserve_completed(
        &mut self,
        echo: &ParseContinuation,
        reservation: &SourceReservation,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseCompletion, ParseError> {
        self.reserve(echo, reservation, sources, budget, admission)
            .map(completed)
    }
    pub fn resume_head_completed(
        &mut self,
        echo: &HeadContinuation,
        reply: crate::head::HeadReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseCompletion, ParseError> {
        self.resume_head(echo, reply, sources, budget, admission)
            .map(completed)
    }
}
