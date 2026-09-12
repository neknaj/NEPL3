use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    value::Integer,
};
use nepl3_math_core::{
    check, environment,
    evaluation::{self, Outcome, Reason},
    exact::arithmetic,
    model::*,
    number,
};
fn b() -> Budget {
    Budget::new(Limits {
        work: 100000000,
        allocation_units: 100000000,
        nodes: 100000,
        depth: 10000,
        ..Limits::default()
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn q(n: i64, d: i64) -> Result<nepl3_core::value::Rational, String> {
    number::ratio(&Integer::from(n), &Integer::from(d), &mut b()).map_err(err)
}
fn n(value: i64) -> Result<MathKind, String> {
    Ok(MathKind::Number {
        value: q(value, 1)?,
        spelling: None,
    })
}
fn model(kinds: Vec<MathKind>) -> MathValue {
    MathValue {
        root: MathRoot::Expr(ExprRef(0)),
        nodes: kinds
            .into_iter()
            .map(|kind| MathNode {
                kind,
                origin: None,
                span: None,
                locations: vec![],
            })
            .collect(),
        embeds: vec![],
    }
}
fn run(v: &MathValue, env: &BindingEnvironment) -> Result<Outcome, String> {
    let input = check::expression(v, &mut b()).map_err(err)?;
    let env = environment::check(env, &mut b()).map_err(err)?;
    let result = evaluation::evaluate(&input, &env, &mut b()).map_err(err)?;
    assert!(core::ptr::eq(result.source, v));
    Ok(result.outcome)
}
fn empty() -> BindingEnvironment {
    BindingEnvironment {
        assignments: vec![],
    }
}
fn scalar(n: i64) -> Result<Outcome, String> {
    Ok(Outcome::Exact(MathExactValue::Scalar { value: q(n, 1)? }))
}
#[test]
fn lexical_evaluation_revisits_shared_nodes_and_scopes_sum_indices() -> Result<(), String> {
    let value = model(vec![
        MathKind::Let {
            name: "x".into(),
            init: ExprRef(1),
            body: ExprRef(2),
        },
        n(2)?,
        MathKind::Add {
            left: ExprRef(3),
            right: ExprRef(4),
        },
        MathKind::Symbol { name: "x".into() },
        MathKind::Sum {
            index: "x".into(),
            lower: ExprRef(5),
            upper: ExprRef(6),
            body: ExprRef(3),
        },
        n(1)?,
        n(3)?,
    ]);
    assert_eq!(run(&value, &empty())?, scalar(8)?);
    let env = BindingEnvironment {
        assignments: vec![MathAssignment {
            name: "x".into(),
            value: MathExactValue::Scalar { value: q(2, 1)? },
        }],
    };
    let sum = model(vec![
        MathKind::Sum {
            index: "x".into(),
            lower: ExprRef(1),
            upper: ExprRef(2),
            body: ExprRef(1),
        },
        MathKind::Symbol { name: "x".into() },
        n(3)?,
    ]);
    assert_eq!(run(&sum, &env)?, scalar(5)?);
    let init = model(vec![
        MathKind::Let {
            name: "x".into(),
            init: ExprRef(1),
            body: ExprRef(1),
        },
        MathKind::Symbol { name: "x".into() },
    ]);
    assert_eq!(run(&init, &env)?, scalar(2)?);
    assert!(
        matches!(run(&init,&empty())?,Outcome::Symbolic(rs) if rs[0].reason==Reason::MissingSymbol)
    );
    Ok(())
}
#[test]
fn exact_and_symbolic_domains_keep_original_notation() -> Result<(), String> {
    for (kind, leaf, reason) in [
        (
            MathKind::Sqrt { value: ExprRef(1) },
            n(2)?,
            Reason::AlgebraicValueRequired,
        ),
        (
            MathKind::Sqrt { value: ExprRef(1) },
            n(-1)?,
            Reason::ComplexValueRequired,
        ),
    ] {
        let value = model(vec![kind, leaf]);
        let before = value.clone();
        assert_eq!(
            run(&value, &empty())?,
            Outcome::Symbolic(vec![evaluation::Requirement {
                expression: ExprRef(0),
                reason
            }])
        );
        assert_eq!(value, before);
    }
    let power = model(vec![
        MathKind::Pow {
            left: ExprRef(1),
            right: ExprRef(2),
        },
        n(2)?,
        MathKind::Frac {
            left: ExprRef(3),
            right: ExprRef(1),
        },
        n(1)?,
    ]);
    assert!(
        matches!(run(&power,&empty())?,Outcome::Symbolic(rs) if rs[0].reason==Reason::NonIntegralExponent)
    );
    let root = model(vec![
        MathKind::Root {
            degree: ExprRef(1),
            radicand: ExprRef(2),
        },
        n(3)?,
        n(-8)?,
    ]);
    assert_eq!(run(&root, &empty())?, scalar(-2)?);
    let notation = model(vec![
        MathKind::Call {
            function: ExprRef(1),
            arguments: vec![ExprRef(2)],
        },
        MathKind::Symbol { name: "f".into() },
        MathKind::Frac {
            left: ExprRef(3),
            right: ExprRef(4),
        },
        n(1)?,
        n(0)?,
    ]);
    assert!(
        matches!(run(&notation,&empty())?,Outcome::Symbolic(rs) if rs[0].reason==Reason::NotationOnly)
    );
    let matrix = model(vec![
        MathKind::Det { value: ExprRef(1) },
        MathKind::Matrix {
            rows: vec![RowRef(2), RowRef(3)],
        },
        MathKind::Row {
            values: vec![ExprRef(4), ExprRef(5)],
        },
        MathKind::Row {
            values: vec![ExprRef(6), ExprRef(7)],
        },
        n(1)?,
        n(2)?,
        n(3)?,
        n(4)?,
    ]);
    assert_eq!(run(&matrix, &empty())?, scalar(-2)?);
    Ok(())
}
#[test]
fn errors_are_ordered_and_empty_sums_do_not_evaluate_body() -> Result<(), String> {
    let value = model(vec![
        MathKind::Add {
            left: ExprRef(1),
            right: ExprRef(2),
        },
        MathKind::Frac {
            left: ExprRef(3),
            right: ExprRef(4),
        },
        MathKind::Det { value: ExprRef(3) },
        n(1)?,
        n(0)?,
    ]);
    let input = check::expression(&value, &mut b()).map_err(err)?;
    let raw = empty();
    let env = environment::check(&raw, &mut b()).map_err(err)?;
    assert!(matches!(
        evaluation::evaluate(&input, &env, &mut b()),
        Err(evaluation::Error::At {
            expression: ExprRef(1),
            error: arithmetic::Error::Arithmetic(number::ArithmeticError::DivisionByZero)
        })
    ));
    let sum = model(vec![
        MathKind::Sum {
            index: "i".into(),
            lower: ExprRef(1),
            upper: ExprRef(2),
            body: ExprRef(3),
        },
        n(1)?,
        n(0)?,
        MathKind::Frac {
            left: ExprRef(1),
            right: ExprRef(2),
        },
    ]);
    assert_eq!(run(&sum, &empty())?, scalar(0)?);
    let sum = model(vec![
        MathKind::Sum {
            index: "i".into(),
            lower: ExprRef(1),
            upper: ExprRef(1),
            body: ExprRef(2),
        },
        n(1)?,
        MathKind::Vector {
            values: vec![ExprRef(1)],
        },
    ]);
    assert!(
        matches!(run(&sum,&empty())?,Outcome::Symbolic(rs) if rs[0].reason==Reason::UnsupportedExactDomain)
    );
    Ok(())
}
#[test]
fn evaluation_uses_one_sticky_budget_and_preserves_input() -> Result<(), String> {
    let value = model(vec![
        MathKind::Sum {
            index: "i".into(),
            lower: ExprRef(1),
            upper: ExprRef(2),
            body: ExprRef(3),
        },
        n(1)?,
        n(8)?,
        MathKind::Symbol { name: "i".into() },
    ]);
    let before = value.clone();
    let input = check::expression(&value, &mut b()).map_err(err)?;
    let raw = empty();
    let env = environment::check(&raw, &mut b()).map_err(err)?;
    let mut measured = b();
    assert_eq!(
        evaluation::evaluate(&input, &env, &mut measured)
            .map_err(err)?
            .outcome,
        scalar(36)?
    );
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
    ] {
        for half in [true, false] {
            let mut limits = b().limits();
            let usage = measured.usage();
            let cap = |v: u64| if half { v / 2 } else { v - 1 };
            match reason {
                StopReason::WorkLimit => limits.work = cap(usage.work),
                StopReason::AllocationLimit => {
                    limits.allocation_units = cap(usage.allocation_units)
                }
                StopReason::NodeLimit => limits.nodes = cap(usage.nodes),
                _ => limits.depth = cap(usage.depth),
            }
            let mut budget = Budget::new(limits);
            assert!(
                matches!(evaluation::evaluate(&input,&env,&mut budget),Err(evaluation::Error::Stopped(r)) if r==reason)
            );
            assert_eq!(budget.poll(), Err(reason));
        }
    }
    let mut budget = b();
    budget.cancel();
    assert!(matches!(
        evaluation::evaluate(&input, &env, &mut budget),
        Err(evaluation::Error::Stopped(StopReason::Cancelled))
    ));
    assert_eq!(value, before);
    Ok(())
}

#[test]
fn known_errors_survive_symbolic_operands_and_nested_scopes_restore() -> Result<(), String> {
    for (kind, left, right, error) in [
        (
            MathKind::Frac {
                left: ExprRef(1),
                right: ExprRef(2),
            },
            MathKind::Symbol {
                name: "missing".into(),
            },
            n(0)?,
            number::ArithmeticError::DivisionByZero,
        ),
        (
            MathKind::Root {
                degree: ExprRef(1),
                radicand: ExprRef(2),
            },
            n(-1)?,
            MathKind::Symbol {
                name: "missing".into(),
            },
            number::ArithmeticError::InvalidRootDegree,
        ),
    ] {
        let value = model(vec![kind, left, right]);
        let input = check::expression(&value, &mut b()).map_err(err)?;
        let raw = empty();
        let env = environment::check(&raw, &mut b()).map_err(err)?;
        assert!(
            matches!(evaluation::evaluate(&input,&env,&mut b()),Err(evaluation::Error::At {expression:ExprRef(0),error:arithmetic::Error::Arithmetic(e)}) if e==error)
        );
    }
    // Read the outer x AFTER the inner sum has restored its binding.
    let value = model(vec![
        MathKind::Let {
            name: "x".into(),
            init: ExprRef(1),
            body: ExprRef(2),
        },
        n(10)?,
        MathKind::Add {
            left: ExprRef(3),
            right: ExprRef(4),
        },
        MathKind::Sum {
            index: "x".into(),
            lower: ExprRef(5),
            upper: ExprRef(6),
            body: ExprRef(4),
        },
        MathKind::Symbol { name: "x".into() },
        n(1)?,
        n(2)?,
    ]);
    assert_eq!(run(&value, &empty())?, scalar(13)?);
    // sum i=1..2 (sum j=1..2 (i+j)) = 12, with one shared outer index node.
    let nested = model(vec![
        MathKind::Sum {
            index: "i".into(),
            lower: ExprRef(1),
            upper: ExprRef(2),
            body: ExprRef(3),
        },
        n(1)?,
        n(2)?,
        MathKind::Sum {
            index: "j".into(),
            lower: ExprRef(1),
            upper: ExprRef(2),
            body: ExprRef(4),
        },
        MathKind::Add {
            left: ExprRef(5),
            right: ExprRef(6),
        },
        MathKind::Symbol { name: "i".into() },
        MathKind::Symbol { name: "j".into() },
    ]);
    assert_eq!(run(&nested, &empty())?, scalar(12)?);
    Ok(())
}

#[test]
fn wide_expression_storage_work_grows_linearly() -> Result<(), String> {
    let mut previous = None;
    for count in [128, 256, 512] {
        let value = model(vec![
            MathKind::Vector {
                values: vec![ExprRef(1); count],
            },
            n(1)?,
        ]);
        let input = check::expression(&value, &mut b()).map_err(err)?;
        let raw = empty();
        let env = environment::check(&raw, &mut b()).map_err(err)?;
        let mut measured = b();
        assert!(
            matches!(evaluation::evaluate(&input,&env,&mut measured).map_err(err)?.outcome,Outcome::Exact(MathExactValue::Vector {values}) if values.len()==count)
        );
        if let Some(old) = previous {
            assert!(measured.usage().work < old * 3);
        }
        previous = Some(measured.usage().work);
    }
    Ok(())
}
