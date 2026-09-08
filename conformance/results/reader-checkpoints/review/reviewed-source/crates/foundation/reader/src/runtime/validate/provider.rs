//! Borrowed validation precedes consuming the session's private pending slot.
use super::*;
#[derive(Clone, Copy)]
pub(crate) enum ProviderReplyRef<'a> {
    Read(&'a ReadReply),
    Transform(&'a TransformReply),
}
impl<'a> From<&'a ProviderReply> for ProviderReplyRef<'a> {
    fn from(value: &'a ProviderReply) -> Self {
        match value {
            ProviderReply::Read(v) => Self::Read(v),
            ProviderReply::Transform(v) => Self::Transform(v),
        }
    }
}
pub(crate) struct ProviderBoundary<'a> {
    pub plan: &'a crate::plan::ReaderPlan,
    pub registry: &'a SchemaRegistry,
    pub snapshot: &'a SourceSnapshot,
    pub declared: &'a [SourceSnapshot],
    pub current: &'a ReaderCheckpoint,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn check_provider(
    machine: &ProviderBoundary<'_>,
    view_offset: usize,
    call: &ProviderCall,
    reply: ProviderReplyRef<'_>,
    saved: Usage,
    original_sources: &SourceStore,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), ReaderError> {
    let (stopped, returned) = match reply {
        ProviderReplyRef::Read(v) => match v {
            ReadReply::Stopped { report, .. } => (true, report),
            ReadReply::Matched { report, .. }
            | ReadReply::NoMatch { report, .. }
            | ReadReply::NeedMore { report, .. }
            | ReadReply::Failed { report, .. }
            | ReadReply::Await { report, .. } => (false, report),
        },
        ProviderReplyRef::Transform(v) => (
            matches!(v.outcome, TransformOutcome::Stopped { .. }),
            &v.report,
        ),
    };
    if !stopped && returned.trace_overflow.is_some() {
        return Err(ReaderError::ProviderContract);
    }
    let (operation, kind) = match call {
        ProviderCall::Read { operation, .. } => (operation, ProviderKind::Read),
        ProviderCall::Transform { operation, .. } => (operation, ProviderKind::Transform),
        ProviderCall::Dependent { operation, .. } => (operation, ProviderKind::Dependent),
    };
    let signature = machine.plan.provider(operation, kind)?;
    let empty = ViewBundle {
        elements: Vec::new(),
        roots: Vec::new(),
    };
    let (added, maps, returned, view, facts, start, end) = match (call, reply) {
        (ProviderCall::Transform { request, .. }, ProviderReplyRef::Transform(reply)) => {
            let (view, facts) = match &reply.outcome {
                TransformOutcome::Complete { value, view, facts } => {
                    machine
                        .registry
                        .validate(&signature.value_output, value, budget)?;
                    check_view_offset(view, view_offset)?;
                    (view, facts.as_slice())
                }
                _ => (&empty, &[][..]),
            };
            (
                &reply.sources,
                &reply.source_maps,
                &reply.report,
                view,
                facts,
                request.span.start(),
                request.span.end(),
            )
        }
        (ProviderCall::Read { request, .. }, ProviderReplyRef::Read(reply))
        | (
            ProviderCall::Dependent {
                request: DependentRequest { request, .. },
                ..
            },
            ProviderReplyRef::Read(reply),
        ) => match reply {
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
                if *end < request.start || *end > request.limit {
                    return Err(ReaderError::ProviderContract);
                }
                machine.snapshot.check_range(request.start, *end)?;
                machine
                    .registry
                    .validate(&signature.value_output, value, budget)?;
                machine
                    .registry
                    .validate(&signature.state_type, new_state, budget)?;
                check_view_offset(view, machine.current.view.elements.len())?;
                (
                    sources,
                    source_maps,
                    report,
                    view,
                    facts.as_slice(),
                    request.start,
                    *end,
                )
            }
            ReadReply::Failed {
                sources,
                source_maps,
                report,
                ..
            }
            | ReadReply::Stopped {
                sources,
                source_maps,
                report,
                ..
            } => (
                sources,
                source_maps,
                report,
                &empty,
                &[][..],
                request.start,
                request.limit,
            ),
            ReadReply::NoMatch {
                expected,
                furthest,
                sources,
                source_maps,
                report: returned,
            } => {
                if *furthest < request.start
                    || *furthest > request.limit
                    || !sources.is_empty()
                    || !source_maps.is_empty()
                    || !returned.diagnostics.is_empty()
                    || !returned.events.is_empty()
                    || returned.trace_overflow.is_some()
                {
                    return Err(ReaderError::ProviderContract);
                }
                machine.snapshot.check_range(*furthest, *furthest)?;
                report(returned, saved, machine.registry, original_sources, budget)?;
                expectations(expected, machine.registry, budget)?;
                return Ok(());
            }
            ReadReply::NeedMore {
                expected,
                sources,
                source_maps,
                report: returned,
            } => {
                if request.final_input
                    || !sources.is_empty()
                    || !source_maps.is_empty()
                    || !returned.diagnostics.is_empty()
                    || !returned.events.is_empty()
                    || returned.trace_overflow.is_some()
                {
                    return Err(ReaderError::ProviderContract);
                }
                report(returned, saved, machine.registry, original_sources, budget)?;
                expectations(expected, machine.registry, budget)?;
                return Ok(());
            }
            ReadReply::Await { .. } => return Err(ReaderError::ProviderContract),
        },
        _ => return Err(ReaderError::ProviderContract),
    };
    if returned.trace_overflow.is_some() && machine.current.trace_overflow.is_some() {
        return Err(ReaderError::ProviderContract);
    }
    let failed = match reply {
        ProviderReplyRef::Read(reply) => matches!(reply, ReadReply::Failed { .. }),
        ProviderReplyRef::Transform(reply) => {
            matches!(reply.outcome, TransformOutcome::Failed { .. })
        }
    };
    if !failed
        && added.is_empty()
        && maps.is_empty()
        && view.elements.is_empty()
        && view.roots.is_empty()
        && facts.is_empty()
        && returned.diagnostics.is_empty()
        && returned.events.is_empty()
    {
        // No new declaration and no returned source-position use. The private
        // request/checkpoint prefix is already validated. Do not rebuild its
        // resolver merely to validate an empty report's usage/overflow fields.
        return report(returned, saved, machine.registry, original_sources, budget);
    }
    let sources = combined(machine, added, original_sources, budget, admission)?;
    artifacts(
        machine, added, maps, view, facts, start, end, &sources, budget, admission,
    )?;
    report(returned, saved, machine.registry, &sources, budget)?;
    let failed = match reply {
        ProviderReplyRef::Read(reply) => match reply {
            ReadReply::Failed {
                diagnostic,
                recovery,
                ..
            } => Some((diagnostic, recovery)),
            _ => None,
        },
        ProviderReplyRef::Transform(reply) => match &reply.outcome {
            TransformOutcome::Failed {
                diagnostic,
                recovery,
            } => Some((diagnostic.as_ref(), recovery)),
            _ => None,
        },
    };
    if let Some((diagnostic, recovery)) = failed {
        // Account variable-size equality before comparing the primary with the
        // returned diagnostic list; the primary itself is not counted twice.
        for item in &returned.diagnostics {
            diagnostic
                .validate(&sources, &[], machine.registry, budget)
                .map_err(report_error)?;
            item.validate(&sources, &[], machine.registry, budget)
                .map_err(report_error)?;
        }
        if returned
            .diagnostics
            .iter()
            .filter(|d| *d == diagnostic)
            .count()
            != 1
        {
            return Err(ReaderError::ProviderContract);
        }
        if let Some(span) = recovery {
            validate_span(span, &sources)?;
        }
    }
    Ok(())
}
fn check_view_offset(view: &ViewBundle, offset: usize) -> Result<(), ReaderError> {
    (offset as u64)
        .checked_add(view.elements.len() as u64)
        .ok_or(ReaderError::ProviderContract)?;
    Ok(())
}

