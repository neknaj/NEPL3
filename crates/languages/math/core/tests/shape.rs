use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    value::Integer,
};
use nepl3_math_core::{
    check::{Category, ShapeError},
    model::*,
    number,
};

fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        depth: 200_000,
        ..Limits::default()
    })
}
fn value(root: MathRoot, kinds: Vec<MathKind>) -> MathValue {
    MathValue {
        root,
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
fn text() -> MathKind {
    MathKind::Text { text: "x".into() }
}

#[test]
fn notation_shapes_enforce_matrix_vector_fence_and_root_constraints() -> Result<(), String> {
    let bad = [
        (
            value(
                MathRoot::Expr(ExprRef(0)),
                vec![MathKind::Vector { values: vec![] }],
            ),
            ShapeError::EmptyVector(0),
        ),
        (
            value(
                MathRoot::Expr(ExprRef(0)),
                vec![MathKind::Matrix { rows: vec![] }],
            ),
            ShapeError::EmptyMatrix(0),
        ),
        (
            value(
                MathRoot::Expr(ExprRef(1)),
                vec![
                    MathKind::Row { values: vec![] },
                    MathKind::Matrix {
                        rows: vec![RowRef(0)],
                    },
                ],
            ),
            ShapeError::MatrixWidth { node: 1, row: 0 },
        ),
        (
            value(
                MathRoot::Expr(ExprRef(1)),
                vec![
                    text(),
                    MathKind::Fence {
                        open: "ab".into(),
                        close: "".into(),
                        value: ExprRef(0),
                    },
                ],
            ),
            ShapeError::FenceWidth(1),
        ),
        (
            value(
                MathRoot::Expr(ExprRef(1)),
                vec![
                    MathKind::Row { values: vec![] },
                    MathKind::Sqrt { value: ExprRef(0) },
                ],
            ),
            ShapeError::Category {
                node: 0,
                expected: Category::Expr,
            },
        ),
    ];
    for (v, expected) in bad {
        assert_eq!(v.validate_shape(&mut budget()).err(), Some(expected));
    }
    let one = number::ratio(&Integer::from(1_i64), &Integer::from(1_i64), &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let zero = number::ratio(&Integer::from(0_i64), &Integer::from(1_i64), &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let third = number::ratio(&Integer::from(1_i64), &Integer::from(3_i64), &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let bad_number = value(
        MathRoot::Expr(ExprRef(0)),
        vec![MathKind::Number {
            value: third,
            spelling: None,
        }],
    );
    assert_eq!(
        bad_number.validate_shape(&mut budget()).err(),
        Some(ShapeError::NonFiniteDecimalNumber(0))
    );
    let bad_degree = value(
        MathRoot::Expr(ExprRef(2)),
        vec![
            MathKind::Number {
                value: zero,
                spelling: None,
            },
            MathKind::Number {
                value: one,
                spelling: None,
            },
            MathKind::Root {
                degree: ExprRef(0),
                radicand: ExprRef(1),
            },
        ],
    );
    assert_eq!(
        bad_degree.validate_shape(&mut budget()).err(),
        Some(ShapeError::InvalidRootDegree(2))
    );
    let allowed = value(
        MathRoot::Expr(ExprRef(1)),
        vec![
            text(),
            MathKind::Fence {
                open: "🙂".into(),
                close: "".into(),
                value: ExprRef(0),
            },
        ],
    );
    allowed
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}
#[test]
fn shared_paths_and_deep_cleanup_keep_limits_and_categories() -> Result<(), ShapeError> {
    let shared = value(
        MathRoot::Expr(ExprRef(2)),
        vec![
            text(),
            MathKind::Sqrt { value: ExprRef(0) },
            MathKind::Add {
                left: ExprRef(0),
                right: ExprRef(1),
            },
        ],
    );
    let mut b = Budget::new(Limits {
        depth: 2,
        ..budget().limits()
    });
    assert_eq!(
        shared.validate_shape(&mut b).err(),
        Some(ShapeError::Stopped(StopReason::DepthLimit))
    );
    let mut b = budget();
    b.with_depth_at_least(7, |b| shared.validate_shape(b))?;
    assert_eq!(b.usage().depth, 10);
    assert_eq!(b.current_depth(), 0);
    let mut kinds = vec![text()];
    for id in 0..100_000 {
        kinds.push(MathKind::Neg { value: ExprRef(id) });
    }
    let deep = value(MathRoot::Expr(ExprRef(100_000)), kinds);
    deep.validate_shape(&mut budget())?;
    let cloned = deep.clone();
    assert_eq!(cloned, deep);
    let mut b = Budget::new(Limits {
        depth: 64,
        ..budget().limits()
    });
    assert_eq!(
        deep.validate_shape(&mut b).err(),
        Some(ShapeError::Stopped(StopReason::DepthLimit))
    );
    assert_eq!(b.poll(), Err(StopReason::DepthLimit));
    let cycle = value(
        MathRoot::Expr(ExprRef(0)),
        vec![MathKind::Neg { value: ExprRef(0) }],
    );
    assert_eq!(
        cycle.validate_shape(&mut budget()).err(),
        Some(ShapeError::Cycle(0))
    );
    Ok(())
}

#[test]
fn rational_constructor_uses_number_only_for_finite_decimals() -> Result<(), String> {
    for (n, d, finite) in [
        (1_i64, 3_i64, false),
        (-1, 3, false),
        (1, 8, true),
        (0, 7, true),
    ] {
        let rational = number::ratio(&Integer::from(n), &Integer::from(d), &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let expression =
            nepl3_math_core::construct::expression_from_rational(&rational, &mut budget())
                .map_err(|e| format!("{e:?}"))?;
        expression
            .validate_shape(&mut budget())
            .map_err(|e| format!("{e:?}"))?;
        if finite {
            assert_eq!(expression.root, MathRoot::Expr(ExprRef(0)));
            assert_eq!(
                expression.nodes[0].kind,
                MathKind::Number {
                    value: rational,
                    spelling: None
                }
            );
        } else {
            assert_eq!(expression.root, MathRoot::Expr(ExprRef(2)));
            assert_eq!(
                expression.nodes[2].kind,
                MathKind::Frac {
                    left: ExprRef(0),
                    right: ExprRef(1)
                }
            );
            let (MathKind::Number { value: left, .. }, MathKind::Number { value: right, .. }) =
                (&expression.nodes[0].kind, &expression.nodes[1].kind)
            else {
                return Err("integer fraction components".into());
            };
            assert!(number::is_integer(left, &mut budget()).map_err(|e| format!("{e:?}"))?);
            assert!(number::is_integer(right, &mut budget()).map_err(|e| format!("{e:?}"))?);
            assert_eq!(
                number::divide(left, right, &mut budget()).map_err(|e| format!("{e:?}"))?,
                rational
            );
        }
    }
    Ok(())
}
