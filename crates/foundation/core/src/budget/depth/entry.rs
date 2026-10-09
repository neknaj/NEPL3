//! Pure entry into a callback's depth scope. Callback execution is separate.
use super::super::StopReason;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    Next,
    AtLeast(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Entry {
    Entered {
        active: u64,
        usage: u64,
        measured: u64,
    },
    Stopped(StopReason),
}

/// Preserve the first stop, then validate the new active depth and raise both
/// independent high-water marks. Historical marks above a lowered ceiling are
/// retained; only the target active depth is tested. No allocation or I/O.
pub(crate) fn enter(
    active: u64,
    target: Target,
    limit: u64,
    usage: u64,
    measured: u64,
    stopped: Option<StopReason>,
) -> Entry {
    if let Some(reason) = stopped {
        return Entry::Stopped(reason);
    }
    let next = match target {
        Target::Next => active.checked_add(1),
        Target::AtLeast(base) => Some(active.max(base)),
    };
    match next {
        Some(active) if active <= limit => Entry::Entered {
            active,
            usage: usage.max(active),
            measured: measured.max(active),
        },
        _ => Entry::Stopped(StopReason::DepthLimit),
    }
}
