use super::model::*;
use crate::{
    builtin::{self, BuiltinReader},
    model::*,
    plan::CheckedPlan,
    runtime::{
        self, ProviderReply, ReaderError, ReaderSession,
        copy::{CopyCost, copy, slot},
    },
};
use alloc::{string::String, vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason, Usage},
    diagnostic::Report,
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceReservation, SourceStore},
    view::{Token, Trivia, TriviaKind, ViewBundle},
};

#[derive(Clone, Copy)]
enum Phase {
    Skip(usize),
    Take(usize),
}
enum Waiting {
    Provider,
    Reservation { request: ReservationRequest },
}
#[derive(Clone, Copy)]
struct Input<'a> {
    snapshot: &'a nepl3_core::source::SourceSnapshot,
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
struct Machine<'a> {
    request: Input<'a>,
    mode: &'a ReaderMode,
    phase: Phase,
    current: ReaderCheckpoint,
    trivia: Vec<Trivia>,
    waiting: Option<Waiting>,
    expected: Vec<Expectation>,
    furthest: u64,
    limits: Limits,
    usage: Usage,
    depth_base: u64,
}
/// Retains private native continuation state. Input/context borrows outlive any suspension.
/// This session does not claim a portable tokenizer continuation codec.
pub struct TokenizationSession<'a> {
    session_id: String,
    modes: &'a [ReaderMode],
    checked: &'a CheckedPlan<'a>,
    registry: &'a SchemaRegistry,
    reader: ReaderSession<'a>,
    pending: Option<Machine<'a>>,
    next_request: u64,
    closed: bool,
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
        })
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.pending = None;
        self.reader.close();
    }
    pub fn read(
        &mut self,
        request: TokenizationRequest<'a, '_>,
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
        let mode = self
            .modes
            .iter()
            .find(|mode| mode.name == request.context.mode)
            .ok_or(ReaderError::Context)?;
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
                Some(reason) => Ok(empty_stop(request.start, reason, budget)),
                None => Err(error),
            };
        }
        let state = match request.state.clone_with_budget(budget) {
            Ok(state) => state,
            Err(reason) => return Ok(empty_stop(request.start, reason, budget)),
        };
        let mut machine = Machine {
            request: Input {
                snapshot: request.snapshot,
                limit: request.limit,
                final_input: request.final_input,
                context: request.context,
            },
            mode,
            phase: Phase::Skip(0),
            current: ReaderCheckpoint {
                cursor: request.start,
                state,
                view: ViewBundle {
                    elements: vec![],
                    roots: vec![],
                },
                facts: vec![],
                diagnostics: vec![],
                events: vec![],
                trace_overflow: None,
                sources: vec![],
                source_maps: vec![],
            },
            trivia: vec![],
            waiting: None,
            expected: vec![],
            furthest: request.start,
            limits: budget.limits(),
            usage: budget.usage(),
            depth_base: budget
                .current_depth()
                .checked_add(1)
                .ok_or_else(|| budget.stop(StopReason::DepthLimit))?,
        };
        let outcome =
            budget.with_depth(|budget| self.drive(&mut machine, None, sources, budget, admission));
        self.finish(machine, outcome, budget)
    }
    pub fn reserve(
        &mut self,
        echo: &ReservationRequest,
        reservation: &SourceReservation,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<TokenizationReply, ReaderError> {
        if self.closed {
            return Err(ReaderError::Closed);
        }
        let pending = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        let Some(Waiting::Reservation { request: expected }) = &pending.waiting else {
            return Err(ReaderError::Continuation);
        };
        if pending.limits != budget.limits()
            || !runtime::usage_at_least(budget.usage(), pending.usage)
        {
            return Err(ReaderError::Continuation);
        }
        if let Err(reason) = budget.poll().and_then(|_| {
            budget.charge(
                Resource::Work,
                (echo.session_id.len() + echo.snapshot.source_id.0.len()) as u64,
            )
        }) {
            let machine = self.pending.take().ok_or(ReaderError::NoPending)?;
            return Ok(reply(
                TokenizationOutcome::Stopped { reason },
                machine.current,
                machine.trivia,
                budget,
            ));
        }
        if expected != echo {
            return Err(ReaderError::Continuation);
        }
        let mut machine = self.pending.take().ok_or(ReaderError::NoPending)?;
        machine.waiting = None;
        let outcome = budget.with_depth_at_least(machine.depth_base, |budget| {
            self.drive(&mut machine, Some(reservation), sources, budget, admission)
        });
        self.finish(machine, outcome, budget)
    }
    pub fn resume(
        &mut self,
        echo: &ReaderContinuation,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<TokenizationReply, ReaderError> {
        if self.closed {
            return Err(ReaderError::Closed);
        }
        let pending = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if !matches!(pending.waiting, Some(Waiting::Provider)) {
            return Err(ReaderError::Continuation);
        }
        let reply = self
            .reader
            .resume(echo, reply, sources, budget, admission)?;
        let mut machine = self.pending.take().ok_or(ReaderError::NoPending)?;
        machine.waiting = None;
        let outcome = match accept(&mut machine, reply, self.registry, budget) {
            Ok(Some(outcome)) => Ok(outcome),
            Ok(None) => budget.with_depth_at_least(machine.depth_base, |budget| {
                self.drive(&mut machine, None, sources, budget, admission)
            }),
            Err(error) => Err(error),
        };
        self.finish(machine, outcome, budget)
    }
    fn drive(
        &mut self,
        machine: &mut Machine<'a>,
        mut reservation: Option<&SourceReservation>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<TokenizationOutcome, ReaderError> {
        loop {
            budget.poll()?;
            budget.charge(Resource::Work, 1)?;
            let reader = match machine.phase {
                Phase::Skip(index) => match machine.mode.skip.get(index) {
                    Some(rule) => &rule.reader,
                    None => {
                        machine.phase = Phase::Take(0);
                        continue;
                    }
                },
                Phase::Take(index) => {
                    if machine.current.cursor == machine.request.limit {
                        return Ok(if machine.request.final_input {
                            TokenizationOutcome::End
                        } else {
                            TokenizationOutcome::NeedMore { expected: vec![] }
                        });
                    }
                    match machine.mode.take.get(index) {
                        Some(rule) => &rule.reader,
                        None => {
                            return Ok(TokenizationOutcome::NoMatch {
                                expected: core::mem::take(&mut machine.expected),
                                furthest: machine.furthest,
                            });
                        }
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
                    self.reader.read_with_report(
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
                    )?
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
                        slot::<ReservationRequest>(budget)?;
                        budget.charge(
                            Resource::AllocationUnits,
                            (request.session_id.len() + request.snapshot.source_id.0.len()) as u64,
                        )?;
                        machine.waiting = Some(Waiting::Reservation {
                            request: request.clone(),
                        });
                        return Ok(TokenizationOutcome::Reserve { request });
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
        mut machine: Machine<'a>,
        result: Result<TokenizationOutcome, ReaderError>,
        budget: &mut Budget,
    ) -> Result<TokenizationReply, ReaderError> {
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => match runtime::stop_reason(&error) {
                Some(reason) => {
                    machine.waiting = None;
                    TokenizationOutcome::Stopped { reason }
                }
                None => {
                    self.reader.discard_pending();
                    return Err(error);
                }
            },
        };
        if machine.waiting.is_some() {
            let copied = (|| -> Result<_, StopReason> {
                Ok((
                    copy(&machine.current, budget)?,
                    copy_trivia(&machine.trivia, budget)?,
                ))
            })();
            match copied {
                Ok((current, trivia)) => {
                    machine.usage = budget.usage();
                    let reply = reply(outcome, current, trivia, budget);
                    self.pending = Some(machine);
                    return Ok(reply);
                }
                Err(reason) => {
                    self.reader.discard_pending();
                    return Ok(reply(
                        TokenizationOutcome::Stopped { reason },
                        machine.current,
                        machine.trivia,
                        budget,
                    ));
                }
            }
        }
        self.reader.discard_pending();
        Ok(reply(outcome, machine.current, machine.trivia, budget))
    }
}
fn empty_stop(cursor: u64, reason: StopReason, budget: &Budget) -> TokenizationReply {
    TokenizationReply {
        outcome: TokenizationOutcome::Stopped { reason },
        cursor,
        new_state: None,
        trivia: vec![],
        facts: vec![],
        sources: vec![],
        source_maps: vec![],
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
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
    machine: &mut Machine<'_>,
    reply: ReadReply,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Option<TokenizationOutcome>, ReaderError> {
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
                Phase::Skip(_) => {
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
                Phase::Take(index) => {
                    let rule = machine.mode.take.get(index).ok_or(ReaderError::Context)?;
                    slot::<Token>(budget)?;
                    rule.kind.schema.charge(budget)?;
                    Some(Token {
                        kind: rule.kind.clone(),
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
                Some(token) => Ok(Some(TokenizationOutcome::Token(token))),
                None => {
                    machine.phase = Phase::Skip(0);
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
                Phase::Skip(i) => machine.phase = Phase::Skip(i + 1),
                Phase::Take(i) => {
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
                    machine.phase = Phase::Take(i + 1);
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
            Ok(Some(TokenizationOutcome::NeedMore { expected }))
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
            Ok(Some(TokenizationOutcome::Failed {
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
            Ok(Some(TokenizationOutcome::Stopped { reason }))
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
            machine.waiting = Some(Waiting::Provider);
            Ok(Some(TokenizationOutcome::Await { call, continuation }))
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
    machine: &mut Machine<'_>,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Option<TokenizationOutcome>, ReaderError> {
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
