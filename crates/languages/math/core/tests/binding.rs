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
fn free_requirements_group_exact_names_and_exclude_bound_occurrences() -> Result<(), String> {
    let v = value(vec![
        MathKind::Let {
            name: "x".into(),
            init: ExprRef(1),
            body: ExprRef(2),
        },
        symbol("x"),
        MathKind::Add {
            left: ExprRef(1),
            right: ExprRef(3),
        },
        MathKind::Add {
            left: ExprRef(4),
            right: ExprRef(4),
        },
        symbol("A"),
    ]);
    let input =
        nepl3_math_core::check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let report =
        nepl3_math_core::free::symbols(&input, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        report.symbols,
        vec![
            MathFreeSymbol {
                name: "A".into(),
                occurrences: vec![5, 6]
            },
            MathFreeSymbol {
                name: "x".into(),
                occurrences: vec![1]
            },
        ]
    );
    for allocation in [true, false] {
        let mut limits = budget().limits();
        if allocation {
            limits.allocation_units = 0;
        } else {
            limits.work = 0;
        }
        let mut b = Budget::new(limits);
        let reason = if allocation {
            StopReason::AllocationLimit
        } else {
            StopReason::WorkLimit
        };
        assert!(matches!(nepl3_math_core::free::symbols(&input, &mut b),
            Err(nepl3_math_core::check::ShapeError::Stopped(actual)) if actual == reason));
        assert_eq!(b.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn free_names_sort_without_normalization_and_stop_inside_sort() -> Result<(), String> {
    let names = ["z", "é", "e\u{301}", "a", "A", "z"];
    let mut kinds = vec![MathKind::Vector {
        values: (1..=names.len()).map(|i| ExprRef(i as u64)).collect(),
    }];
    kinds.extend(names.into_iter().map(symbol));
    let v = value(kinds);
    let input =
        nepl3_math_core::check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut full = budget();
    let report = nepl3_math_core::free::symbols(&input, &mut full).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        report
            .symbols
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "a", "e\u{301}", "z", "é"]
    );
    assert_eq!(report.symbols[3].occurrences, vec![1, 6]);
    for cap in 0..full.usage().work {
        let mut limits = budget().limits();
        limits.work = cap;
        let mut b = Budget::new(limits);
        assert!(matches!(
            nepl3_math_core::free::symbols(&input, &mut b),
            Err(nepl3_math_core::check::ShapeError::Stopped(
                StopReason::WorkLimit
            ))
        ));
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    }
    Ok(())
}

#[test]
fn non_binding_depth_does_not_multiply_symbol_lookup_work() -> Result<(), String> {
    let mut previous = None;
    for count in [64, 128, 256] {
        let mut kinds = Vec::new();
        for index in 0..count {
            kinds.push(MathKind::Add {
                left: ExprRef(2 * index + 1),
                right: ExprRef(2 * index + 2),
            });
            kinds.push(symbol("free"));
        }
        kinds.push(symbol("free"));
        let v = value(kinds);
        let shape = v
            .validate_shape(&mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let mut b = budget();
        let report = binding::analyze(&shape, &mut b).map_err(|e| format!("{e:?}"))?;
        assert!(report.definitions.is_empty());
        assert_eq!(report.uses.len() as u64, count + 1);
        assert!(report.uses.iter().all(|usage| usage.binding.is_none()));
        // Doubling this skew tree doubles visits. No binder exists, so scanning
        // its increasingly deep arithmetic frames would be quadratic overhead.
        if let Some(work) = previous {
            assert!(b.usage().work < 3 * work);
        }
        previous = Some(b.usage().work);
    }
    Ok(())
}

#[test]
fn checked_expression_keeps_free_symbols_and_rejects_non_expressions() -> Result<(), String> {
    use nepl3_math_core::check::{self, Category, ShapeError};
    let v = value(vec![symbol("free")]);
    let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(core::ptr::eq(checked.value(), &v));
    assert_eq!(checked.bindings().uses[0].binding, None);
    let mut row = value(vec![MathKind::Row { values: vec![] }]);
    row.root = MathRoot::Row(RowRef(0));
    assert!(matches!(
        check::expression(&row, &mut budget()),
        Err(ShapeError::Category {
            node: 0,
            expected: Category::Expr
        })
    ));
    let cycle = value(vec![MathKind::Add {
        left: ExprRef(0),
        right: ExprRef(0),
    }]);
    assert!(matches!(
        check::expression(&cycle, &mut budget()),
        Err(ShapeError::Cycle(0))
    ));
    Ok(())
}

#[test]
fn checked_expression_shares_shape_and_binding_budget() -> Result<(), String> {
    use nepl3_math_core::check::{self, ShapeError};
    let v = value(vec![symbol("x")]);
    let mut measured = budget();
    v.validate_shape(&mut measured)
        .map_err(|e| format!("{e:?}"))?;
    let mut limits = budget().limits();
    limits.nodes = measured.usage().nodes;
    let mut limited = Budget::new(limits);
    // Shape alone fits; visiting the symbol for binding must use the same cap.
    assert!(matches!(
        check::expression(&v, &mut limited),
        Err(ShapeError::Stopped(StopReason::NodeLimit))
    ));
    assert_eq!(limited.poll(), Err(StopReason::NodeLimit));
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(matches!(
        check::expression(&v, &mut cancelled),
        Err(ShapeError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
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
