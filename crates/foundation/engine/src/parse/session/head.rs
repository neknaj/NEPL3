use super::*;
use crate::head::{
    HeadCall, HeadCallIdentity, HeadError, HeadOutcome, HeadReply, HeadRequest, ProjectedHead,
};

pub(super) struct HeadPending {
    machine: Machine,
    call: Box<HeadCall>,
    token: Option<Box<Token>>,
    usage: Usage,
    limits: Limits,
    depth_base: u64,
}
struct HeadApplication<'a> {
    call: &'a HeadCall,
    token: Option<Box<Token>>,
    outcome: HeadOutcome,
}
impl ParseSession<'_> {
    pub(super) fn service_head_host(
        &mut self,
        mut reply: ParseReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        host: &mut impl super::super::ParseHost,
    ) -> Result<super::super::ParseHostReply, ParseError> {
        loop {
            let attempt = (|| -> Result<Option<ParseReply>, ParseError> {
                match &reply.outcome {
                    ParseOutcome::AwaitHead { call, continuation } => {
                        let requirement =
                            self.profile.provider(&call.identity.operation, budget)?;
                        match budget.with_depth_at_least(call.depth_base, |budget| {
                            host.head(call, requirement, budget, admission)
                        }) {
                            Ok(Some(value)) => self
                                .resume_head(continuation, value, sources, budget, admission)
                                .map(Some),
                            Ok(None) => Ok(None),
                            Err(error) => Err(error),
                        }
                    }
                    // Nested reader operations use the existing owned validation
                    // path once a head has required an externally reviewable slot.
                    ParseOutcome::Await { call, continuation } => {
                        use nepl3_reader::model::ProviderCall;
                        let (operation, depth) = match call.as_ref() {
                            ProviderCall::Read {
                                operation,
                                depth_base,
                                ..
                            }
                            | ProviderCall::Transform {
                                operation,
                                depth_base,
                                ..
                            }
                            | ProviderCall::Dependent {
                                operation,
                                depth_base,
                                ..
                            } => (operation, *depth_base),
                        };
                        let requirement = self.profile.provider(operation, budget)?;
                        match budget.with_depth_at_least(depth, |budget| {
                            host.provider(call, requirement, budget, admission)
                        }) {
                            Ok(Some(value)) => self
                                .resume(continuation, value, sources, budget, admission)
                                .map(Some),
                            Ok(None) => Ok(None),
                            Err(error) => Err(error),
                        }
                    }
                    ParseOutcome::Reserve {
                        request,
                        continuation,
                    } => {
                        match budget
                            .with_depth_at_least(continuation.tokenizer.depth_base, |budget| {
                                host.reservation(request, budget, admission)
                            }) {
                            Ok(Some(value)) => self
                                .reserve(continuation, &value, sources, budget, admission)
                                .map(Some),
                            Ok(None) => Ok(None),
                            Err(error) => Err(error),
                        }
                    }
                    _ => Ok(None),
                }
            })();
            match attempt {
                Ok(Some(next)) => reply = next,
                Ok(None) => {
                    return Ok(super::super::ParseHostReply {
                        reply,
                        host_error: None,
                    });
                }
                Err(error) => {
                    // Cancellation and a sticky resource stop return the formal
                    // collector from the private slot, not the older public copy.
                    if let Err(reason) = budget.poll() {
                        if let Some(pending) = self.head_pending.take() {
                            reply = self.stop(pending.machine, reason, budget)?;
                        } else if let Some(pending) = self.pending.take() {
                            reply = self.stop(pending.machine, reason, budget)?;
                        }
                    }
                    return Ok(super::super::ParseHostReply {
                        reply,
                        host_error: Some(error),
                    });
                }
            }
        }
    }
    pub(super) fn make_head_call(
        &mut self,
        machine: &Machine,
        token: &Token,
        request: HeadRequest,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<HeadCall, ParseError> {
        let frame = machine
            .progress
            .frames
            .last()
            .ok_or(ParseError::Reference)?;
        let provider = self
            .profile
            .head_provider(&frame.entry.alias, &frame.entry.category, budget)?
            .ok_or(HeadError::Signature)?;
        let operation = match request {
            HeadRequest::Shape => &provider.shape,
            HeadRequest::ChildContext { .. } => &provider.child_context,
        };
        self.profile.provider(operation, budget)?;
        budget.charge(
            Resource::Work,
            (operation.schema.package.len() + operation.name.len()) as u64 + 34,
        )?;
        budget.charge(
            Resource::AllocationUnits,
            (operation.schema.package.len() + operation.name.len()) as u64,
        )?;
        let operation = operation.clone();
        let arena = machine
            .progress
            .arenas
            .get(usize::try_from(frame.arena).map_err(|_| ParseError::Reference)?)
            .ok_or(ParseError::Reference)?;
        let mut source = None;
        for candidate in &arena.sources {
            budget.charge(
                Resource::Work,
                (candidate.identity().source.0.len() + token.head.snapshot_ref().source.0.len())
                    as u64
                    + 34,
            )?;
            if candidate.identity() == token.head.snapshot_ref() {
                source = Some(candidate);
                break;
            }
        }
        let source = source.ok_or(SourceError::MissingSnapshot)?;
        let head = ProjectedHead::capture(token, source, budget, admission)?;
        let entry = copy::entry(&frame.entry, budget)?;
        let execution_digest = self.profile.execution_digest(&entry.alias, budget)?;
        budget.charge(
            Resource::Work,
            (machine.progress.request.environments.len() as u64)
                .saturating_mul(entry.alias.len() as u64 + 1),
        )?;
        let environment = machine
            .progress
            .request
            .environments
            .iter()
            .find(|v| v.alias == entry.alias)
            .ok_or(ParseError::Context)?
            .environment
            .clone();
        let depth_base = budget
            .current_depth()
            .checked_add(machine.progress.frames.len() as u64)
            .ok_or_else(|| budget.stop(StopReason::DepthLimit))?;
        budget.observe_depth(machine.progress.frames.len() as u64)?;
        build::slot::<HeadCall>(budget)?;
        let session_id = build::text(&self.session_id, budget)?;
        let call_id = self.next_head_call;
        self.next_head_call = call_id.checked_add(1).ok_or(ParseError::Reference)?;
        Ok(HeadCall {
            identity: HeadCallIdentity {
                session_id,
                call_id,
                operation,
                profile_digest: self.profile.digest(),
                execution_digest,
            },
            depth_base,
            entry,
            environment,
            head,
            request,
        })
    }
    pub(super) fn child_head(
        &mut self,
        machine: &Machine,
        _sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<Halt>, ParseError> {
        let frame = machine
            .progress
            .frames
            .last()
            .ok_or(ParseError::Reference)?;
        let Some(ShapeSelection::Dynamic {
            shape,
            child_contexts,
            ..
        }) = &frame.selection
        else {
            return Ok(None);
        };
        let index = frame.children.len();
        if child_contexts.len() > index {
            return Ok(None);
        }
        if child_contexts.len() != index
            || index >= shape.fields.len()
            || shape.fields.len() as u64 != frame.arity
        {
            return Err(HeadError::Shape.into());
        }
        let arena = machine
            .progress
            .arenas
            .get(usize::try_from(frame.arena).map_err(|_| ParseError::Reference)?)
            .ok_or(ParseError::Reference)?;
        let node = arena
            .nodes
            .get(
                usize::try_from(frame.node.ok_or(ParseError::Reference)?.0)
                    .map_err(|_| ParseError::Reference)?,
            )
            .ok_or(ParseError::Reference)?;
        let token = arena
            .tokens
            .get(
                usize::try_from(node.token.ok_or(ParseError::Reference)?.0)
                    .map_err(|_| ParseError::Reference)?,
            )
            .ok_or(ParseError::Reference)?;
        let completed = crate::head::capture_completed(
            &arena.nodes,
            &arena.tokens,
            &arena.sources,
            &frame.children,
            budget,
            admission,
        )?;
        let request = HeadRequest::ChildContext {
            shape: Box::new(shape.clone_with_budget(budget)?),
            index: index as u64,
            completed,
        };
        let call = self.make_head_call(machine, token, request, budget, admission)?;
        Ok(Some(Halt::Head(Box::new(call), None)))
    }
    pub(super) fn suspend_head(
        &mut self,
        machine: Machine,
        call: Box<HeadCall>,
        token: Option<Box<Token>>,
        depth_base: u64,
        budget: &mut Budget,
    ) -> Result<ParseReply, ParseError> {
        let prepared = (|| -> Result<_, ParseError> {
            let progress = machine.progress.clone_with_budget(budget)?;
            let inner = call.clone_with_budget(budget)?;
            let outer = Box::new(call.clone_with_budget(budget)?);
            let pending_token = token
                .as_ref()
                .map(|v| v.clone_with_budget(budget))
                .transpose()?;
            build::slot::<HeadContinuation>(budget)?;
            let session_id = build::text(&self.session_id, budget)?;
            let accepted = machine
                .accepted
                .as_ref()
                .ok_or(ParseError::Reference)?
                .checkpoint(budget)?;
            Ok((progress, inner, outer, pending_token, session_id, accepted))
        })();
        let (progress, inner, outer, pending_token, session_id, accepted) = match prepared {
            Ok(v) => v,
            Err(error) => return self.finish(machine, Err(error), depth_base, budget),
        };
        let usage = budget.usage();
        let continuation = Box::new(HeadContinuation {
            session_id,
            usage,
            depth_base,
            progress,
            call: inner,
            pending_token,
        });
        self.head_pending = Some(HeadPending {
            machine,
            call,
            token,
            usage,
            limits: budget.limits(),
            depth_base,
        });
        let (mut report, sources, source_maps) = accepted.into_parts();
        report.usage = usage;
        Ok(ParseReply {
            outcome: ParseOutcome::AwaitHead {
                call: outer,
                continuation,
            },
            report,
            sources,
            source_maps,
        })
    }
    /// Resume only the private saved call. Echo and reply identity failures leave
    /// the slot available; a valid operation stop consumes it and returns its collector.
    /// The caller keeps the primary parse input declared with identical digest
    /// and URI. Auxiliary context snapshots and generated artifacts may remain
    /// in the saved closure; they need no caller-store registration.
    pub fn resume_head(
        &mut self,
        echo: &HeadContinuation,
        reply: HeadReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        if self.closed {
            return Err(ParseError::Closed);
        }
        let pending = self.head_pending.as_ref().ok_or(ParseError::NoPending)?;
        if pending.limits != budget.limits() || !usage_at_least(budget.usage(), pending.usage) {
            return Err(ParseError::Continuation);
        }
        if let Err(reason) = budget.poll() {
            let pending = self.head_pending.take().ok_or(ParseError::NoPending)?;
            return self.stop(pending.machine, reason, budget);
        }
        let charged =
            budget.with_depth_at_least(pending.depth_base, |budget| -> Result<(), ParseError> {
                budget.charge(
                    Resource::Work,
                    (self.session_id.len()
                        + echo.session_id.len()
                        + reply.identity.session_id.len()
                        + reply.identity.operation.name.len()
                        + reply.identity.operation.schema.package.len()) as u64
                        + 100,
                )?;
                echo.progress.charge_clone(budget)?;
                echo.call.charge_clone(budget)?;
                if let Some(token) = &echo.pending_token {
                    token.charge_clone(budget)?;
                }
                Ok(())
            });
        if let Err(error) = charged {
            let pending = self.head_pending.take().ok_or(ParseError::NoPending)?;
            return self.finish(pending.machine, Err(error), pending.depth_base, budget);
        }
        let pending = self.head_pending.as_ref().ok_or(ParseError::NoPending)?;
        if echo.session_id != self.session_id
            || echo.progress != pending.machine.progress
            || echo.call != *pending.call
            || echo.pending_token.as_ref() != pending.token.as_deref()
            || echo.usage != pending.usage
            || echo.depth_base != pending.depth_base
        {
            return Err(ParseError::Continuation);
        }
        if reply.identity != pending.call.identity {
            return Err(HeadError::Identity.into());
        }
        let checked = budget.with_depth_at_least(pending.call.depth_base, |budget| {
            Self::resume_sources(
                &pending.machine.progress.request,
                sources,
                budget,
                admission,
            )?;
            self.check_head_reply(pending, &reply, budget)
        });
        if let Err(error) = checked {
            if budget.poll().is_err() {
                let pending = self.head_pending.take().ok_or(ParseError::NoPending)?;
                return self.finish(pending.machine, Err(error), pending.depth_base, budget);
            }
            return Err(error);
        }
        let restored = budget.with_depth_at_least(
            pending.call.depth_base,
            |budget| -> Result<_, ParseError> {
                let pending = self.head_pending.as_ref().ok_or(ParseError::NoPending)?;
                let mut declared = Vec::new();
                for slice in core::iter::once(sources.snapshots())
                    .chain(
                        pending
                            .machine
                            .progress
                            .arenas
                            .iter()
                            .map(|v| v.sources.as_slice()),
                    )
                    .chain(core::iter::once(
                        pending
                            .machine
                            .accepted
                            .as_ref()
                            .ok_or(ParseError::Reference)?
                            .sources(),
                    ))
                {
                    build::slot::<&[SourceSnapshot]>(budget)?;
                    declared.push(slice);
                }
                let report = pending.call.restore_report(
                    reply.report,
                    &declared,
                    self.profile.registry(),
                    budget,
                )?;
                Ok(report)
            },
        );
        let report = match restored {
            Ok(report) => report,
            Err(error) => {
                if budget.poll().is_err() {
                    let pending = self.head_pending.take().ok_or(ParseError::NoPending)?;
                    return self.finish(pending.machine, Err(error), pending.depth_base, budget);
                }
                return Err(error);
            }
        };
        let pending = self.head_pending.as_mut().ok_or(ParseError::NoPending)?;
        let appended = budget.with_depth_at_least(pending.call.depth_base, |budget| {
            pending
                .machine
                .accepted
                .as_mut()
                .ok_or(ParseError::Reference)?
                .append_report(
                    report,
                    pending.usage,
                    sources,
                    self.profile.registry(),
                    budget,
                    admission,
                )
                .map_err(ParseError::from)
        });
        if let Err(error) = appended {
            if budget.poll().is_err() {
                let pending = self.head_pending.take().ok_or(ParseError::NoPending)?;
                return self.finish(pending.machine, Err(error), pending.depth_base, budget);
            }
            return Err(error);
        }
        let mut pending = self.head_pending.take().ok_or(ParseError::NoPending)?;
        let result = budget.with_depth_at_least(pending.call.depth_base, |budget| {
            self.apply_head_reply(
                &mut pending.machine,
                HeadApplication {
                    call: &pending.call,
                    token: pending.token.take(),
                    outcome: reply.outcome,
                },
                sources,
                budget,
                admission,
            )
        });
        let result = match result {
            Ok(Some(halt)) => Ok(halt),
            Ok(None) => budget.with_depth_at_least(pending.depth_base, |budget| {
                self.drive(&mut pending.machine, sources, budget, admission)
            }),
            Err(error) => Err(error),
        };
        self.finish(pending.machine, result, pending.depth_base, budget)
    }
    fn resume_sources(
        request: &OwnedParseRequest,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), ParseError> {
        for expected in &request.sources {
            let mut found = None;
            for source in sources.snapshots() {
                budget.charge(
                    Resource::Work,
                    (expected.identity().source.0.len() as u64)
                        .saturating_add(source.identity().source.0.len() as u64)
                        .saturating_add(34),
                )?;
                if source.identity().source == expected.identity().source
                    && source.identity().revision == expected.identity().revision
                {
                    budget.charge(
                        Resource::Work,
                        (expected.uri().len() as u64)
                            .saturating_add(source.uri().len() as u64)
                            .saturating_add(33),
                    )?;
                    if source.identity().digest != expected.identity().digest
                        || source.uri() != expected.uri()
                    {
                        return Err(SourceError::IdentityConflict.into());
                    }
                    found = Some(source);
                    break;
                }
            }
            if let Some(source) = found {
                admission.admit_existing(source, budget)?;
            } else {
                budget.charge(
                    Resource::Work,
                    (expected.identity().source.0.len() + request.snapshot.source_id.0.len())
                        as u64
                        + 34,
                )?;
                if expected.identity().source == request.snapshot.source_id
                    && expected.identity().revision == request.snapshot.revision
                    && expected.identity().digest == request.snapshot.digest
                {
                    return Err(SourceError::MissingSnapshot.into());
                }
            }
        }
        Ok(())
    }
    fn check_head_reply(
        &self,
        pending: &HeadPending,
        reply: &HeadReply,
        budget: &mut Budget,
    ) -> Result<(), ParseError> {
        if reply.report.trace_overflow.is_some()
            && !matches!(reply.outcome, HeadOutcome::Stopped { .. })
        {
            return Err(HeadError::Report.into());
        }
        reply.charge_clone(budget)?;
        if !usage_at_least(reply.report.usage, pending.usage)
            || !usage_at_least(budget.usage(), reply.report.usage)
            || reply
                .report
                .usage
                .diagnostics
                .saturating_sub(pending.usage.diagnostics)
                < reply.report.diagnostics.len() as u64
            || reply
                .report
                .usage
                .events
                .saturating_sub(pending.usage.events)
                < reply.report.events.len() as u64
        {
            return Err(HeadError::Report.into());
        }
        match (&pending.call.request, &reply.outcome) {
            (HeadRequest::Shape, HeadOutcome::Shape { shape }) => {
                if let Some(shape) = shape {
                    self.profile
                        .checked(&pending.call.entry.alias, budget)?
                        .validate_head_shape(shape, budget)?;
                }
            }
            (
                HeadRequest::ChildContext { shape, index, .. },
                HeadOutcome::ChildContext { context },
            ) => {
                self.profile.validate_entry(context, budget)?;
                let field = shape
                    .fields
                    .get(usize::try_from(*index).map_err(|_| HeadError::Shape)?)
                    .ok_or(HeadError::Shape)?;
                let expected = self
                    .profile
                    .read_entry(&pending.call.entry, field.read, budget)?;
                if (expected.read.is_some() && *context != expected.entry)
                    || (!expected.foreign && context.alias != pending.call.entry.alias)
                {
                    return Err(HeadError::Context.into());
                }
            }
            (_, HeadOutcome::Failed { diagnostic }) => {
                // The Failed diagnostic is a transport re-exposure of the primary
                // report entry; it is never appended or counted a second time.
                if !reply
                    .report
                    .diagnostics
                    .iter()
                    .any(|v| v == diagnostic.as_ref())
                {
                    return Err(HeadError::Report.into());
                }
            }
            (_, HeadOutcome::Stopped { .. }) => {}
            _ => return Err(HeadError::ReplyKind.into()),
        }
        Ok(())
    }
    fn apply_head_reply(
        &self,
        machine: &mut Machine,
        response: HeadApplication<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<Halt>, ParseError> {
        let HeadApplication {
            call,
            token,
            outcome,
        } = response;
        match outcome {
            HeadOutcome::Shape { shape: Some(shape) } => {
                let provider = self
                    .profile
                    .head_provider(&call.entry.alias, &call.entry.category, budget)?
                    .ok_or(HeadError::Signature)?;
                budget.charge(
                    Resource::AllocationUnits,
                    (provider.shape.schema.package.len()
                        + provider.shape.name.len()
                        + provider.child_context.schema.package.len()
                        + provider.child_context.name.len()
                        + shape.kind.schema.package.len()) as u64,
                )?;
                let kind = shape.kind.clone();
                let arity = shape.fields.len() as u64;
                self.install_head(
                    machine,
                    *token.ok_or(ParseError::Reference)?,
                    &kind,
                    ShapeSelection::Dynamic {
                        provider: provider.clone(),
                        shape,
                        child_contexts: Vec::new(),
                    },
                    arity,
                    budget,
                )?;
            }
            HeadOutcome::Shape { shape: None } => {
                let token = *token.ok_or(ParseError::Reference)?;
                let frame = machine
                    .progress
                    .frames
                    .last()
                    .ok_or(ParseError::Reference)?;
                let package = self.profile.language(&frame.entry.alias, budget)?;
                if let Some(chosen) = select::leaf(
                    package,
                    &frame.entry,
                    &token,
                    self.profile.registry(),
                    budget,
                )? {
                    self.install_head(
                        machine,
                        token,
                        chosen.kind,
                        chosen.selection,
                        chosen.arity,
                        budget,
                    )?;
                } else {
                    self.recover(machine, Some(token), false, sources, budget, admission)?;
                }
            }
            HeadOutcome::ChildContext { context } => {
                let frame = machine
                    .progress
                    .frames
                    .last_mut()
                    .ok_or(ParseError::Reference)?;
                let Some(ShapeSelection::Dynamic { child_contexts, .. }) = &mut frame.selection
                else {
                    return Err(ParseError::Reference);
                };
                let copied = copy::entry(&context, budget)?;
                build::slot::<crate::package::EntryContext>(budget)?;
                let arena = machine
                    .progress
                    .arenas
                    .get_mut(usize::try_from(frame.arena).map_err(|_| ParseError::Reference)?)
                    .ok_or(ParseError::Reference)?;
                budget.charge(Resource::Work, arena.selections.len() as u64 + 1)?;
                let selected = arena
                    .selections
                    .iter_mut()
                    .find(|v| Some(v.node) == frame.node)
                    .ok_or(ParseError::Reference)?;
                let ShapeSelection::Dynamic {
                    child_contexts: saved,
                    ..
                } = &mut selected.shape
                else {
                    return Err(ParseError::Reference);
                };
                saved.push(copied);
                child_contexts.push(context);
            }
            HeadOutcome::Failed { .. } => {
                if let HeadRequest::ChildContext { shape, index, .. } = &call.request {
                    let field = shape
                        .fields
                        .get(usize::try_from(*index).map_err(|_| ParseError::Reference)?)
                        .ok_or(ParseError::Reference)?;
                    let context = self
                        .profile
                        .read_entry(&call.entry, field.read, budget)?
                        .entry;
                    self.apply_head_reply(
                        machine,
                        HeadApplication {
                            call,
                            token: None,
                            outcome: HeadOutcome::ChildContext { context },
                        },
                        sources,
                        budget,
                        admission,
                    )?;
                    self.child(machine, sources, budget)?;
                }
                self.recover(
                    machine,
                    token.map(|v| *v),
                    false,
                    sources,
                    budget,
                    admission,
                )?;
                if let Some(RecoveryEntry {
                    kind: RecoveryKind::Unparsed { reason, .. },
                    ..
                }) = machine
                    .progress
                    .recovery
                    .last_mut()
                    .and_then(|v| v.entries.last_mut())
                {
                    *reason = UnparsedReason::ProviderFailure;
                }
            }
            HeadOutcome::Stopped { reason } => return Err(budget.stop(reason).into()),
        }
        Ok(None)
    }
}
