//! Unfold anonymous DAG sharing; retain named rules and Ref recursion.
use super::{PackageError, sorted};
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_reader::plan::{
    CharClass, ProviderSignature, ReaderExpr, ReaderId, ReaderPlan, ReaderRule,
};

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
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut Budget) -> Result<(), PackageError> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    values.push(value);
    Ok(())
}
fn copied(
    expr: &ReaderExpr,
    ids: Vec<ReaderId>,
    budget: &mut Budget,
) -> Result<ReaderExpr, PackageError> {
    let strings = match expr {
        ReaderExpr::Literal(s) | ReaderExpr::Ref(s) | ReaderExpr::Until(s) => s.len(),
        ReaderExpr::Scalar(CharClass::Chars(s) | CharClass::Except(s)) => s.len(),
        ReaderExpr::Capture { name, .. } => name.len(),
        ReaderExpr::Region { class, .. } => class.schema.package.len() + class.name.len(),
        ReaderExpr::Node { kind, .. } => kind.schema.package.len(),
        ReaderExpr::Decode { provider, .. }
        | ReaderExpr::Map { provider, .. }
        | ReaderExpr::Then { provider, .. }
        | ReaderExpr::Call(provider) => provider.schema.package.len() + provider.name.len(),
        _ => 0,
    };
    budget.charge(Resource::AllocationUnits, strings as u64)?;
    budget.charge(Resource::Work, strings as u64 + 1)?;
    if matches!(expr, ReaderExpr::Seq(_)) {
        return Ok(ReaderExpr::Seq(ids));
    }
    if matches!(expr, ReaderExpr::Choice(_)) {
        return Ok(ReaderExpr::Choice(ids));
    }
    let mut result = expr.clone();
    let target = match &mut result {
        ReaderExpr::Many(v)
        | ReaderExpr::Some(v)
        | ReaderExpr::Optional(v)
        | ReaderExpr::Look(v)
        | ReaderExpr::Not(v)
        | ReaderExpr::Commit(v)
        | ReaderExpr::Discard(v) => Some(v),
        ReaderExpr::Repeat { body, .. }
        | ReaderExpr::Capture { body, .. }
        | ReaderExpr::Region { body, .. }
        | ReaderExpr::Node { body, .. }
        | ReaderExpr::Decode { body, .. }
        | ReaderExpr::Map { body, .. } => Some(body),
        ReaderExpr::Then { first, .. } => Some(first),
        _ => None,
    };
    match target {
        Some(target) => {
            let [id] = ids.as_slice() else {
                return Err(PackageError::InvalidRead);
            };
            *target = *id;
        }
        None if !ids.is_empty() => return Err(PackageError::InvalidRead),
        _ => {}
    }
    Ok(result)
}
pub(super) fn charge_plan_sort(plan: &ReaderPlan, budget: &mut Budget) -> Result<(), PackageError> {
    let rule_width = plan
        .rules
        .iter()
        .map(|r| r.name.len() as u64 + 1)
        .max()
        .unwrap_or(1);
    let provider_width = plan
        .providers
        .iter()
        .map(|p| p.operation.schema.package.len() as u64 + p.operation.name.len() as u64 + 33)
        .max()
        .unwrap_or(1);
    budget.charge(
        Resource::Work,
        (plan.rules.len() as u64)
            .saturating_mul(plan.rules.len() as u64)
            .saturating_mul(rule_width)
            .saturating_add(
                (plan.providers.len() as u64)
                    .saturating_mul(plan.providers.len() as u64)
                    .saturating_mul(provider_width),
            ),
    )?;
    Ok(())
}
pub(super) fn normal_form(plan: &ReaderPlan, budget: &mut Budget) -> Result<Vec<u8>, PackageError> {
    let rules = sorted(&plan.rules, |r| (&r.name, ""), budget)?;
    budget.charge(Resource::Work, plan.schema.package.len() as u64 + 1)?;
    budget.charge(Resource::AllocationUnits, plan.schema.package.len() as u64)?;
    let mut normalized = ReaderPlan {
        schema: plan.schema.clone(),
        state_type: plan.state_type.clone_with_budget(budget)?,
        expressions: Vec::new(),
        rules: Vec::new(),
        providers: Vec::new(),
    };
    for provider in &plan.providers {
        budget.charge(
            Resource::Work,
            (provider.operation.schema.package.len() + provider.operation.name.len()) as u64 + 1,
        )?;
        budget.charge(
            Resource::AllocationUnits,
            (provider.operation.schema.package.len() + provider.operation.name.len()) as u64,
        )?;
        let provider = ProviderSignature {
            operation: provider.operation.clone(),
            kind: provider.kind,
            pure: provider.pure,
            value_input: provider.value_input.clone_with_budget(budget)?,
            value_output: provider.value_output.clone_with_budget(budget)?,
            state_type: provider.state_type.clone_with_budget(budget)?,
            continuation_type: provider.continuation_type.clone_with_budget(budget)?,
        };
        push(&mut normalized.providers, provider, budget)?;
    }
    for rule in rules {
        let mut pending = Vec::new();
        let mut results = Vec::new();
        push(&mut pending, (rule.root, false, 1u64), budget)?;
        while let Some((id, exit, depth)) = pending.pop() {
            budget.charge(Resource::Work, 1)?;
            budget.observe_depth(depth)?;
            let expr = plan.expression(id)?;
            if !exit {
                push(&mut pending, (id, true, depth), budget)?;
                let next = depth.checked_add(1).ok_or(StopReason::DepthLimit)?;
                for child in children(expr).iter().rev() {
                    push(&mut pending, (*child, false, next), budget)?;
                }
            } else {
                let start = results
                    .len()
                    .checked_sub(children(expr).len())
                    .ok_or(PackageError::InvalidRead)?;
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of_val(children(expr)) as u64,
                )?;
                let ids = results.drain(start..).collect();
                let expr = copied(expr, ids, budget)?;
                let id = ReaderId(normalized.expressions.len() as u64);
                push(&mut normalized.expressions, expr, budget)?;
                push(&mut results, id, budget)?;
            }
        }
        let [root] = results.as_slice() else {
            return Err(PackageError::InvalidRead);
        };
        budget.charge(Resource::Work, rule.name.len() as u64 + 1)?;
        budget.charge(Resource::AllocationUnits, rule.name.len() as u64)?;
        let rule = ReaderRule {
            name: rule.name.clone(),
            root: *root,
            output: rule.output.clone_with_budget(budget)?,
        };
        push(&mut normalized.rules, rule, budget)?;
    }
    charge_plan_sort(&normalized, budget)?;
    Ok(normalized.canonical_json(budget)?)
}
