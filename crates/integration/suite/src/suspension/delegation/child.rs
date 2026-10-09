//! Terminal trusted local execution in a separately admitted child context.
use super::{IssuedBudget, SettlementError, fits};
use nepl3_core::{
    budget::{Budget, Limits, StopReason, Usage},
    source::SourceAdmission,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Setup(StopReason),
    ChangedBudget,
    Settlement(SettlementError),
}

/// Retaining callback output never certifies it as successful or admitted.
#[derive(Debug)]
pub struct Failure<T> {
    pub error: Error,
    pub output: Option<T>,
}

/// Actual local observation, settled once. This does not authenticate wire data.
/// The callback's semantic result and the child's stop must both be inspected.
#[derive(Debug)]
pub struct Execution<T> {
    output: T,
    basis: Usage,
    cumulative: Usage,
    increment: Usage,
    stopped: Option<StopReason>,
}
impl<T> Execution<T> {
    pub fn output(&self) -> &T {
        &self.output
    }
    pub fn into_output(self) -> T {
        self.output
    }
    pub fn basis(&self) -> Usage {
        self.basis
    }
    pub fn cumulative_usage(&self) -> Usage {
        self.cumulative
    }
    /// Seven additive differences; depth is an absolute peak including history.
    pub fn settled_usage(&self) -> Usage {
        self.increment
    }
    pub fn stopped(&self) -> Option<StopReason> {
        self.stopped
    }
}

fn ceiling(base: Usage, grant: Limits, outer: Limits) -> Result<Limits, StopReason> {
    fn add(a: u64, b: u64, max: u64, reason: StopReason) -> Result<u64, StopReason> {
        a.checked_add(b).filter(|v| *v <= max).ok_or(reason)
    }
    if base.depth > grant.depth || grant.depth > outer.depth {
        return Err(StopReason::DepthLimit);
    }
    Ok(Limits {
        source_bytes: add(
            base.source_bytes,
            grant.source_bytes,
            outer.source_bytes,
            StopReason::SourceLimit,
        )?,
        work: add(base.work, grant.work, outer.work, StopReason::WorkLimit)?,
        depth: grant.depth,
        nodes: add(base.nodes, grant.nodes, outer.nodes, StopReason::NodeLimit)?,
        allocation_units: add(
            base.allocation_units,
            grant.allocation_units,
            outer.allocation_units,
            StopReason::AllocationLimit,
        )?,
        output_bytes: add(
            base.output_bytes,
            grant.output_bytes,
            outer.output_bytes,
            StopReason::OutputLimit,
        )?,
        diagnostics: add(
            base.diagnostics,
            grant.diagnostics,
            outer.diagnostics,
            StopReason::DiagnosticLimit,
        )?,
        events: add(
            base.events,
            grant.events,
            outer.events,
            StopReason::EventLimit,
        )?,
    })
}

fn difference(after: Usage, before: Usage) -> Option<Usage> {
    if after.depth < before.depth {
        return None;
    }
    Some(Usage {
        source_bytes: after.source_bytes.checked_sub(before.source_bytes)?,
        work: after.work.checked_sub(before.work)?,
        depth: after.depth,
        nodes: after.nodes.checked_sub(before.nodes)?,
        allocation_units: after
            .allocation_units
            .checked_sub(before.allocation_units)?,
        output_bytes: after.output_bytes.checked_sub(before.output_bytes)?,
        diagnostics: after.diagnostics.checked_sub(before.diagnostics)?,
        events: after.events.checked_sub(before.events)?,
    })
}

