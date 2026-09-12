use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    value::Integer,
};
use nepl3_math_core::{
    exact::{self, ExactValueError},
    model::MathExactValue,
    number,
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 100000,
        nodes: 1000,
        allocation_units: 100000,
        ..Limits::default()
    })
}

#[test]
fn exact_shapes_allow_nondecimal_rationals_and_preserve_borrow() -> Result<(), String> {
    let q = number::ratio(&Integer::from(1_i64), &Integer::from(3_i64), &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(!number::finite_decimal(&q, &mut budget()).map_err(|e| format!("{e:?}"))?);
    for value in [
        MathExactValue::Scalar { value: q.clone() },
        MathExactValue::Vector {
            values: vec![q.clone()],
        },
        MathExactValue::Matrix {
            rows: 1,
            cols: 2,
            values: vec![q.clone(), q],
        },
        MathExactValue::Truth { value: false },
    ] {
        let checked = exact::check(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert!(core::ptr::eq(checked.value(), &value));
        let mut measured = budget();
        exact::check(&value, &mut measured).map_err(|e| format!("{e:?}"))?;
        for nodes in [true, false] {
            let mut limits = budget().limits();
            if nodes {
                limits.nodes = measured.usage().nodes - 1;
            } else {
                limits.work = measured.usage().work - 1;
            }
            let mut b = Budget::new(limits);
            let reason = if nodes {
                StopReason::NodeLimit
            } else {
                StopReason::WorkLimit
            };
            assert!(
                matches!(exact::check(&value, &mut b), Err(ExactValueError::Stopped(actual)) if actual == reason)
            );
            assert_eq!(b.poll(), Err(reason));
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            exact::check(&value, &mut cancelled),
            Err(ExactValueError::Stopped(StopReason::Cancelled))
        ));
    }
    Ok(())
}
#[test]
fn exact_shapes_reject_empty_mismatch_overflow_and_budget_stop() {
    for (value, error) in [
        (
            MathExactValue::Vector { values: vec![] },
            ExactValueError::EmptyVector,
        ),
        (
            MathExactValue::Matrix {
                rows: 0,
                cols: 2,
                values: vec![],
            },
            ExactValueError::EmptyMatrix,
        ),
        (
            MathExactValue::Matrix {
                rows: u64::MAX,
                cols: 2,
                values: vec![],
            },
            ExactValueError::MatrixDimensionsOverflow,
        ),
        (
            MathExactValue::Matrix {
                rows: 1,
                cols: 2,
                values: vec![],
            },
            ExactValueError::MatrixElementCount {
                expected: 2,
                actual: 0,
            },
        ),
    ] {
        assert!(matches!(exact::check(&value, &mut budget()), Err(actual) if actual == error));
    }
    let value = MathExactValue::Truth { value: true };
    let mut b = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        exact::check(&value, &mut b),
        Err(ExactValueError::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
}
