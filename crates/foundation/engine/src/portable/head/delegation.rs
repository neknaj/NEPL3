//! Relative remote work and absolute depth are reconciled only with a private,
//! host-issued dispatch. Partial windows never enter core SourceAdmission.
use super::*;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Limits, Resource, StopReason, Usage},
    source::{SourceAdmission, SourceSnapshot},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadDelegation {
    pub call: HeadCall,
    pub limits: Limits,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadDelivery {
    pub reply: HeadReply,
    pub usage: Usage,
}
/// Host policy for its own send/receive framing and validation. This capacity
/// is withheld from the child grant; it is not recorded as consumed Usage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HeadTransportReserve {
    pub work: u64,
    pub nodes: u64,
    pub allocation_units: u64,
    pub output_bytes: u64,
    pub diagnostics: u64,
    pub events: u64,
}

/// Consumed on delivery acceptance. Its source exemption can only be obtained
/// by comparing every window to explicitly supplied actual full snapshots.
pub struct IssuedHeadDelegation<'a, 'p> {
    call: &'a HeadCall,
    profile: &'a ResolvedParseProfile<'p>,
    parent_limits: Limits,
    baseline: Usage,
    limits: Limits,
    window_bytes: u64,
    budget: &'a mut Budget,
    settled: Option<Usage>,
}
impl<'a, 'p> IssuedHeadDelegation<'a, 'p> {
    /// Called by the host for its saved dispatch slot, after its own execution
    /// authorization. Never promote an arbitrary newly received call by merely
    /// finding compatible source bytes in an ambient store.
    pub fn issue(
        call: &'a HeadCall,
        profile: &'a ResolvedParseProfile<'p>,
        sources: &[&[SourceSnapshot]],
        reserve: HeadTransportReserve,
        budget: &'a mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, crate::head::HeadError> {
        use crate::head::HeadError;
        call.validate_projection(profile, budget)?;
        let mut window_bytes = 0u64;
        let mut previous = Vec::new();
        for window in call.windows() {
            let mut actual = None;
            for source in sources.iter().flat_map(|v| v.iter()) {
                budget.charge(
                    Resource::Work,
                    (source.identity().source.0.len() as u64)
                        .saturating_add(window.span.source.source_id.0.len() as u64)
                        .saturating_add(34),
                )?;
                if source.identity().source == window.span.source.source_id
                    && source.identity().revision == window.span.source.revision
                {
                    admission.admit_existing(source, budget)?;
                    if source.identity().digest != window.span.source.digest {
                        return Err(HeadError::Identity);
                    }
                    let bytes = source
                        .slice_range(window.span.start, window.span.end)?
                        .as_bytes();
                    budget.charge(Resource::Work, bytes.len() as u64)?;
                    if bytes != window.bytes {
                        return Err(HeadError::Projection);
                    }
                    actual = Some(source);
                }
            }
            if actual.is_none() {
                return Err(nepl3_core::source::SourceError::MissingSnapshot.into());
            }
            window_bytes = window_bytes
                .checked_add(super::windows::uncovered(
                    window,
                    previous.iter().copied(),
                    budget,
                )?)
                .ok_or(HeadError::Projection)?;
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<&crate::head::SourceWindow>() as u64,
            )?;
            previous.push(window);
        }
        let baseline = budget.usage();
        let parent_limits = budget.limits();
        let limits = Limits {
            // Receiver gets only these explicit windows, even when the parent
            // has already exhausted SourceBytes by admitting the full snapshot.
            source_bytes: window_bytes,
            work: quota(
                parent_limits.work - baseline.work,
                reserve.work,
                StopReason::WorkLimit,
                budget,
            )?,
            depth: parent_limits.depth,
            nodes: quota(
                parent_limits.nodes - baseline.nodes,
                reserve.nodes,
                StopReason::NodeLimit,
                budget,
            )?,
            allocation_units: quota(
                parent_limits.allocation_units - baseline.allocation_units,
                reserve.allocation_units,
                StopReason::AllocationLimit,
                budget,
            )?,
            output_bytes: quota(
                parent_limits.output_bytes - baseline.output_bytes,
                reserve.output_bytes,
                StopReason::OutputLimit,
                budget,
            )?,
            diagnostics: quota(
                parent_limits.diagnostics - baseline.diagnostics,
                reserve.diagnostics,
                StopReason::DiagnosticLimit,
                budget,
            )?,
            events: quota(
                parent_limits.events - baseline.events,
                reserve.events,
                StopReason::EventLimit,
                budget,
            )?,
        };
        Ok(Self {
            call,
            profile,
            parent_limits,
            baseline,
            limits,
            window_bytes,
            budget,
            settled: None,
        })
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    pub fn parent_usage(&self) -> Usage {
        self.budget.usage()
    }
    /// All local framing and codecs run here while the child owns its grant.
    /// The mutable Budget cannot escape this closure or alias the parent.
    pub fn transport<T, E: From<StopReason>>(
        &mut self,
        operation: impl FnOnce(&mut Budget) -> Result<T, E>,
    ) -> Result<T, E> {
        let ceiling = if self.settled.is_some() {
            self.parent_limits
        } else {
            Limits {
                source_bytes: self.parent_limits.source_bytes,
                depth: self.parent_limits.depth,
                work: self.parent_limits.work - self.limits.work,
                nodes: self.parent_limits.nodes - self.limits.nodes,
                allocation_units: self.parent_limits.allocation_units
                    - self.limits.allocation_units,
                output_bytes: self.parent_limits.output_bytes - self.limits.output_bytes,
                diagnostics: self.parent_limits.diagnostics - self.limits.diagnostics,
                events: self.parent_limits.events - self.limits.events,
            }
        };
        self.budget.with_ceiling(ceiling, operation)
    }
    /// Record the host-authenticated completed child observation exactly once,
    /// even if local framing stopped. Unused grant capacity then returns to the
    /// parent. This does not accept an untrusted reply's self-reported counters.
    pub fn settle(&mut self, observed: Usage) -> Result<(), crate::head::HeadError> {
        if self.settled.is_some()
            || !within(observed, self.limits)
            || observed.source_bytes > self.window_bytes
        {
            return Err(crate::head::HeadError::Report);
        }
        let charged = Usage {
            source_bytes: 0,
            ..observed
        };
        self.settled = Some(observed);
        self.budget.record_observed_usage(charged)?;
        Ok(())
    }
    pub fn to_value<C: FoundationValueCodec>(
        &mut self,
        codec: &mut C,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let profile = self.profile;
        let call = self.call;
        let limits = self.limits;
        self.transport(|budget| {
            let s = Schemas::new(profile.registry())?;
            let value = record(
                s.engine,
                "HeadDelegation",
                [
                    call_to_value(call, profile, codec, budget)?,
                    limits.value(&s, codec, budget)?,
                ],
                budget,
            )?;
            profile
                .registry()
                .validate(&expected("HeadDelegation", budget)?, &value, budget)?;
            Ok(value)
        })
    }
    /// `observed_remote` includes framing, receiver decoding, execution and final
    /// encoding. It comes from the authenticated host transport's metering, not
    /// a second self-authorizing field in the untrusted reply. Packet Usage is
    /// sampled before final encoding (avoiding a self-referential counter).
    /// Local packet decoding has already charged this parent Budget separately.
    pub fn accept(
        mut self,
        mut delivery: HeadDelivery,
        observed_remote: Usage,
    ) -> Result<HeadReply, crate::head::HeadError> {
        use crate::head::HeadError;
        match self.settled {
            None => self.settle(observed_remote)?,
            Some(prior) if prior != observed_remote => return Err(HeadError::Report),
            Some(_) => {}
        }
        self.budget.charge(Resource::Work, 1)?;
        if !at_least(observed_remote, delivery.usage)
            || !at_least(delivery.usage, delivery.reply.report.usage)
            || !within(observed_remote, self.limits)
            || observed_remote.source_bytes > self.window_bytes
            || (!matches!(
                delivery.reply.outcome,
                crate::head::HeadOutcome::Stopped { .. }
            ) && observed_remote.source_bytes != self.window_bytes)
        {
            return Err(HeadError::Report);
        }
        self.call
            .validate_reply(&delivery.reply, self.profile, self.budget)?;
        // Advance the report's accounting observation to the delivery cutoff,
        // including framing that preceded the callback. The report data itself
        // is unchanged; final encoding is absorbed above through observed_remote.
        let relative = delivery.usage;
        delivery.reply.report.usage = Usage {
            source_bytes: self.baseline.source_bytes,
            depth: self.baseline.depth.max(relative.depth),
            work: add(self.baseline.work, relative.work)?,
            nodes: add(self.baseline.nodes, relative.nodes)?,
            allocation_units: add(self.baseline.allocation_units, relative.allocation_units)?,
            output_bytes: add(self.baseline.output_bytes, relative.output_bytes)?,
            diagnostics: add(self.baseline.diagnostics, relative.diagnostics)?,
            events: add(self.baseline.events, relative.events)?,
        };
        Ok(delivery.reply)
    }
}
impl Drop for IssuedHeadDelegation<'_, '_> {
    fn drop(&mut self) {
        if self.settled.is_none() {
            self.budget.stop(StopReason::Cancelled);
        }
    }
}
fn quota(
    remaining: u64,
    reserve: u64,
    reason: StopReason,
    budget: &mut Budget,
) -> Result<u64, crate::head::HeadError> {
    remaining
        .checked_sub(reserve)
        .ok_or_else(|| budget.stop(reason).into())
}
fn add(a: u64, b: u64) -> Result<u64, crate::head::HeadError> {
    a.checked_add(b).ok_or(crate::head::HeadError::Report)
}
pub(super) fn at_least(a: Usage, b: Usage) -> bool {
    a.source_bytes >= b.source_bytes
        && a.work >= b.work
        && a.depth >= b.depth
        && a.nodes >= b.nodes
        && a.allocation_units >= b.allocation_units
        && a.output_bytes >= b.output_bytes
        && a.diagnostics >= b.diagnostics
        && a.events >= b.events
}
fn within(usage: Usage, limits: Limits) -> bool {
    at_least(
        Usage {
            source_bytes: limits.source_bytes,
            work: limits.work,
            depth: limits.depth,
            nodes: limits.nodes,
            allocation_units: limits.allocation_units,
            output_bytes: limits.output_bytes,
            diagnostics: limits.diagnostics,
            events: limits.events,
        },
        usage,
    )
}
/// Combine independently metered framing and operation costs. Both depth values
/// are absolute peaks. This arithmetic does not authenticate either observation.
pub fn metered_usage(a: Usage, b: Usage) -> Result<Usage, crate::head::HeadError> {
    Ok(Usage {
        source_bytes: add(a.source_bytes, b.source_bytes)?,
        work: add(a.work, b.work)?,
        depth: a.depth.max(b.depth),
        nodes: add(a.nodes, b.nodes)?,
        allocation_units: add(a.allocation_units, b.allocation_units)?,
        output_bytes: add(a.output_bytes, b.output_bytes)?,
        diagnostics: add(a.diagnostics, b.diagnostics)?,
        events: add(a.events, b.events)?,
    })
}
/// Reserve already metered framing from the declared quota before running the
/// child. Framing cannot admit semantic sources; only call_decode owns windows.
pub fn receiver_limits(limits: Limits, framing: Usage) -> Result<Limits, crate::head::HeadError> {
    if framing.source_bytes != 0 || !within(framing, limits) {
        return Err(crate::head::HeadError::Report);
    }
    Ok(Limits {
        source_bytes: limits.source_bytes,
        depth: limits.depth,
        work: limits.work - framing.work,
        nodes: limits.nodes - framing.nodes,
        allocation_units: limits.allocation_units - framing.allocation_units,
        output_bytes: limits.output_bytes - framing.output_bytes,
        diagnostics: limits.diagnostics - framing.diagnostics,
        events: limits.events - framing.events,
    })
}
/// Read only the scalar limit header under an outer, host-selected transport
/// budget. That transport framing cost is separate from the delegated operation.
pub fn delegation_limits<C: FoundationValueCodec>(
    value: &NdfValue,
    registry: &nepl3_core::schema::SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<Limits, PortableError<C::Error>> {
    budget.charge(Resource::Work, 1)?;
    let s = Schemas::new(registry)?;
    let f = fields(value, s.engine, "HeadDelegation", 2)?;
    budget.charge(
        Resource::AllocationUnits,
        ("nepl3.foundation".len() + "Limits".len()) as u64,
    )?;
    registry.validate(
        &nepl3_core::schema::TypeDescriptor::Named(nepl3_core::schema::TypeRef {
            package: "nepl3.foundation".into(),
            revision: 1,
            name: "Limits".into(),
        }),
        &f[1],
        budget,
    )?;
    Limits::read(&f[1], &s, codec, budget)
}
pub fn delegation_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<HeadDelegation, PortableError<C::Error>> {
    let s = Schemas::new(profile.registry())?;
    profile
        .registry()
        .validate(&expected("HeadDelegation", budget)?, value, budget)?;
    let f = fields(value, s.engine, "HeadDelegation", 2)?;
    let limits = Limits::read(&f[1], &s, codec, budget)?;
    let actual = budget.limits();
    if !within(
        Usage {
            source_bytes: actual.source_bytes,
            work: actual.work,
            depth: actual.depth,
            nodes: actual.nodes,
            allocation_units: actual.allocation_units,
            output_bytes: actual.output_bytes,
            diagnostics: actual.diagnostics,
            events: actual.events,
        },
        limits,
    ) {
        return Err(crate::head::HeadError::Report.into());
    }
    let call = call_decode(
        &f[0],
        profile,
        codec,
        &mut WindowAdmission::default(),
        budget,
    )?;
    Ok(HeadDelegation { call, limits })
}
/// Usage cutoff is immediately before final delivery encoding. The caller must
/// separately retain the actual Budget usage after this function for its trusted
/// transport metering; encoding costs are not pretended to fit inside themselves.
pub fn delivery_to_value<C: FoundationValueCodec>(
    reply: &HeadReply,
    call: &HeadCall,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    budget: &mut Budget,
    framing: Usage,
) -> Result<NdfValue, PortableError<C::Error>> {
    if !at_least(budget.usage(), reply.report.usage) {
        return Err(crate::head::HeadError::Report.into());
    }
    let usage = metered_usage(framing, budget.usage())?;
    let s = Schemas::new(profile.registry())?;
    let value = record(
        s.engine,
        "HeadDelivery",
        [
            reply_to_value(reply, call, profile, codec, budget)?,
            usage.value(&s, codec, budget)?,
        ],
        budget,
    )?;
    profile
        .registry()
        .validate(&expected("HeadDelivery", budget)?, &value, budget)?;
    Ok(value)
}
pub fn delivery_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    issued: &mut IssuedHeadDelegation<'_, '_>,
    codec: &mut C,
) -> Result<HeadDelivery, PortableError<C::Error>> {
    let profile = issued.profile;
    let call = issued.call;
    issued.transport(|budget| {
        let s = Schemas::new(profile.registry())?;
        profile
            .registry()
            .validate(&expected("HeadDelivery", budget)?, value, budget)?;
        let f = fields(value, s.engine, "HeadDelivery", 2)?;
        Ok(HeadDelivery {
            reply: reply_decode(&f[0], call, profile, codec, budget)?,
            usage: Usage::read(&f[1], &s, codec, budget)?,
        })
    })
}
