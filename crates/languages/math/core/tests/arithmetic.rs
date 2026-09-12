use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    value::{Integer, Rational},
};
use nepl3_math_core::{
    exact::{
        self,
        arithmetic::{self, Additive, Error},
    },
    model::MathExactValue as Value,
    number,
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 10000000,
        allocation_units: 10000000,
        nodes: 10000,
        ..Limits::default()
    })
}
fn q(n: i64) -> Result<Rational, String> {
    number::ratio(&Integer::from(n), &Integer::from(1_i64), &mut budget()).map_err(err)
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn vector(ns: &[i64]) -> Result<Value, String> {
    Ok(Value::Vector {
        values: ns.iter().map(|n| q(*n)).collect::<Result<_, _>>()?,
    })
}
fn matrix(rows: u64, cols: u64, ns: &[i64]) -> Result<Value, String> {
    Ok(Value::Matrix {
        rows,
        cols,
        values: ns.iter().map(|n| q(*n)).collect::<Result<_, _>>()?,
    })
}
fn product(a: &Value, c: &Value, b: &mut Budget) -> Result<Value, String> {
    arithmetic::multiply(
        &exact::check(a, &mut budget()).map_err(err)?,
        &exact::check(c, &mut budget()).map_err(err)?,
        b,
    )
    .map_err(err)
}
#[test]
fn row_major_products_scaling_and_additive_shapes() -> Result<(), String> {
    let a = matrix(2, 3, &[1, 2, 3, 4, 5, 6])?;
    let c = matrix(3, 2, &[7, 8, 9, 10, 11, 12])?;
    // Dot products: 1*7+2*9+3*11=58, etc. Not an implementation-derived golden.
    assert_eq!(
        product(&a, &c, &mut budget())?,
        matrix(2, 2, &[58, 64, 139, 154])?
    );
    assert_eq!(
        product(&a, &vector(&[2, -1, 3])?, &mut budget())?,
        vector(&[9, 21])?
    );
    let scale = Value::Scalar { value: q(-2)? };
    for value in [&a, &vector(&[1, 2])?, &Value::Scalar { value: q(3)? }] {
        assert_eq!(
            product(&scale, value, &mut budget())?,
            product(value, &scale, &mut budget())?
        );
    }
    assert_eq!(
        product(&scale, &a, &mut budget())?,
        matrix(2, 3, &[-2, -4, -6, -8, -10, -12])?
    );
    for value in [&a, &vector(&[1, 2])?, &Value::Scalar { value: q(3)? }] {
        let checked = exact::check(value, &mut budget()).map_err(err)?;
        let neg = arithmetic::negate(&checked, &mut budget()).map_err(err)?;
        let doubled =
            arithmetic::additive(&checked, &checked, Additive::Add, &mut budget()).map_err(err)?;
        assert_eq!(
            doubled,
            product(&Value::Scalar { value: q(2)? }, value, &mut budget())?
        );
        let zero = arithmetic::additive(&checked, &checked, Additive::Subtract, &mut budget())
            .map_err(err)?;
        assert_eq!(
            zero,
            arithmetic::additive(
                &checked,
                &exact::check(&neg, &mut budget()).map_err(err)?,
                Additive::Add,
                &mut budget()
            )
            .map_err(err)?
        );
    }
    // Non-square shapes with inner dimension 1 exercise the first-term path.
    assert_eq!(
        product(
            &matrix(2, 1, &[2, 3])?,
            &matrix(1, 2, &[4, 5])?,
            &mut budget()
        )?,
        matrix(2, 2, &[8, 10, 12, 15])?
    );
    Ok(())
}
#[test]
fn rejects_kind_and_dimension_coercions() -> Result<(), String> {
    for (a, c) in [
        (vector(&[1, 2])?, vector(&[1, 2])?),
        (vector(&[1, 2])?, matrix(2, 1, &[1, 2])?),
        (matrix(2, 1, &[1, 2])?, vector(&[1, 2])?),
        (Value::Truth { value: true }, Value::Scalar { value: q(1)? }),
    ] {
        let l = exact::check(&a, &mut budget()).map_err(err)?;
        let r = exact::check(&c, &mut budget()).map_err(err)?;
        assert_eq!(
            arithmetic::multiply(&l, &r, &mut budget()),
            Err(Error::OperandShapeMismatch)
        );
    }
    for (a, c) in [
        (vector(&[1])?, vector(&[1, 2])?),
        (matrix(1, 2, &[1, 2])?, matrix(2, 1, &[1, 2])?),
        (Value::Scalar { value: q(1)? }, vector(&[1])?),
        (Value::Truth { value: true }, Value::Truth { value: false }),
    ] {
        for op in [Additive::Add, Additive::Subtract] {
            assert_eq!(
                arithmetic::additive(
                    &exact::check(&a, &mut budget()).map_err(err)?,
                    &exact::check(&c, &mut budget()).map_err(err)?,
                    op,
                    &mut budget()
                ),
                Err(Error::OperandShapeMismatch)
            );
        }
    }
    assert_eq!(
        arithmetic::negate(
            &exact::check(&Value::Truth { value: true }, &mut budget()).map_err(err)?,
            &mut budget()
        ),
        Err(Error::OperandShapeMismatch)
    );
    Ok(())
}
#[test]
fn matrix_product_stops_atomically_and_charges_result_storage() -> Result<(), String> {
    let a = matrix(2, 2, &[1, 2, 3, 4])?;
    let before = a.clone();
    let checked = exact::check(&a, &mut budget()).map_err(err)?;
    let mut measured = budget();
    arithmetic::multiply(&checked, &checked, &mut measured).map_err(err)?;
    for (reason, used) in [
        (StopReason::WorkLimit, measured.usage().work),
        (
            StopReason::AllocationLimit,
            measured.usage().allocation_units,
        ),
        (StopReason::NodeLimit, measured.usage().nodes),
    ] {
        for cap in [0, used / 2, used - 1] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = cap,
                StopReason::AllocationLimit => limits.allocation_units = cap,
                _ => limits.nodes = cap,
            }
            let mut b = Budget::new(limits);
            assert_eq!(
                arithmetic::multiply(&checked, &checked, &mut b),
                Err(Error::Stopped(reason))
            );
            assert_eq!(b.poll(), Err(reason));
        }
    }
    let mut b = budget();
    b.cancel();
    assert_eq!(
        arithmetic::multiply(&checked, &checked, &mut b),
        Err(Error::Stopped(StopReason::Cancelled))
    );
    assert_eq!(
        arithmetic::negate(&checked, &mut b),
        Err(Error::Stopped(StopReason::Cancelled))
    );
    assert_eq!(
        arithmetic::additive(&checked, &checked, Additive::Add, &mut b),
        Err(Error::Stopped(StopReason::Cancelled))
    );
    assert_eq!(a, before);
    Ok(())
}
