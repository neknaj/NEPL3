//! Symbolic payload inference precedes surface-schema identity. No provisional
//! KindRef or fake descriptor digest is introduced for Node/Region annotations.
use super::*;

pub(crate) fn infer_document(
    document: &CheckedDocument<'_>,
    imports: &[ReaderImport],
    budget: &mut Budget,
) -> Result<Vec<(String, TypeDescriptor)>, CompileError> {
    let doc = document.document();
    let mut roots = Vec::new();
    for (index, node) in doc.nodes.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        if let NodeKind::Reader { name, expression } = &node.kind {
            for (previous, _) in &roots {
                let previous: &String = previous;
                budget.charge(
                    Resource::Work,
                    previous.len().min(name.value.len()) as u64 + 1,
                )?;
                if previous == &name.value {
                    return Err(CompileError::DuplicateRule.at(&name.span, None, budget));
                }
            }
            let _ = index;
            push(
                &mut roots,
                (text(&name.value, budget)?, *expression),
                budget,
            )?;
        }
    }
    budget.charge(
        Resource::AllocationUnits,
        (doc.nodes.len() as u64)
            .saturating_mul(core::mem::size_of::<Option<TypeDescriptor>>() as u64),
    )?;
    let mut types = alloc::vec![None;doc.nodes.len()];
    loop {
        let mut progress = false;
        for (i, node) in doc.nodes.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            if types[i].is_some() || node.kind.category() != Category::ReaderExpr {
                continue;
            }
            let get = |id: NodeId| types.get(id.0 as usize).and_then(Option::as_ref);
            let borrowed = match &node.kind {
                NodeKind::Commit { body }
                | NodeKind::Capture { body, .. }
                | NodeKind::Region { body, .. }
                | NodeKind::Node { body, .. } => get(*body),
                NodeKind::Choice { alternatives } => {
                    alternatives.items.iter().find_map(|id| get(*id))
                }
                NodeKind::Ref { name } => {
                    budget.charge(
                        Resource::Work,
                        (roots.len() as u64).saturating_mul(name.value.len() as u64 + 1),
                    )?;
                    get(roots
                        .iter()
                        .find(|(n, _)| n == &name.value)
                        .ok_or_else(|| CompileError::MissingRule.at(&name.span, None, budget))?
                        .1)
                }
                NodeKind::Call { provider }
                | NodeKind::Map { provider, .. }
                | NodeKind::Then { provider, .. }
                | NodeKind::Decode {
                    decoder: provider, ..
                } => {
                    budget.charge(
                        Resource::Work,
                        (imports.len() as u64).saturating_mul(provider.value.len() as u64 + 1),
                    )?;
                    Some(
                        &imports
                            .iter()
                            .find(|p| p.provider == provider.value)
                            .ok_or_else(|| {
                                CompileError::MissingProvider.at(&provider.span, None, budget)
                            })?
                            .signature
                            .value_output,
                    )
                }
                _ => None,
            };
            let value = if let Some(t) = borrowed {
                Some(t.clone_with_budget(budget)?)
            } else {
                match &node.kind {
                    NodeKind::Literal { .. }
                    | NodeKind::Look { .. }
                    | NodeKind::Not { .. }
                    | NodeKind::Discard { .. }
                    | NodeKind::Eof => Some(TypeDescriptor::Unit),
                    NodeKind::Scalar { .. }
                    | NodeKind::Takecount { .. }
                    | NodeKind::Until { .. } => Some(TypeDescriptor::Text),
                    NodeKind::Seq { .. } => {
                        budget.charge(
                            Resource::AllocationUnits,
                            core::mem::size_of::<TypeDescriptor>() as u64,
                        )?;
                        Some(TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)))
                    }
                    NodeKind::Many { body }
                    | NodeKind::Some { body }
                    | NodeKind::Repeat { body, .. }
                    | NodeKind::Optional { body } => {
                        if let Some(t) = get(*body) {
                            let t = t.clone_with_budget(budget)?;
                            budget.charge(
                                Resource::AllocationUnits,
                                core::mem::size_of::<TypeDescriptor>() as u64,
                            )?;
                            Some(if matches!(node.kind, NodeKind::Optional { .. }) {
                                TypeDescriptor::Option(Box::new(t))
                            } else {
                                TypeDescriptor::List(Box::new(t))
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            };
            if let Some(value) = value {
                types[i] = Some(value);
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
    let mut out = Vec::new();
    for (name, id) in roots {
        let ty = types
            .get(id.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| CompileError::OutputType.at_node(doc, id, budget))?
            .clone_with_budget(budget)?;
        push(&mut out, (name, ty), budget)?;
    }
    // This is a payload projection, not a checked executable plan. Full lowering
    // subsequently checks every branch, provider signature and nullable recursion.
    Ok(out)
}
