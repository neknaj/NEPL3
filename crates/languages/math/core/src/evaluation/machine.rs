use super::*;
use crate::model::MathExactValue as V;
use crate::{
    check::{CheckedExpression, edges},
    environment::CheckedEnvironment,
};

enum Task<'a> {
    Visit(ExprRef, u64),
    Apply(ExprRef, usize),
    Bind(&'a str, ExprRef, u64),
    Unbind,
    SumBounds(ExprRef, &'a str, ExprRef, u64),
    SumNext(Sum<'a>),
    SumCollect(Sum<'a>),
}
struct Sum<'a> {
    node: ExprRef,
    name: &'a str,
    body: ExprRef,
    depth: u64,
    current: Rational,
    upper: Rational,
    total: Rational,
    requirements: Vec<Requirement>,
}

/// Evaluate occurrences in source order using explicit stacks (no recursive
/// Rust calls). Shared nodes are reevaluated in their lexical/iteration context.
/// Work includes visited occurrences and sum iterations; arbitrary back-to-back
/// nested sums are bounded by the same caller budget, without replenishment.
pub fn evaluate<'a>(
    input: &'a CheckedExpression<'a>,
    environment: &CheckedEnvironment<'_>,
    b: &mut Budget,
) -> Result<Evaluation<'a>, Error> {
    b.poll()?;
    let source = input.value();
    let MathRoot::Expr(root) = source.root else {
        return Err(Error::InvalidState);
    };
    let mut tasks = Vec::new();
    let mut results = Vec::new();
    let mut bindings: Vec<(&str, Outcome)> = Vec::new();
    let caller = b.current_depth();
    push(&mut tasks, Task::Visit(root, 1), b)?;
    while let Some(task) = tasks.pop() {
        b.charge(Resource::Work, 1)?;
        match task {
            Task::Visit(node, depth) => {
                b.with_depth_at_least::<_, StopReason>(caller.saturating_add(depth), |_| Ok(()))?;
                b.charge(Resource::Nodes, 1)?;
                let kind = &source
                    .nodes
                    .get(node.0 as usize)
                    .ok_or(Error::InvalidState)?
                    .kind;
                match kind {
                    MathKind::Number { value, .. } => {
                        let value = number::clone_with_budget(value, b)?;
                        push(&mut results, Outcome::Exact(V::Scalar { value }), b)?;
                    }
                    MathKind::Symbol { name } => {
                        let mut found = None;
                        for (key, value) in bindings.iter().rev() {
                            b.charge(
                                Resource::Work,
                                (key.len().min(name.len()) as u64).saturating_add(1),
                            )?;
                            if *key == name {
                                found = Some(copy(value, b)?);
                                break;
                            }
                        }
                        let value = match found {
                            Some(v) => v,
                            None => match environment.get(name, b)? {
                                Some(v) => Outcome::Exact(copy_value(v, b)?),
                                None => symbolic(node, Reason::MissingSymbol, b)?,
                            },
                        };
                        push(&mut results, value, b)?;
                    }
                    MathKind::Text { .. }
                    | MathKind::Sequence { .. }
                    | MathKind::Subscript { .. }
                    | MathKind::Superscript { .. }
                    | MathKind::Scripts { .. }
                    | MathKind::Call { .. }
                    | MathKind::Integral { .. } => {
                        let value = symbolic(node, Reason::NotationOnly, b)?;
                        push(&mut results, value, b)?;
                    }
                    MathKind::Let { name, init, body } => {
                        push(
                            &mut tasks,
                            Task::Bind(name, *body, depth.saturating_add(1)),
                            b,
                        )?;
                        push(&mut tasks, Task::Visit(*init, depth.saturating_add(1)), b)?;
                    }
                    MathKind::Sum {
                        index,
                        lower,
                        upper,
                        body,
                    } => {
                        push(
                            &mut tasks,
                            Task::SumBounds(node, index, *body, depth.saturating_add(1)),
                            b,
                        )?;
                        push(&mut tasks, Task::Visit(*upper, depth.saturating_add(1)), b)?;
                        push(&mut tasks, Task::Visit(*lower, depth.saturating_add(1)), b)?;
                    }
                    MathKind::SentenceGuest { .. } => return Err(Error::InvalidState),
                    _ => {
                        let mut count = 0;
                        while edges::edge(kind, count).is_some() {
                            b.charge(Resource::Work, 1)?;
                            count += 1;
                            if matches!(kind, MathKind::Label { .. }) {
                                break;
                            }
                        }
                        push(&mut tasks, Task::Apply(node, count), b)?;
                        for i in (0..count).rev() {
                            let (child, _) = edges::edge(kind, i).ok_or(Error::InvalidState)?;
                            push(
                                &mut tasks,
                                Task::Visit(ExprRef(child), depth.saturating_add(1)),
                                b,
                            )?;
                        }
                    }
                }
            }
            Task::Apply(node, count) => {
                let start = results
                    .len()
                    .checked_sub(count)
                    .ok_or(Error::InvalidState)?;
                let mut operands = Vec::new();
                for result in results.drain(start..) {
                    push(&mut operands, result, b)?;
                }
                let outcome = apply::apply(node, &source.nodes[node.0 as usize].kind, operands, b)?;
                push(&mut results, outcome, b)?;
            }
            Task::Bind(name, body, depth) => {
                let value = pop(&mut results)?;
                push(&mut bindings, (name, value), b)?;
                push(&mut tasks, Task::Unbind, b)?;
                push(&mut tasks, Task::Visit(body, depth), b)?;
            }
            Task::Unbind => {
                pop(&mut bindings)?;
            }
            Task::SumBounds(node, name, body, depth) => {
                let upper = pop(&mut results)?;
                let lower = pop(&mut results)?;
                match (lower, upper) {
                    (
                        Outcome::Exact(V::Scalar { value: current }),
                        Outcome::Exact(V::Scalar { value: upper }),
                    ) if number::is_integer(&current, b)? && number::is_integer(&upper, b)? => {
                        let total = scalar(0, node, b)?;
                        push(
                            &mut tasks,
                            Task::SumNext(Sum {
                                node,
                                name,
                                body,
                                depth,
                                current,
                                upper,
                                total,
                                requirements: Vec::new(),
                            }),
                            b,
                        )?;
                    }
                    (lower, upper) => {
                        let mut rs = Vec::new();
                        for outcome in [lower, upper] {
                            if let Outcome::Symbolic(items) = outcome {
                                for r in items {
                                    push(&mut rs, r, b)?;
                                }
                            }
                        }
                        if rs.is_empty() {
                            push(
                                &mut rs,
                                Requirement {
                                    expression: node,
                                    reason: Reason::UnsupportedExactDomain,
                                },
                                b,
                            )?;
                        }
                        push(&mut results, Outcome::Symbolic(rs), b)?;
                    }
                }
            }
            Task::SumNext(sum) => {
                if number::compare(&sum.current, &sum.upper, b)?.is_gt() {
                    let result = if sum.requirements.is_empty() {
                        Outcome::Exact(V::Scalar { value: sum.total })
                    } else {
                        Outcome::Symbolic(sum.requirements)
                    };
                    push(&mut results, result, b)?;
                } else {
                    let value = number::clone_with_budget(&sum.current, b)?;
                    push(
                        &mut bindings,
                        (sum.name, Outcome::Exact(V::Scalar { value })),
                        b,
                    )?;
                    let body = sum.body;
                    let depth = sum.depth;
                    push(&mut tasks, Task::SumCollect(sum), b)?;
                    push(&mut tasks, Task::Visit(body, depth), b)?;
                }
            }
            Task::SumCollect(mut sum) => {
                pop(&mut bindings)?;
                match pop(&mut results)? {
                    Outcome::Exact(V::Scalar { value }) => {
                        sum.total =
                            number::add(&sum.total, &value, b).map_err(|e| numeric(sum.node, e))?;
                    }
                    Outcome::Exact(_) => push(
                        &mut sum.requirements,
                        Requirement {
                            expression: sum.node,
                            reason: Reason::UnsupportedExactDomain,
                        },
                        b,
                    )?,
                    Outcome::Symbolic(rs) => {
                        for r in rs {
                            push(&mut sum.requirements, r, b)?;
                        }
                    }
                }
                let one = scalar(1, sum.node, b)?;
                sum.current =
                    number::add(&sum.current, &one, b).map_err(|e| numeric(sum.node, e))?;
                push(&mut tasks, Task::SumNext(sum), b)?;
            }
        }
    }
    if results.len() != 1 || !bindings.is_empty() {
        return Err(Error::InvalidState);
    }
    Ok(Evaluation {
        source,
        outcome: pop(&mut results)?,
    })
}
