//! Evaluation borrows the author's notation and never rewrites its arena.
mod apply;
mod machine;
use crate::{exact::arithmetic, model::*, number};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value::{Integer, Rational},
};

pub use crate::model::{
    MathEvaluationOutcome as Outcome, MathEvaluationReason as Reason,
    MathEvaluationRequirement as Requirement,
};
/// Node references in requirements are interpreted only against this exact input.
pub struct Evaluation<'a> {
    pub source: &'a MathValue,
    pub outcome: Outcome,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    At {
        expression: ExprRef,
        error: arithmetic::Error,
    },
    Stopped(StopReason),
    InvalidState,
}
impl From<StopReason> for Error {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
pub use machine::evaluate;

fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, 1)?;
    if items.len() == items.capacity() {
        let capacity = items
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(4))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        let additional = capacity - items.capacity();
        let bytes = (additional as u64)
            .checked_mul(core::mem::size_of::<T>() as u64)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes)?;
        b.charge(Resource::Work, items.len() as u64)?;
        items
            .try_reserve_exact(capacity - items.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    items.push(item);
    Ok(())
}
fn pop<T>(items: &mut Vec<T>) -> Result<T, Error> {
    items.pop().ok_or(Error::InvalidState)
}
fn at(node: ExprRef, error: arithmetic::Error) -> Error {
    match error {
        arithmetic::Error::Stopped(reason) => Error::Stopped(reason),
        error => Error::At {
            expression: node,
            error,
        },
    }
}
fn numeric(node: ExprRef, error: number::ArithmeticError) -> Error {
    at(node, error.into())
}
fn scalar(n: i64, node: ExprRef, b: &mut Budget) -> Result<Rational, Error> {
    number::ratio(&Integer::from(n), &Integer::from(1_i64), b).map_err(|e| numeric(node, e))
}
fn symbolic(node: ExprRef, reason: Reason, b: &mut Budget) -> Result<Outcome, Error> {
    let mut requirements = Vec::new();
    push(
        &mut requirements,
        Requirement {
            expression: node,
            reason,
        },
        b,
    )?;
    Ok(Outcome::Symbolic(requirements))
}
fn copy(value: &Outcome, b: &mut Budget) -> Result<Outcome, Error> {
    Ok(match value {
        Outcome::Symbolic(items) => {
            let mut out = Vec::new();
            for item in items {
                push(&mut out, item.clone(), b)?;
            }
            Outcome::Symbolic(out)
        }
        Outcome::Exact(value) => Outcome::Exact(copy_value(value, b)?),
    })
}
fn copy_value(value: &MathExactValue, b: &mut Budget) -> Result<MathExactValue, Error> {
    b.charge(Resource::Nodes, 1)?;
    Ok(match value {
        MathExactValue::Scalar { value } => MathExactValue::Scalar {
            value: number::clone_with_budget(value, b)?,
        },
        MathExactValue::Truth { value } => MathExactValue::Truth { value: *value },
        MathExactValue::Vector { values } | MathExactValue::Matrix { values, .. } => {
            let mut out = Vec::new();
            for item in values {
                b.charge(Resource::Nodes, 1)?;
                let item = number::clone_with_budget(item, b)?;
                push(&mut out, item, b)?;
            }
            match value {
                MathExactValue::Matrix { rows, cols, .. } => MathExactValue::Matrix {
                    rows: *rows,
                    cols: *cols,
                    values: out,
                },
                _ => MathExactValue::Vector { values: out },
            }
        }
    })
}
