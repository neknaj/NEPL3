//! Iterative transactional reader execution. Host calls are explicit suspension points.
pub(crate) mod copy;
pub(crate) mod validate;
use crate::{
    model::*,
    plan::{CheckedPlan, PlanError, ProviderKind, ReaderExpr, ReaderId},
    primitive::{self, Primitive, PrimitiveError},
    schema,
};
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use copy::{CopyCost, copy, slot};
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason, Usage},
    diagnostic::{Diagnostic, Report, Severity},
    origin::OriginError,
    schema::{SchemaError, SchemaRegistry},
    source::{Digest, SourceAdmission, SourceError, SourceSnapshot, SourceStore},
    value::{NdfValue, SchemaRef},
    view::{ViewBundle, ViewElement, ViewError, ViewField, ViewRef},
};
#[derive(Debug, Eq, PartialEq)]
pub enum ReaderError {
    Stopped(StopReason),
    Plan(PlanError),
    Schema(SchemaError),
    Source(SourceError),
    Origin(OriginError),
    View(ViewError),
    Context,
    Busy,
    Closed,
    NoPending,
    Continuation,
    ProviderContract,
}
impl ReaderError {
    /// Nested typed resource stops retain their original cause at host boundaries.
    pub fn stop_reason(&self) -> Option<StopReason> {
        stop_reason(self)
    }
}
impl From<StopReason> for ReaderError {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<PlanError> for ReaderError {
    fn from(e: PlanError) -> Self {
        Self::Plan(e)
    }
}
impl From<SchemaError> for ReaderError {
    fn from(e: SchemaError) -> Self {
        Self::Schema(e)
    }
}
impl From<SourceError> for ReaderError {
    fn from(e: SourceError) -> Self {
        Self::Source(e)
    }
}
impl From<OriginError> for ReaderError {
    fn from(e: OriginError) -> Self {
        Self::Origin(e)
    }
}
impl From<ViewError> for ReaderError {
    fn from(e: ViewError) -> Self {
        Self::View(e)
    }
}
impl From<PrimitiveError> for ReaderError {
    fn from(e: PrimitiveError) -> Self {
        match e {
            PrimitiveError::Stopped(s) => s.into(),
            PrimitiveError::Source(s) => s.into(),
        }
    }
}
/// Providers finish their own nested suspensions before returning a terminal reply here.
#[derive(Debug, Eq, PartialEq)]
pub enum ProviderReply {
    Read(Box<ReadReply>),
    Transform(Box<TransformReply>),
}
pub(crate) struct AcceptedReport {
    pub report: Report,
    pub sources: Vec<SourceSnapshot>,
    pub source_maps: Vec<nepl3_core::origin::Mapping>,
}
struct Pending {
    continuation: ReaderContinuation,
    limits: Limits,
}
/// Owns the trusted continuation slot; the echoed wire representation is never authority.
/// The host retains one operation Budget and SourceAdmission across all reads and resumes.
pub struct ReaderSession<'a> {
    session_id: String,
    closed: bool,
    checked: &'a CheckedPlan<'a>,
    registry: &'a SchemaRegistry,
    reader_schema: &'a SchemaRef,
    foundation: &'a SchemaRef,
    digest: Digest,
    pending: Option<Pending>,
    next_call: u64,
}
impl<'a> ReaderSession<'a> {
    pub fn new(
        session_id: String,
        checked: &'a CheckedPlan<'a>,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<Self, ReaderError> {
        if session_id.is_empty() || !core::ptr::eq(checked.registry(), registry) {
            return Err(ReaderError::Context);
        }
        let reader_schema = registry
            .selected(schema::PACKAGE, schema::REVISION)
            .ok_or(SchemaError::UnknownSchema)?;
        let foundation = registry
            .selected("nepl3.foundation", 1)
            .ok_or(SchemaError::UnknownSchema)?;
        Ok(Self {
            session_id,
            closed: false,
            checked,
            registry,
            reader_schema,
            foundation,
            digest: checked.plan().digest(budget)?,
            pending: None,
            next_call: 0,
        })
    }
    pub fn read(
        &mut self,
        rule: &str,
        request: ReadRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ReadReply, ReaderError> {
        self.read_with_report(
            rule,
            request,
            sources,
            budget,
            admission,
            AcceptedReport {
                report: Report::default(),
                sources: Vec::new(),
                source_maps: Vec::new(),
            },
        )
    }
    /// The mode tokenizer moves its already accepted report into the same collector.
    /// This is internal operation state, not a provider request field.
    pub(crate) fn read_with_report(
        &mut self,
        rule: &str,
        request: ReadRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        seed: AcceptedReport,
    ) -> Result<ReadReply, ReaderError> {
        let mut prefix = seed.report;
        let mut seed_sources = seed.sources;
        let mut seed_maps = seed.source_maps;
        if self.closed {
            return Err(ReaderError::Closed);
        }
        if self.pending.is_some() {
            return Err(ReaderError::Busy);
        }
        let result = (|| {
            validate::request(
                &request,
                sources,
                self.registry,
                &self.checked.plan().state_type,
                budget,
                admission,
            )?;
            let root = self.checked.plan().rule(rule)?.root;
            let state = copy(request.state, budget)?;
            let current = ReaderCheckpoint {
                cursor: request.start,
                state,
                view: ViewBundle {
                    elements: Vec::new(),
                    roots: Vec::new(),
                },
                facts: Vec::new(),
                diagnostics: core::mem::take(&mut prefix.diagnostics),
                events: core::mem::take(&mut prefix.events),
                trace_overflow: prefix.trace_overflow.take(),
                sources: core::mem::take(&mut seed_sources),
                source_maps: core::mem::take(&mut seed_maps),
            };
            let mut machine = Machine {
                session_id: &self.session_id,
                checked: self.checked,
                registry: self.registry,
                reader_schema: self.reader_schema,
                foundation: self.foundation,
                request,
                current,
                frames: Vec::new(),
                base: budget.current_depth(),
                next_call: &mut self.next_call,
            };
            if let Err(error) = machine.push(root, budget) {
                return match stop_reason(&error) {
                    Some(reason) => Ok(checkpoint_stopped(machine.current, reason, budget)),
                    None => Err(error),
                };
            }
            let control = machine.drive(None, budget)?;
            Self::finish(machine, control, self.digest, &mut self.pending, budget)
        })();
        match result {
            Err(error) if stop_reason(&error).is_some() => {
                let reason = stop_reason(&error).ok_or(ReaderError::Context)?;
                prefix.usage = budget.usage();
                Ok(ReadReply::Stopped {
                    reason,
                    sources: seed_sources,
                    source_maps: seed_maps,
                    report: prefix,
                })
            }
            result => result,
        }
    }
    /// Echo must match the saved host slot byte-for-value. A successful resume consumes it once.
    pub fn resume(
        &mut self,
        echo: &ReaderContinuation,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ReadReply, ReaderError> {
        if self.closed {
            return Err(ReaderError::Closed);
        }
        let saved = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if saved.limits != budget.limits()
            || !usage_at_least(budget.usage(), saved.continuation.usage)
            || saved.continuation.session_id != echo.session_id
        {
            return Err(ReaderError::Continuation);
        }
        if let Err(reason) = budget.poll() {
            return self.stop_pending(reason, budget);
        }
        if let Err(reason) = echo.charge(budget) {
            return self.stop_pending(reason, budget);
        }
        if &saved.continuation != echo {
            return Err(ReaderError::Continuation);
        }
        self.resume_saved(reply, sources, budget, admission)
    }
    /// Only the owning tokenizer may use this entry, after its complete external
    /// TokenizationContinuation echo (including this nested ReaderContinuation's
    /// request, plan, provider, current state and all frame checkpoints) matched
    /// its immutable private pending slot. The value passed here is moved from
    /// that private slot, never taken from the caller's echo.
    pub(crate) fn resume_from_tokenizer(
        &mut self,
        owned: &ReaderContinuation,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ReadReply, ReaderError> {
        if self.closed {
            return Err(ReaderError::Closed);
        }
        let saved = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if saved.limits != budget.limits()
            || !usage_at_least(budget.usage(), saved.continuation.usage)
        {
            return Err(ReaderError::Continuation);
        }
        if let Err(reason) = budget.poll() {
            return self.stop_pending(reason, budget);
        }
        // Only ownership metadata is compared here; the outer boundary already
        // validated the entire nested value. No allocation occurs for this check.
        if let Err(reason) = budget.charge(Resource::Work, self.session_id.len() as u64 + 49) {
            return self.stop_pending(reason, budget);
        }
        let saved = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if saved.continuation.session_id != owned.session_id
            || saved.continuation.plan_digest != owned.plan_digest
            || saved.continuation.usage != owned.usage
            || call_identity(&saved.continuation.pending) != call_identity(&owned.pending)
        {
            return Err(ReaderError::Continuation);
        }
        self.resume_saved(reply, sources, budget, admission)
    }
    /// Called only by the owning tokenizer while servicing an immediate native
    /// callback. No external continuation was received: the reader's private
    /// pending slot is still exclusively owned through this mutable borrow.
    /// Payload/context/source/report validation remains in resume_saved.
    pub(crate) fn resume_native_callback(
        &mut self,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ReadReply, ReaderError> {
        if self.closed {
            return Err(ReaderError::Closed);
        }
        let saved = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if saved.limits != budget.limits()
            || !usage_at_least(budget.usage(), saved.continuation.usage)
        {
            return Err(ReaderError::Continuation);
        }
        if let Err(reason) = budget.poll() {
            return self.stop_pending(reason, budget);
        }
        self.resume_saved(reply, sources, budget, admission)
    }
    fn resume_saved(
        &mut self,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ReadReply, ReaderError> {
        // Validate against borrowed private state. A non-stopping rejection
        // does not take the slot or modify its formal collector.
        if let Err(error) = self.check_pending_reply(&reply, sources, budget, admission) {
            return match stop_reason(&error) {
                Some(reason) => self.stop_pending(reason, budget),
                None => Err(error),
            };
        }
        let Pending {
            continuation: mut c,
            ..
        } = self.pending.take().ok_or(ReaderError::NoPending)?;
        // Preserve c.current until every fallible resume preparation step has completed.
        let prepared = (|| -> Result<_, ReaderError> {
            let snapshot = sources
                .resolve(&c.request.snapshot)
                .ok_or(SourceError::MissingSnapshot)?;
            let context = crate::context::restored(
                &c.request.context,
                self.foundation,
                &c.request.sources,
                budget,
            )?;
            // The same immutable request/store were checked before take.
            let frame = c.frames.pop().ok_or(ReaderError::Continuation)?;
            let (call_id, depth_base) = call_identity(&c.pending);
            if frame.phase != (FramePhase::Provider { call_id })
                || depth_base
                    != c.depth_base
                        .checked_add(c.frames.len() as u64 + 1)
                        .ok_or(StopReason::DepthLimit)?
            {
                return Err(ReaderError::Continuation);
            }
            Ok((snapshot, context, frame, depth_base))
        })();
        let (snapshot, context, frame, depth_base) = match prepared {
            Ok(values) => values,
            Err(error) => {
                return if let Some(reason) = stop_reason(&error) {
                    Ok(checkpoint_stopped(c.current, reason, budget))
                } else {
                    Err(error)
                };
            }
        };
        let request = ReadRequest {
            snapshot,
            start: c.request.start,
            limit: c.request.limit,
            final_input: c.request.final_input,
            context: &context,
            state: &c.request.state,
        };
        let mut machine = Machine {
            session_id: &self.session_id,
            checked: self.checked,
            registry: self.registry,
            reader_schema: self.reader_schema,
            foundation: self.foundation,
            request,
            current: c.current,
            frames: c.frames,
            base: c.depth_base,
            next_call: &mut self.next_call,
        };
        let outcome = budget.with_depth_at_least(depth_base, |budget| {
            validate::apply_provider(&mut machine, &frame, reply, budget)
        });
        let control = match outcome {
            Ok(outcome) => machine.drive(Some(outcome), budget)?,
            Err(error) => {
                if let Some(reason) = stop_reason(&error) {
                    Control::Done(Outcome::Stopped(reason))
                } else {
                    return Err(error);
                }
            }
        };
        Self::finish(machine, control, self.digest, &mut self.pending, budget)
    }
    fn check_pending_reply(
        &self,
        reply: &ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), ReaderError> {
        reply.validate_outcome()?;
        let c = &self
            .pending
            .as_ref()
            .ok_or(ReaderError::NoPending)?
            .continuation;
        let snapshot = sources
            .resolve(&c.request.snapshot)
            .ok_or(SourceError::MissingSnapshot)?;
        let context = crate::context::restored(
            &c.request.context,
            self.foundation,
            &c.request.sources,
            budget,
        )?;
        validate::request(
            &ReadRequest {
                snapshot,
                start: c.request.start,
                limit: c.request.limit,
                final_input: c.request.final_input,
                context: &context,
                state: &c.request.state,
            },
            sources,
            self.registry,
            &self.checked.plan().state_type,
            budget,
            admission,
        )?;
        let frame = c.frames.last().ok_or(ReaderError::Continuation)?;
        let (call_id, depth_base) = call_identity(&c.pending);
        if frame.phase != (FramePhase::Provider { call_id })
            || depth_base
                != c.depth_base
                    .checked_add(c.frames.len() as u64)
                    .ok_or(StopReason::DepthLimit)?
        {
            return Err(ReaderError::Continuation);
        }
        let boundary = validate::ProviderBoundary {
            plan: self.checked.plan(),
            registry: self.registry,
            snapshot,
            declared: &c.request.sources,
            current: &c.current,
        };
        budget.with_depth_at_least(depth_base, |budget| {
            validate::check_provider(
                &boundary,
                frame,
                &c.pending,
                reply.into(),
                c.usage,
                sources,
                budget,
                admission,
            )
        })
    }
    fn stop_pending(
        &mut self,
        reason: StopReason,
        budget: &Budget,
    ) -> Result<ReadReply, ReaderError> {
        let pending = self.pending.take().ok_or(ReaderError::NoPending)?;
        Ok(checkpoint_stopped(
            pending.continuation.current,
            reason,
            budget,
        ))
    }

    pub fn close(&mut self) {
        self.pending = None;
        self.closed = true;
    }
    /// Borrow the exact saved transform dispatch for reply codecs. This proof
    /// does not consume the slot and cannot be issued for an arbitrary call.
    pub fn pending_transform(
        &self,
    ) -> Result<crate::portable::transform::TransformReplyContext<'_>, ReaderError> {
        let saved = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if !matches!(saved.continuation.pending, ProviderCall::Transform { .. }) {
            return Err(ReaderError::ProviderContract);
        }
        Ok(crate::portable::transform::TransformReplyContext {
            continuation: &saved.continuation,
            plan: self.checked.plan(),
            registry: self.registry,
        })
    }
    /// A containing tokenizer terminates its operation if it cannot publish an Await envelope.
    pub(crate) fn discard_pending(&mut self) {
        self.pending = None;
    }
    fn finish(
        machine: Machine<'_, '_>,
        control: Control,
        digest: Digest,
        pending: &mut Option<Pending>,
        budget: &mut Budget,
    ) -> Result<ReadReply, ReaderError> {
        match control {
            Control::Done(outcome) => Ok(machine.reply(outcome, budget)),
            Control::Suspend(call) => {
                let call = *call;
                let preparation = (|| -> Result<_, ReaderError> {
                    let request = machine.owned_request(
                        machine.request.start,
                        machine.request.state,
                        budget,
                    )?;
                    let report = machine.report(budget)?;
                    let session_id = copy(machine.session_id, budget)?;
                    let plan_schema = copy(&machine.checked.plan().schema, budget)?;
                    let outward_call = copy(&call, budget)?;
                    // All fallible storage charges happen while committed reports remain in Machine.
                    slot::<ReaderContinuation>(budget)?;
                    slot::<ProviderCall>(budget)?;
                    session_id.charge_copy(budget)?;
                    plan_schema.charge_copy(budget)?;
                    request.charge_copy(budget)?;
                    machine.frames.charge_copy(budget)?;
                    machine.current.charge_copy(budget)?;
                    call.charge_copy(budget)?;
                    report.charge_copy(budget)?;
                    report.charge_copy(budget)?;
                    Ok((request, report, session_id, plan_schema, outward_call))
                })();
                let (request, mut report, session_id, plan_schema, outward_call) = match preparation
                {
                    Ok(values) => values,
                    Err(error) => {
                        return if let Some(reason) = stop_reason(&error) {
                            Ok(machine.reply(Outcome::Stopped(reason), budget))
                        } else {
                            Err(error)
                        };
                    }
                };
                report.usage = budget.usage();
                let continuation = ReaderContinuation {
                    session_id,
                    depth_base: machine.base,
                    plan_schema,
                    plan_digest: digest,
                    request,
                    frames: machine.frames,
                    current: machine.current,
                    pending: call,
                    usage: budget.usage(),
                    report,
                };
                let outward = continuation.clone();
                let report = continuation.report.clone();
                *pending = Some(Pending {
                    continuation,
                    limits: budget.limits(),
                });
                Ok(ReadReply::Await {
                    call: Box::new(outward_call),
                    continuation: Box::new(outward),
                    report,
                })
            }
        }
    }
}
fn checkpoint_stopped(current: ReaderCheckpoint, reason: StopReason, budget: &Budget) -> ReadReply {
    let overflow = if reason == StopReason::EventLimit {
        Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 })
    } else {
        current.trace_overflow
    };
    ReadReply::Stopped {
        reason,
        sources: current.sources,
        source_maps: current.source_maps,
        report: Report {
            diagnostics: current.diagnostics,
            events: current.events,
            trace_overflow: overflow,
            usage: budget.usage(),
        },
    }
}
pub(crate) fn stop_reason(error: &ReaderError) -> Option<StopReason> {
    match error {
        ReaderError::Stopped(reason)
        | ReaderError::Schema(SchemaError::Stopped(reason))
        | ReaderError::Source(SourceError::Stopped(reason))
        | ReaderError::Origin(OriginError::Stopped(reason))
        | ReaderError::View(ViewError::Stopped(reason)) => Some(*reason),
        _ => None,
    }
}
pub(crate) fn stopped(
    result: Result<ReadReply, ReaderError>,
    budget: &Budget,
) -> Result<ReadReply, ReaderError> {
    match result {
        Err(ReaderError::Stopped(reason))
        | Err(ReaderError::Schema(SchemaError::Stopped(reason)))
        | Err(ReaderError::Source(SourceError::Stopped(reason)))
        | Err(ReaderError::Origin(OriginError::Stopped(reason)))
        | Err(ReaderError::View(ViewError::Stopped(reason))) => Ok(ReadReply::Stopped {
            reason,
            sources: Vec::new(),
            source_maps: Vec::new(),
            report: Report {
                usage: budget.usage(),
                ..Report::default()
            },
        }),
        result => result,
    }
}
#[derive(Debug)]
enum Outcome {
    Matched(NdfValue),
    NoMatch {
        expected: Vec<Expectation>,
        furthest: u64,
    },
    NeedMore(Vec<Expectation>),
    Failed {
        diagnostic: Box<Diagnostic>,
        recovery: Option<nepl3_core::source::Span>,
    },
    Stopped(StopReason),
}
enum Control {
    Done(Outcome),
    Suspend(Box<ProviderCall>),
}
struct Machine<'a, 'r> {
    session_id: &'r String,
    checked: &'a CheckedPlan<'a>,
    registry: &'a SchemaRegistry,
    reader_schema: &'a SchemaRef,
    foundation: &'a SchemaRef,
    request: ReadRequest<'r>,
    current: ReaderCheckpoint,
    frames: Vec<ReaderFrame>,
    base: u64,
    next_call: &'r mut u64,
}
impl Machine<'_, '_> {
    fn push(&mut self, expression: ReaderId, budget: &mut Budget) -> Result<(), ReaderError> {
        let target = self
            .base
            .max(budget.current_depth())
            .checked_add(self.frames.len() as u64 + 1)
            .ok_or(StopReason::DepthLimit)?;
        budget.observe_depth(target - budget.current_depth())?;
        slot::<ReaderFrame>(budget)?;
        self.frames.push(ReaderFrame {
            expression,
            start: self.current.cursor,
            checkpoint: copy(&self.current, budget)?,
            phase: FramePhase::Enter,
        });
        Ok(())
    }
    fn child(
        &mut self,
        mut frame: ReaderFrame,
        phase: FramePhase,
        child: ReaderId,
        budget: &mut Budget,
    ) -> Result<(), ReaderError> {
        frame.phase = phase;
        slot::<ReaderFrame>(budget)?;
        self.frames.push(frame);
        self.push(child, budget)
    }
    fn drive(
        &mut self,
        outcome: Option<Outcome>,
        budget: &mut Budget,
    ) -> Result<Control, ReaderError> {
        match self.drive_inner(outcome, budget) {
            Ok(control) => Ok(control),
            Err(error) => {
                if let Some(reason) = stop_reason(&error) {
                    Ok(Control::Done(Outcome::Stopped(reason)))
                } else {
                    Err(error)
                }
            }
        }
    }
    fn drive_inner(
        &mut self,
        mut outcome: Option<Outcome>,
        budget: &mut Budget,
    ) -> Result<Control, ReaderError> {
        loop {
            budget.charge(Resource::Work, 1)?;
            let Some(mut frame) = self.frames.pop() else {
                return Ok(Control::Done(outcome.ok_or(ReaderError::Continuation)?));
            };
            budget.observe_depth(
                self.base
                    .checked_add(self.frames.len() as u64 + 1)
                    .ok_or(StopReason::DepthLimit)?
                    .saturating_sub(budget.current_depth()),
            )?;
            let expr = self.checked.plan().expression(frame.expression)?;
            if let Some(result) = outcome.take() {
                match result {
                    Outcome::Stopped(reason) => return Ok(Control::Done(Outcome::Stopped(reason))),
                    Outcome::Failed {
                        diagnostic,
                        recovery,
                    } => {
                        outcome = Some(Outcome::Failed {
                            diagnostic,
                            recovery,
                        })
                    }
                    Outcome::NeedMore(expected) => {
                        self.current = frame.checkpoint;
                        outcome = Some(Outcome::NeedMore(expected));
                    }
                    Outcome::NoMatch {
                        mut expected,
                        furthest,
                    } => {
                        if !matches!(&frame.phase, FramePhase::Repeat { .. }) {
                            self.current = copy(&frame.checkpoint, budget)?;
                        }
                        match (expr, frame.phase) {
                            (
                                ReaderExpr::Choice(parts),
                                FramePhase::Choice {
                                    next,
                                    furthest: prior,
                                    expected: mut previous,
                                },
                            ) => {
                                let farthest = prior.max(furthest);
                                if furthest > prior {
                                    previous.clear();
                                }
                                if furthest >= prior {
                                    merge_expected(&mut previous, &mut expected, budget)?;
                                }
                                if let Some(child) = parts.get(next as usize) {
                                    frame.phase = FramePhase::Enter;
                                    self.child(
                                        frame,
                                        FramePhase::Choice {
                                            next: next + 1,
                                            furthest: farthest,
                                            expected: previous,
                                        },
                                        *child,
                                        budget,
                                    )?;
                                } else {
                                    outcome = Some(Outcome::NoMatch {
                                        expected: previous,
                                        furthest: farthest,
                                    });
                                }
                            }
                            (
                                ReaderExpr::Many(_)
                                | ReaderExpr::Some(_)
                                | ReaderExpr::Repeat { .. },
                                FramePhase::Repeat { count, values, .. },
                            ) => {
                                let min = match expr {
                                    ReaderExpr::Some(_) => 1,
                                    ReaderExpr::Repeat { min, .. } => *min,
                                    _ => 0,
                                };
                                if count >= min {
                                    outcome = Some(Outcome::Matched(NdfValue::List(values)));
                                } else {
                                    self.current = frame.checkpoint;
                                    outcome = Some(Outcome::NoMatch { expected, furthest });
                                }
                            }
                            (ReaderExpr::Optional(_), _) => {
                                outcome = Some(Outcome::Matched(NdfValue::None))
                            }
                            (ReaderExpr::Not(_), _) => {
                                outcome = Some(Outcome::Matched(NdfValue::Unit))
                            }
                            (ReaderExpr::Commit(_), _) => {
                                outcome = Some(self.failure(
                                    "ExpectedInput",
                                    &expected,
                                    furthest,
                                    budget,
                                )?)
                            }
                            _ => outcome = Some(Outcome::NoMatch { expected, furthest }),
                        }
                    }
                    Outcome::Matched(value) => match (expr, frame.phase) {
                        (ReaderExpr::Seq(parts), FramePhase::Seq { next, mut values }) => {
                            slot::<NdfValue>(budget)?;
                            values.push(value);
                            if let Some(child) = parts.get(next as usize) {
                                frame.phase = FramePhase::Enter;
                                self.child(
                                    frame,
                                    FramePhase::Seq {
                                        next: next + 1,
                                        values,
                                    },
                                    *child,
                                    budget,
                                )?;
                            } else {
                                outcome = Some(Outcome::Matched(NdfValue::List(values)));
                            }
                        }
                        (ReaderExpr::Choice(_), _) => outcome = Some(Outcome::Matched(value)),
                        (
                            ReaderExpr::Many(body)
                            | ReaderExpr::Some(body)
                            | ReaderExpr::Repeat { body, .. },
                            FramePhase::Repeat {
                                count,
                                iteration_start,
                                mut values,
                            },
                        ) => {
                            if self.current.cursor == iteration_start {
                                outcome = Some(self.failure(
                                    "NonProgress",
                                    &[],
                                    self.current.cursor,
                                    budget,
                                )?);
                                continue;
                            }
                            let count = count.checked_add(1).ok_or(StopReason::WorkLimit)?;
                            slot::<NdfValue>(budget)?;
                            values.push(value);
                            let max = match expr {
                                ReaderExpr::Repeat { max, .. } => *max,
                                _ => u64::MAX,
                            };
                            if count >= max {
                                outcome = Some(Outcome::Matched(NdfValue::List(values)));
                            } else {
                                // Keep the original checkpoint; each child owns its iteration rollback point.
                                let iteration_start = self.current.cursor;
                                frame.phase = FramePhase::Enter;
                                self.child(
                                    frame,
                                    FramePhase::Repeat {
                                        count,
                                        iteration_start,
                                        values,
                                    },
                                    *body,
                                    budget,
                                )?;
                            }
                        }
                        (ReaderExpr::Optional(_), _) => {
                            slot::<NdfValue>(budget)?;
                            outcome = Some(Outcome::Matched(NdfValue::Some(Box::new(value))));
                        }
                        (ReaderExpr::Look(_), _) => {
                            self.current = frame.checkpoint;
                            outcome = Some(Outcome::Matched(NdfValue::Unit));
                        }
                        (ReaderExpr::Not(_), _) => {
                            self.current = frame.checkpoint;
                            outcome = Some(Outcome::NoMatch {
                                expected: Vec::new(),
                                furthest: frame.start,
                            });
                        }
                        (ReaderExpr::Discard(_), _) => {
                            outcome = Some(Outcome::Matched(NdfValue::Unit))
                        }
                        (ReaderExpr::Capture { name, .. }, _) => {
                            slot::<ReaderFact>(budget)?;
                            let span = self.request.snapshot.span_with_budget(
                                frame.start,
                                self.current.cursor,
                                budget,
                            )?;
                            self.current.facts.push(ReaderFact::Capture {
                                name: copy(name, budget)?,
                                span,
                            });
                            outcome = Some(Outcome::Matched(value));
                        }
                        (ReaderExpr::Region { class, .. }, _) => {
                            slot::<ReaderFact>(budget)?;
                            let span = self.request.snapshot.span_with_budget(
                                frame.start,
                                self.current.cursor,
                                budget,
                            )?;
                            self.current.facts.push(ReaderFact::Presentation {
                                class: copy(class, budget)?,
                                span,
                            });
                            outcome = Some(Outcome::Matched(value));
                        }
                        (ReaderExpr::Node { kind, .. }, _) => {
                            slot::<ViewElement>(budget)?;
                            budget.charge(Resource::Nodes, 1)?;
                            let roots = self
                                .current
                                .view
                                .roots
                                .split_off(frame.checkpoint.view.roots.len());
                            let id = ViewRef(self.current.view.elements.len() as u64);
                            slot::<ViewRef>(budget)?;
                            slot::<ViewField>(budget)?;
                            budget.charge(Resource::AllocationUnits, 8)?;
                            self.current.view.elements.push(ViewElement {
                                kind: copy(kind, budget)?,
                                span: self.request.snapshot.span_with_budget(
                                    frame.start,
                                    self.current.cursor,
                                    budget,
                                )?,
                                fields: vec![ViewField {
                                    name: "children".into(),
                                    children: roots,
                                }],
                                roles: Vec::new(),
                                relations: Vec::new(),
                            });
                            self.current.view.roots.push(id);
                            outcome = Some(Outcome::Matched(value));
                        }
                        (
                            ReaderExpr::Decode { provider, .. } | ReaderExpr::Map { provider, .. },
                            _,
                        ) => {
                            let view = fragment(
                                &self.current.view,
                                frame.checkpoint.view.elements.len(),
                                frame.checkpoint.view.roots.len(),
                                budget,
                            )?;
                            let request = TransformRequest {
                                value,
                                span: self.request.snapshot.span_with_budget(
                                    frame.start,
                                    self.current.cursor,
                                    budget,
                                )?,
                                view,
                                context: copy(self.request.context.raw(), budget)?,
                            };
                            let call =
                                self.make_call(provider, CallRequest::Transform(request), budget)?;
                            frame.phase = FramePhase::Provider {
                                call_id: call_identity(&call).0,
                            };
                            slot::<ReaderFrame>(budget)?;
                            self.frames.push(frame);
                            slot::<ProviderCall>(budget)?;
                            return Ok(Control::Suspend(Box::new(call)));
                        }
                        (ReaderExpr::Then { provider, .. }, _) => {
                            let request = DependentRequest {
                                first: value,
                                end: self.current.cursor,
                                request: self.owned_request(
                                    self.current.cursor,
                                    &self.current.state,
                                    budget,
                                )?,
                            };
                            let call =
                                self.make_call(provider, CallRequest::Dependent(request), budget)?;
                            frame.phase = FramePhase::Provider {
                                call_id: call_identity(&call).0,
                            };
                            slot::<ReaderFrame>(budget)?;
                            self.frames.push(frame);
                            slot::<ProviderCall>(budget)?;
                            return Ok(Control::Suspend(Box::new(call)));
                        }
                        (ReaderExpr::Ref(_) | ReaderExpr::Commit(_), _) => {
                            outcome = Some(Outcome::Matched(value))
                        }
                        _ => return Err(ReaderError::Continuation),
                    },
                }
                continue;
            }
            if frame.phase != FramePhase::Enter {
                return Err(ReaderError::Continuation);
            }
            if let Some(result) = primitive::recognize(
                expr,
                self.request.snapshot,
                self.current.cursor,
                self.request.limit,
                self.request.final_input,
                budget,
            )? {
                outcome = Some(match result {
                    Primitive::Matched { value, end } => {
                        self.current.cursor = end;
                        Outcome::Matched(value)
                    }
                    Primitive::NoMatch { expected, furthest } => {
                        self.current = frame.checkpoint;
                        Outcome::NoMatch { expected, furthest }
                    }
                    Primitive::NeedMore { expected } => {
                        self.current = frame.checkpoint;
                        Outcome::NeedMore(expected)
                    }
                });
                continue;
            }
            match expr{
                ReaderExpr::Seq(parts)=>if let Some(child)=parts.first(){self.child(frame,FramePhase::Seq{next:1,values:Vec::new()},*child,budget)?;}else{outcome=Some(Outcome::Matched(NdfValue::List(Vec::new())));},
                ReaderExpr::Choice(parts)=>if let Some(child)=parts.first(){let start=frame.start;self.child(frame,FramePhase::Choice{next:1,furthest:start,expected:Vec::new()},*child,budget)?;}else{outcome=Some(Outcome::NoMatch{expected:Vec::new(),furthest:frame.start});},
                ReaderExpr::Repeat{max:0,..}=>outcome=Some(Outcome::Matched(NdfValue::List(Vec::new()))),
                ReaderExpr::Many(body)|ReaderExpr::Some(body)|ReaderExpr::Repeat{body,..}=>{let iteration_start=self.current.cursor;self.child(frame,FramePhase::Repeat{count:0,iteration_start,values:Vec::new()},*body,budget)?},
                ReaderExpr::Optional(body)|ReaderExpr::Look(body)|ReaderExpr::Not(body)|ReaderExpr::Commit(body)|ReaderExpr::Capture{body,..}|ReaderExpr::Region{body,..}|ReaderExpr::Node{body,..}|ReaderExpr::Discard(body)|ReaderExpr::Decode{body,..}|ReaderExpr::Map{body,..}=>self.child(frame,FramePhase::AwaitChild,*body,budget)?,
                ReaderExpr::Then{first,..}=>self.child(frame,FramePhase::AwaitChild,*first,budget)?,
                ReaderExpr::Ref(name)=>{
                    if self.frames.iter().any(|parent|parent.start==frame.start&&matches!(self.checked.plan().expression(parent.expression),Ok(ReaderExpr::Ref(other)) if other==name)){
                        outcome=Some(self.failure("NonProgressRecursion",&[],frame.start,budget)?);
                    }else{self.child(frame,FramePhase::AwaitChild,self.checked.plan().rule(name)?.root,budget)?;}
                },
                ReaderExpr::Call(provider)=>{
                    let request=self.owned_request(self.current.cursor,&self.current.state,budget)?;let call=self.make_call(provider,CallRequest::Read(request),budget)?;frame.phase=FramePhase::Provider{call_id:call_identity(&call).0};slot::<ReaderFrame>(budget)?;self.frames.push(frame);slot::<ProviderCall>(budget)?;return Ok(Control::Suspend(Box::new(call)));
                },
                _=>return Err(ReaderError::Continuation),
            }
        }
    }
    fn owned_request(
        &self,
        start: u64,
        state: &NdfValue,
        budget: &mut Budget,
    ) -> Result<OwnedReadRequest, ReaderError> {
        slot::<OwnedReadRequest>(budget)?;
        slot::<SourceSnapshot>(budget)?;
        let mut sources: Vec<SourceSnapshot> = Vec::new();
        for source in core::iter::once(self.request.snapshot)
            .chain(self.request.context.sources().iter().copied())
            .chain(&self.current.sources)
        {
            if !sources.iter().any(|s| s.identity() == source.identity()) {
                sources.push(copy(source, budget)?);
            }
        }
        budget.charge(
            Resource::AllocationUnits,
            self.request.snapshot.identity().source.0.len() as u64,
        )?;
        Ok(OwnedReadRequest {
            snapshot: self.request.snapshot.reference(),
            sources,
            start,
            limit: self.request.limit,
            final_input: self.request.final_input,
            context: copy(self.request.context.raw(), budget)?,
            state: copy(state, budget)?,
        })
    }
    fn make_call(
        &mut self,
        operation: &nepl3_core::value::OperationRef,
        request: CallRequest,
        budget: &mut Budget,
    ) -> Result<ProviderCall, ReaderError> {
        let call_id = *self.next_call;
        *self.next_call = self.next_call.checked_add(1).ok_or(StopReason::WorkLimit)?;
        let depth_base = self
            .base
            .checked_add(self.frames.len() as u64 + 1)
            .ok_or(StopReason::DepthLimit)?;
        budget.observe_depth(depth_base.saturating_sub(budget.current_depth()))?;
        let operation = copy(operation, budget)?;
        slot::<ProviderCall>(budget)?;
        Ok(match request {
            CallRequest::Read(request) => ProviderCall::Read {
                session_id: copy(self.session_id, budget)?,
                call_id,
                depth_base,
                operation,
                request,
            },
            CallRequest::Transform(request) => ProviderCall::Transform {
                session_id: copy(self.session_id, budget)?,
                call_id,
                depth_base,
                operation,
                request,
            },
            CallRequest::Dependent(request) => ProviderCall::Dependent {
                session_id: copy(self.session_id, budget)?,
                call_id,
                depth_base,
                operation,
                request,
            },
        })
    }
    fn failure(
        &mut self,
        code: &str,
        expected: &[Expectation],
        offset: u64,
        budget: &mut Budget,
    ) -> Result<Outcome, ReaderError> {
        let arguments = schema::arguments(
            self.reader_schema,
            self.foundation,
            expected,
            offset,
            budget,
        )?;
        self.registry.validate_typed(&arguments, budget)?;
        let primary = Some(
            self.request
                .snapshot
                .span_with_budget(offset, offset, budget)?,
        );
        budget.charge(Resource::AllocationUnits, (code.len() + 6) as u64)?;
        let diagnostic = Diagnostic {
            schema: copy(self.reader_schema, budget)?,
            code: code.into(),
            severity: Severity::Error,
            stage: "reader".into(),
            arguments,
            primary,
            related: Vec::new(),
            fixes: Vec::new(),
        };
        budget.charge(Resource::Diagnostics, 1)?;
        let duplicate = copy(&diagnostic, budget)?;
        slot::<Diagnostic>(budget)?;
        self.current.diagnostics.push(diagnostic);
        slot::<Diagnostic>(budget)?;
        Ok(Outcome::Failed {
            diagnostic: Box::new(duplicate),
            recovery: None,
        })
    }
    fn report(&self, budget: &mut Budget) -> Result<Report, ReaderError> {
        Ok(Report {
            diagnostics: copy(&self.current.diagnostics, budget)?,
            events: copy(&self.current.events, budget)?,
            trace_overflow: copy(&self.current.trace_overflow, budget)?,
            usage: budget.usage(),
        })
    }
    fn reply(self, outcome: Outcome, budget: &Budget) -> ReadReply {
        let report = Report {
            diagnostics: self.current.diagnostics,
            events: self.current.events,
            trace_overflow: self.current.trace_overflow,
            usage: budget.usage(),
        };
        match outcome {
            Outcome::Matched(value) => ReadReply::Matched {
                value,
                end: self.current.cursor,
                new_state: self.current.state,
                view: self.current.view,
                facts: self.current.facts,
                sources: self.current.sources,
                source_maps: self.current.source_maps,
                report,
            },
            Outcome::NoMatch { expected, furthest } => ReadReply::NoMatch {
                expected,
                furthest,
                sources: self.current.sources,
                source_maps: self.current.source_maps,
                report,
            },
            Outcome::NeedMore(expected) => ReadReply::NeedMore {
                expected,
                sources: self.current.sources,
                source_maps: self.current.source_maps,
                report,
            },
            Outcome::Failed {
                diagnostic,
                recovery,
            } => ReadReply::Failed {
                diagnostic: *diagnostic,
                recovery,
                sources: self.current.sources,
                source_maps: self.current.source_maps,
                report,
            },
            Outcome::Stopped(reason) => ReadReply::Stopped {
                reason,
                sources: self.current.sources,
                source_maps: self.current.source_maps,
                report,
            },
        }
    }
}
enum CallRequest {
    Read(OwnedReadRequest),
    Transform(TransformRequest),
    Dependent(DependentRequest),
}
fn call_identity(call: &ProviderCall) -> (u64, u64) {
    match call {
        ProviderCall::Read {
            call_id,
            depth_base,
            ..
        }
        | ProviderCall::Transform {
            call_id,
            depth_base,
            ..
        }
        | ProviderCall::Dependent {
            call_id,
            depth_base,
            ..
        } => (*call_id, *depth_base),
    }
}
pub(crate) fn usage_at_least(a: Usage, b: Usage) -> bool {
    a.source_bytes >= b.source_bytes
        && a.work >= b.work
        && a.depth >= b.depth
        && a.nodes >= b.nodes
        && a.allocation_units >= b.allocation_units
        && a.output_bytes >= b.output_bytes
        && a.diagnostics >= b.diagnostics
        && a.events >= b.events
}
fn merge_expected(
    target: &mut Vec<Expectation>,
    incoming: &mut Vec<Expectation>,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    for e in incoming.drain(..) {
        budget.charge(Resource::Work, target.len() as u64 + 1)?;
        if !target.contains(&e) {
            slot::<Expectation>(budget)?;
            target.push(e);
        }
    }
    Ok(())
}
fn fragment(
    view: &ViewBundle,
    base: usize,
    root_base: usize,
    budget: &mut Budget,
) -> Result<ViewBundle, ReaderError> {
    let mut result = ViewBundle {
        elements: Vec::new(),
        roots: Vec::new(),
    };
    for element in view.elements.get(base..).ok_or(ReaderError::Continuation)? {
        result.elements.push(copy(element, budget)?);
    }
    for root in view
        .roots
        .get(root_base..)
        .ok_or(ReaderError::Continuation)?
    {
        slot::<ViewRef>(budget)?;
        result.roots.push(*root);
    }
    reindex(&mut result, base as u64, false)?;
    Ok(result)
}
fn reindex(view: &mut ViewBundle, offset: u64, add: bool) -> Result<(), ReaderError> {
    let adjust = |id: &mut ViewRef| -> Result<(), ReaderError> {
        id.0 = if add {
            id.0.checked_add(offset)
        } else {
            id.0.checked_sub(offset)
        }
        .ok_or(ReaderError::ProviderContract)?;
        Ok(())
    };
    for root in &mut view.roots {
        adjust(root)?;
    }
    for element in &mut view.elements {
        for field in &mut element.fields {
            for child in &mut field.children {
                adjust(child)?;
            }
        }
        for relation in &mut element.relations {
            adjust(&mut relation.target)?;
        }
    }
    Ok(())
}
