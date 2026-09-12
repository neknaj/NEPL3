use super::*;
use crate::exact;
use crate::model::MathExactValue as V;

pub(super) fn apply(
    node: ExprRef,
    kind: &MathKind,
    operands: Vec<Outcome>,
    b: &mut Budget,
) -> Result<Outcome, Error> {
    // Unknown operands cannot erase independently established domain failures.
    for (index, operand) in operands.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let Outcome::Exact(value) = operand else {
            continue;
        };
        let scalar_only = matches!(
            kind,
            MathKind::Frac { .. }
                | MathKind::Pow { .. }
                | MathKind::Root { .. }
                | MathKind::Sqrt { .. }
                | MathKind::Lt { .. }
                | MathKind::Le { .. }
                | MathKind::Vector { .. }
                | MathKind::Row { .. }
        );
        if scalar_only && !matches!(value, V::Scalar { .. }) {
            return Err(at(node, arithmetic::Error::OperandShapeMismatch));
        }
        if matches!(
            kind,
            MathKind::Add { .. }
                | MathKind::Sub { .. }
                | MathKind::Mul { .. }
                | MathKind::Neg { .. }
        ) && matches!(value, V::Truth { .. })
        {
            return Err(at(node, arithmetic::Error::OperandShapeMismatch));
        }
        if let V::Scalar { value } = value {
            if matches!(kind, MathKind::Frac { .. }) && index == 1 && number::is_zero(value, b)? {
                return Err(numeric(node, number::ArithmeticError::DivisionByZero));
            }
            if matches!(kind, MathKind::Root { .. })
                && index == 0
                && (!number::is_integer(value, b)?
                    || value.numerator().as_bigint() <= &num_bigint::BigInt::from(0_u64))
            {
                return Err(numeric(node, number::ArithmeticError::InvalidRootDegree));
            }
        }
    }
    let mut requirements = Vec::new();
    let mut values = Vec::new();
    for operand in operands {
        match operand {
            Outcome::Exact(v) => push(&mut values, v, b)?,
            Outcome::Symbolic(rs) => {
                for r in rs {
                    push(&mut requirements, r, b)?;
                }
            }
        }
    }
    if !requirements.is_empty() {
        return Ok(Outcome::Symbolic(requirements));
    }
    let mismatch = || at(node, arithmetic::Error::OperandShapeMismatch);
    if matches!(
        kind,
        MathKind::Vector { .. } | MathKind::Row { .. } | MathKind::Matrix { .. }
    ) {
        let mut out = Vec::new();
        let rows = values.len() as u64;
        let mut cols = None;
        for value in values {
            match (kind, value) {
                (MathKind::Matrix { .. }, V::Vector { values }) => {
                    if cols.is_some_and(|n| n != values.len() as u64) {
                        return Err(mismatch());
                    }
                    cols = Some(values.len() as u64);
                    for value in values {
                        push(&mut out, value, b)?;
                    }
                }
                (MathKind::Vector { .. } | MathKind::Row { .. }, V::Scalar { value }) => {
                    push(&mut out, value, b)?
                }
                _ => return Err(mismatch()),
            }
        }
        return Ok(Outcome::Exact(if matches!(kind, MathKind::Matrix { .. }) {
            V::Matrix {
                rows,
                cols: cols.ok_or(Error::InvalidState)?,
                values: out,
            }
        } else {
            V::Vector { values: out }
        }));
    }
    if matches!(kind, MathKind::Fence { .. } | MathKind::Label { .. }) {
        return Ok(Outcome::Exact(pop(&mut values)?));
    }
    let left = values.first().ok_or(Error::InvalidState)?;
    let checked = exact::check(left, b).map_err(|e| match e {
        exact::ExactValueError::Stopped(r) => Error::Stopped(r),
        _ => Error::InvalidState,
    })?;
    let unary = match kind {
        MathKind::Neg { .. } => Some(arithmetic::negate(&checked, b)),
        MathKind::Transpose { .. } => Some(arithmetic::transpose(&checked, b)),
        MathKind::Det { .. } => Some(arithmetic::determinant(&checked, b)),
        _ => None,
    };
    if let Some(result) = unary {
        return result.map(Outcome::Exact).map_err(|e| at(node, e));
    }
    if matches!(
        kind,
        MathKind::Sqrt { .. } | MathKind::Root { .. } | MathKind::Pow { .. }
    ) {
        let V::Scalar { value: l } = left else {
            return Err(mismatch());
        };
        let result = match kind {
            MathKind::Sqrt { .. } => number::root_integer(l, &Integer::from(2_i64), b),
            MathKind::Root { .. } => {
                if !number::is_integer(l, b)?
                    || l.numerator().as_bigint() <= &num_bigint::BigInt::from(0_u64)
                {
                    return Err(numeric(node, number::ArithmeticError::InvalidRootDegree));
                }
                let Some(V::Scalar { value: r }) = values.get(1) else {
                    return Err(mismatch());
                };
                number::root_integer(r, l.numerator(), b)
            }
            MathKind::Pow { .. } => {
                let Some(V::Scalar { value: r }) = values.get(1) else {
                    return Err(mismatch());
                };
                if !number::is_integer(r, b)? {
                    return symbolic(node, Reason::NonIntegralExponent, b);
                }
                return number::pow_integer(l, r.numerator(), b)
                    .map(|value| Outcome::Exact(V::Scalar { value }))
                    .map_err(|e| numeric(node, e));
            }
            _ => return Err(Error::InvalidState),
        }
        .map_err(|e| numeric(node, e))?;
        return match result {
            number::RootValue::Exact(value) => Ok(Outcome::Exact(V::Scalar { value })),
            number::RootValue::AlgebraicValueRequired => {
                symbolic(node, Reason::AlgebraicValueRequired, b)
            }
            number::RootValue::ComplexValueRequired => {
                symbolic(node, Reason::ComplexValueRequired, b)
            }
        };
    }
    let right = values.get(1).ok_or(Error::InvalidState)?;
    let rhs = exact::check(right, b).map_err(|e| match e {
        exact::ExactValueError::Stopped(r) => Error::Stopped(r),
        _ => Error::InvalidState,
    })?;
    let result = match kind {
        MathKind::Add { .. } => arithmetic::additive(&checked, &rhs, arithmetic::Additive::Add, b),
        MathKind::Sub { .. } => {
            arithmetic::additive(&checked, &rhs, arithmetic::Additive::Subtract, b)
        }
        MathKind::Mul { .. } => arithmetic::multiply(&checked, &rhs, b),
        MathKind::Frac { .. } => arithmetic::divide(&checked, &rhs, b),
        MathKind::Equal { .. } => {
            arithmetic::compare(&checked, &rhs, arithmetic::Comparison::Equal, b)
        }
        MathKind::Lt { .. } => arithmetic::compare(&checked, &rhs, arithmetic::Comparison::Less, b),
        MathKind::Le { .. } => {
            arithmetic::compare(&checked, &rhs, arithmetic::Comparison::LessEqual, b)
        }
        _ => return Err(Error::InvalidState),
    };
    result.map(Outcome::Exact).map_err(|e| at(node, e))
}
