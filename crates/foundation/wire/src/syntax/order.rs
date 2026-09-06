use super::*;

/// Only node arena indices are renumbered. Other tables have declared slot order.
pub(super) fn order(
    bundle: &SyntaxBundle,
    budget: &mut Budget,
) -> Result<(Vec<usize>, Vec<u64>), WireError> {
    budget.charge(
        Resource::AllocationUnits,
        (bundle.nodes.len() * core::mem::size_of::<u64>()) as u64,
    )?;
    let mut indices = alloc::vec![u64::MAX;bundle.nodes.len()];
    let mut order = Vec::new();
    let mut pending = Vec::new();
    push(&mut pending, bundle.root, budget)?;
    while let Some(id) = pending.pop() {
        budget.charge(Resource::Work, 1)?;
        let index = usize::try_from(id.0).map_err(|_| WireError::InvalidType)?;
        let old = indices.get_mut(index).ok_or(WireError::InvalidType)?;
        if *old != u64::MAX {
            continue;
        }
        *old = order.len() as u64;
        push(&mut order, index, budget)?;
        let node = bundle.nodes.get(index).ok_or(WireError::InvalidType)?;
        for field in node.fields.iter().rev() {
            match field {
                FieldValue::Child(id) => push(&mut pending, *id, budget)?,
                FieldValue::Children(ids) => {
                    for id in ids.iter().rev() {
                        push(&mut pending, *id, budget)?;
                    }
                }
                FieldValue::Atom(_) | FieldValue::Foreign(_) => {}
            }
        }
    }
    if order.len() != bundle.nodes.len() {
        return Err(WireError::UnreachableNode);
    }
    Ok((order, indices))
}
pub(super) fn mapped(id: NodeRef, indices: &[u64]) -> Result<u64, WireError> {
    indices
        .get(usize::try_from(id.0).map_err(|_| WireError::InvalidType)?)
        .copied()
        .filter(|id| *id != u64::MAX)
        .ok_or(WireError::InvalidType)
}
