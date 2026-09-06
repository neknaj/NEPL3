//! Direct arena edges are finite; named Ref recursion remains a reader contract.
use super::PackageError;
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource};
use nepl3_reader::plan::{ReaderExpr, ReaderId, ReaderPlan};

fn children(expr: &ReaderExpr) -> &[ReaderId] {
    match expr {
        ReaderExpr::Seq(v) | ReaderExpr::Choice(v) => v,
        ReaderExpr::Many(v)
        | ReaderExpr::Some(v)
        | ReaderExpr::Optional(v)
        | ReaderExpr::Look(v)
        | ReaderExpr::Not(v)
        | ReaderExpr::Commit(v)
        | ReaderExpr::Discard(v) => core::slice::from_ref(v),
        ReaderExpr::Repeat { body, .. }
        | ReaderExpr::Capture { body, .. }
        | ReaderExpr::Region { body, .. }
        | ReaderExpr::Node { body, .. }
        | ReaderExpr::Decode { body, .. }
        | ReaderExpr::Map { body, .. } => core::slice::from_ref(body),
        ReaderExpr::Then { first, .. } => core::slice::from_ref(first),
        _ => &[],
    }
}
fn push<T>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), PackageError> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    v.push(item);
    Ok(())
}
pub(super) fn check_dag(plan: &ReaderPlan, budget: &mut Budget) -> Result<(), PackageError> {
    budget.charge(Resource::AllocationUnits, plan.expressions.len() as u64)?;
    let mut state = alloc::vec![0u8;plan.expressions.len()];
    for root in 0..plan.expressions.len() {
        let mut pending = Vec::new();
        push(&mut pending, (ReaderId(root as u64), false, 1u64), budget)?;
        while let Some((id, exit, depth)) = pending.pop() {
            budget.charge(Resource::Work, 1)?;
            budget.observe_depth(depth)?;
            let expr = plan.expression(id)?;
            let index = usize::try_from(id.0).map_err(|_| PackageError::InvalidRead)?;
            if exit {
                state[index] = 2;
                continue;
            }
            match state[index] {
                1 => return Err(PackageError::DirectCycle),
                2 => continue,
                _ => {}
            }
            state[index] = 1;
            push(&mut pending, (id, true, depth), budget)?;
            for child in children(expr).iter().rev() {
                push(
                    &mut pending,
                    (
                        *child,
                        false,
                        depth
                            .checked_add(1)
                            .ok_or(nepl3_core::budget::StopReason::DepthLimit)?,
                    ),
                    budget,
                )?;
            }
        }
    }
    Ok(())
}
