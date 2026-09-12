//! Shape-aware exact operations. These do not traverse or rewrite source syntax.
use super::CheckedExactValue;
use crate::{
    model::MathExactValue as Value,
    number::{self, ArithmeticError},
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value::Rational,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    OperandShapeMismatch,
    NotSquare,
    Arithmetic(ArithmeticError),
    Stopped(StopReason),
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<ArithmeticError> for Error {
    fn from(error: ArithmeticError) -> Self {
        match error {
            ArithmeticError::Stopped(reason) => Self::Stopped(reason),
            other => Self::Arithmetic(other),
        }
    }
}

fn storage(count: usize, b: &mut Budget) -> Result<Vec<Rational>, Error> {
    let bytes = (count as u64)
        .checked_mul(core::mem::size_of::<Rational>() as u64)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes)?;
    b.charge(Resource::Nodes, count as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(values)
}
fn start(b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, 1)?;
    b.charge(Resource::Nodes, 1)?;
    Ok(())
}

fn map(
    value: &Value,
    b: &mut Budget,
    mut f: impl FnMut(&Rational, &mut Budget) -> Result<Rational, ArithmeticError>,
) -> Result<Value, Error> {
    let (items, dimensions) = match value {
        Value::Scalar { value } => {
            return Ok(Value::Scalar {
                value: f(value, b)?,
            });
        }
        Value::Vector { values } => (values, None),
        Value::Matrix { rows, cols, values } => (values, Some((*rows, *cols))),
        Value::Truth { .. } => return Err(Error::OperandShapeMismatch),
    };
    let mut values = storage(items.len(), b)?;
    for item in items {
        values.push(f(item, b)?);
    }
    Ok(match dimensions {
        Some((rows, cols)) => Value::Matrix { rows, cols, values },
        None => Value::Vector { values },
    })
}

/// O(N) scalar operations and O(N) result storage; bigint cost is charged by number.
pub fn negate(value: &CheckedExactValue<'_>, b: &mut Budget) -> Result<Value, Error> {
    start(b)?;
    map(value.value(), b, number::negate)
}

#[derive(Clone, Copy)]
pub enum Additive {
    Add,
    Subtract,
}

/// Scalar division only. In particular, division by a vector is not elementwise.
pub fn divide(
    left: &CheckedExactValue<'_>,
    right: &CheckedExactValue<'_>,
    b: &mut Budget,
) -> Result<Value, Error> {
    start(b)?;
    match (left.value(), right.value()) {
        (Value::Scalar { value: l }, Value::Scalar { value: r }) => Ok(Value::Scalar {
            value: number::divide(l, r, b)?,
        }),
        _ => Err(Error::OperandShapeMismatch),
    }
}

#[derive(Clone, Copy)]
pub enum Comparison {
    Equal,
    Less,
    LessEqual,
}

/// Matrix transpose; Vector becomes a 1*n row Matrix. O(N) scalar copies/storage.
pub fn transpose(input: &CheckedExactValue<'_>, b: &mut Budget) -> Result<Value, Error> {
    start(b)?;
    if let Value::Vector { values: source } = input.value() {
        let mut values = storage(source.len(), b)?;
        for value in source {
            values.push(number::clone_with_budget(value, b)?);
        }
        return Ok(Value::Matrix {
            rows: 1,
            cols: source.len() as u64,
            values,
        });
    }
    let Value::Matrix {
        rows,
        cols,
        values: source,
    } = input.value()
    else {
        return Err(Error::OperandShapeMismatch);
    };
    let mut values = storage(source.len(), b)?;
    for col in 0..*cols as usize {
        for row in 0..*rows as usize {
            values.push(number::clone_with_budget(
                &source[row * *cols as usize + col],
                b,
            )?);
        }
    }
    Ok(Value::Matrix {
        rows: *cols,
        cols: *rows,
        values,
    })
}

/// Exact Gaussian elimination with row pivoting. Square matrices only.
/// O(n^3) rational operations and O(n^2) temporary storage; rational bit growth
/// is separately metered. A missing pivot proves determinant zero, not failure.
pub fn determinant(input: &CheckedExactValue<'_>, b: &mut Budget) -> Result<Value, Error> {
    start(b)?;
    let Value::Matrix {
        rows,
        cols,
        values: source,
    } = input.value()
    else {
        return Err(Error::NotSquare);
    };
    if rows != cols {
        return Err(Error::NotSquare);
    }
    let n = *rows as usize;
    let mut values = storage(source.len(), b)?;
    for value in source {
        values.push(number::clone_with_budget(value, b)?);
    }
    let mut determinant = number::ratio(
        &nepl3_core::value::Integer::from(1_i64),
        &nepl3_core::value::Integer::from(1_i64),
        b,
    )?;
    for col in 0..n {
        let mut pivot = col;
        while pivot < n && number::is_zero(&values[pivot * n + col], b)? {
            pivot += 1;
        }
        if pivot == n {
            return Ok(Value::Scalar {
                value: number::ratio(
                    &nepl3_core::value::Integer::from(0_i64),
                    &nepl3_core::value::Integer::from(1_i64),
                    b,
                )?,
            });
        }
        if pivot != col {
            for k in col..n {
                b.charge(Resource::Work, 1)?;
                values.swap(col * n + k, pivot * n + k);
            }
            determinant = number::negate(&determinant, b)?;
        }
        determinant = number::multiply(&determinant, &values[col * n + col], b)?;
        for row in col + 1..n {
            if number::is_zero(&values[row * n + col], b)? {
                continue;
            }
            let factor = number::divide(&values[row * n + col], &values[col * n + col], b)?;
            // Earlier columns are no longer read. No need to allocate zeros.
            for k in col + 1..n {
                let product = number::multiply(&factor, &values[col * n + k], b)?;
                values[row * n + k] = number::subtract(&values[row * n + k], &product, b)?;
            }
        }
    }
    Ok(Value::Scalar { value: determinant })
}

