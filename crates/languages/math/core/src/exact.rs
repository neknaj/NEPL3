//! Shapes of exact evaluation values, independent of source notation.
pub mod arithmetic;
use crate::model::MathExactValue;
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactValueError {
    EmptyVector,
    EmptyMatrix,
    MatrixDimensionsOverflow,
    MatrixElementCount { expected: u64, actual: u64 },
    Stopped(StopReason),
}
impl From<StopReason> for ExactValueError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Borrows exactly the checked value. It proves shape, not an evaluated source
/// expression, environment binding, or an arithmetic operation's domain.
pub struct CheckedExactValue<'a>(&'a MathExactValue);
impl<'a> CheckedExactValue<'a> {
    pub fn value(&self) -> &'a MathExactValue {
        self.0
    }
}

/// O(1) shape inspection and O(1) extra space. Canonical Rational is already a
/// checked core type; this operation borrows it without copying its integers.
/// Logical nodes/work still account for every scalar element in a container.
pub fn check<'a>(
    value: &'a MathExactValue,
    budget: &mut Budget,
) -> Result<CheckedExactValue<'a>, ExactValueError> {
    budget.charge(Resource::Work, 1)?;
    budget.charge(Resource::Nodes, 1)?;
    let count = match value {
        MathExactValue::Scalar { .. } | MathExactValue::Truth { .. } => 0,
        MathExactValue::Vector { values } => {
            if values.is_empty() {
                return Err(ExactValueError::EmptyVector);
            }
            values.len() as u64
        }
        MathExactValue::Matrix { rows, cols, values } => {
            if *rows == 0 || *cols == 0 {
                return Err(ExactValueError::EmptyMatrix);
            }
            let expected = rows
                .checked_mul(*cols)
                .ok_or(ExactValueError::MatrixDimensionsOverflow)?;
            let actual = values.len() as u64;
            if expected != actual {
                return Err(ExactValueError::MatrixElementCount { expected, actual });
            }
            actual
        }
    };
    budget.charge(Resource::Work, count)?;
    budget.charge(Resource::Nodes, count)?;
    Ok(CheckedExactValue(value))
}
