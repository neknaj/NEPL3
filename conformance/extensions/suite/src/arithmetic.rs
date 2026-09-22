//! Exact signed integer semantics. Source attribution belongs to the adapter.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value::Integer,
};

/// Typed operands distinguish unary negation from the two binary operations.
pub enum Application<'a> {
    Neg(&'a Integer),
    Add(&'a Integer, &'a Integer),
    Mul(&'a Integer, &'a Integer),
}

/// Compute only after charging a conservative logical bound for integer work
/// and temporary storage. Resource exhaustion is an explicit stop; integers
/// have no fixed-width overflow or truncation.
pub fn apply(application: Application<'_>, budget: &mut Budget) -> Result<Integer, StopReason> {
    budget.poll()?;
    let bits = match &application {
        Application::Neg(value) => value.as_bigint().bits(),
        Application::Add(left, right) | Application::Mul(left, right) => left
            .as_bigint()
            .bits()
            .checked_add(right.as_bigint().bits())
            .ok_or_else(|| budget.stop(StopReason::WorkLimit))?,
    };
    // Sum of operand bit lengths bounds output size for addition/multiplication.
    // One extra byte covers carry/sign and gives zero a nonzero operation cost.
    let bytes = bits / 8 + 1;
    let work = bytes
        .checked_mul(bytes)
        .and_then(|n| n.checked_mul(64))
        .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
    let storage = bytes
        .checked_mul(64)
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::Work, work)?;
    budget.charge(Resource::AllocationUnits, storage)?;
    Ok(Integer::from_bigint(match application {
        Application::Neg(value) => -value.as_bigint(),
        Application::Add(left, right) => left.as_bigint() + right.as_bigint(),
        Application::Mul(left, right) => left.as_bigint() * right.as_bigint(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use external_hello_language::budget;

    #[test]
    fn exact_signed_operations_and_large_values() -> Result<(), StopReason> {
        let seven = Integer::from(7_i64);
        let two = Integer::from(2_i64);
        let negative = apply(Application::Neg(&seven), &mut budget())?;
        assert_eq!(negative, Integer::from(-7_i64));
        assert_eq!(
            apply(Application::Add(&negative, &two), &mut budget())?,
            Integer::from(-5_i64)
        );
        assert_eq!(
            apply(Application::Mul(&negative, &two), &mut budget())?,
            Integer::from(-14_i64)
        );
        let large = Integer::from(u64::MAX);
        let square = apply(Application::Mul(&large, &large), &mut budget())?;
        // (2^64 - 1)^2 = 2^128 - 2^65 + 1, represented independently as bytes.
        let expected = Integer::from_canonical(
            false,
            &[
                255, 255, 255, 255, 255, 255, 255, 254, 0, 0, 0, 0, 0, 0, 0, 1,
            ],
        );
        assert_eq!(Ok(square), expected);
        assert_eq!(
            apply(Application::Neg(&Integer::from(0_i64)), &mut budget())?,
            Integer::from(0_i64)
        );
        Ok(())
    }

    #[test]
    fn resource_stops_precede_arithmetic() {
        let value = Integer::from(7_i64);
        let mut limits = budget().limits();
        limits.work = 0;
        let mut work = Budget::new(limits);
        assert_eq!(
            apply(Application::Neg(&value), &mut work),
            Err(StopReason::WorkLimit)
        );
        let mut limits = budget().limits();
        limits.allocation_units = 0;
        let mut allocation = Budget::new(limits);
        assert_eq!(
            apply(Application::Mul(&value, &value), &mut allocation),
            Err(StopReason::AllocationLimit)
        );
        let mut cancelled = budget();
        cancelled.stop(StopReason::Cancelled);
        let before = cancelled.usage();
        assert_eq!(
            apply(Application::Add(&value, &value), &mut cancelled),
            Err(StopReason::Cancelled)
        );
        assert_eq!(cancelled.usage(), before);
    }
}
