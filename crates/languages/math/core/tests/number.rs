//! Specification 06 numeric controls; these exercise the arithmetic substrate,
//! not the yet separate expression evaluator.
use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    value::{Integer, Rational},
};
use nepl3_math_core::number::{self, ArithmeticError};

fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        ..Limits::default()
    })
}
fn q(n: i64, d: i64) -> Result<Rational, ArithmeticError> {
    number::ratio(&Integer::from(n), &Integer::from(d), &mut budget())
}
#[test]
fn canonical_rationals_keep_exact_decimal_and_fraction_values() -> Result<(), ArithmeticError> {
    assert_eq!(number::add(&q(1, 2)?, &q(1, 3)?, &mut budget())?, q(5, 6)?);
    assert_eq!(
        number::add(&q(1, 10)?, &q(1, 5)?, &mut budget())?,
        q(3, 10)?
    );
    assert_eq!(
        number::subtract(&q(2, 3)?, &q(7, 6)?, &mut budget())?,
        q(-1, 2)?
    );
    assert_eq!(
        number::multiply(&q(21, 10)?, &q(-5, 7)?, &mut budget())?,
        q(-3, 2)?
    );
    assert_eq!(
        number::divide(&q(21, 10)?, &q(-7, 5)?, &mut budget())?,
        q(-3, 2)?
    );
    assert_eq!(number::negate(&q(-3, 2)?, &mut budget())?, q(3, 2)?);
    let large = Integer::from_bigint(num_bigint::BigInt::from(1_u64) << 128_usize);
    let value = number::ratio(&large, &Integer::from(1_i64), &mut budget())?;
    let plus = number::add(&value, &q(1, 1)?, &mut budget())?;
    assert_eq!(number::subtract(&plus, &value, &mut budget())?, q(1, 1)?);
    for (n, d, finite) in [
        (0, 7, true),
        (-25, 8, true),
        (3, 125, true),
        (7, 12, false),
        (1, 3, false),
    ] {
        assert_eq!(number::finite_decimal(&q(n, d)?, &mut budget())?, finite);
    }
    assert!(number::is_integer(&q(9, 3)?, &mut budget())?);
    assert!(!number::is_integer(&q(9, 4)?, &mut budget())?);
    assert!(number::is_zero(&q(0, 17)?, &mut budget())?);
    Ok(())
}
#[test]
fn arithmetic_stops_before_big_integer_work_and_retains_operands() -> Result<(), ArithmeticError> {
    let left = q(123456789, 125)?;
    let right = q(-987654321, 64)?;
    let originals = (left.clone(), right.clone());
    assert_eq!(
        number::divide(&left, &q(0, 1)?, &mut budget()),
        Err(ArithmeticError::DivisionByZero)
    );
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = budget().limits();
        if reason == StopReason::WorkLimit {
            limits.work = 0;
        }
        if reason == StopReason::AllocationLimit {
            limits.allocation_units = 0;
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        assert_eq!(
            number::add(&left, &right, &mut b),
            Err(ArithmeticError::Stopped(reason))
        );
        assert_eq!(b.poll(), Err(reason));
        assert_eq!((&left, &right), (&originals.0, &originals.1));
    }
    Ok(())
}
