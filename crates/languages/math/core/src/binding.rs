//! Lexical binding only; no evaluation or guest-language name resolution.
use crate::{
    check::{ValidatedMathShape, edges},
    model::*,
};
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource, StopReason};

struct Frame {
    node: usize,
    occurrence: u64,
    next: usize,
    active: bool,
}

fn binder(kind: &MathKind) -> Option<(&str, usize)> {
    match kind {
        MathKind::Let { name, .. } => Some((name, 1)),
        MathKind::Sum { index, .. } | MathKind::Integral { index, .. } => Some((index, 2)),
        _ => None,
    }
}

/// Resolve every symbol occurrence using exact names and nearest enclosing scope.
/// The input proof validates references/cycles. Output refers only to that input;
/// node field locations preserve its source/Origin without inventing positions.
/// Visits occurrences, not unique arena nodes. Symbol lookup scans only active
/// binders: O(U * B * L) worst case for U uses, B active binders and name length
/// L, in addition to traversal. Frame storage is O(depth), binder storage O(B).
pub fn analyze(shape: &ValidatedMathShape<'_>, b: &mut Budget) -> Result<MathBindings, StopReason> {
    b.poll()?;
    let value = shape.value();
    let mut result = MathBindings {
        definitions: Vec::new(),
        uses: Vec::new(),
    };
    let mut frames: Vec<Frame> = Vec::new();
    let mut active: Vec<(&str, u64)> = Vec::new();
    let mut pending = Some(edges::root(value.root).0 as usize);
    let mut occurrence = 0_u64;
    let caller = b.current_depth();
    loop {
        b.charge(Resource::Work, 1)?;
        if let Some(node) = pending.take() {
            b.with_depth_at_least::<_, StopReason>(
                caller.saturating_add(frames.len() as u64).saturating_add(1),
                |_| Ok(()),
            )?;
            b.charge(Resource::Nodes, 1)?;
            b.charge(Resource::AllocationUnits, 64)?;
            let id = occurrence;
            occurrence = occurrence.checked_add(1).ok_or(StopReason::NodeLimit)?;
            let kind = &value.nodes[node].kind;
            if binder(kind).is_some() {
                b.charge(Resource::AllocationUnits, 32)?;
                result.definitions.push(MathBinding {
                    occurrence: id,
                    node: ExprRef(node as u64),
                });
            }
            if let MathKind::Symbol { name } = kind {
                let mut binding = None;
                for (bound, occurrence) in active.iter().rev() {
                    b.charge(Resource::Work, name.len().min(bound.len()) as u64 + 1)?;
                    if name == bound {
                        binding = Some(*occurrence);
                        break;
                    }
                }
                b.charge(Resource::AllocationUnits, 48)?;
                result.uses.push(MathSymbolUse {
                    occurrence: id,
                    node: ExprRef(node as u64),
                    binding,
                });
            }
            frames.push(Frame {
                node,
                occurrence: id,
                next: 0,
                active: false,
            });
        }
        let Some(frame) = frames.last_mut() else {
            break;
        };
        // The preceding child has returned. Only a binder's body activates it;
        // remove that scope before visiting any sibling or leaving the frame.
        if frame.active {
            active.pop();
            frame.active = false;
        }
        let kind = &value.nodes[frame.node].kind;
        if let Some((child, _)) = edges::edge(kind, frame.next) {
            if let Some((name, body)) = binder(kind)
                && body == frame.next
            {
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<(&str, u64)>() as u64,
                )?;
                active.push((name, frame.occurrence));
                frame.active = true;
            }
            frame.next += 1;
            pending = Some(child as usize);
        } else {
            frames.pop();
        }
    }
    Ok(result)
}
