fn check_shell_depth(
    fragment: &nepl3_markup::html::HtmlFragment,
    budget: &mut nepl3_core::budget::Budget,
) -> Result<(), String> {
    use nepl3_core::budget::Resource;
    use nepl3_markup::html::HtmlNode;
    let mut pending = Vec::new();
    let slot = 2 * std::mem::size_of::<(u64, u64)>() as u64;
    budget
        .charge(Resource::AllocationUnits, slot)
        .map_err(err)?;
    pending.push((fragment.root, 3u64));
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
                budget
                    .charge(Resource::AllocationUnits, slot)
                    .map_err(err)?;
                pending.push((*child, depth + 1));
            }
        }
    }
    Ok(())
}
