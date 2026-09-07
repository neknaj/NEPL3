use super::*;

/// Only node arena indices are renumbered. Other tables have declared slot order.
pub(super) fn order(
    bundle: &SyntaxBundle,
    budget: &mut Budget,
) -> Result<(Vec<usize>, Vec<u64>), WireError> {
    Ok(nepl3_core::syntax::canonical::NodeMapping::new(bundle, budget)?.into_parts())
}

pub(super) fn mapped(id: NodeRef, indices: &[u64]) -> Result<u64, WireError> {
    indices
        .get(usize::try_from(id.0).map_err(|_| WireError::InvalidType)?)
        .copied()
        .filter(|id| *id != u64::MAX)
        .ok_or(WireError::InvalidType)
}
