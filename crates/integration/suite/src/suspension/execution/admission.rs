//! Pure derivation of persisted ancestor bounds; no Budget is read or mutated.
use super::{ExecutionScope, Limits, StopReason, intersection};

pub(super) fn transition(
    parent: ExecutionScope,
    requested: Limits,
) -> Result<ExecutionScope, StopReason> {
    let limits = intersection(parent.limits, requested);
    let depth = parent
        .depth
        .checked_add(1)
        .filter(|depth| *depth <= limits.depth)
        .ok_or(StopReason::DepthLimit)?;
    Ok(ExecutionScope { limits, depth })
}
