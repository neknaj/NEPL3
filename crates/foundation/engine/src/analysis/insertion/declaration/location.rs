//! Locate native birth coordinates in their own tree, then prove one structural path.
use super::*;
use alloc::vec::Vec;
use nepl3_core::{
    budget::Resource,
    syntax::{FieldValue, NodeRef, SyntaxBundle, canonical::BundleMappings},
};

pub(super) struct Location<'a> {
    pub bundle: &'a SyntaxBundle,
    pub node: NodeRef,
    pub path: Vec<ExpectedReadStep>,
}

fn local(
    maps: &BundleMappings<'_>,
    bundle: &SyntaxBundle,
    canonical: NodeRef,
    b: &mut Budget,
) -> Result<NodeRef, DeclarationError> {
    for map in maps.entries() {
        b.charge(Resource::Work, 1)?;
        if core::ptr::eq(map.bundle(), bundle) {
            let index = usize::try_from(canonical.0).map_err(|_| DeclarationError::Structure)?;
            return map
                .order()
                .get(index)
                .copied()
                .map(|n| NodeRef(n as u64))
                .ok_or(DeclarationError::Structure);
        }
    }
    Err(DeclarationError::Structure)
}

/// Canonical numbering is bundle-local. Invert each foreign owner before walking it.
fn invert<'a>(
    prepared: &'a PreparedBindingRequest<'_, '_>,
    target: &crate::binding::CanonicalBindingTarget,
    b: &mut Budget,
) -> Result<(&'a SyntaxBundle, NodeRef), DeclarationError> {
    let tree = prepared.tree.tree();
    let maps = BundleMappings::new(&tree.bundle, b).map_err(DeclarationError::Canonical)?;
    let mut bundle = &tree.bundle;
    for (depth, step) in target.path.iter().enumerate() {
        b.observe_depth((depth as u64).saturating_add(1))?;
        let node = local(&maps, bundle, step.node, b)?;
        b.charge(Resource::Work, step.field.len() as u64 + 1)?;
        b.charge(Resource::AllocationUnits, step.field.len() as u64)?;
        let actual = crate::recovery::ForeignStep {
            node,
            field: step.field.clone(),
        };
        bundle = crate::tree::path(
            bundle,
            core::slice::from_ref(&actual),
            prepared.profile.registry(),
            b,
        )
        .map_err(DeclarationError::Tree)?;
    }
    Ok((bundle, local(&maps, bundle, target.node, b)?))
}

struct Pending<'a> {
    bundle: &'a SyntaxBundle,
    node: NodeRef,
    path: Vec<ExpectedReadStep>,
}
fn push<'a>(
    pending: &mut Vec<Pending<'a>>,
    bundle: &'a SyntaxBundle,
    node: NodeRef,
    parent: &[ExpectedReadStep],
    step: Option<ExpectedReadStep>,
    b: &mut Budget,
) -> Result<(), DeclarationError> {
    let length = parent
        .len()
        .checked_add(usize::from(step.is_some()))
        .ok_or_else(|| DeclarationError::Stopped(b.stop(StopReason::AllocationLimit)))?;
    b.charge(Resource::Work, parent.len() as u64 + 1)?;
    b.charge(
        Resource::AllocationUnits,
        (length as u64)
            .saturating_mul(core::mem::size_of::<ExpectedReadStep>() as u64)
            .saturating_add(core::mem::size_of::<Pending>() as u64),
    )?;
    let mut path = Vec::with_capacity(length);
    path.extend_from_slice(parent);
    if let Some(step) = step {
        path.push(step);
    }
    pending.push(Pending { bundle, node, path });
    Ok(())
}

