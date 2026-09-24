//! Structural guest discovery over the exact immutable checked Doc arena.
use super::{ShapeError, ValidatedDocShape, edges};
use crate::model::{EmbedKind, EmbedRef};
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource, StopReason};

/// One structural occurrence. Repeated nodes and embeds retain separate entries.
/// Depth counts Doc nodes from the root (one), before entering the guest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForeignOccurrence {
    pub node: u64,
    pub embed: EmbedRef,
    pub kind: EmbedKind,
    pub depth: u64,
}

fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), ShapeError> {
    b.charge(Resource::Work, 1)?;
    if items.len() == items.capacity() {
        let capacity = items
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(1))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        let bytes = capacity
            .checked_mul(core::mem::size_of::<T>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        items
            .try_reserve_exact(capacity - items.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    items.push(item);
    Ok(())
}
fn zeros(count: usize, b: &mut Budget) -> Result<Vec<u64>, ShapeError> {
    let bytes = count
        .checked_mul(core::mem::size_of::<u64>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::Work, count as u64)?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    values.resize(count, 0);
    Ok(values)
}
impl ValidatedDocShape<'_> {
    /// Enumerate all guest slots in preorder of the typed Doc child sequence.
    /// An owner's slot precedes its local children (for example, a Circuit
    /// figure precedes its caption). All Parallel variants and optional content
    /// participate; display-language selection is a separate operation.
    ///
    /// This checks only shape. No guest is decoded, validated or executed.
    /// Shared subtrees expand under the cumulative Budget; a stop returns no
    /// partial sequence. Traversal uses O(depth) stack storage.
    pub fn foreign_occurrences(
        &self,
        b: &mut Budget,
    ) -> Result<Vec<ForeignOccurrence>, ShapeError> {
        b.poll()?;
        let mut stack = Vec::new();
        let mut output = Vec::new();
        push(
            &mut stack,
            (edges::root(self.value.root).0 as usize, 0usize),
            b,
        )?;
        let base = b.current_depth();
        while let Some(&(node, next)) = stack.last() {
            let depth = stack.len() as u64;
            b.with_depth_at_least::<_, ShapeError>(base.saturating_add(depth), |b| {
                b.charge(Resource::Work, 1)?;
                if next == 0
                    && let Some((embed, kind)) = self.value.nodes[node].kind.embedded()
                {
                    push(
                        &mut output,
                        ForeignOccurrence {
                            node: node as u64,
                            embed,
                            kind,
                            depth,
                        },
                        b,
                    )?;
                }
                Ok(())
            })?;
            if let Some((child, _)) = edges::edge(&self.value.nodes[node].kind, next) {
                if let Some(frame) = stack.last_mut() {
                    frame.1 += 1;
                }
                push(&mut stack, (child as usize, 0), b)?;
            } else {
                stack.pop();
            }
        }
        Ok(output)
    }

    /// Maximum owner depth for each embed, in embed-table order. This visits
    /// nodes and edges once, retaining the longest path through a shared DAG.
    /// The returned depths are relative to the Doc root, excluding caller depth.
    pub fn foreign_depths(&self, b: &mut Budget) -> Result<Vec<u64>, ShapeError> {
        b.poll()?;
        let mut nodes = zeros(self.value.nodes.len(), b)?;
        let mut embeds = zeros(self.value.embeds.len(), b)?;
        nodes[edges::root(self.value.root).0 as usize] = 1;
        let base = b.current_depth();
        for node in self.order.iter().rev().copied() {
            b.with_depth_at_least::<_, ShapeError>(base.saturating_add(nodes[node]), |b| {
                b.charge(Resource::Work, 1)?;
                Ok(())
            })?;
            let mut index = 0;
            while let Some((child, _)) = edges::edge(&self.value.nodes[node].kind, index) {
                b.charge(Resource::Work, 1)?;
                nodes[child as usize] = nodes[child as usize].max(nodes[node].saturating_add(1));
                index += 1;
            }
            if let Some((embed, _)) = self.value.nodes[node].kind.embedded() {
                embeds[embed.0 as usize] = embeds[embed.0 as usize].max(nodes[node]);
            }
        }
        Ok(embeds)
    }
}
