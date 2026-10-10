//! Pure completion of a local depth measurement.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::budget) struct Completed {
    pub merged: u64,
    pub relative: u64,
}

/// Retain the enclosing high-water mark independently from the local result.
/// Saturation defines the defensive boundary when a callback lowers its mark.
/// Validation callbacks are required not to replace the Budget itself.
pub(in crate::budget) fn finish(base: u64, previous: u64, measured: u64) -> Completed {
    Completed {
        merged: previous.max(measured),
        relative: measured.saturating_sub(base),
    }
}
