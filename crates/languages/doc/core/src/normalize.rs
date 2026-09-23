//! Canonical local arena order. Guest content and provenance remain unchanged.
use crate::{
    check::{ShapeError, StructureError, edges},
    model::{DocNode, DocValue, DocumentSyntax},
};
use alloc::{vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    source::SourceAdmission,
};

/// Reindex Doc nodes in dependency order. Sentence normalization belongs to
/// the selected Sentence operation; this operation never rewrites a closure.
pub fn document(
    input: &DocumentSyntax,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<DocumentSyntax, StructureError> {
    input.validate_structure(registry, b, admission)?;
    let mut output = input.clone_with_budget(b)?;
    output.value = compact(output.value, b)?;
    output.validate_structure(registry, b, admission)?;
    Ok(output)
}

/// Lowering consumes auxiliary surface constructors into parent fields. Only
/// reachable local nodes enter the resulting semantic arena.
pub(crate) fn compact(mut value: DocValue, b: &mut Budget) -> Result<DocValue, ShapeError> {
    let order = value.walk_shape(b, false)?.into_order();
    b.charge(
        Resource::AllocationUnits,
        (value.nodes.len() as u64)
            .saturating_mul((core::mem::size_of::<Option<DocNode>>() + 8) as u64),
    )?;
    let mut nodes: Vec<_> = core::mem::take(&mut value.nodes)
        .into_iter()
        .map(Some)
        .collect();
    let mut mapping = vec![0u64; nodes.len()];
    for (new, old) in order.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        mapping[*old] = new as u64;
    }
    b.charge(
        Resource::AllocationUnits,
        (order.len() as u64).saturating_mul(core::mem::size_of::<DocNode>() as u64),
    )?;
    for old in order {
        let mut node = nodes[old].take().ok_or(ShapeError::Reference(old as u64))?;
        edges::rewrite(&mut node.kind, |id| {
            b.charge(Resource::Work, 1)?;
            mapping
                .get(id as usize)
                .copied()
                .ok_or(ShapeError::Reference(id))
        })?;
        value.nodes.push(node);
    }
    edges::rewrite_root(&mut value.root, |id| {
        mapping
            .get(id as usize)
            .copied()
            .ok_or(ShapeError::Reference(id))
    })?;
    Ok(value)
}
