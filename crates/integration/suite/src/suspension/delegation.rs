//! Exclusive capacity reservation for one host-authorized delegated execution.
use nepl3_core::budget::{Budget, Limits, StopReason, Usage};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementError {
    ObservationExceedsGrant,
    Stopped(StopReason),
}

/// Relative additive quotas and an absolute depth peak. This value is issued
/// after request/provider authorization and ancestor ceiling selection. It is
/// host accounting state, not a wire capability or a provider observation.
///
/// The exclusive parent borrow prevents concurrent reuse of the reserved quota.
/// Local metered work uses `run_local`; a trusted completed observation settles
/// the grant once. Dropping an unsettled grant cancels the parent, since unknown
/// remote consumption cannot safely be treated as unused capacity.
#[must_use = "settle an authenticated observation or cancel the parent"]
pub struct IssuedBudget<'a> {
    parent: &'a mut Budget,
    allowance: Limits,
    local: Limits,
    settled: bool,
}

fn remaining(limit: u64, used: u64, grant: u64, reason: StopReason) -> Result<u64, StopReason> {
    let local = limit.checked_sub(grant).ok_or(reason)?;
    if used > local {
        return Err(reason);
    }
    Ok(local)
}

fn fits(usage: Usage, limits: Limits) -> bool {
    usage.source_bytes <= limits.source_bytes
        && usage.work <= limits.work
        && usage.depth <= limits.depth
        && usage.nodes <= limits.nodes
        && usage.allocation_units <= limits.allocation_units
        && usage.output_bytes <= limits.output_bytes
        && usage.diagnostics <= limits.diagnostics
        && usage.events <= limits.events
}

impl<'a> IssuedBudget<'a> {
    /// Reserve capacity without reporting it as consumed Usage. Zero grants are
    /// valid; requesting more than the remaining capacity stops the parent.
    /// No source-admission exemption is implicit: SourceBytes is additive too.
    pub fn issue(parent: &'a mut Budget, allowance: Limits) -> Result<Self, StopReason> {
        parent.poll()?;
        let limits = parent.limits();
        let usage = parent.usage();
        let local = (|| {
            if allowance.depth > limits.depth {
                return Err(StopReason::DepthLimit);
            }
            Ok(Limits {
                source_bytes: remaining(
                    limits.source_bytes,
                    usage.source_bytes,
                    allowance.source_bytes,
                    StopReason::SourceLimit,
                )?,
                work: remaining(
                    limits.work,
                    usage.work,
                    allowance.work,
                    StopReason::WorkLimit,
                )?,
                depth: limits.depth,
                nodes: remaining(
                    limits.nodes,
                    usage.nodes,
                    allowance.nodes,
                    StopReason::NodeLimit,
                )?,
                allocation_units: remaining(
                    limits.allocation_units,
                    usage.allocation_units,
                    allowance.allocation_units,
                    StopReason::AllocationLimit,
                )?,
                output_bytes: remaining(
                    limits.output_bytes,
                    usage.output_bytes,
                    allowance.output_bytes,
                    StopReason::OutputLimit,
                )?,
                diagnostics: remaining(
                    limits.diagnostics,
                    usage.diagnostics,
                    allowance.diagnostics,
                    StopReason::DiagnosticLimit,
                )?,
                events: remaining(
                    limits.events,
                    usage.events,
                    allowance.events,
                    StopReason::EventLimit,
                )?,
            })
        })()
        .map_err(|reason| parent.stop(reason))?;
        Ok(Self {
            parent,
            allowance,
            local,
            settled: false,
        })
    }

    pub fn limits(&self) -> Limits {
        self.allowance
    }

    pub fn parent_usage(&self) -> Usage {
        self.parent.usage()
    }

    /// Run trusted host framing, validation or local dependencies within the
    /// unreserved capacity. The callback must retain the supplied Budget and
    /// meter its work; it must not replace it with a fresh Budget.
    pub fn run_local<T>(
        &mut self,
        operation: impl FnOnce(&mut Budget) -> Result<T, StopReason>,
    ) -> Result<T, StopReason> {
        let result = self.parent.with_ceiling(self.local, operation);
        if let Err(reason) = result {
            return Err(self.parent.stop(reason));
        }
        self.parent.poll()?;
        result
    }

    /// Consume one independently authenticated observation, including when
    /// local execution has already stopped. Wire Report counters alone do not
    /// authenticate it. The host verifies request/context and the measurement's
    /// provenance before this call. Depth is an absolute peak, never added.
    /// Unused capacity is released; original stop reasons remain sticky.
    pub fn settle(mut self, observed: Usage) -> Result<(), SettlementError> {
        if !fits(observed, self.allowance) {
            self.parent.stop(StopReason::Cancelled);
            return Err(SettlementError::ObservationExceedsGrant);
        }
        self.settled = true;
        self.parent
            .record_observed_usage(observed)
            .map_err(SettlementError::Stopped)
    }
}

impl Drop for IssuedBudget<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.parent.stop(StopReason::Cancelled);
        }
    }
}