impl IssuedBudget<'_> {
    /// Consume this grant with one trusted terminal callback. Its child starts
    /// from the actual parent's cumulative history and uses relative additive
    /// capacity plus absolute depth. Only measured differences settle back.
    ///
    /// This explicitly creates a separate admission context: sources are admitted
    /// again there, without transferring its proof to the parent. It is not a
    /// resumable child, remote protocol, or default-dispatch adapter. A dispatch
    /// interpreting relative request limits as cumulative ceilings is unsuitable.
    ///
    /// The callback must retain this Budget/admission and meter all work. Checks
    /// catch observable resets, not dishonest trusted code reconstructing state.
    /// Rust unwind leaves the unresolved grant to cancel its parent; no partial
    /// consumption is fabricated. Child stops are returned, not silently promoted
    /// to success or automatically applied as the parent's semantic stop policy.
    pub fn execute_local_child<T>(
        self,
        saved_depth: u64,
        operation: impl FnOnce(&mut Budget, &mut SourceAdmission) -> T,
    ) -> Result<Execution<T>, Failure<T>> {
        let basis = self.parent.usage();
        let depth = saved_depth.max(self.parent.current_depth());
        let setup = self.parent.poll().and_then(|()| {
            let limits = ceiling(basis, self.allowance, self.parent.limits())?;
            if depth > limits.depth {
                return Err(StopReason::DepthLimit);
            }
            Ok(limits)
        });
        let limits = match setup {
            Ok(limits) => limits,
            Err(reason) => {
                return Err(Failure {
                    error: Error::Setup(self.parent.stop(reason)),
                    output: None,
                });
            }
        };
        let mut child = Budget::new(limits);
        if let Err(reason) = child.record_observed_usage(basis) {
            return Err(Failure {
                error: Error::Setup(self.parent.stop(reason)),
                output: None,
            });
        }
        let mut admission = SourceAdmission::default();
        let result = child.with_depth_at_least(depth, |child| {
            let output = operation(child, &mut admission);
            // Inspect before the depth guard restores fields: restoration must
            // not conceal a replacement of the active child Budget.
            let observed = child.usage();
            let increment = difference(observed, basis).filter(|v| fits(*v, self.allowance));
            let intact = child.limits() == limits
                && child.current_depth() == depth
                && observed.depth >= depth
                && fits(observed, limits);
            Ok::<_, StopReason>((
                output,
                observed,
                increment.filter(|_| intact),
                child.poll().err(),
            ))
        });
        let (output, cumulative, increment, stopped) = match result {
            Ok(result) => result,
            Err(reason) => {
                return Err(Failure {
                    error: Error::Setup(self.parent.stop(reason)),
                    output: None,
                });
            }
        };
        let Some(increment) = increment else {
            return Err(Failure {
                error: Error::ChangedBudget,
                output: Some(output),
            });
        };
        if let Err(error) = self.settle(increment) {
            return Err(Failure {
                error: Error::Settlement(error),
                output: Some(output),
            });
        }
        Ok(Execution {
            output,
            basis,
            cumulative,
            increment,
            stopped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cumulative_ceilings_check_all_additive_fields_and_overflow() {
        let base = Usage {
            source_bytes: 3,
            work: 3,
            depth: 7,
            nodes: 3,
            allocation_units: 3,
            output_bytes: 3,
            diagnostics: 3,
            events: 3,
        };
        let outer = Limits {
            source_bytes: 10,
            work: 10,
            depth: 10,
            nodes: 10,
            allocation_units: 10,
            output_bytes: 10,
            diagnostics: 10,
            events: 10,
        };
        let grant = Limits {
            source_bytes: 7,
            work: 7,
            depth: 10,
            nodes: 7,
            allocation_units: 7,
            output_bytes: 7,
            diagnostics: 7,
            events: 7,
        };
        assert_eq!(ceiling(base, grant, outer), Ok(outer));
        for excess in [8, u64::MAX] {
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
                let mut bad = grant;
                match field {
                    0 => bad.source_bytes = excess,
                    1 => bad.work = excess,
                    2 => bad.nodes = excess,
                    3 => bad.allocation_units = excess,
                    4 => bad.output_bytes = excess,
                    5 => bad.diagnostics = excess,
                    _ => bad.events = excess,
                }
                assert_eq!(ceiling(base, bad, outer), Err(reason));
            }
        }
        for depth in [6, 11] {
            assert_eq!(
                ceiling(base, Limits { depth, ..grant }, outer),
                Err(StopReason::DepthLimit)
            );
        }
    }
}
