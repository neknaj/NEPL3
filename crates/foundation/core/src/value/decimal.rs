//! Budgeted finite decimal constructors; lexical spelling policy belongs to the reader.
use super::{Integer, Rational};
use crate::budget::{Budget, Resource, StopReason};
use alloc::vec::Vec;
use num_bigint::BigInt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecimalError {
    InvalidDigits,
    Stopped(StopReason),
}
impl From<StopReason> for DecimalError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}

fn prepare(integer: &str, fraction: &str, budget: &mut Budget) -> Result<(), DecimalError> {
    let size = (integer.len() as u64)
        .checked_add(fraction.len() as u64)
        .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
    budget.charge(Resource::Work, size)?;
    if integer.is_empty()
        || !integer
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return Err(DecimalError::InvalidDigits);
    }
    // Conservative logical work covers radix conversion, power construction and reduction.
    // This is an operation budget, not an assertion about allocator internals or physical OOM.
    let work = size
        .checked_add(1)
        .and_then(|n| n.checked_mul(n))
        .and_then(|n| n.checked_mul(16))
        .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
    budget.charge(Resource::Work, work)?;
    let storage = size
        .checked_add(1)
        .and_then(|n| n.checked_mul(64))
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, storage)?;
    Ok(())
}

impl Integer {
    /// Parses unsigned ASCII digits exactly, after charging conversion work and storage.
    /// Leading zeros are accepted here; a language's lexical reader may reject them.
    pub fn from_decimal_digits(digits: &str, budget: &mut Budget) -> Result<Self, DecimalError> {
        prepare(digits, "", budget)?;
        BigInt::parse_bytes(digits.as_bytes(), 10)
            .map(Self::from_bigint)
            .ok_or(DecimalError::InvalidDigits)
    }
}
impl Rational {
    /// Constructs an exact finite decimal. An empty fraction denotes an integer spelling.
    pub fn from_decimal_parts(
        negative: bool,
        integer: &str,
        fraction: &str,
        budget: &mut Budget,
    ) -> Result<Self, DecimalError> {
        prepare(integer, fraction, budget)?;
        let mut digits = Vec::with_capacity(integer.len() + fraction.len());
        digits.extend_from_slice(integer.as_bytes());
        digits.extend_from_slice(fraction.as_bytes());
        let numerator = BigInt::parse_bytes(&digits, 10).ok_or(DecimalError::InvalidDigits)?;
        let mut denominator = Vec::with_capacity(fraction.len() + 1);
        denominator.push(b'1');
        denominator.resize(fraction.len() + 1, b'0');
        let denominator =
            BigInt::parse_bytes(&denominator, 10).ok_or(DecimalError::InvalidDigits)?;
        Self::new(if negative { -numerator } else { numerator }, denominator)
            .map_err(|_| DecimalError::InvalidDigits)
    }
}
