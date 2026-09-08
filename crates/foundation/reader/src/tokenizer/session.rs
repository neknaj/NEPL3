use super::{
    AcceptedTokenizationFailure, AcceptedTokenizationReply, AcceptedTokenizationReport,
    model::*,
    recovery::{Prefix, rejected},
};
use crate::{
    builtin::{self, BuiltinReader},
    model::*,
    plan::CheckedPlan,
    runtime::{
        self, ProviderReply, ReaderError, ReaderSession,
        copy::{CopyCost, copy, slot},
    },
};
use alloc::{boxed::Box, rc::Rc, string::String, vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason},
    diagnostic::Report,
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceError, SourceReservation, SourceStore},
    view::{Token, Trivia, TriviaKind, ViewBundle},
};
#[cfg(test)]
mod ownership_tests;

enum Resume<'a> {
    Reservation(&'a SourceReservation),
    Provider(ProviderReply),
}
enum Outcome {
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
        diagnostic: nepl3_core::diagnostic::Diagnostic,
        recovery: Option<nepl3_core::source::Span>,
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
/// The public waiting payload after moving the reader-owned continuation into
/// the tokenizer's private slot. It does not duplicate the nested VM state.
enum WaitingOutcome {
    Provider(Box<ProviderCall>),
    Reservation(ReservationRequest),
}
impl WaitingOutcome {
    fn public(self, continuation: Box<TokenizationContinuation>) -> TokenizationOutcome {
        match self {
            Self::Provider(call) => TokenizationOutcome::Await { call, continuation },
            Self::Reservation(request) => TokenizationOutcome::Reserve {
                request,
                continuation,
            },
        }
    }
}
impl Outcome {
    fn public(
        self,
        continuation: Option<Box<TokenizationContinuation>>,
    ) -> Result<TokenizationOutcome, ReaderError> {
        Ok(match self {
            Self::Token(v) => TokenizationOutcome::Token(v),
            Self::End => TokenizationOutcome::End,
            Self::NoMatch { expected, furthest } => {
                TokenizationOutcome::NoMatch { expected, furthest }
            }
            Self::NeedMore { expected } => TokenizationOutcome::NeedMore { expected },
            Self::Failed {
                diagnostic,
                recovery,
            } => TokenizationOutcome::Failed {
                diagnostic,
                recovery,
            },
            Self::Stopped { reason } => TokenizationOutcome::Stopped { reason },
            Self::Await { call, .. } => TokenizationOutcome::Await {
                call,
                continuation: continuation.ok_or(ReaderError::Continuation)?,
            },
            Self::Reserve { request } => TokenizationOutcome::Reserve {
                request,
                continuation: continuation.ok_or(ReaderError::Continuation)?,
            },
        })
    }
}
#[derive(Clone, Copy)]
struct Input<'a> {
    snapshot: &'a nepl3_core::source::SourceSnapshot,
    start: u64,
    initial_state: &'a nepl3_core::value::NdfValue,
    limit: u64,
    final_input: bool,
    context: &'a crate::context::CheckedReaderContext<'a>,
}
impl Input<'_> {
    fn request<'s>(
        &'s self,
        start: u64,
        state: &'s nepl3_core::value::NdfValue,
    ) -> ReadRequest<'s> {
        ReadRequest {
            snapshot: self.snapshot,
            start,
            limit: self.limit,
            final_input: self.final_input,
            context: self.context,
            state,
        }
    }
}
struct Machine<'a, 'i> {
    request: Input<'i>,
    mode: &'a ReaderMode,
    target: TokenTarget,
    phase: TokenizationPhase,
    current: ReaderCheckpoint,
    trivia: Vec<Trivia>,
    waiting: bool,
    expected: Vec<Expectation>,
    furthest: u64,
    limits: Limits,
    depth_base: u64,
}
struct Pending {
    continuation: TokenizationContinuation,
    limits: Limits,
}
/// Owns its saved continuation; no caller source/context borrow survives a suspension.
pub struct TokenizationSession<'a> {
    session_id: String,
    modes: &'a [ReaderMode],
    checked: &'a CheckedPlan<'a>,
    registry: &'a SchemaRegistry,
    reader: ReaderSession<'a>,
    pending: Option<Pending>,
    next_request: u64,
    closed: bool,
    plan_digest: Digest,
    configuration_digest: Digest,
    scope: Option<Rc<TokenizationScope>>,
    next_operation: u64,
}
impl<'a> TokenizationSession<'a> {
    pub fn new(
        session_id: String,
        modes: &'a [ReaderMode],
        checked: &'a CheckedPlan<'a>,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<Self, ReaderError> {
        for (index, mode) in modes.iter().enumerate() {
            budget.charge(Resource::Work, (index + mode.name.len()) as u64)?;
            if mode.name.is_empty() || modes[..index].iter().any(|m| m.name == mode.name) {
                return Err(ReaderError::Context);
            }
            for reader in mode
                .skip
                .iter()
                .map(|s| &s.reader)
                .chain(mode.take.iter().map(|t| &t.reader))
            {
                if let TokenReader::Rule(name) = reader {
                    checked.plan().rule(name)?;
                }
            }
            for take in &mode.take {
                registry.kind_name(&take.kind.schema, take.kind.local_kind)?;
            }
        }
        let plan_digest = checked.plan().digest(budget)?;
        let configuration_digest = super::identity::digest(modes, plan_digest, budget)?;
        let reader = ReaderSession::new(copy(&session_id, budget)?, checked, registry, budget)?;
        Ok(Self {
            session_id,
            modes,
            checked,
            registry,
            reader,
            pending: None,
            next_request: 0,
            closed: false,
            plan_digest,
            configuration_digest,
            scope: None,
            next_operation: 0,
        })
    }
    /// Terminate the current suspended call after its enclosing operation has retained its report.
    pub fn discard_pending(&mut self) {
        self.pending = None;
        self.reader.discard_pending();
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.pending = None;
        self.reader.close();
    }
    pub fn read(
        &mut self,
        request: TokenizationRequest<'_, '_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<TokenizationReply, ReaderError> {
        self.read_target(TokenTarget::Mode, request, sources, budget, admission)
    }
    pub fn read_target(
        &mut self,
        target: TokenTarget,
        request: TokenizationRequest<'_, '_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<TokenizationReply, ReaderError> {
        if self.closed {
            return Err(ReaderError::Closed);
        }
        if self.pending.is_some() {
            return Err(ReaderError::Busy);
        }
        let prepared = (|| -> Result<_, ReaderError> {
            self.next_operation = self
                .next_operation
                .checked_add(1)
                .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
            budget.charge(
                Resource::AllocationUnits,
                self.session_id.len() as u64
                    + 22
                    + request.snapshot.identity().source.0.len() as u64,
            )?;
            let scope = TokenizationScope {
                operation_id: alloc::format!("{}:{}", self.session_id, self.next_operation),
                profile_digest: self.configuration_digest,
                snapshot: request.snapshot.reference(),
            };
            AcceptedTokenizationReport::empty(scope, budget)
        })();
        let accepted = match prepared {
            Ok(v) => v,
            Err(error) => {
                return match runtime::stop_reason(&error) {
                    Some(reason) => Ok(TokenizationReply {
                        outcome: TokenizationOutcome::Stopped { reason },
                        cursor: request.start,
                        new_state: None,
                        trivia: vec![],
                        facts: vec![],
                        sources: vec![],
                        source_maps: vec![],
                        report: Report {
                            usage: budget.usage(),
                            ..Report::default()
                        },
                    }),
                    None => Err(error),
                };
            }
        };
        let scope = Rc::clone(&accepted.scope);
        self.read_with_accepted(
            ScopedTokenizationRequest {
                scope: &scope,
                target,
                input: request,
            },
            sources,
            budget,
            admission,
            accepted,
        )
        .map(AcceptedTokenizationReply::into_raw)
    }

    pub fn read_with_accepted(
        &mut self,
        request: ScopedTokenizationRequest<'_, '_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        accepted: AcceptedTokenizationReport,
    ) -> Result<AcceptedTokenizationReply, ReaderError> {
        self.read_with_accepted_recover(request, sources, budget, admission, accepted)
            .map_err(AcceptedTokenizationFailure::into_error)
    }
    pub fn read_accepted_with_host(
        &mut self,
        request: ScopedTokenizationRequest<'_, '_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        accepted: AcceptedTokenizationReport,
        host: &mut impl super::TokenizationHost,
    ) -> Result<super::TokenizationHostReply, ReaderError> {
        self.read_accepted_with_host_recover(request, sources, budget, admission, accepted, host)
            .map_err(AcceptedTokenizationFailure::into_error)
    }
    /// Like `read_with_accepted`, retaining the original collector on a hard error.
    /// Normal stops retain the live collector; suspension still exports owned state.
    #[allow(clippy::result_large_err)]
    pub fn read_with_accepted_recover(
        &mut self,
        request: ScopedTokenizationRequest<'_, '_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        accepted: AcceptedTokenizationReport,
    ) -> Result<AcceptedTokenizationReply, AcceptedTokenizationFailure> {
        self.read_accepted_dispatch(
            request, sources, budget, admission, accepted, None, &mut None,
        )
    }
    /// Complete synchronous calls before exporting a tokenizer checkpoint.
    /// None or a rejected callback still publishes the regular owned boundary.
    #[allow(clippy::result_large_err)]
    pub fn read_accepted_with_host_recover(
        &mut self,
        request: ScopedTokenizationRequest<'_, '_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        accepted: AcceptedTokenizationReport,
        host: &mut impl super::TokenizationHost,
    ) -> Result<super::TokenizationHostReply, AcceptedTokenizationFailure> {
        let mut host_error = None;
        let reply = self.read_accepted_dispatch(
            request,
            sources,
            budget,
            admission,
            accepted,
            Some(host),
            &mut host_error,
        )?;
        Ok(super::TokenizationHostReply { reply, host_error })
    }
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    fn read_accepted_dispatch(
        &mut self,
        request: ScopedTokenizationRequest<'_, '_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        accepted: AcceptedTokenizationReport,
        host: Option<&mut dyn super::TokenizationHost>,
        host_error: &mut Option<ReaderError>,
    ) -> Result<AcceptedTokenizationReply, AcceptedTokenizationFailure> {
        let ScopedTokenizationRequest {
            scope,
            target,
            input: request,
        } = request;
        if accepted.limits != budget.limits()
            || !runtime::usage_at_least(budget.usage(), accepted.report.usage)
        {
            return Err(rejected(ReaderError::Continuation, accepted, budget));
        }
        if let Err(reason) = budget.charge(
            Resource::Work,
            scope.operation_id.len() as u64
                + scope.snapshot.source_id.0.len() as u64
                + request.snapshot.identity().source.0.len() as u64
                + 65,
        ) {
            let scope = Rc::clone(&accepted.scope);
            return Ok(AcceptedTokenizationReply::from_native(
                empty_stop(request.start, reason, accepted, budget),
                scope,
                budget,
            ));
        }
        if scope != accepted.scope()
            || scope.snapshot.source_id != request.snapshot.identity().source
            || scope.snapshot.revision != request.snapshot.identity().revision
            || scope.snapshot.digest != request.snapshot.identity().digest
        {
            return Err(rejected(ReaderError::Continuation, accepted, budget));
        }
        let scope = Rc::clone(&accepted.scope);
        let prefix = Prefix::capture(&accepted);
        let result = self.read_seed(
            target, request, sources, budget, admission, accepted, host, host_error,
        );
        match result {
            Ok(reply) => Ok(AcceptedTokenizationReply::from_native(reply, scope, budget)),
            Err(failure) => match prefix.restore(failure.accepted) {
                Ok(accepted) => Err(AcceptedTokenizationFailure::Recoverable {
                    error: failure.error,
                    accepted,
                }),
                Err(()) => {
                    self.close();
                    Err(AcceptedTokenizationFailure::BrokenPrefix {
                        original_error: failure.error,
                        observed_stop: budget.poll().err(),
                    })
                }
            },
        }
    }
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    fn read_seed(
        &mut self,
        target: TokenTarget,
        request: TokenizationRequest<'_, '_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        accepted: AcceptedTokenizationReport,
        host: Option<&mut dyn super::TokenizationHost>,
        host_error: &mut Option<ReaderError>,
    ) -> Result<TokenizationReply, runtime::ReadFailure> {
        if accepted.limits != budget.limits()
            || !runtime::usage_at_least(budget.usage(), accepted.report.usage)
        {
            return Err(seed_failure(ReaderError::Continuation, accepted, budget));
        }
        if let TokenTarget::Builtin { token_kind, .. } = &target
            && let Err(error) = self
                .registry
                .kind_name(&token_kind.schema, token_kind.local_kind)
        {
            return Err(seed_failure(error.into(), accepted, budget));
        }
        if self.closed {
            return Err(seed_failure(ReaderError::Closed, accepted, budget));
        }
        if self.pending.is_some() {
            return Err(seed_failure(ReaderError::Busy, accepted, budget));
        }
        self.scope = Some(Rc::clone(&accepted.scope));
        let Some(mode) = self
            .modes
            .iter()
            .find(|mode| mode.name == request.context.mode)
        else {
            return Err(seed_failure(ReaderError::Context, accepted, budget));
        };
        // Validation precedes owned checkpoint copies, including long SourceIds and state values.
        if let Err(error) = runtime::validate::request(
            &ReadRequest {
                snapshot: request.snapshot,
                start: request.start,
                limit: request.limit,
                final_input: request.final_input,
                context: request.context,
                state: request.state,
            },
            sources,
            self.registry,
            &self.checked.plan().state_type,
            budget,
            admission,
        ) {
            return match runtime::stop_reason(&error) {
                Some(reason) => Ok(empty_stop(request.start, reason, accepted, budget)),
                None => Err(seed_failure(error, accepted, budget)),
            };
        }
        let accepted_check = (|| -> Result<(), ReaderError> {
            for added in &accepted.sources {
                if let Some(prior) = sources.get_revision_with_budget(
                    &added.identity().source,
                    added.identity().revision,
                    budget,
                )? {
                    budget.charge(Resource::Work, prior.uri().len() as u64 + 33)?;
                    if prior.identity() != added.identity() || prior.uri() != added.uri() {
                        return Err(SourceError::IdentityConflict.into());
                    }
                }
                admission.admit_existing(added, budget)?;
            }
            if accepted.report.diagnostics.is_empty() && accepted.report.events.is_empty() {
                // The loop above checked/admitted every source in this private
                // prefix. An empty report has no range to resolve; retain its
                // usage/overflow validation without rebuilding that same index.
                return runtime::validate::accepted_report(
                    &accepted.report,
                    sources,
                    &[],
                    self.registry,
                    budget,
                );
            }
            runtime::validate::accepted_report(
                &accepted.report,
                sources,
                &accepted.sources,
                self.registry,
                budget,
            )
        })();
        if let Err(error) = accepted_check {
            return match runtime::stop_reason(&error) {
                Some(reason) => Ok(empty_stop(request.start, reason, accepted, budget)),
                None => Err(seed_failure(error, accepted, budget)),
            };
        }
        let state = match request.state.clone_with_budget(budget) {
            Ok(state) => state,
            Err(reason) => return Ok(empty_stop(request.start, reason, accepted, budget)),
        };
        // Check before moving the accepted collector into the machine.
        let depth_base = match budget.current_depth().checked_add(1) {
            Some(depth) => depth,
            None => {
                let reason = budget.stop(StopReason::DepthLimit);
                return Ok(empty_stop(request.start, reason, accepted, budget));
            }
        };
        let mut machine = Machine {
            request: Input {
                snapshot: request.snapshot,
                start: request.start,
                initial_state: request.state,
                limit: request.limit,
                final_input: request.final_input,
                context: request.context,
            },
            mode,
            target,
            phase: TokenizationPhase::Skip { next: 0 },
            current: ReaderCheckpoint {
                cursor: request.start,
                state,
                view: ViewBundle {
                    elements: vec![],
                    roots: vec![],
                },
                facts: vec![],
                diagnostics: accepted.report.diagnostics,
                events: accepted.report.events,
                trace_overflow: accepted.report.trace_overflow,
                sources: accepted.sources,
                source_maps: accepted.source_maps,
            },
            trivia: vec![],
            waiting: false,
            expected: vec![],
            furthest: request.start,
            limits: budget.limits(),
            depth_base,
        };
        let outcome = budget.with_depth(|budget| match host {
            Some(host) => {
                self.drive_host(&mut machine, sources, budget, admission, host, host_error)
            }
            None => self.drive(&mut machine, None, sources, budget, admission),
        });
        self.finish_recover(machine, outcome, budget)
    }
    pub fn reserve(
        &mut self,
        echo: &TokenizationContinuation,
        reservation: &SourceReservation,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<TokenizationReply, ReaderError> {
        if self.closed {
            return Err(ReaderError::Closed);
        }
        let saved = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if saved.limits != budget.limits()
            || !runtime::usage_at_least(budget.usage(), saved.continuation.usage)
            || saved.continuation.session_id != echo.session_id
        {
            return Err(ReaderError::Continuation);
        }
        if let Err(reason) = budget.poll() {
            return self.stop_pending(reason, budget);
        }
        if !matches!(
            self.pending
                .as_ref()
                .ok_or(ReaderError::NoPending)?
                .continuation
                .pending,
            TokenizationWait::Reservation { .. }
        ) {
            return Err(ReaderError::Continuation);
        }
        let pending = match self.take_pending(echo, budget) {
            Ok(pending) => pending,
            Err(error) => {
                return match runtime::stop_reason(&error) {
                    Some(reason) => self.stop_pending(reason, budget),
                    None => Err(error),
                };
            }
        };
        self.restore(
            pending,
            Resume::Reservation(reservation),
            sources,
            budget,
            admission,
        )
    }
    pub fn resume(
        &mut self,
        echo: &TokenizationContinuation,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<TokenizationReply, ReaderError> {
        if self.closed {
            return Err(ReaderError::Closed);
        }
        let saved = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if saved.limits != budget.limits()
            || !runtime::usage_at_least(budget.usage(), saved.continuation.usage)
            || saved.continuation.session_id != echo.session_id
        {
            return Err(ReaderError::Continuation);
        }
        if let Err(reason) = budget.poll() {
            return self.stop_pending(reason, budget);
        }
        if !matches!(
            self.pending
                .as_ref()
                .ok_or(ReaderError::NoPending)?
                .continuation
                .pending,
            TokenizationWait::Provider { .. }
        ) {
            return Err(ReaderError::Continuation);
        }
        reply.validate_outcome()?;
        let pending = match self.take_pending(echo, budget) {
            Ok(pending) => pending,
            Err(error) => {
                return match runtime::stop_reason(&error) {
                    Some(reason) => self.stop_pending(reason, budget),
                    None => Err(error),
                };
            }
        };
        self.restore(pending, Resume::Provider(reply), sources, budget, admission)
    }
    pub fn reserve_accepted(
        &mut self,
        echo: &TokenizationContinuation,
        reservation: &SourceReservation,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<AcceptedTokenizationReply, ReaderError> {
        let scope = Rc::clone(self.scope.as_ref().ok_or(ReaderError::NoPending)?);
        self.reserve(echo, reservation, sources, budget, admission)
            .map(|reply| AcceptedTokenizationReply::from_native(reply, scope, budget))
    }
    pub fn resume_accepted(
        &mut self,
        echo: &TokenizationContinuation,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<AcceptedTokenizationReply, ReaderError> {
        let scope = Rc::clone(self.scope.as_ref().ok_or(ReaderError::NoPending)?);
        self.resume(echo, reply, sources, budget, admission)
            .map(|reply| AcceptedTokenizationReply::from_native(reply, scope, budget))
    }
    fn take_pending(
        &mut self,
        echo: &TokenizationContinuation,
        budget: &mut Budget,
    ) -> Result<Pending, ReaderError> {
        let pending = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if pending.limits != budget.limits()
            || !runtime::usage_at_least(budget.usage(), pending.continuation.usage)
        {
            return Err(ReaderError::Continuation);
        }
        echo.charge(budget)?;
        if &pending.continuation != echo {
            return Err(ReaderError::Continuation);
        }
        self.pending.take().ok_or(ReaderError::NoPending)
    }
    fn stop_pending(
        &mut self,
        reason: StopReason,
        budget: &Budget,
    ) -> Result<TokenizationReply, ReaderError> {
        let pending = self.pending.take().ok_or(ReaderError::NoPending)?;
        self.reader.discard_pending();
        Ok(reply(
            TokenizationOutcome::Stopped { reason },
            pending.continuation.current,
            pending.continuation.trivia,
            budget,
        ))
    }
    fn restore(
        &mut self,
        pending: Pending,
        resume: Resume<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<TokenizationReply, ReaderError> {
        let c = pending.continuation;
        // Keep owned current/report/source artifacts until every fallible proof restoration succeeds.
        let prepared = (|| -> Result<_, ReaderError> {
            let snapshot = sources
                .resolve(&c.request.snapshot)
                .ok_or(SourceError::MissingSnapshot)?;
            let foundation = self
                .registry
                .selected("nepl3.foundation", 1)
                .ok_or(nepl3_core::schema::SchemaError::UnknownSchema)?;
            let context = crate::context::restored(
                &c.request.context,
                foundation,
                &c.request.sources,
                budget,
            )?;
            runtime::validate::request(
                &ReadRequest {
                    snapshot,
                    start: c.current.cursor,
                    limit: c.request.limit,
                    final_input: c.request.final_input,
                    context: &context,
                    state: &c.current.state,
                },
                sources,
                self.registry,
                &self.checked.plan().state_type,
                budget,
                admission,
            )?;
            let mode = self
                .modes
                .iter()
                .find(|mode| mode.name == c.mode)
                .ok_or(ReaderError::Context)?;
            Ok((snapshot, context, mode))
        })();
        let (snapshot, context, mode) = match prepared {
            Ok(v) => v,
            Err(error) => {
                return match runtime::stop_reason(&error) {
                    Some(reason) => {
                        self.reader.discard_pending();
                        Ok(reply(
                            TokenizationOutcome::Stopped { reason },
                            c.current,
                            c.trivia,
                            budget,
                        ))
                    }
                    None => {
                        self.pending = Some(Pending {
                            continuation: c,
                            limits: pending.limits,
                        });
                        Err(error)
                    }
                };
            }
        };
        // The outer checkpoint remains owned and untouched until the inner
        // reader accepts the response. Rejection restores the same outer slot,
        // without cloning its state or refunding consumed budget/admission.
        let (provider_reply, reservation) = match (resume, &c.pending) {
            (Resume::Provider(response), TokenizationWait::Provider { continuation }) => match self
                .reader
                .resume_from_tokenizer(continuation, response, sources, budget, admission)
            {
                Ok(reply) => (Some(reply), None),
                Err(error) => {
                    drop(context);
                    if let Some(reason) = runtime::stop_reason(&error) {
                        self.reader.discard_pending();
                        return Ok(reply(
                            TokenizationOutcome::Stopped { reason },
                            c.current,
                            c.trivia,
                            budget,
                        ));
                    }
                    self.pending = Some(Pending {
                        continuation: c,
                        limits: pending.limits,
                    });
                    return Err(error);
                }
            },
            (Resume::Reservation(reservation), TokenizationWait::Reservation { .. }) => {
                (None, Some(reservation))
            }
            _ => {
                drop(context);
                self.pending = Some(Pending {
                    continuation: c,
                    limits: pending.limits,
                });
                return Err(ReaderError::Continuation);
            }
        };
        let mut machine = Machine {
            request: Input {
                snapshot,
                start: c.request.start,
                initial_state: &c.request.state,
                limit: c.request.limit,
                final_input: c.request.final_input,
                context: &context,
            },
            mode,
            target: c.target,
            phase: c.phase,
            current: c.current,
            trivia: c.trivia,
            waiting: false,
            expected: c.expected,
            furthest: c.furthest,
            limits: pending.limits,
            depth_base: c.depth_base,
        };
        let outcome = match provider_reply {
            None => budget.with_depth_at_least(machine.depth_base, |budget| {
                self.drive(&mut machine, reservation, sources, budget, admission)
            }),
            Some(reply) => match accept(&mut machine, reply, self.registry, budget) {
                Ok(Some(outcome)) => Ok(outcome),
                Ok(None) => budget.with_depth_at_least(machine.depth_base, |budget| {
                    self.drive(&mut machine, None, sources, budget, admission)
                }),
                Err(error) => Err(error),
            },
        };
        self.finish(machine, outcome, budget)
    }
    fn drive_host(
        &mut self,
        machine: &mut Machine<'a, '_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        host: &mut dyn super::TokenizationHost,
        host_error: &mut Option<ReaderError>,
    ) -> Result<Outcome, ReaderError> {
        let mut native = runtime::NativeDispatch {
            host,
            error: host_error,
            deferred: false,
        };
        let mut outcome =
            self.drive_inner(machine, None, sources, budget, admission, Some(&mut native))?;
        loop {
            match &outcome {
                Outcome::Await { call, .. } => {
                    if native.error.is_some() || native.deferred {
                        return Ok(outcome);
                    }
                    let base = match call.as_ref() {
                        ProviderCall::Read { depth_base, .. }
                        | ProviderCall::Transform { depth_base, .. }
                        | ProviderCall::Dependent { depth_base, .. } => *depth_base,
                    };
                    let reply = match budget.with_depth_at_least(base, |budget| {
                        native.host.provider(call, budget, admission)
                    }) {
                        Ok(Some(reply)) => reply,
                        Ok(None) => return Ok(outcome),
                        Err(error) => {
                            if let Some(reason) = error.stop_reason() {
                                budget.stop(reason);
                            }
                            *native.error = Some(error);
                            return Ok(outcome);
                        }
                    };
                    // The callback receives only a borrowed call, not a mutable
                    // continuation. The reader retains the exclusive private slot.
                    // External fallback still publishes/checks the complete echo.
                    let reply = match self
                        .reader
                        .resume_native_callback(reply, sources, budget, admission)
                    {
                        Ok(reply) => reply,
                        Err(error) => {
                            if let Some(reason) = error.stop_reason() {
                                budget.stop(reason);
                            }
                            *native.error = Some(error);
                            return Ok(outcome);
                        }
                    };
                    machine.waiting = false;
                    outcome = match accept(machine, reply, self.registry, budget)? {
                        Some(outcome) => outcome,
                        None => self.drive_inner(
                            machine,
                            None,
                            sources,
                            budget,
                            admission,
                            Some(&mut native),
                        )?,
                    };
                }
                Outcome::Reserve { request } => {
                    let reservation = match native.host.reservation(request, budget, admission) {
                        Ok(Some(reservation)) => reservation,
                        Ok(None) => return Ok(outcome),
                        Err(error) => {
                            if let Some(reason) = error.stop_reason() {
                                budget.stop(reason);
                            }
                            *native.error = Some(error);
                            return Ok(outcome);
                        }
                    };
                    machine.waiting = false;
                    outcome = self.drive_inner(
                        machine,
                        Some(&reservation),
                        sources,
                        budget,
                        admission,
                        Some(&mut native),
                    )?;
                }
                _ => return Ok(outcome),
            }
        }
    }
    fn drive(
        &mut self,
        machine: &mut Machine<'a, '_>,
        reservation: Option<&SourceReservation>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Outcome, ReaderError> {
        self.drive_inner(machine, reservation, sources, budget, admission, None)
    }
    #[allow(clippy::too_many_arguments)]
    fn drive_inner(
        &mut self,
        machine: &mut Machine<'a, '_>,
        mut reservation: Option<&SourceReservation>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        mut native: Option<&mut runtime::NativeDispatch<'_>>,
    ) -> Result<Outcome, ReaderError> {
        loop {
            budget.poll()?;
            budget.charge(Resource::Work, 1)?;
            let builtin_target;
            let reader =
                match machine.phase {
                    TokenizationPhase::Skip { next: index } => match machine
                        .mode
                        .skip
                        .get(usize::try_from(index).map_err(|_| ReaderError::Continuation)?)
                    {
                        Some(rule) => &rule.reader,
                        None => {
                            machine.phase = TokenizationPhase::Take { next: 0 };
                            continue;
                        }
                    },
                    TokenizationPhase::Take { next: index } => {
                        if machine.current.cursor == machine.request.limit {
                            return Ok(if machine.request.final_input {
                                Outcome::End
                            } else {
                                Outcome::NeedMore { expected: vec![] }
                            });
                        }
                        match (&machine.target, index) {
                            (TokenTarget::Builtin { reader, .. }, 0) => {
                                builtin_target = TokenReader::Builtin(*reader);
                                &builtin_target
                            }
                            (TokenTarget::Builtin { .. }, _) => {
                                return Ok(Outcome::NoMatch {
                                    expected: core::mem::take(&mut machine.expected),
                                    furthest: machine.furthest,
                                });
                            }
                            (TokenTarget::Mode, _) => match machine.mode.take.get(
                                usize::try_from(index).map_err(|_| ReaderError::Continuation)?,
                            ) {
                                Some(rule) => &rule.reader,
                                None => {
                                    return Ok(Outcome::NoMatch {
                                        expected: core::mem::take(&mut machine.expected),
                                        furthest: machine.furthest,
                                    });
                                }
                            },
                        }
                    }
                };
            let reply = match reader {
                TokenReader::Rule(name) => {
                    let report = take_report(&mut machine.current, budget);
                    let retained_sources = core::mem::take(&mut machine.current.sources);
                    let retained_maps = core::mem::take(&mut machine.current.source_maps);
                    let request = machine
                        .request
                        .request(machine.current.cursor, &machine.current.state);
                    match self.reader.read_with_report_host_recover(
                        name,
                        request,
                        sources,
                        budget,
                        admission,
                        runtime::AcceptedReport {
                            report,
                            sources: retained_sources,
                            source_maps: retained_maps,
                        },
                        native.as_deref_mut(),
                    ) {
                        Ok(reply) => reply,
                        Err(failure) => {
                            // A rejected nested operation must return ownership
                            // before the containing tokenizer handles the error.
                            let accepted = failure.accepted;
                            machine.current.diagnostics = accepted.report.diagnostics;
                            machine.current.events = accepted.report.events;
                            machine.current.trace_overflow = accepted.report.trace_overflow;
                            machine.current.sources = accepted.sources;
                            machine.current.source_maps = accepted.source_maps;
                            return Err(failure.error);
                        }
                    }
                }
                TokenReader::Builtin(kind) => {
                    let request = machine
                        .request
                        .request(machine.current.cursor, &machine.current.state);
                    let is_text = *kind == BuiltinReader::Text;
                    if is_text && reservation.is_none() {
                        // A losing Text candidate neither requests nor consumes a source reservation.
                        let input = request.snapshot.slice_range(request.start, request.limit)?;
                        if !input.starts_with('"') {
                            let reply = if input.is_empty() && !request.final_input {
                                ReadReply::NeedMore {
                                    sources: vec![],
                                    source_maps: vec![],
                                    expected: vec![],
                                    report: Report {
                                        usage: budget.usage(),
                                        ..Report::default()
                                    },
                                }
                            } else {
                                ReadReply::NoMatch {
                                    sources: vec![],
                                    source_maps: vec![],
                                    expected: vec![],
                                    furthest: request.start,
                                    report: Report {
                                        usage: budget.usage(),
                                        ..Report::default()
                                    },
                                }
                            };
                            let reply = prefix_builtin(reply, &mut machine.current, budget)?;
                            if let Some(outcome) = accept(machine, reply, self.registry, budget)? {
                                return Ok(outcome);
                            }
                            continue;
                        }
                        self.next_request = self
                            .next_request
                            .checked_add(1)
                            .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
                        slot::<ReservationRequest>(budget)?;
                        let request = ReservationRequest {
                            session_id: copy(&self.session_id, budget)?,
                            request_id: self.next_request,
                            snapshot: {
                                budget.charge(
                                    Resource::AllocationUnits,
                                    request.snapshot.identity().source.0.len() as u64
                                        + core::mem::size_of::<nepl3_core::source::SourceRef>()
                                            as u64,
                                )?;
                                request.snapshot.reference()
                            },
                            start: request.start,
                            limit: request.limit,
                        };
                        machine.waiting = true;
                        return Ok(Outcome::Reserve { request });
                    }
                    {
                        // A builtin emits at most one diagnostic and no event. Reserve only that
                        // collector slot before running it, so a later stop can retain the prefix.
                        budget.charge(
                            Resource::AllocationUnits,
                            core::mem::size_of::<nepl3_core::diagnostic::Diagnostic>() as u64,
                        )?;
                        machine.current.diagnostics.reserve(1);
                        let reply = builtin::read(
                            *kind,
                            request,
                            if is_text { reservation.take() } else { None },
                            self.registry,
                            sources,
                            budget,
                            admission,
                        )?;
                        prefix_builtin(reply, &mut machine.current, budget)?
                    }
                }
            };
            if let Some(outcome) = accept(machine, reply, self.registry, budget)? {
                return Ok(outcome);
            }
        }
    }
    fn finish(
        &mut self,
        machine: Machine<'a, '_>,
        result: Result<Outcome, ReaderError>,
        budget: &mut Budget,
    ) -> Result<TokenizationReply, ReaderError> {
        self.finish_recover(machine, result, budget)
            .map_err(|failure| failure.error)
    }
    // Recovery must work with an exhausted allocation budget; do not box its collector.
    #[allow(clippy::result_large_err)]
    fn finish_recover(
        &mut self,
        mut machine: Machine<'a, '_>,
        result: Result<Outcome, ReaderError>,
        budget: &mut Budget,
    ) -> Result<TokenizationReply, runtime::ReadFailure> {
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => match runtime::stop_reason(&error) {
                Some(reason) => {
                    machine.waiting = false;
                    Outcome::Stopped { reason }
                }
                None => {
                    self.reader.discard_pending();
                    return Err(current_failure(error, machine.current, budget));
                }
            },
        };
        if machine.waiting {
            let prepared = (|| -> Result<_, ReaderError> {
                let (pending, waiting) = match outcome {
                    Outcome::Reserve { request } => (
                        TokenizationWait::Reservation {
                            request: copy(&request, budget)?,
                        },
                        WaitingOutcome::Reservation(request),
                    ),
                    Outcome::Await { call, continuation } => (
                        TokenizationWait::Provider { continuation },
                        WaitingOutcome::Provider(call),
                    ),
                    _ => return Err(ReaderError::Continuation),
                };
                let current = copy(&machine.current, budget)?;
                let trivia = copy(&machine.trivia, budget)?;
                let mut c = TokenizationContinuation {
                    scope: copy(
                        self.scope
                            .as_ref()
                            .ok_or(ReaderError::Continuation)?
                            .as_ref(),
                        budget,
                    )?,
                    session_id: copy(&self.session_id, budget)?,
                    reader_schema: copy(&self.checked.plan().schema, budget)?,
                    reader_plan_digest: self.plan_digest,
                    configuration_digest: self.configuration_digest,
                    request: owned_request(&machine, budget)?,
                    mode: copy(&machine.mode.name, budget)?,
                    target: copy(&machine.target, budget)?,
                    phase: machine.phase,
                    current: copy(&machine.current, budget)?,
                    trivia: copy(&machine.trivia, budget)?,
                    expected: copy(&machine.expected, budget)?,
                    furthest: machine.furthest,
                    pending,
                    depth_base: machine.depth_base,
                    usage: budget.usage(),
                    report: Report {
                        diagnostics: copy(&machine.current.diagnostics, budget)?,
                        events: copy(&machine.current.events, budget)?,
                        trace_overflow: machine.current.trace_overflow.clone(),
                        usage: budget.usage(),
                    },
                };
                c.charge_copy(budget)?;
                slot::<TokenizationContinuation>(budget)?;
                c.usage = budget.usage();
                c.report.usage = c.usage;
                let outward = Box::new(c.clone());
                Ok((c, outward, current, trivia, waiting))
            })();
            match prepared {
                Ok((c, outward, current, trivia, waiting)) => {
                    let public = waiting.public(outward);
                    self.pending = Some(Pending {
                        continuation: c,
                        limits: machine.limits,
                    });
                    return Ok(reply(public, current, trivia, budget));
                }
                Err(error) => {
                    self.reader.discard_pending();
                    return match runtime::stop_reason(&error) {
                        Some(reason) => Ok(reply(
                            TokenizationOutcome::Stopped { reason },
                            machine.current,
                            machine.trivia,
                            budget,
                        )),
                        None => Err(current_failure(error, machine.current, budget)),
                    };
                }
            }
        }
        self.reader.discard_pending();
        let public = match outcome.public(None) {
            Ok(public) => public,
            Err(error) => return Err(current_failure(error, machine.current, budget)),
        };
        Ok(reply(public, machine.current, machine.trivia, budget))
    }
}
fn seed_failure(
    error: ReaderError,
    mut accepted: AcceptedTokenizationReport,
    budget: &Budget,
) -> runtime::ReadFailure {
    // A rejected fresh or foreign budget is not an observation of this
    // operation. Do not erase the accepted usage while returning its owner.
    if accepted.limits == budget.limits()
        && runtime::usage_at_least(budget.usage(), accepted.report.usage)
    {
        accepted.report.usage = budget.usage();
    }
    runtime::ReadFailure {
        error,
        accepted: runtime::AcceptedReport {
            report: accepted.report,
            sources: accepted.sources,
            source_maps: accepted.source_maps,
        },
    }
}
fn current_failure(
    error: ReaderError,
    current: ReaderCheckpoint,
    budget: &Budget,
) -> runtime::ReadFailure {
    runtime::ReadFailure {
        error,
        accepted: runtime::AcceptedReport {
            report: Report {
                diagnostics: current.diagnostics,
                events: current.events,
                trace_overflow: current.trace_overflow,
                usage: budget.usage(),
            },
            sources: current.sources,
            source_maps: current.source_maps,
        },
    }
}

fn owned_request(
    machine: &Machine<'_, '_>,
    budget: &mut Budget,
) -> Result<OwnedReadRequest, ReaderError> {
    let mut sources = Vec::new();
    for source in core::iter::once(machine.request.snapshot)
        .chain(machine.request.context.sources().iter().copied())
        .chain(machine.current.sources.iter())
    {
        budget.charge(
            Resource::Work,
            sources.len() as u64 + source.identity().source.0.len() as u64,
        )?;
        if !sources
            .iter()
            .any(|prior: &nepl3_core::source::SourceSnapshot| prior.identity() == source.identity())
        {
            slot::<nepl3_core::source::SourceSnapshot>(budget)?;
            sources.push(copy(source, budget)?);
        }
    }
    budget.charge(
        Resource::AllocationUnits,
        machine.request.snapshot.identity().source.0.len() as u64,
    )?;
    Ok(OwnedReadRequest {
        snapshot: machine.request.snapshot.reference(),
        sources,
        start: machine.request.start,
        limit: machine.request.limit,
        final_input: machine.request.final_input,
        context: copy(machine.request.context.raw(), budget)?,
        state: machine.request.initial_state.clone_with_budget(budget)?,
    })
}
fn empty_stop(
    cursor: u64,
    reason: StopReason,
    mut accepted: AcceptedTokenizationReport,
    budget: &Budget,
) -> TokenizationReply {
    accepted.report.usage = budget.usage();
    TokenizationReply {
        outcome: TokenizationOutcome::Stopped { reason },
        cursor,
        new_state: None,
        trivia: vec![],
        facts: vec![],
        sources: accepted.sources,
        source_maps: accepted.source_maps,
        report: accepted.report,
    }
}

fn reply(
    outcome: TokenizationOutcome,
    current: ReaderCheckpoint,
    trivia: Vec<Trivia>,
    budget: &Budget,
) -> TokenizationReply {
    TokenizationReply {
        outcome,
        cursor: current.cursor,
        new_state: Some(current.state),
        trivia,
        facts: current.facts,
        sources: current.sources,
        source_maps: current.source_maps,
        report: Report {
            diagnostics: current.diagnostics,
            events: current.events,
            trace_overflow: current.trace_overflow,
            usage: budget.usage(),
        },
    }
}
fn copy_trivia(trivia: &[Trivia], budget: &mut Budget) -> Result<Vec<Trivia>, StopReason> {
    let mut out = Vec::new();
    for v in trivia {
        slot::<Trivia>(budget)?;
        out.push(Trivia {
            span: copy(&v.span, budget)?,
            kind: v.kind,
        });
    }
    Ok(out)
}
fn accept(
    machine: &mut Machine<'_, '_>,
    reply: ReadReply,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Option<Outcome>, ReaderError> {
    match reply {
        ReadReply::Matched {
            value,
            end,
            new_state,
            view,
            facts,
            sources,
            source_maps,
            report,
        } => {
            set_report(&mut machine.current, report);
            machine.current.sources = sources;
            machine.current.source_maps = source_maps;
            if end <= machine.current.cursor {
                return nonprogress(machine, registry, budget);
            }
            let start = machine.current.cursor;
            let token = match machine.phase {
                TokenizationPhase::Skip { next: _ } => {
                    let raw = machine.request.snapshot.slice_range(start, end)?;
                    let kind = if start == 0 && raw == "\u{feff}" {
                        TriviaKind::Bom
                    } else if raw.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
                        TriviaKind::Whitespace
                    } else if raw.starts_with('#') && !raw.contains(['\r', '\n']) {
                        TriviaKind::Comment
                    } else {
                        TriviaKind::Skipped
                    };
                    slot::<Trivia>(budget)?;
                    machine.trivia.push(Trivia {
                        span: machine
                            .request
                            .snapshot
                            .span_with_budget(start, end, budget)?,
                        kind,
                    });
                    None
                }
                TokenizationPhase::Take { next: index } => {
                    let kind = match &machine.target {
                        TokenTarget::Builtin { token_kind, .. } => token_kind,
                        TokenTarget::Mode => {
                            &machine
                                .mode
                                .take
                                .get(
                                    usize::try_from(index)
                                        .map_err(|_| ReaderError::Continuation)?,
                                )
                                .ok_or(ReaderError::Context)?
                                .kind
                        }
                    };
                    slot::<Token>(budget)?;
                    kind.schema.charge(budget)?;
                    Some(Token {
                        kind: kind.clone(),
                        head: machine
                            .request
                            .snapshot
                            .span_with_budget(start, end, budget)?,
                        payload: value,
                        views: view,
                        leading_trivia: copy_trivia(&machine.trivia, budget)?,
                    })
                }
            };
            // Accept complete candidate artifacts only after token/trivia construction has succeeded.
            append(&mut machine.current.facts, facts, budget)?;
            machine.current.cursor = end;
            machine.current.state = new_state;
            match token {
                Some(token) => Ok(Some(Outcome::Token(token))),
                None => {
                    machine.phase = TokenizationPhase::Skip { next: 0 };
                    Ok(None)
                }
            }
        }
        ReadReply::NoMatch {
            expected,
            furthest,
            sources,
            source_maps,
            report,
        } => {
            machine.current.sources = sources;
            machine.current.source_maps = source_maps;
            set_report(&mut machine.current, report);
            match machine.phase {
                TokenizationPhase::Skip { next: i } => {
                    machine.phase = TokenizationPhase::Skip { next: i + 1 }
                }
                TokenizationPhase::Take { next: i } => {
                    if furthest > machine.furthest {
                        machine.expected.clear();
                        machine.furthest = furthest;
                    }
                    if furthest == machine.furthest {
                        for e in expected {
                            budget.charge(Resource::Work, machine.expected.len() as u64)?;
                            if !machine.expected.contains(&e) {
                                slot::<Expectation>(budget)?;
                                machine.expected.push(e);
                            }
                        }
                    }
                    machine.phase = TokenizationPhase::Take { next: i + 1 };
                }
            }
            Ok(None)
        }
        ReadReply::NeedMore {
            expected,
            sources,
            source_maps,
            report,
        } => {
            machine.current.sources = sources;
            machine.current.source_maps = source_maps;
            set_report(&mut machine.current, report);
            Ok(Some(Outcome::NeedMore { expected }))
        }
        ReadReply::Failed {
            diagnostic,
            recovery,
            sources,
            source_maps,
            report,
        } => {
            machine.current.sources = sources;
            machine.current.source_maps = source_maps;
            set_report(&mut machine.current, report);
            Ok(Some(Outcome::Failed {
                diagnostic,
                recovery,
            }))
        }
        ReadReply::Stopped {
            reason,
            sources,
            source_maps,
            report,
        } => {
            machine.current.sources = sources;
            machine.current.source_maps = source_maps;
            set_report(&mut machine.current, report);
            Ok(Some(Outcome::Stopped { reason }))
        }
        ReadReply::Await {
            call,
            mut continuation,
            report,
        } => {
            let closure = (|| -> Result<_, StopReason> {
                Ok((
                    copy(&continuation.current.sources, budget)?,
                    copy(&continuation.current.source_maps, budget)?,
                ))
            })();
            set_report(&mut machine.current, report);
            match closure {
                Ok((sources, maps)) => {
                    machine.current.sources = sources;
                    machine.current.source_maps = maps;
                }
                Err(reason) => {
                    machine.current.sources = core::mem::take(&mut continuation.current.sources);
                    machine.current.source_maps =
                        core::mem::take(&mut continuation.current.source_maps);
                    return Err(reason.into());
                }
            }
            machine.waiting = true;
            Ok(Some(Outcome::Await { call, continuation }))
        }
    }
}
fn append<T>(target: &mut Vec<T>, source: Vec<T>, budget: &mut Budget) -> Result<(), StopReason> {
    budget.charge(
        Resource::AllocationUnits,
        source.len() as u64 * core::mem::size_of::<T>() as u64,
    )?;
    target.extend(source);
    Ok(())
}
fn take_report(current: &mut ReaderCheckpoint, budget: &Budget) -> Report {
    Report {
        diagnostics: core::mem::take(&mut current.diagnostics),
        events: core::mem::take(&mut current.events),
        trace_overflow: current.trace_overflow.take(),
        usage: budget.usage(),
    }
}
fn set_report(current: &mut ReaderCheckpoint, report: Report) {
    current.diagnostics = report.diagnostics;
    current.events = report.events;
    current.trace_overflow = report.trace_overflow;
}
fn prefix_builtin(
    mut reply: ReadReply,
    current: &mut ReaderCheckpoint,
    budget: &mut Budget,
) -> Result<ReadReply, ReaderError> {
    let (report, sources, maps) = match &mut reply {
        ReadReply::Matched {
            report,
            sources,
            source_maps,
            ..
        }
        | ReadReply::NoMatch {
            report,
            sources,
            source_maps,
            ..
        }
        | ReadReply::NeedMore {
            report,
            sources,
            source_maps,
            ..
        }
        | ReadReply::Failed {
            report,
            sources,
            source_maps,
            ..
        }
        | ReadReply::Stopped {
            report,
            sources,
            source_maps,
            ..
        } => (report, sources, source_maps),
        ReadReply::Await { .. } => return Err(ReaderError::Context),
    };
    // New builtin artifacts exist only on Matched; incomplete/failed readers have none.
    // Charge append storage before changing the accepted checkpoint.
    if !sources.is_empty() {
        budget.charge(
            Resource::AllocationUnits,
            sources.len() as u64
                * core::mem::size_of::<nepl3_core::source::SourceSnapshot>() as u64,
        )?;
    }
    if !maps.is_empty() {
        budget.charge(
            Resource::AllocationUnits,
            maps.len() as u64 * core::mem::size_of::<nepl3_core::origin::Mapping>() as u64,
        )?;
    }
    current.sources.append(sources);
    current.source_maps.append(maps);
    *sources = core::mem::take(&mut current.sources);
    *maps = core::mem::take(&mut current.source_maps);
    // All builtins have at most one diagnostic; reserve(1) and its charge precede the call.
    current.diagnostics.append(&mut report.diagnostics);
    let mut prefix = take_report(current, budget);
    if report.trace_overflow.is_some() {
        prefix.trace_overflow = report.trace_overflow.take();
    }
    *report = prefix;
    Ok(reply)
}
fn nonprogress(
    machine: &mut Machine<'_, '_>,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Option<Outcome>, ReaderError> {
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<nepl3_core::diagnostic::Diagnostic>() as u64,
    )?;
    machine.current.diagnostics.reserve(1);
    let request = machine
        .request
        .request(machine.current.cursor, &machine.current.state);
    let reply = builtin::rejection(
        builtin::lexical::Scan::Failed("NonProgress", 0),
        BuiltinReader::Trivia,
        &request,
        registry,
        budget,
    )?;
    let reply = prefix_builtin(reply, &mut machine.current, budget)?;
    accept(machine, reply, registry, budget)
}
