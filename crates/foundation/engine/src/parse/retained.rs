//! Immutable input provenance is retained by the owner of the actual session.
//! This native association is never reconstructed from wire data or raw trees.
use super::*;
use crate::profile::ResolvedParseProfile;
use alloc::string::String;
use nepl3_core::{
    budget::Budget,
    source::{SourceAdmission, SourceReservation, SourceStore},
};
use nepl3_reader::runtime::ProviderReply;

pub struct RetainedParseSeed<'a> {
    request: ParseRequest<'a>,
    sources: &'a SourceStore,
    profile: &'a ResolvedParseProfile<'a>,
    environments: &'a ParseEnvironmentSet<'a>,
}
impl<'a> RetainedParseSeed<'a> {
    pub fn request(&self) -> ParseRequest<'a> {
        ParseRequest {
            snapshot: self.request.snapshot,
            start: self.request.start,
            limit: self.request.limit,
            final_input: self.request.final_input,
            entry: self.request.entry,
            states: self.request.states,
        }
    }
    pub fn sources(&self) -> &'a SourceStore {
        self.sources
    }
    pub fn profile(&self) -> &'a ResolvedParseProfile<'a> {
        self.profile
    }
    pub fn environments(&self) -> &'a ParseEnvironmentSet<'a> {
        self.environments
    }
    fn duplicate(&self) -> Self {
        Self {
            request: self.request(),
            sources: self.sources,
            profile: self.profile,
            environments: self.environments,
        }
    }
}
pub struct RetainedParse<'a> {
    execution: ExecutedParse,
    seed: RetainedParseSeed<'a>,
}
impl<'a> RetainedParse<'a> {
    pub fn execution(&self) -> &ExecutedParse {
        &self.execution
    }
    pub fn seed(&self) -> &RetainedParseSeed<'a> {
        &self.seed
    }
    /// Consumes both the execution proof and the retained input association.
    pub fn into_reply(self) -> ParseReply {
        self.execution.into_reply()
    }
}
pub type RetainedParseExecution<'a> = core::ops::ControlFlow<ParseReply, RetainedParse<'a>>;
pub struct RetainedParseHostReply<'a> {
    pub execution: RetainedParseExecution<'a>,
    pub host_error: Option<ParseError>,
}
/// One starting attempt and its continuations, under immutable input context.
/// Continuations use the retained store; an accepted append may replace it.
/// Callback implementations remain the registered host's responsibility.
pub struct RetainedParseSession<'a> {
    session: ParseSession<'a>,
    seed: RetainedParseSeed<'a>,
    started: bool,
}
impl<'a> RetainedParseSession<'a> {
    pub fn new(
        session_id: String,
        profile: &'a ResolvedParseProfile<'a>,
        environments: &'a ParseEnvironmentSet<'a>,
        request: ParseRequest<'a>,
        sources: &'a SourceStore,
        budget: &mut Budget,
    ) -> Result<Self, ParseError> {
        Ok(Self {
            session: ParseSession::new(session_id, profile, environments, budget)?,
            seed: RetainedParseSeed {
                request,
                sources,
                profile,
                environments,
            },
            started: false,
        })
    }
    fn bind(&mut self, value: ParseExecution) -> RetainedParseExecution<'a> {
        match value {
            ParseExecution::Continue(execution) => {
                RetainedParseExecution::Continue(RetainedParse {
                    execution,
                    seed: self.seed.duplicate(),
                })
            }
            ParseExecution::Break(reply) => {
                if matches!(reply.outcome, ParseOutcome::Stopped { .. }) {
                    self.session.close();
                }
                RetainedParseExecution::Break(reply)
            }
        }
    }
    pub fn close(&mut self) {
        self.session.close();
    }
    pub fn read(
        &mut self,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<RetainedParseExecution<'a>, ParseError> {
        if self.started {
            return Err(ParseError::Busy);
        }
        self.started = true;
        let value = self.session.read_executed(
            self.seed.request(),
            self.seed.sources,
            budget,
            admission,
        )?;
        Ok(self.bind(value))
    }
    pub fn read_with_host(
        &mut self,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        host: &mut impl ParseHost,
    ) -> Result<RetainedParseHostReply<'a>, ParseError> {
        if self.started {
            return Err(ParseError::Busy);
        }
        self.started = true;
        let value = self.session.read_executed_with_host(
            self.seed.request(),
            self.seed.sources,
            budget,
            admission,
            host,
        )?;
        Ok(RetainedParseHostReply {
            execution: self.bind(value.execution),
            host_error: value.host_error,
        })
    }
    pub fn resume(
        &mut self,
        echo: &ParseContinuation,
        reply: ProviderReply,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<RetainedParseExecution<'a>, ParseError> {
        let value =
            self.session
                .resume_executed(echo, reply, self.seed.sources, budget, admission)?;
        Ok(self.bind(value))
    }
    pub fn reserve(
        &mut self,
        echo: &ParseContinuation,
        reservation: &SourceReservation,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<RetainedParseExecution<'a>, ParseError> {
        let value = self.session.reserve_executed(
            echo,
            reservation,
            self.seed.sources,
            budget,
            admission,
        )?;
        Ok(self.bind(value))
    }
    pub fn resume_head(
        &mut self,
        echo: &HeadContinuation,
        reply: crate::head::HeadReply,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<RetainedParseExecution<'a>, ParseError> {
        let value =
            self.session
                .resume_head_executed(echo, reply, self.seed.sources, budget, admission)?;
        Ok(self.bind(value))
    }
    pub fn continue_input(
        &mut self,
        echo: &ParseProgress,
        request: ParseRequest<'a>,
        sources: &'a SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<RetainedParseExecution<'a>, ParseError> {
        let retained = ParseRequest {
            snapshot: request.snapshot,
            start: request.start,
            limit: request.limit,
            final_input: request.final_input,
            entry: request.entry,
            states: request.states,
        };
        let value = self
            .session
            .continue_input_executed(echo, request, sources, budget, admission)?;
        // Ok(Stopped) may precede validation of the replacement request. It
        // therefore never replaces the retained seed, and bind closes the session.
        if !matches!(
            &value,
            ParseExecution::Break(ParseReply {
                outcome: ParseOutcome::Stopped { .. },
                ..
            })
        ) {
            self.seed.request = retained;
            self.seed.sources = sources;
        }
        Ok(self.bind(value))
    }
}
