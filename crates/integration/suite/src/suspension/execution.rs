//! Saved ancestor ceilings for host frames sharing one execution Budget.
use nepl3_core::budget::{Budget, Limits, StopReason};

/// Immutable execution bounds retained across Await. Hosts keep using the same
/// mutable Budget for the entire root operation; this value contains no Usage
/// and cannot reset accounting. It is host scheduling data, not a wire grant.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionScope {
    limits: Limits,
    depth: u64,
}

fn intersection(a: Limits, b: Limits) -> Limits {
    Limits {
        source_bytes: a.source_bytes.min(b.source_bytes),
        work: a.work.min(b.work),
        depth: a.depth.min(b.depth),
        nodes: a.nodes.min(b.nodes),
        allocation_units: a.allocation_units.min(b.allocation_units),
        output_bytes: a.output_bytes.min(b.output_bytes),
        diagnostics: a.diagnostics.min(b.diagnostics),
        events: a.events.min(b.events),
    }
}

impl ExecutionScope {
    /// Capture the root request's ceiling and one operation frame above the
    /// caller's active depth. Limits are absolute cumulative ceilings.
    pub fn root(budget: &mut Budget, requested: Limits) -> Result<Self, StopReason> {
        budget.poll()?;
        let scope = Self {
            limits: intersection(budget.limits(), requested),
            depth: budget.current_depth(),
        };
        scope.child(requested, budget)
    }

    /// Derive a dependency frame without inspecting all ancestors again.
    /// A failed depth admission stops the shared execution Budget.
    pub fn child(&self, requested: Limits, budget: &mut Budget) -> Result<Self, StopReason> {
        budget.poll()?;
        let limits = intersection(self.limits, requested);
        let depth = self
            .depth
            .checked_add(1)
            .filter(|depth| *depth <= limits.depth)
            .ok_or_else(|| budget.stop(StopReason::DepthLimit))?;
        Ok(Self { limits, depth })
    }

    /// Restore this frame for one Invoke or Resume callback. Ancestor ceilings
    /// apply even after the Rust call stack of the parent Invoke has returned.
    /// Every Result path restores outer limits/depth and retains charges/stops.
    /// The callback must return its recorded StopReason on execution failure.
    pub fn run<T, E: From<StopReason>>(
        &self,
        budget: &mut Budget,
        operation: impl FnOnce(&mut Budget) -> Result<T, E>,
    ) -> Result<T, E> {
        budget.with_ceiling(self.limits, |budget| {
            budget.with_depth_at_least(self.depth, operation)
        })
    }
}
