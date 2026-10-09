//! Native pending-call dispatch. No new Budget or source-admission ledger is made.
use super::*;
use crate::plan::ProviderKind;
use nepl3_core::budget::{Limits, Usage};

fn ceiling(
    base: Usage,
    grant: Limits,
    outer: Limits,
    b: &mut Budget,
) -> Result<Limits, nepl3_core::budget::StopReason> {
    use nepl3_core::budget::StopReason;
    b.poll()?;
    fn add(
        base: u64,
        grant: u64,
        outer: u64,
        reason: StopReason,
        b: &mut Budget,
    ) -> Result<u64, StopReason> {
        base.checked_add(grant)
            .filter(|v| *v <= outer)
            .ok_or_else(|| b.stop(reason))
    }
    let depth = if base.depth <= grant.depth && grant.depth <= outer.depth {
        grant.depth
    } else {
        return Err(b.stop(StopReason::DepthLimit));
    };
    Ok(Limits {
        source_bytes: add(
            base.source_bytes,
            grant.source_bytes,
            outer.source_bytes,
            StopReason::SourceLimit,
            b,
        )?,
        work: add(base.work, grant.work, outer.work, StopReason::WorkLimit, b)?,
        depth,
        nodes: add(
            base.nodes,
            grant.nodes,
            outer.nodes,
            StopReason::NodeLimit,
            b,
        )?,
        allocation_units: add(
            base.allocation_units,
            grant.allocation_units,
            outer.allocation_units,
            StopReason::AllocationLimit,
            b,
        )?,
        output_bytes: add(
            base.output_bytes,
            grant.output_bytes,
            outer.output_bytes,
            StopReason::OutputLimit,
            b,
        )?,
        diagnostics: add(
            base.diagnostics,
            grant.diagnostics,
            outer.diagnostics,
            StopReason::DiagnosticLimit,
            b,
        )?,
        events: add(
            base.events,
            grant.events,
            outer.events,
            StopReason::EventLimit,
            b,
        )?,
    })
}

