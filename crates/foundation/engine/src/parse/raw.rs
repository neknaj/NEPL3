//! Raw transport adapters consume the internal native validation proof.
use super::*;
use crate::head::HeadReply;
use crate::tree::OwnedValidatedParseTree;
use nepl3_core::{
    budget::Budget,
    source::{SourceAdmission, SourceReservation, SourceStore},
};
use nepl3_reader::runtime::ProviderReply;
impl ParseSession<'_, '_> {
    pub fn read(
        &mut self,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        self.read_validated(request, sources, budget, admission)
            .map(|reply| reply.map_tree(OwnedValidatedParseTree::into_inner))
    }
    pub fn read_with_host(
        &mut self,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        host: &mut impl super::ParseHost,
    ) -> Result<super::ParseHostReply, ParseError> {
        let reply = self.read_with_host_validated(request, sources, budget, admission, host)?;
        Ok(super::ParseHostReply {
            reply: reply.reply.map_tree(OwnedValidatedParseTree::into_inner),
            host_error: reply.host_error,
        })
    }
    pub fn continue_input(
        &mut self,
        echo: &ParseProgress,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        self.continue_input_validated(echo, request, sources, budget, admission)
            .map(|reply| reply.map_tree(OwnedValidatedParseTree::into_inner))
    }
    pub fn reserve(
        &mut self,
        echo: &ParseContinuation,
        reservation: &SourceReservation,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        self.reserve_validated(echo, reservation, sources, budget, admission)
            .map(|reply| reply.map_tree(OwnedValidatedParseTree::into_inner))
    }
    pub fn resume(
        &mut self,
        echo: &ParseContinuation,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        self.resume_validated(echo, reply, sources, budget, admission)
            .map(|reply| reply.map_tree(OwnedValidatedParseTree::into_inner))
    }
    pub fn resume_head(
        &mut self,
        echo: &HeadContinuation,
        reply: HeadReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        self.resume_head_validated(echo, reply, sources, budget, admission)
            .map(|reply| reply.map_tree(OwnedValidatedParseTree::into_inner))
    }
}
