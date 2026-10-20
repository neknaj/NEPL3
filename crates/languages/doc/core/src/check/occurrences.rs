//! Budgeted structural expansion of a checked DAG. This is not display order.
use super::{ValidatedDocShape, edges};
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource, StopReason};

/// Parent occurrence in the returned sequence and its constructor child index.
/// Equal node indices do not imply equal occurrences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceParent {
    pub occurrence: u64,
    pub child: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralOccurrence {
    pub node: u64,
    pub parent: Option<OccurrenceParent>,
    /// Relative constructor depth; the root is one. Not HTML insertion depth.
    pub depth: u64,
}
fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<T>() as u64,
    )?;
    items
        .try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    items.push(item);
    Ok(())
}
impl ValidatedDocShape<'_> {
    /// Expand local constructor edges in preorder, preserving every shared-node
    /// occurrence. Includes all variants and image alt fields; it neither selects
    /// visible output nor enters foreign closures. Source/closure admission and
    /// guest semantics still require validation by the consuming operation.
    ///
    /// Cost is proportional to expanded occurrences, potentially exponential in
    /// DAG size. Work and allocation are charged before growth; a stop returns no
    /// partial sequence. Do not replace linear shape/duplicate-label checks with
    /// this operation. Current caller depth is checked even for a leaf root.
    pub fn structural_occurrences(
        &self,
        b: &mut Budget,
    ) -> Result<Vec<StructuralOccurrence>, StopReason> {
        b.poll()?;
        let base = b.current_depth();
        let root = StructuralOccurrence {
            node: edges::root(self.value.root).0,
            parent: None,
            depth: 1,
        };
        let mut pending = Vec::new();
        b.charge(Resource::Work, 1)?;
        push(&mut pending, root, b)?;
        let mut output = Vec::new();
        while let Some(current) = pending.pop() {
            let depth = base
                .checked_add(current.depth)
                .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
            b.with_depth_at_least(depth, |b| b.charge(Resource::Work, 1))?;
            let occurrence = output.len() as u64;
            push(&mut output, current, b)?;
            // Shape validation and the immutable borrow establish these bounds.
            let kind = &self.value.nodes[current.node as usize].kind;
            let start = pending.len();
            let mut child = 0;
            while let Some((node, _)) = edges::edge(kind, child) {
                b.charge(Resource::Work, 1)?;
                let depth = current
                    .depth
                    .checked_add(1)
                    .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
                push(
                    &mut pending,
                    StructuralOccurrence {
                        node,
                        depth,
                        parent: Some(OccurrenceParent {
                            occurrence,
                            child: child as u64,
                        }),
                    },
                    b,
                )?;
                child += 1;
            }
            b.charge(Resource::Work, (pending.len() - start) as u64)?;
            pending[start..].reverse();
        }
        b.poll()?;
        Ok(output)
    }
}