pub(crate) fn execute<'a, C: FoundationValueCodec>(
    context: ReadReplyContext<'a>,
    saved_limits: Limits,
    grant: Limits,
    codec: &mut C,
    parent: &mut Budget,
) -> Result<sender::ExecutedName<'a>, PortableError<C::Error>> {
    parent.poll()?;
    let saved = context.continuation;
    if parent.limits() != saved_limits
        || !crate::runtime::usage_at_least(parent.usage(), saved.usage)
    {
        return Err(reader(ReaderError::Continuation));
    }
    let ProviderCall::Read {
        operation,
        request,
        depth_base,
        ..
    } = &saved.pending
    else {
        return Err(reader(ReaderError::ProviderContract));
    };
    let signature = context.signature;
    if signature.kind != ProviderKind::Read
        || !signature.pure
        || !matches!(signature.value_input, TypeDescriptor::Unit)
        || !matches!(signature.value_output, TypeDescriptor::Text)
        || !matches!(signature.state_type, TypeDescriptor::Unit)
    {
        return Err(reader(ReaderError::ProviderContract));
    }
    let TypeDescriptor::Named(continuation) = &signature.continuation_type else {
        return Err(reader(ReaderError::ProviderContract));
    };
    parent.charge(
        Resource::Work,
        (continuation.package.len() as u64)
            .saturating_add(continuation.name.len() as u64)
            .saturating_add(32),
    )?;
    if continuation.package != "nepl3.reader"
        || continuation.revision != 1
        || continuation.name != "ReaderContinuation"
    {
        return Err(reader(ReaderError::ProviderContract));
    }
    let expected = crate::builtin::provider::operation(
        crate::builtin::BuiltinReader::Name,
        context.registry,
        parent,
    )
    .map_err(reader)?;
    parent.charge(
        Resource::Work,
        (operation.name.len() as u64)
            .saturating_add(operation.schema.package.len() as u64)
            .saturating_add(signature.operation.name.len() as u64)
            .saturating_add(signature.operation.schema.package.len() as u64)
            .saturating_add(66),
    )?;
    if operation != &expected || signature.operation != expected {
        return Err(reader(ReaderError::ProviderContract));
    }
    let sources =
        super::super::transform::dispatch_sources(saved, &[], parent, codec.source_admission())
            .map_err(reader)?;
    let reply = {
        let mut scoped = codec.scoped(&sources);
        let checked = request
            .context
            .check(&mut scoped, &sources, context.registry, parent)
            .map_err(PortableError::Context)?;
        let snapshot = sources
            .resolve_with_budget(&request.snapshot, parent)?
            .ok_or(PortableError::UndeclaredSource)?;
        let request = crate::model::ReadRequest {
            snapshot,
            start: request.start,
            limit: request.limit,
            final_input: request.final_input,
            context: &checked,
            state: &request.state,
        };
        // Typed preparation was metered on the parent. No NDF projection or
        // encode/decode roundtrip is introduced into native token execution.
        let depth = (*depth_base).max(parent.current_depth());
        let limits = ceiling(parent.usage(), grant, parent.limits(), parent)?;
        if depth > limits.depth {
            return Err(parent
                .stop(nepl3_core::budget::StopReason::DepthLimit)
                .into());
        }
        parent
            .with_ceiling(limits, |b| {
                b.with_depth_at_least(depth, |b| {
                    crate::builtin::provider::read(
                        operation,
                        request,
                        context.registry,
                        &sources,
                        b,
                        scoped.source_admission(),
                    )
                })
            })
            .map_err(reader)?
    };
    Ok(sender::issued(reply, context.registry, sources, parent))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::budget::StopReason;
    #[test]
    fn ceilings_reject_overflow_and_keep_depth_absolute() -> Result<(), StopReason> {
        let outer = Limits {
            source_bytes: u64::MAX,
            work: u64::MAX,
            depth: 100,
            nodes: 100,
            allocation_units: 100,
            output_bytes: 100,
            diagnostics: 100,
            events: 100,
        };
        let base = Usage {
            source_bytes: 5,
            work: 7,
            depth: 20,
            ..Usage::default()
        };
        let grant = Limits {
            source_bytes: 3,
            work: 4,
            depth: 30,
            ..Limits::default()
        };
        let mut b = Budget::new(outer);
        let c = ceiling(base, grant, outer, &mut b)?;
        assert_eq!(c.source_bytes, 8);
        assert_eq!(c.work, 11);
        assert_eq!(c.depth, 30);
        let mut b = Budget::new(outer);
        let invalid = Limits {
            work: u64::MAX,
            ..grant
        };
        assert_eq!(
            ceiling(base, invalid, outer, &mut b),
            Err(StopReason::WorkLimit)
        );
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        let mut b = Budget::new(outer);
        assert_eq!(
            ceiling(base, Limits { depth: 19, ..grant }, outer, &mut b),
            Err(StopReason::DepthLimit)
        );
        Ok(())
    }
    #[test]
    fn all_additive_ceilings_are_exact_and_stops_remain_sticky() -> Result<(), StopReason> {
        let outer = Limits {
            source_bytes: 100,
            work: 100,
            depth: 100,
            nodes: 100,
            allocation_units: 100,
            output_bytes: 100,
            diagnostics: 100,
            events: 100,
        };
        let base = Usage {
            source_bytes: 5,
            work: 5,
            depth: 20,
            nodes: 5,
            allocation_units: 5,
            output_bytes: 5,
            diagnostics: 5,
            events: 5,
        };
        let exact = Limits {
            source_bytes: 95,
            work: 95,
            depth: 100,
            nodes: 95,
            allocation_units: 95,
            output_bytes: 95,
            diagnostics: 95,
            events: 95,
        };
        assert_eq!(ceiling(base, exact, outer, &mut Budget::new(outer))?, outer);
        for (field, reason) in [
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::NodeLimit,
            StopReason::AllocationLimit,
            StopReason::OutputLimit,
            StopReason::DiagnosticLimit,
            StopReason::EventLimit,
        ]
        .into_iter()
        .enumerate()
        {
            let mut grant = exact;
            match field {
                0 => grant.source_bytes = 96,
                1 => grant.work = 96,
                2 => grant.nodes = 96,
                3 => grant.allocation_units = 96,
                4 => grant.output_bytes = 96,
                5 => grant.diagnostics = 96,
                _ => grant.events = 96,
            };
            let mut b = Budget::new(outer);
            let used = b.usage();
            assert_eq!(ceiling(base, grant, outer, &mut b), Err(reason));
            assert_eq!(b.usage(), used);
            assert_eq!(ceiling(base, exact, outer, &mut b), Err(reason));
        }
        let mut b = Budget::new(outer);
        b.cancel();
        assert_eq!(
            ceiling(base, exact, outer, &mut b),
            Err(StopReason::Cancelled)
        );
        Ok(())
    }
}
