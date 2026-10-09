//! Pure admission of already completed, host-authorized resource observations.
use super::{Limits, StopReason, Usage};

/// The complete next state of the fields changed by observation recording.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Recorded {
    pub usage: Usage,
    pub measured: u64,
    pub stopped: Option<StopReason>,
}

/// Add all cumulative counters and take the depth maximum atomically.
/// A prior stop is preserved but does not suppress recording admitted work.
/// Failure retains both old marks and selects the first field error unless a
/// stop already exists. This constant-time operation allocates nothing and does
/// not authorize or authenticate observations; the host must do that first.
pub(super) fn record(
    usage: Usage,
    observed: Usage,
    limits: Limits,
    measured: u64,
    stopped: Option<StopReason>,
) -> Recorded {
    fn sum(a: u64, b: u64, limit: u64, reason: StopReason) -> Result<u64, StopReason> {
        a.checked_add(b).filter(|v| *v <= limit).ok_or(reason)
    }
    let next = (|| {
        Ok(Usage {
            source_bytes: sum(
                usage.source_bytes,
                observed.source_bytes,
                limits.source_bytes,
                StopReason::SourceLimit,
            )?,
            work: sum(
                usage.work,
                observed.work,
                limits.work,
                StopReason::WorkLimit,
            )?,
            depth: {
                let depth = usage.depth.max(observed.depth);
                if depth > limits.depth {
                    return Err(StopReason::DepthLimit);
                }
                depth
            },
            nodes: sum(
                usage.nodes,
                observed.nodes,
                limits.nodes,
                StopReason::NodeLimit,
            )?,
            allocation_units: sum(
                usage.allocation_units,
                observed.allocation_units,
                limits.allocation_units,
                StopReason::AllocationLimit,
            )?,
            output_bytes: sum(
                usage.output_bytes,
                observed.output_bytes,
                limits.output_bytes,
                StopReason::OutputLimit,
            )?,
            diagnostics: sum(
                usage.diagnostics,
                observed.diagnostics,
                limits.diagnostics,
                StopReason::DiagnosticLimit,
            )?,
            events: sum(
                usage.events,
                observed.events,
                limits.events,
                StopReason::EventLimit,
            )?,
        })
    })();
    match next {
        Ok(usage) => Recorded {
            usage,
            measured: measured.max(observed.depth),
            stopped,
        },
        Err(reason) => Recorded {
            usage,
            measured,
            stopped: stopped.or(Some(reason)),
        },
    }
}
