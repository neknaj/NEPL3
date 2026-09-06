use nepl3_core::{
    budget::*,
    value::{Integer, Rational, decimal::DecimalError},
};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: 64,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: 100,
        events: 100,
    })
}
#[test]
fn exact_decimal_constructor_is_independent_of_lexical_leading_zero_policy()
-> Result<(), DecimalError> {
    let v = Rational::from_decimal_parts(true, "0012", "340", &mut budget())?;
    assert_eq!(v.numerator().as_bigint().to_string(), "-617");
    assert_eq!(v.denominator().to_string(), "50");
    let zero = Rational::from_decimal_parts(true, "0", "000", &mut budget())?;
    assert_eq!(zero.numerator().as_bigint().to_string(), "0");
    assert_eq!(zero.denominator().to_string(), "1");
    let digits = "1234567890".repeat(30);
    assert_eq!(
        Integer::from_decimal_digits(&digits, &mut budget())?
            .as_bigint()
            .to_string(),
        digits
    );
    Ok(())
}
#[test]
fn invalid_digits_and_resource_stops_precede_numeric_allocation() {
    for (integer, fraction) in [("", ""), ("-1", ""), ("1", "e2"), ("１", ""), ("1", "_")] {
        assert_eq!(
            Rational::from_decimal_parts(false, integer, fraction, &mut budget()),
            Err(DecimalError::InvalidDigits)
        );
    }
    for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
        let mut limits = budget().limits();
        if reason == StopReason::WorkLimit {
            limits.work = 0;
        } else {
            limits.allocation_units = 0;
        }
        let mut b = Budget::new(limits);
        assert_eq!(
            Integer::from_decimal_digits("123", &mut b),
            Err(DecimalError::Stopped(reason))
        );
        assert_eq!(b.poll(), Err(reason));
        assert_eq!(b.usage().allocation_units, 0);
    }
}
