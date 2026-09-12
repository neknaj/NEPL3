use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_math_core::{binding, model::*};

fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        nodes: 100_000,
        allocation_units: 10_000_000,
        depth: 10_000,
        ..Limits::default()
    })
}
fn value(kinds: Vec<MathKind>) -> MathValue {
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
fn symbol(name: &str) -> MathKind {
    MathKind::Symbol { name: name.into() }
}

#[test]
fn shared_node_has_free_and_bound_occurrences() -> Result<(), String> {
    // let x x x: init and body deliberately share a node, but only body binds.
    let v = value(vec![
        MathKind::Let {
            name: "x".into(),
            init: ExprRef(1),
            body: ExprRef(1),
        },
        symbol("x"),
    ]);
    let mut b = budget();
    let shape = v.validate_shape(&mut b).map_err(|e| format!("{e:?}"))?;
    let result = binding::analyze(&shape, &mut b).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        result.definitions,
        vec![MathBinding {
            occurrence: 0,
            node: ExprRef(0)
        }]
    );
    assert_eq!(
        result.uses,
        vec![
            MathSymbolUse {
                occurrence: 1,
                node: ExprRef(1),
                binding: None
            },
            MathSymbolUse {
                occurrence: 2,
                node: ExprRef(1),
                binding: Some(0)
            }
        ]
    );
    Ok(())
}

#[test]
fn bounds_use_outer_scope_and_shadowing_is_local_to_body() -> Result<(), String> {
    for integral in [false, true] {
        let nested = if integral {
            MathKind::Integral {
                index: "x".into(),
                lower: ExprRef(1),
                upper: ExprRef(1),
                body: ExprRef(1),
            }
        } else {
            MathKind::Sum {
                index: "x".into(),
                lower: ExprRef(1),
                upper: ExprRef(1),
                body: ExprRef(1),
            }
        };
        let v = value(vec![
            MathKind::Let {
                name: "x".into(),
                init: ExprRef(1),
                body: ExprRef(2),
            },
            symbol("x"),
            nested,
        ]);
        let mut b = budget();
        let shape = v.validate_shape(&mut b).map_err(|e| format!("{e:?}"))?;
        let report = binding::analyze(&shape, &mut b).map_err(|e| format!("{e:?}"))?;
        // preorder: let=0, init=1, sum/integral=2, lower=3, upper=4, body=5.
        assert_eq!(
            report.uses.iter().map(|v| v.binding).collect::<Vec<_>>(),
            vec![None, Some(0), Some(0), Some(2)]
        );
    }
    Ok(())
}

#[test]
fn exponential_shared_dag_is_stopped_per_occurrence() -> Result<(), String> {
    let mut nodes = Vec::new();
    for i in 0..25 {
        nodes.push(MathKind::Add {
            left: ExprRef(i + 1),
            right: ExprRef(i + 1),
        });
    }
    nodes.push(symbol("free"));
    let v = value(nodes);
    let shape = v
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut b = Budget::new(Limits {
        nodes: 100,
        work: 1_000_000,
        allocation_units: 10_000_000,
        depth: 10_000,
        ..Limits::default()
    });
    assert_eq!(binding::analyze(&shape, &mut b), Err(StopReason::NodeLimit));
    assert_eq!(binding::analyze(&shape, &mut b), Err(StopReason::NodeLimit));
    Ok(())
}

#[test]
fn limits_and_cancellation_stay_stopped() -> Result<(), String> {
    let v = value(vec![symbol("x")]);
    let shape = v
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::DepthLimit => limits.depth = 0,
            _ => {}
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        assert_eq!(binding::analyze(&shape, &mut b), Err(reason));
        assert_eq!(binding::analyze(&shape, &mut b), Err(reason));
        assert_eq!(b.current_depth(), 0);
    }
    Ok(())
}

#[test]
fn shared_binder_visits_are_distinct_and_do_not_leak_to_siblings() -> Result<(), String> {
    // Row( let x x x, let x x x, x ), with both let and symbol shared.
    let mut v = value(vec![
        MathKind::Row {
            values: vec![ExprRef(1), ExprRef(1), ExprRef(2)],
        },
        MathKind::Let {
            name: "x".into(),
            init: ExprRef(2),
            body: ExprRef(2),
        },
        symbol("x"),
    ]);
    v.root = MathRoot::Row(RowRef(0));
    let shape = v
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let report = binding::analyze(&shape, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        report
            .definitions
            .iter()
            .map(|d| d.occurrence)
            .collect::<Vec<_>>(),
        vec![1, 4]
    );
    assert_eq!(
        report
            .uses
            .iter()
            .map(|d| (d.occurrence, d.binding))
            .collect::<Vec<_>>(),
        vec![(2, None), (3, Some(1)), (5, None), (6, Some(4)), (7, None)]
    );
    Ok(())
}
