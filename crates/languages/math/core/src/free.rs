//! Input requirements from checked lexical binding; no evaluation.
use crate::{
    check::{CheckedExpression, ShapeError},
    model::*,
};
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};

// In-place heapsort keeps comparisons and swaps interruptible and metered;
// no constant factor of the standard library's sort is assumed as a work cap.
fn sift(
    entries: &mut [(&str, u64)],
    mut root: usize,
    budget: &mut Budget,
) -> Result<(), StopReason> {
    while root < entries.len() / 2 {
        budget.charge(Resource::Work, 1)?;
        let mut child = root * 2 + 1;
        if child + 1 < entries.len() {
            budget.charge(
                Resource::Work,
                entries[child].0.len().min(entries[child + 1].0.len()) as u64 + 1,
            )?;
            if entries[child] < entries[child + 1] {
                child += 1;
            }
        }
        budget.charge(
            Resource::Work,
            entries[root].0.len().min(entries[child].0.len()) as u64 + 1,
        )?;
        if entries[root] >= entries[child] {
            break;
        }
        budget.charge(Resource::Work, 1)?;
        entries.swap(root, child);
        root = child;
    }
    Ok(())
}

/// Group free occurrences by exact symbol name. Does not resolve guest languages
/// or treat bound symbols as environment inputs. For U uses and maximum name
/// length L, sorting costs O(U log U * L), with O(U) temporary entries plus output.
pub fn symbols(
    input: &CheckedExpression<'_>,
    budget: &mut Budget,
) -> Result<MathFreeSymbols, ShapeError> {
    budget.poll()?;
    let mut uses = Vec::new();
    for usage in &input.bindings().uses {
        budget.charge(Resource::Work, 1)?;
        if usage.binding.is_some() {
            continue;
        }
        let node = usage.node.0;
        let kind = input
            .value()
            .nodes
            .get(usize::try_from(node).map_err(|_| ShapeError::Reference(node))?)
            .ok_or(ShapeError::Reference(node))?;
        let MathKind::Symbol { name } = &kind.kind else {
            return Err(ShapeError::Reference(node));
        };
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(&str, u64)>() as u64,
        )?;
        uses.push((name.as_str(), usage.occurrence));
    }
    for root in (0..uses.len() / 2).rev() {
        sift(&mut uses, root, budget)?;
    }
    for end in (1..uses.len()).rev() {
        budget.charge(Resource::Work, 1)?;
        uses.swap(0, end);
        sift(&mut uses[..end], 0, budget)?;
    }
    let mut result = MathFreeSymbols {
        symbols: Vec::new(),
    };
    for (name, occurrence) in uses {
        budget.charge(Resource::Work, (name.len() as u64).saturating_add(1))?;
        if result.symbols.last().is_none_or(|last| last.name != name) {
            budget.charge(
                Resource::AllocationUnits,
                (core::mem::size_of::<MathFreeSymbol>() as u64).saturating_add(name.len() as u64),
            )?;
            result.symbols.push(MathFreeSymbol {
                name: String::from(name),
                occurrences: Vec::new(),
            });
        }
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<u64>() as u64,
        )?;
        let last = result
            .symbols
            .last_mut()
            .ok_or(ShapeError::Reference(occurrence))?;
        last.occurrences.push(occurrence);
    }
    Ok(result)
}
