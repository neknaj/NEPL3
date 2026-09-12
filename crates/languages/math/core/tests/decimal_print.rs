use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    value::{Integer, Rational},
};
use nepl3_math_core::number::{self, DecimalPrintError};

fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10000,
        ..Limits::default()
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn q(n: i64, d: i64) -> Result<Rational, String> {
    number::ratio(&Integer::from(n), &Integer::from(d), &mut budget()).map_err(err)
}

#[test]
fn canonical_decimal_examples_and_exact_reparse() -> Result<(), String> {
    // Independent positional-decimal expectations, including zeros before the first digit.
    for (n, d, text) in [
        (0, 1, "0"),
        (42, 1, "42"),
        (-7, 1, "-7"),
        (1, 2, "0.5"),
        (-25, 8, "-3.125"),
        (3, 125, "0.024"),
        (100, 40, "2.5"),
        (1, 1048576, "0.00000095367431640625"),
    ] {
        let mut b = budget();
        assert_eq!(number::print_decimal(&q(n, d)?, &mut b), Ok(text.into()));
        assert_eq!(b.usage().output_bytes, text.len() as u64);
    }
    let huge = Integer::from_bigint(num_bigint::BigInt::from(1_u32) << 128_usize);
    let huge = number::ratio(&huge, &Integer::from(1_i64), &mut budget()).map_err(err)?;
    assert_eq!(
        number::print_decimal(&huge, &mut budget()).map_err(err)?,
        "340282366920938463463374607431768211456"
    );
    for a in 0..7 {
        for b in 0..6 {
            for n in [-31, -1, 0, 1, 17] {
                let value = q(n, 2_i64.pow(a) * 5_i64.pow(b))?;
                let text = number::print_decimal(&value, &mut budget()).map_err(err)?;
                let unsigned = text.strip_prefix('-').unwrap_or(&text);
                let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
                assert!(!fraction.ends_with('0'));
                assert!(integer == "0" || !integer.starts_with('0'));
                assert!(!text.contains(['e', 'E', '+']));
                let reparsed = Rational::from_decimal_parts(
                    text.starts_with('-'),
                    integer,
                    fraction,
                    &mut budget(),
                )
                .map_err(err)?;
                assert_eq!(reparsed, value);
            }
        }
    }
    for d in [3, 7, 12, 30] {
        assert_eq!(
            number::print_decimal(&q(1, d)?, &mut budget()),
            Err(DecimalPrintError::NonFiniteDecimal)
        );
    }
    Ok(())
}

#[test]
fn decimal_print_stops_without_mutating_input_or_resetting_budget() -> Result<(), String> {
    let value = q(-1, 1048576)?;
    let before = value.clone();
    let mut full = budget();
    let text = number::print_decimal(&value, &mut full).map_err(err)?;
    let used = full.usage();
    for (reason, amount) in [
        (StopReason::WorkLimit, used.work),
        (StopReason::AllocationLimit, used.allocation_units),
        (StopReason::OutputLimit, used.output_bytes),
    ] {
        for cap in [0, amount / 2, amount - 1] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = cap,
                StopReason::AllocationLimit => limits.allocation_units = cap,
                _ => limits.output_bytes = cap,
            }
            let mut b = Budget::new(limits);
            assert_eq!(
                number::print_decimal(&value, &mut b),
                Err(DecimalPrintError::Stopped(reason))
            );
            assert_eq!(b.poll(), Err(reason));
            assert_eq!(value, before);
        }
    }
    let mut limits = budget().limits();
    limits.output_bytes = text.len() as u64;
    assert_eq!(
        number::print_decimal(&value, &mut Budget::new(limits)),
        Ok(text)
    );
    let mut b = budget();
    b.cancel();
    assert_eq!(
        number::print_decimal(&value, &mut b),
        Err(DecimalPrintError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}
