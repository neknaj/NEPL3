use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    value::Integer,
};
use nepl3_math_core::{
    construct::expression_from_rational,
    model::{ExprRef, MathKind, MathRoot},
    number::{self, ArithmeticError},
};

fn budget(depth: u64) -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        nodes: 100,
        depth,
        ..Limits::default()
    })
}

#[test]
fn fresh_rational_notation_obeys_structure_depth_and_caller() -> Result<(), ArithmeticError> {
    // A terminating decimal needs one Number node. A nonterminating rational
    // needs a Frac with two integer leaves, hence three nodes and depth two.
    for (denominator, depth, nodes) in [(8_i64, 1, 1), (3, 2, 3)] {
        let value = number::ratio(
            &Integer::from(-5_i64),
            &Integer::from(denominator),
            &mut budget(10),
        )?;
        let original = value.clone();
        let mut too_small = budget(depth - 1);
        assert_eq!(
            expression_from_rational(&value, &mut too_small),
            Err(ArithmeticError::Stopped(StopReason::DepthLimit))
        );
        assert_eq!(too_small.poll(), Err(StopReason::DepthLimit));
        let mut enough = budget(depth);
        let expression = expression_from_rational(&value, &mut enough)?;
        assert_eq!(expression.nodes.len(), nodes);
        assert_eq!(enough.usage().depth, depth);
        assert_eq!(enough.current_depth(), 0);
        assert_eq!(value, original);
        if denominator == 8 {
            assert_eq!(expression.root, MathRoot::Expr(ExprRef(0)));
            assert_eq!(
                expression.nodes[0].kind,
                MathKind::Number {
                    value: original.clone(),
                    spelling: None,
                }
            );
        } else {
            assert_eq!(expression.root, MathRoot::Expr(ExprRef(2)));
            assert_eq!(
                expression.nodes[2].kind,
                MathKind::Frac {
                    left: ExprRef(0),
                    right: ExprRef(1),
                }
            );
            for (index, integer) in [(0, -5_i64), (1, 3)] {
                assert_eq!(
                    expression.nodes[index].kind,
                    MathKind::Number {
                        value: number::ratio(
                            &Integer::from(integer),
                            &Integer::from(1_i64),
                            &mut budget(10)
                        )?,
                        spelling: None,
                    }
                );
            }
        }
        assert!(expression.validate_shape(&mut budget(10)).is_ok());

        let mut nested = budget(7 + depth);
        let result = nested.with_depth_at_least(7, |b| expression_from_rational(&value, b))?;
        assert_eq!(result, expression);
        assert_eq!(nested.usage().depth, 7 + depth);
        assert_eq!(nested.current_depth(), 0);
        let mut short = budget(7 + depth - 1);
        assert_eq!(
            short.with_depth_at_least(7, |b| expression_from_rational(&value, b)),
            Err(ArithmeticError::Stopped(StopReason::DepthLimit))
        );
        assert_eq!(short.current_depth(), 0);
    }
    Ok(())
}