/// Exhaust all paths before claiming uniqueness. DAG sharing is not deduplicated.
pub(super) fn locate<'a>(
    prepared: &'a PreparedBindingRequest<'_, '_>,
    target: &crate::binding::CanonicalBindingTarget,
    b: &mut Budget,
) -> Result<Option<Location<'a>>, DeclarationError> {
    let (wanted_bundle, wanted_node) = invert(prepared, target, b)?;
    let root = &prepared.tree.tree().bundle;
    unique_location(root, wanted_bundle, wanted_node, b)
}
fn unique_location<'a>(
    root: &'a SyntaxBundle,
    wanted_bundle: &SyntaxBundle,
    wanted_node: NodeRef,
    b: &mut Budget,
) -> Result<Option<Location<'a>>, DeclarationError> {
    let mut pending = Vec::new();
    push(&mut pending, root, root.root, &[], None, b)?;
    let mut found = None;
    while let Some(current) = pending.pop() {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        b.observe_depth((current.path.len() as u64).saturating_add(1))?;
        if core::ptr::eq(current.bundle, wanted_bundle) && current.node == wanted_node {
            if found.is_some() {
                return Ok(None);
            }
            b.charge(Resource::Work, current.path.len() as u64)?;
            b.charge(
                Resource::AllocationUnits,
                (current.path.len() as u64)
                    .saturating_mul(core::mem::size_of::<ExpectedReadStep>() as u64),
            )?;
            found = Some(Location {
                bundle: current.bundle,
                node: current.node,
                path: current.path.clone(),
            });
        }
        let node = usize::try_from(current.node.0)
            .ok()
            .and_then(|i| current.bundle.nodes.get(i))
            .ok_or(DeclarationError::Structure)?;
        for (index, field) in node.fields.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let (bundle, node, step) = match field {
                FieldValue::Child(node) => (
                    current.bundle,
                    *node,
                    ExpectedReadStep::Child {
                        field: index as u64,
                    },
                ),
                FieldValue::Foreign(value) => (
                    &value.bundle,
                    value.bundle.root,
                    ExpectedReadStep::Foreign {
                        field: index as u64,
                    },
                ),
                FieldValue::Atom(_) => continue,
                FieldValue::Children(_) => return Err(DeclarationError::Structure),
            };
            push(&mut pending, bundle, node, &current.path, Some(step), b)?;
        }
    }
    found.map(Some).ok_or(DeclarationError::Structure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::{
        budget::Limits, origin::OriginId, source::Digest, syntax::SyntaxNode, value::SchemaRef,
    };
    fn bundle(children: &[u64]) -> SyntaxBundle {
        let node = |fields| SyntaxNode {
            schema: SchemaRef {
                package: "test.paths".into(),
                revision: 1,
                digest: Digest::of(b"paths"),
            },
            kind: "Node".into(),
            fields,
            head: None,
            cover: None,
            origin: OriginId(0),
            token: None,
        };
        SyntaxBundle {
            sources: Vec::new(),
            origins: Vec::new(),
            environments: Vec::new(),
            tokens: Vec::new(),
            source_maps: Vec::new(),
            root: NodeRef(0),
            nodes: alloc::vec![
                node(
                    children
                        .iter()
                        .map(|n| FieldValue::Child(NodeRef(*n)))
                        .collect()
                ),
                node(Vec::new()),
                node(Vec::new())
            ],
        }
    }
    #[test]
    fn structural_search_exhausts_after_first_match_and_rejects_shared_paths()
    -> Result<(), DeclarationError> {
        let root = bundle(&[1, 2]);
        let limits = Limits {
            work: 100_000,
            nodes: 100_000,
            allocation_units: 100_000,
            depth: 100,
            ..Limits::default()
        };
        let mut b = Budget::new(limits);
        let found = unique_location(&root, &root, NodeRef(2), &mut b)?
            .ok_or(DeclarationError::Structure)?;
        assert_eq!(
            found.path,
            alloc::vec![ExpectedReadStep::Child { field: 1 }]
        );
        assert_eq!(b.usage().nodes, 3); // The left sibling is visited after the target.
        let mut stopped = Budget::new(Limits { nodes: 2, ..limits });
        assert!(matches!(
            unique_location(&root, &root, NodeRef(2), &mut stopped),
            Err(DeclarationError::Stopped(StopReason::NodeLimit))
        ));
        let shared = bundle(&[1, 1]);
        assert!(unique_location(&shared, &shared, NodeRef(1), &mut Budget::new(limits))?.is_none());
        Ok(())
    }
    #[test]
    fn canonical_inverse_uses_mapping_order_not_arena_index() -> Result<(), DeclarationError> {
        let mut root = bundle(&[]);
        root.root = NodeRef(2);
        root.nodes[2].fields =
            alloc::vec![FieldValue::Child(NodeRef(0)), FieldValue::Child(NodeRef(1))];
        let mut b = Budget::new(Limits {
            work: 100_000,
            nodes: 100_000,
            allocation_units: 100_000,
            depth: 100,
            ..Limits::default()
        });
        let mappings = BundleMappings::new(&root, &mut b).map_err(DeclarationError::Canonical)?;
        assert_eq!(local(&mappings, &root, NodeRef(0), &mut b)?, NodeRef(2));
        assert_eq!(local(&mappings, &root, NodeRef(1), &mut b)?, NodeRef(0));
        Ok(())
    }
}