/// Equal accepts identical kinds and shapes, including Truth. Ordering accepts
/// only Scalar. At most O(N) rational comparisons and no container allocation;
/// exact bigint comparison has its own charged temporary storage. A shape error
/// is not false, and scalar/array coercion is never performed.
pub fn compare(
    left: &CheckedExactValue<'_>,
    right: &CheckedExactValue<'_>,
    op: Comparison,
    b: &mut Budget,
) -> Result<Value, Error> {
    start(b)?;
    let (lhs, rhs) = match (left.value(), right.value()) {
        (Value::Scalar { value: l }, Value::Scalar { value: r }) => {
            let ordering = number::compare(l, r, b)?;
            return Ok(Value::Truth {
                value: match op {
                    Comparison::Equal => ordering.is_eq(),
                    Comparison::Less => ordering.is_lt(),
                    Comparison::LessEqual => ordering.is_le(),
                },
            });
        }
        (Value::Truth { value: l }, Value::Truth { value: r })
            if matches!(op, Comparison::Equal) =>
        {
            return Ok(Value::Truth { value: l == r });
        }
        (Value::Vector { values: l }, Value::Vector { values: r })
            if matches!(op, Comparison::Equal) && l.len() == r.len() =>
        {
            (l, r)
        }
        (
            Value::Matrix {
                rows: lr,
                cols: lc,
                values: l,
            },
            Value::Matrix {
                rows: rr,
                cols: rc,
                values: r,
            },
        ) if matches!(op, Comparison::Equal) && lr == rr && lc == rc => (l, r),
        _ => return Err(Error::OperandShapeMismatch),
    };
    for (l, r) in lhs.iter().zip(rhs) {
        if !number::compare(l, r, b)?.is_eq() {
            return Ok(Value::Truth { value: false });
        }
    }
    Ok(Value::Truth { value: true })
}
/// Equal kinds and dimensions only; no scalar broadcasting or truth coercion.
/// O(N) rational operations and O(N) result storage.
pub fn additive(
    left: &CheckedExactValue<'_>,
    right: &CheckedExactValue<'_>,
    op: Additive,
    b: &mut Budget,
) -> Result<Value, Error> {
    start(b)?;
    let f = match op {
        Additive::Add => number::add,
        Additive::Subtract => number::subtract,
    };
    let (lhs, rhs, dimensions) = match (left.value(), right.value()) {
        (Value::Scalar { value: l }, Value::Scalar { value: r }) => {
            return Ok(Value::Scalar { value: f(l, r, b)? });
        }
        (Value::Vector { values: l }, Value::Vector { values: r }) if l.len() == r.len() => {
            (l, r, None)
        }
        (
            Value::Matrix {
                rows: lr,
                cols: lc,
                values: l,
            },
            Value::Matrix {
                rows: rr,
                cols: rc,
                values: r,
            },
        ) if lr == rr && lc == rc => (l, r, Some((*lr, *lc))),
        _ => return Err(Error::OperandShapeMismatch),
    };
    let mut values = storage(lhs.len(), b)?;
    for (l, r) in lhs.iter().zip(rhs) {
        values.push(f(l, r, b)?);
    }
    Ok(match dimensions {
        Some((rows, cols)) => Value::Matrix { rows, cols, values },
        None => Value::Vector { values },
    })
}

/// Scalar scaling on either side, matrix-matrix and matrix-vector product.
/// Vector-vector and vector-matrix multiplication have no implicit meaning.
/// An m*k by k*n product uses O(m*k*n) rational operations, O(m*n) storage.
pub fn multiply(
    left: &CheckedExactValue<'_>,
    right: &CheckedExactValue<'_>,
    b: &mut Budget,
) -> Result<Value, Error> {
    start(b)?;
    match (left.value(), right.value()) {
        (Value::Scalar { value }, other) => {
            return map(other, b, |r, b| number::multiply(value, r, b));
        }
        (other, Value::Scalar { value }) => {
            return map(other, b, |l, b| number::multiply(l, value, b));
        }
        _ => {}
    }
    let (rows, inner, cols, lhs, rhs, vector) = match (left.value(), right.value()) {
        (
            Value::Matrix {
                rows,
                cols: inner,
                values: l,
            },
            Value::Matrix {
                rows: rr,
                cols,
                values: r,
            },
        ) if inner == rr => (*rows as usize, *inner as usize, *cols as usize, l, r, false),
        (
            Value::Matrix {
                rows,
                cols: inner,
                values: l,
            },
            Value::Vector { values: r },
        ) if *inner == r.len() as u64 => (*rows as usize, *inner as usize, 1, l, r, true),
        _ => return Err(Error::OperandShapeMismatch),
    };
    // Checked nonempty shapes guarantee representable input offsets. Result
    // dimensions need their own check: both inputs fitting is not sufficient.
    let count = rows
        .checked_mul(cols)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    let mut values = storage(count, b)?;
    for row in 0..rows {
        for col in 0..cols {
            let mut sum = number::multiply(&lhs[row * inner], &rhs[col], b)?;
            for k in 1..inner {
                let product = number::multiply(&lhs[row * inner + k], &rhs[k * cols + col], b)?;
                sum = number::add(&sum, &product, b)?;
            }
            values.push(sum);
        }
    }
    Ok(if vector {
        Value::Vector { values }
    } else {
        Value::Matrix {
            rows: rows as u64,
            cols: cols as u64,
            values,
        }
    })
}
