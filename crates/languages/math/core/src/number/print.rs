use super::{bytes, charge, finite_decimal};
use alloc::string::String;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value::Rational,
};
use num_traits::Zero;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecimalPrintError {
    NonFiniteDecimal,
    Stopped(StopReason),
}
impl From<StopReason> for DecimalPrintError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Print a canonical Math Number without exponents or redundant trailing zeros.
/// Non-finite decimals are rejected, not rounded or replaced by another form.
/// The input is immutable. Output bytes are charged before insertion; temporary
/// integer work and storage use the numeric module's conservative accounting.
/// For B input bits and D fractional digits, the digit loop uses O(D*B) work
/// (at most nine B-bit subtractions per digit); its metering charges O(D*B²).
/// Factorization, initial integer division and radix conversion have separate costs.
/// Storage is O(B+D), including the finite-decimal output bound reserved upfront.
pub fn print_decimal(value: &Rational, budget: &mut Budget) -> Result<String, DecimalPrintError> {
    if !finite_decimal(value, budget)? {
        return Err(DecimalPrintError::NonFiniteDecimal);
    }
    let numerator = value.numerator().as_bigint();
    let denominator = value.denominator();
    // For denominator 2^a*5^b, fractional digits max(a,b) <= denominator bits.
    // Integer decimal digits are at most max(1, numerator bits).
    let capacity = numerator
        .bits()
        .max(1)
        .checked_add(denominator.bits())
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, capacity)?;
    let capacity =
        usize::try_from(capacity).map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    let mut output = String::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    charge(bytes(value), budget)?;
    let whole = numerator.magnitude() / denominator;
    charge(bytes(value), budget)?;
    let mut remainder = numerator.magnitude() % denominator;
    charge(whole.bits().div_ceil(8), budget)?;
    let digits = whole.to_str_radix(10);
    if value.numerator().is_negative() {
        append(&mut output, "-", budget)?;
    }
    append(&mut output, &digits, budget)?;
    if !remainder.is_zero() {
        append(&mut output, ".", budget)?;
    }
    while !remainder.is_zero() {
        charge(denominator.bits().saturating_add(4).div_ceil(8), budget)?;
        remainder *= 10_u32;
        let mut digit = b'0';
        // The previous remainder is below denominator, so at most nine steps.
        while &remainder >= denominator {
            budget.charge(
                Resource::Work,
                denominator.bits().div_ceil(8).saturating_add(1),
            )?;
            remainder -= denominator;
            digit += 1;
        }
        budget.charge(Resource::OutputBytes, 1)?;
        output.push(char::from(digit));
    }
    Ok(output)
}
fn append(output: &mut String, text: &str, budget: &mut Budget) -> Result<(), StopReason> {
    budget.charge(Resource::Work, text.len() as u64)?;
    budget.charge(Resource::OutputBytes, text.len() as u64)?;
    output.push_str(text);
    Ok(())
}
