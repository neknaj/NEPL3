fn check_shell_depth(
    fragment: &nepl3_markup::html::HtmlFragment,
    budget: &mut nepl3_core::budget::Budget,
) -> Result<(), String> {
    use nepl3_core::budget::Resource;
    use nepl3_markup::html::HtmlNode;
    let mut pending = Vec::new();
    push_pending(&mut pending, (fragment.root, 3), budget)?;
    while let Some((node, depth)) = pending.pop() {
        budget.charge(Resource::Work, 1).map_err(err)?;
        if depth > 256 {
            return Err(format!(
                "OutputDepth {{ element: {node}, shell_depth: {depth} }}"
            ));
        }
        budget.observe_depth(depth).map_err(err)?;
        if let HtmlNode::Element { children, .. } = &fragment.nodes[node as usize] {
            for child in children.iter().rev() {
                push_pending(&mut pending, (*child, depth + 1), budget)?;
            }
        }
    }
    Ok(())
}

fn push_pending(
    pending: &mut Vec<(u64, u64)>,
    value: (u64, u64),
    budget: &mut nepl3_core::budget::Budget,
) -> Result<(), String> {
    use nepl3_core::budget::{Resource, StopReason};
    budget.charge(Resource::Work, 1).map_err(err)?;
    if pending.len() == pending.capacity() {
        let size = std::mem::size_of::<(u64, u64)>();
        let Some((capacity, bytes)) = pending
            .capacity()
            .checked_mul(2)
            .map(|v| v.max(1))
            .and_then(|c| c.checked_mul(size).map(|bytes| (c, bytes)))
        else {
            return Err(err(budget.stop(StopReason::AllocationLimit)));
        };
        budget
            .charge(Resource::AllocationUnits, bytes as u64)
            .map_err(err)?;
        pending.reserve_exact(capacity - pending.len());
    }
    pending.push(value);
    Ok(())
}
