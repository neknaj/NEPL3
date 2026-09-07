//! Exact arithmetic on the common canonical Rational. These helpers do not
//! decide expression domains or replace the author's notation.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value::{Integer, Rational},
};
use num_bigint::BigInt;
use num_traits::{One, Zero};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithmeticError {
    DivisionByZero,
    Stopped(StopReason),
}
impl From<StopReason> for ArithmeticError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

fn bytes(value: &Rational) -> u64 {
    value
        .numerator()
        .as_bigint()
        .bits()
        .saturating_add(value.denominator().bits())
        .div_ceil(8)
}
fn charge(size: u64, b: &mut Budget) -> Result<(), StopReason> {
    let n = size
        .checked_add(1)
        .ok_or_else(|| b.stop(StopReason::WorkLimit))?;
    let work = n
        .checked_mul(n)
        .and_then(|v| v.checked_mul(64))
        .ok_or_else(|| b.stop(StopReason::WorkLimit))?;
    b.charge(Resource::Work, work)?;
    let allocation = n
        .checked_mul(128)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, allocation)
}

/// A reduced positive denominator has a finite decimal expansion exactly when
/// its prime factors are only 2 and 5. This is the Math:Number constraint, not a
/// restriction on evaluation values of type Rational.
pub fn finite_decimal(value: &Rational, b: &mut Budget) -> Result<bool, StopReason> {
    b.poll()?;
    charge(bytes(value), b)?;
    let mut denominator = value.denominator().clone();
    for factor in [2_u32, 5] {
        loop {
            charge(denominator.bits().div_ceil(8), b)?;
            if denominator.is_one() || !(&denominator % factor).is_zero() {
                break;
            }
            denominator /= factor;
        }
    }
    Ok(denominator.is_one())
}
pub fn is_zero(value: &Rational, b: &mut Budget) -> Result<bool, StopReason> {
    b.charge(Resource::Work, 1)?;
    Ok(value.numerator().as_bigint().is_zero())
}
pub fn is_integer(value: &Rational, b: &mut Budget) -> Result<bool, StopReason> {
    b.charge(Resource::Work, 1)?;
    Ok(value.denominator().is_one())
}

pub fn clone_with_budget(value: &Rational, b: &mut Budget) -> Result<Rational, StopReason> {
    charge(bytes(value), b)?;
    Ok(value.clone())
}
pub(crate) fn integer_components(
    value: &Rational,
    b: &mut Budget,
) -> Result<(Rational, Rational), ArithmeticError> {
    charge(bytes(value), b)?;
    let numerator = Rational::new(value.numerator().as_bigint().clone(), BigInt::one())
        .map_err(|_| ArithmeticError::DivisionByZero)?;
    let denominator = Rational::new(BigInt::from(value.denominator().clone()), BigInt::one())
        .map_err(|_| ArithmeticError::DivisionByZero)?;
    Ok((numerator, denominator))
}

/// Normalizes sign and gcd after charging the integer construction and reduction.
pub fn ratio(
    numerator: &Integer,
    denominator: &Integer,
    b: &mut Budget,
) -> Result<Rational, ArithmeticError> {
    b.charge(Resource::Work, 1)?;
    if denominator.as_bigint().is_zero() {
        return Err(ArithmeticError::DivisionByZero);
    }
    charge(
        numerator
            .as_bigint()
            .bits()
            .saturating_add(denominator.as_bigint().bits())
            .div_ceil(8),
        b,
    )?;
    Rational::new(
        numerator.as_bigint().clone(),
        denominator.as_bigint().clone(),
    )
    .map_err(|_| ArithmeticError::DivisionByZero)
}

#[derive(Clone, Copy)]
enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
}
fn binary(
    left: &Rational,
    right: &Rational,
    op: Binary,
    b: &mut Budget,
) -> Result<Rational, ArithmeticError> {
    b.charge(Resource::Work, 1)?;
    if matches!(op, Binary::Divide) && right.numerator().as_bigint().is_zero() {
        return Err(ArithmeticError::DivisionByZero);
    }
    charge(bytes(left).saturating_add(bytes(right)), b)?;
    let ln = left.numerator().as_bigint();
    let rn = right.numerator().as_bigint();
    let ld = BigInt::from(left.denominator().clone());
    let rd = BigInt::from(right.denominator().clone());
    let (numerator, denominator) = match op {
        Binary::Add => (ln * &rd + rn * &ld, ld * rd),
        Binary::Subtract => (ln * &rd - rn * &ld, ld * rd),
        Binary::Multiply => (ln * rn, ld * rd),
        Binary::Divide => (ln * rd, ld * rn),
    };
    Rational::new(numerator, denominator).map_err(|_| ArithmeticError::DivisionByZero)
}
pub fn add(left: &Rational, right: &Rational, b: &mut Budget) -> Result<Rational, ArithmeticError> {
    binary(left, right, Binary::Add, b)
}
pub fn subtract(
    left: &Rational,
    right: &Rational,
    b: &mut Budget,
) -> Result<Rational, ArithmeticError> {
    binary(left, right, Binary::Subtract, b)
}
pub fn multiply(
    left: &Rational,
    right: &Rational,
    b: &mut Budget,
) -> Result<Rational, ArithmeticError> {
    binary(left, right, Binary::Multiply, b)
}
pub fn divide(
    left: &Rational,
    right: &Rational,
    b: &mut Budget,
) -> Result<Rational, ArithmeticError> {
    binary(left, right, Binary::Divide, b)
}
pub fn negate(value: &Rational, b: &mut Budget) -> Result<Rational, ArithmeticError> {
    charge(bytes(value), b)?;
    Rational::new(
        -value.numerator().as_bigint(),
        BigInt::from(value.denominator().clone()),
    )
    .map_err(|_| ArithmeticError::DivisionByZero)
}