/// Only called for the exact owned reply checked above. All contract checks
/// precede this phase; allocation stops still preserve accepted artifacts.
pub(in crate::runtime) fn apply_provider(
    machine: &mut Machine<'_, '_>,
    frame: super::super::checkpoint::Frame,
    reply: ProviderReply,
    budget: &mut Budget,
) -> Result<Outcome, ReaderError> {
    match reply {
        ProviderReply::Transform(reply) => {
            let TransformReply {
                outcome,
                sources,
                source_maps,
                report,
            } = *reply;
            match outcome {
                TransformOutcome::Complete {
                    value,
                    mut view,
                    facts,
                } => {
                    machine
                        .current
                        .view
                        .elements
                        .truncate(frame.checkpoint.elements());
                    machine
                        .current
                        .view
                        .roots
                        .truncate(frame.checkpoint.roots());
                    append_view(&mut machine.current.view, &mut view, budget)?;
                    append_artifacts(machine, facts, sources, source_maps, report, budget)?;
                    Ok(Outcome::Matched(value))
                }
                TransformOutcome::Failed {
                    diagnostic,
                    recovery,
                } => {
                    append_artifacts(machine, Vec::new(), sources, source_maps, report, budget)?;
                    Ok(Outcome::Failed {
                        diagnostic,
                        recovery,
                    })
                }
                TransformOutcome::Stopped { reason } => {
                    append_artifacts(machine, Vec::new(), sources, source_maps, report, budget)?;
                    Ok(Outcome::Stopped(reason))
                }
            }
        }
        ProviderReply::Read(reply) => match *reply {
            ReadReply::Matched {
                value,
                end,
                new_state,
                mut view,
                facts,
                sources,
                source_maps,
                report,
            } => {
                append_view(&mut machine.current.view, &mut view, budget)?;
                machine.current.cursor = end;
                machine.current.state = new_state;
                append_artifacts(machine, facts, sources, source_maps, report, budget)?;
                Ok(Outcome::Matched(value))
            }
            ReadReply::NoMatch {
                expected, furthest, ..
            } => {
                frame.checkpoint.restore(&mut machine.current)?;
                Ok(Outcome::NoMatch { expected, furthest })
            }
            ReadReply::NeedMore { expected, .. } => {
                frame.checkpoint.restore(&mut machine.current)?;
                Ok(Outcome::NeedMore(expected))
            }
            ReadReply::Failed {
                diagnostic,
                recovery,
                sources,
                source_maps,
                report,
            } => {
                append_artifacts(machine, Vec::new(), sources, source_maps, report, budget)?;
                slot::<Diagnostic>(budget)?;
                Ok(Outcome::Failed {
                    diagnostic: Box::new(diagnostic),
                    recovery,
                })
            }
            ReadReply::Stopped {
                reason,
                sources,
                source_maps,
                report,
            } => {
                append_artifacts(machine, Vec::new(), sources, source_maps, report, budget)?;
                Ok(Outcome::Stopped(reason))
            }
            ReadReply::Await { .. } => Err(ReaderError::ProviderContract),
        },
    }
}
