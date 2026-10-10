//! Pure admission to a temporary ceiling. No usage is consumed by entry.
use super::{Limits, StopReason, Usage};

pub(super) fn enter(
    outer: Limits,
    requested: Limits,
    usage: Usage,
    stopped: Option<StopReason>,
) -> Result<Limits, StopReason> {
    if let Some(reason) = stopped {
        return Err(reason);
    }
    let limits = Limits {
        source_bytes: outer.source_bytes.min(requested.source_bytes),
        work: outer.work.min(requested.work),
        nodes: outer.nodes.min(requested.nodes),
        allocation_units: outer.allocation_units.min(requested.allocation_units),
        output_bytes: outer.output_bytes.min(requested.output_bytes),
        diagnostics: outer.diagnostics.min(requested.diagnostics),
        events: outer.events.min(requested.events),
        depth: outer.depth.min(requested.depth),
    };
    // Preserve the public operation's priority: depth is checked last.
    if usage.source_bytes > limits.source_bytes {
        return Err(StopReason::SourceLimit);
    }
    if usage.work > limits.work {
        return Err(StopReason::WorkLimit);
    }
    if usage.nodes > limits.nodes {
        return Err(StopReason::NodeLimit);
    }
    if usage.allocation_units > limits.allocation_units {
        return Err(StopReason::AllocationLimit);
    }
    if usage.output_bytes > limits.output_bytes {
        return Err(StopReason::OutputLimit);
    }
    if usage.diagnostics > limits.diagnostics {
        return Err(StopReason::DiagnosticLimit);
    }
    if usage.events > limits.events {
        return Err(StopReason::EventLimit);
    }
    if usage.depth > limits.depth {
        return Err(StopReason::DepthLimit);
    }
    Ok(limits)
}
