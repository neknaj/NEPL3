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

#[test]
fn scalar_order_and_integer_powers_use_exact_arithmetic() -> Result<(), ArithmeticError> {
    use core::cmp::Ordering::{Equal, Greater, Less};
    // Independent small rational values, including sign and reciprocal rules.
    for (a, c, expected) in [
        ((1, 3), (2, 6), Equal),
        ((-1, 3), (-1, 2), Greater),
        ((2, 3), (3, 4), Less),
        ((0, 1), (-1, 100), Greater),
    ] {
        assert_eq!(
            number::compare(&q(a.0, a.1)?, &q(c.0, c.1)?, &mut budget())?,
            expected
        );
    }
    for (n, d, exponent, expected_n, expected_d) in [
        (0, 1, 0_i64, 1, 1),
        (0, 1, 3, 0, 1),
        (-2, 3, 3, -8, 27),
        (-2, 3, -3, -27, 8),
        (2, 1, -3, 1, 8),
        (-2, 1, 4, 16, 1),
    ] {
        assert_eq!(
            number::pow_integer(&q(n, d)?, &Integer::from(exponent), &mut budget())?,
            q(expected_n, expected_d)?
        );
    }
    assert_eq!(
        number::pow_integer(&q(0, 1)?, &Integer::from(-1_i64), &mut budget()),
        Err(ArithmeticError::DivisionByZero)
    );
    // Exponents are arbitrary precision, not truncated to a machine-sized count.
    let even = num_bigint::BigInt::from(1_u64) << 100_usize;
    let odd = &even + 1_u32;
    for (e, expected) in [(even, 1), (odd, -1)] {
        assert_eq!(
            number::pow_integer(&q(-1, 1)?, &Integer::from_bigint(e), &mut budget())?,
            q(expected, 1)?
        );
    }
    // Adjacent >64-bit integers would compare equal after f64 conversion.
    let large = num_bigint::BigInt::from(1_u64) << 100_usize;
    let a = number::ratio(
        &Integer::from_bigint(large.clone()),
        &Integer::from(1_i64),
        &mut budget(),
    )?;
    let c = number::ratio(
        &Integer::from_bigint(large + 1_u32),
        &Integer::from(1_i64),
        &mut budget(),
    )?;
    assert_eq!(number::compare(&a, &c, &mut budget())?, Less);
    Ok(())
}

#[test]
fn powers_and_comparison_stop_without_mutation() -> Result<(), ArithmeticError> {
    let base = q(2, 3)?;
    let exponent = Integer::from(-13_i64);
    let before = (base.clone(), exponent.clone());
    let mut measured = budget();
    number::pow_integer(&base, &exponent, &mut measured)?;
    for (reason, used) in [
        (StopReason::WorkLimit, measured.usage().work),
        (
            StopReason::AllocationLimit,
            measured.usage().allocation_units,
        ),
    ] {
        for cap in [0, used / 2, used - 1] {
            let mut limits = budget().limits();
            if reason == StopReason::WorkLimit {
                limits.work = cap;
            } else {
                limits.allocation_units = cap;
            }
            let mut b = Budget::new(limits);
            assert_eq!(
                number::pow_integer(&base, &exponent, &mut b),
                Err(ArithmeticError::Stopped(reason))
            );
            assert_eq!(b.poll(), Err(reason));
        }
        let mut limits = budget().limits();
        if reason == StopReason::WorkLimit {
            limits.work = 0;
        } else {
            limits.allocation_units = 0;
        }
        assert_eq!(
            number::compare(&base, &base, &mut Budget::new(limits)),
            Err(reason)
        );
    }
    let mut b = budget();
    b.cancel();
    assert_eq!(
        number::pow_integer(&base, &Integer::from(0_i64), &mut b),
        Err(ArithmeticError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(
        number::compare(&base, &base, &mut b),
        Err(StopReason::Cancelled)
    );
    assert_eq!((base, exponent), before);
    Ok(())
}

#[test]
fn rational_roots_distinguish_exact_algebraic_and_complex() -> Result<(), ArithmeticError> {
    use number::RootValue::{AlgebraicValueRequired, ComplexValueRequired, Exact};
    // An independent small-integer oracle enumerates possible roots; it does
    // not use the production bisection or rational exponentiation routines.
    for degree in 2_u32..=6 {
        for value in 0_i64..=64 {
            let root = (0_i64..=value)
                .find(|candidate| i128::from(*candidate).pow(degree) == i128::from(value));
            let expected = match root {
                Some(v) => Exact(q(v, 1)?),
                None => AlgebraicValueRequired,
            };
            assert_eq!(
                number::root_integer(
                    &q(value, 1)?,
                    &Integer::from(u64::from(degree)),
                    &mut budget()
                )?,
                expected,
                "value={value} degree={degree}"
            );
        }
    }
    for (n, d, degree, expected) in [
        (4, 9, 2_i64, Exact(q(2, 3)?)),
        (-8, 27, 3, Exact(q(-2, 3)?)),
        (2, 9, 2, AlgebraicValueRequired),
        (4, 3, 2, AlgebraicValueRequired),
        (-4, 9, 2, ComplexValueRequired),
        (-2, 1, 3, AlgebraicValueRequired),
        (-13, 17, 1, Exact(q(-13, 17)?)),
    ] {
        assert_eq!(
            number::root_integer(&q(n, d)?, &Integer::from(degree), &mut budget())?,
            expected
        );
    }
    for degree in [0_i64, -1, -2] {
        assert_eq!(
            number::root_integer(&q(0, 1)?, &Integer::from(degree), &mut budget()),
            Err(ArithmeticError::InvalidRootDegree)
        );
    }
    let huge = num_bigint::BigInt::from(1_u64) << 100_usize;
    for (n, degree, expected) in [
        (0, huge.clone(), Exact(q(0, 1)?)),
        (1, huge.clone(), Exact(q(1, 1)?)),
        (2, huge.clone(), AlgebraicValueRequired),
        (-1, huge.clone(), ComplexValueRequired),
        (-1, huge + 1_u32, Exact(q(-1, 1)?)),
    ] {
        assert_eq!(
            number::root_integer(&q(n, 1)?, &Integer::from_bigint(degree), &mut budget())?,
            expected
        );
    }
    Ok(())
}

#[test]
fn root_resource_stop_is_never_a_symbolic_result() -> Result<(), ArithmeticError> {
    let value = q(81, 625)?;
    let degree = Integer::from(4_i64);
    let originals = (value.clone(), degree.clone());
    let mut measured = budget();
    assert_eq!(
        number::root_integer(&value, &degree, &mut measured)?,
        number::RootValue::Exact(q(3, 5)?)
    );
    for (reason, used) in [
        (StopReason::WorkLimit, measured.usage().work),
        (
            StopReason::AllocationLimit,
            measured.usage().allocation_units,
        ),
    ] {
        for cap in [0, used / 2, used - 1] {
            let mut limits = budget().limits();
            if reason == StopReason::WorkLimit {
                limits.work = cap;
            } else {
                limits.allocation_units = cap;
            }
            let mut b = Budget::new(limits);
            assert_eq!(
                number::root_integer(&value, &degree, &mut b),
                Err(ArithmeticError::Stopped(reason))
            );
            assert_eq!(b.poll(), Err(reason));
        }
    }
    let mut b = budget();
    b.cancel();
    assert_eq!(
        number::root_integer(&q(-1, 1)?, &Integer::from(2_i64), &mut b),
        Err(ArithmeticError::Stopped(StopReason::Cancelled))
    );
    assert_eq!((value, degree), originals);
    Ok(())
}
