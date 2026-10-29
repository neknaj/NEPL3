use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_math_core::{
    check,
    model::{ExprRef, MathKind as K, MathNode, MathRoot, MathValue},
};
use nepl3_math_tex::{Error, Unsupported, render};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 1000,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
fn value(kinds: Vec<K>) -> MathValue {
    MathValue {
        root: MathRoot::Expr(ExprRef((kinds.len() - 1) as u64)),
        embeds: vec![],
        nodes: kinds
            .into_iter()
            .map(|kind| MathNode {
                kind,
                span: None,
                origin: None,
                locations: vec![],
            })
            .collect(),
    }
}
#[test]
fn grouping_and_shared_occurrences() -> Result<(), String> {
    let v = value(vec![
        K::Symbol { name: "x".into() },
        K::Sub {
            left: ExprRef(0),
            right: ExprRef(0),
        },
        K::Frac {
            left: ExprRef(1),
            right: ExprRef(0),
        },
    ]);
    let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let out = render(&checked, &mut budget()).map_err(|e| format!("{e:?}"))?;
    // A fraction must preserve its unevaluated subtracting numerator and denominator.
    assert_eq!(
        out.tex(),
        r"\frac{\mathord{\textit{x}}-\mathord{\textit{x}}}{\mathord{\textit{x}}}"
    );
    assert!(std::ptr::eq(out.source(), &v));
    assert_eq!(
        out.occurrences().iter().map(|r| r.node).collect::<Vec<_>>(),
        [2, 1, 0, 0, 0]
    );
    for range in out.occurrences() {
        assert!(
            out.tex()
                .get(range.start as usize..range.end as usize)
                .is_some()
        );
        assert!(range.start < range.end);
    }
    Ok(())
}
#[test]
fn literal_text_cannot_inject_commands() -> Result<(), String> {
    let v = value(vec![K::Text {
        text: "\\input{x}$%_&#^~--日本".into(),
    }]);
    let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let out = render(&checked, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(!out.tex().contains(r"\input"));
    assert!(out.tex().contains(r"\textbackslash{}input\{x\}"));
    assert!(
        out.tex()
            .contains(r"\$\%\_\&\#\textasciicircum{}\textasciitilde{}{-}{-}日本")
    );
    Ok(())
}
#[test]
fn unsupported_text_and_fences_are_explicit() -> Result<(), String> {
    for (v, reason) in [
        (
            value(vec![K::Text {
                text: "a\nb".into(),
            }]),
            Unsupported::ControlCharacter,
        ),
        (
            value(vec![
                K::Symbol { name: "x".into() },
                K::Fence {
                    open: "!".into(),
                    close: "".into(),
                    value: ExprRef(0),
                },
            ]),
            Unsupported::Delimiter,
        ),
    ] {
        let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert!(
            matches!(render(&checked, &mut budget()), Err(Error::Unsupported { reason: r, .. }) if r == reason)
        );
    }
    Ok(())
}
#[test]
fn stops_are_sticky_and_never_return_partial_tex() -> Result<(), String> {
    let v = value(vec![K::Symbol {
        name: "long".into(),
    }]);
    let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::OutputLimit => limits.output_bytes = 0,
            StopReason::NodeLimit => limits.nodes = 0,
            StopReason::DepthLimit => limits.depth = 0,
            _ => (),
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        assert!(matches!(render(&checked, &mut b), Err(Error::Stopped(s)) if s == reason));
        assert_eq!(b.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn sequence_unicode_and_quotes_keep_literal_contract() -> Result<(), String> {
    let v = value(vec![
        K::Text {
            text: "e\u{301}".into(),
        },
        K::Text {
            text: "日本--".into(),
        },
        K::Sequence {
            values: vec![ExprRef(0), ExprRef(1)],
        },
    ]);
    let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let out = render(&checked, &mut budget()).map_err(|e| format!("{e:?}"))?;
    // Sequence composes notation without inserting commas; combining marks stay
    // attached to their base, while ASCII dashes cannot become a TeX ligature.
    assert_eq!(out.tex(), "{\\text{e\u{301}}}{\\text{日本{-}{-}}}");
    for text in ["'", "`"] {
        let v = value(vec![K::Text { text: text.into() }]);
        let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert!(matches!(
            render(&checked, &mut budget()),
            Err(Error::Unsupported {
                reason: Unsupported::LiteralQuote,
                ..
            })
        ));
    }
    Ok(())
}

#[test]
fn deep_input_and_shared_expansion_obey_exact_output_ceiling() -> Result<(), String> {
    let mut kinds = vec![K::Symbol { name: "x".into() }];
    for i in 0..150 {
        kinds.push(K::Neg { value: ExprRef(i) });
    }
    let v = value(kinds);
    let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut measured = budget();
    let out = render(&checked, &mut measured).map_err(|e| format!("{e:?}"))?;
    assert_eq!(out.occurrences().len(), 151);
    assert_eq!(
        measured.usage().depth,
        151,
        "group wrappers are not semantic nodes"
    );
    let mut depth_limits = budget().limits();
    depth_limits.depth = 151;
    assert!(render(&checked, &mut Budget::new(depth_limits)).is_ok());
    depth_limits.depth = 150;
    assert!(matches!(
        render(&checked, &mut Budget::new(depth_limits)),
        Err(Error::Stopped(StopReason::DepthLimit))
    ));
    let mut work_limits = budget().limits();
    work_limits.work = measured.usage().work;
    assert!(render(&checked, &mut Budget::new(work_limits)).is_ok());
    work_limits.work -= 1;
    assert!(matches!(
        render(&checked, &mut Budget::new(work_limits)),
        Err(Error::Stopped(StopReason::WorkLimit))
    ));

    assert_eq!(measured.usage().output_bytes, out.tex().len() as u64);
    let mut limits = budget().limits();
    limits.output_bytes = out.tex().len() as u64;
    assert!(render(&checked, &mut Budget::new(limits)).is_ok());
    limits.output_bytes -= 1;
    assert!(matches!(
        render(&checked, &mut Budget::new(limits)),
        Err(Error::Stopped(StopReason::OutputLimit))
    ));

    let mut kinds = vec![K::Symbol { name: "x".into() }];
    for i in 0..12 {
        kinds.push(K::Sequence {
            values: vec![ExprRef(i), ExprRef(i)],
        });
    }
    let v = value(kinds);
    let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    // A compact DAG must not evade the emitted-occurrence/output budget.
    limits.output_bytes = 100;
    assert!(matches!(
        render(&checked, &mut Budget::new(limits)),
        Err(Error::Stopped(StopReason::OutputLimit))
    ));
    Ok(())
}

fn rendered(mut kinds: Vec<K>) -> Result<String, String> {
    // Test cases share a symbol pool; retain only the reachable checked arena.
    fn refs(k: &mut K, mut f: impl FnMut(&mut ExprRef)) -> Result<(), String> {
        match k {
            K::Symbol { .. } => (),
            K::Add { left, right }
            | K::Sub { left, right }
            | K::Mul { left, right }
            | K::Pow { left, right }
            | K::Equal { left, right } => {
                f(left);
                f(right);
            }
            K::Neg { value } | K::Fence { value, .. } => f(value),
            K::Call {
                function,
                arguments,
            } => {
                f(function);
                for arg in arguments {
                    f(arg);
                }
            }
            K::Integral {
                lower, upper, body, ..
            }
            | K::Sum {
                lower, upper, body, ..
            } => {
                f(lower);
                f(upper);
                f(body);
            }
            _ => return Err("extend test arena remapper for this constructor".into()),
        }
        Ok(())
    }
    let mut used = vec![false; kinds.len()];
    let mut stack = vec![kinds.len() - 1];
    while let Some(i) = stack.pop() {
        if used[i] {
            continue;
        }
        used[i] = true;
        refs(&mut kinds[i], |r| stack.push(r.0 as usize))?;
    }
    let mut map = vec![0; kinds.len()];
    let mut next = 0;
    for (i, keep) in used.iter().enumerate() {
        if *keep {
            map[i] = next;
            next += 1;
        }
    }
    let mut compact = Vec::new();
    for (i, mut kind) in kinds.into_iter().enumerate() {
        if used[i] {
            refs(&mut kind, |r| r.0 = map[r.0 as usize])?;
            compact.push(kind);
        }
    }
    let v = value(compact);
    let checked = check::expression(&v, &mut budget()).map_err(|e| format!("{e:?}"))?;
    Ok(render(&checked, &mut budget())
        .map_err(|e| format!("{e:?}"))?
        .tex()
        .into())
}
fn symbols() -> Vec<K> {
    ["a", "b", "c"]
        .into_iter()
        .map(|s| K::Symbol { name: s.into() })
        .collect()
}
#[test]
fn operator_context_preserves_required_grouping() -> Result<(), String> {
    for (child, parent, required) in [
        (
            K::Sub {
                left: ExprRef(1),
                right: ExprRef(2),
            },
            K::Sub {
                left: ExprRef(0),
                right: ExprRef(3),
            },
            true,
        ),
        (
            K::Add {
                left: ExprRef(0),
                right: ExprRef(1),
            },
            K::Mul {
                left: ExprRef(3),
                right: ExprRef(2),
            },
            true,
        ),
        (
            K::Add {
                left: ExprRef(0),
                right: ExprRef(1),
            },
            K::Neg { value: ExprRef(3) },
            true,
        ),
        (
            K::Neg { value: ExprRef(0) },
            K::Neg { value: ExprRef(3) },
            true,
        ),
        (
            K::Pow {
                left: ExprRef(0),
                right: ExprRef(1),
            },
            K::Pow {
                left: ExprRef(3),
                right: ExprRef(2),
            },
            true,
        ),
        (
            K::Pow {
                left: ExprRef(1),
                right: ExprRef(2),
            },
            K::Pow {
                left: ExprRef(0),
                right: ExprRef(3),
            },
            false,
        ),
        (
            K::Equal {
                left: ExprRef(0),
                right: ExprRef(1),
            },
            K::Equal {
                left: ExprRef(3),
                right: ExprRef(2),
            },
            true,
        ),
        (
            K::Add {
                left: ExprRef(0),
                right: ExprRef(1),
            },
            K::Equal {
                left: ExprRef(3),
                right: ExprRef(2),
            },
            false,
        ),
    ] {
        let mut kinds = symbols();
        kinds.extend([child, parent]);
        let tex = rendered(kinds)?;
        assert_eq!(tex.contains(r"\left("), required, "{tex}");
    }
    Ok(())
}
#[test]
fn function_names_and_explicit_fences_have_distinct_roles() -> Result<(), String> {
    let mut kinds = symbols();
    kinds.push(K::Call {
        function: ExprRef(0),
        arguments: vec![ExprRef(1), ExprRef(2)],
    });
    assert_eq!(rendered(kinds)?.matches(r"\left(").count(), 1);
    let mut kinds = symbols();
    kinds.extend([
        K::Add {
            left: ExprRef(0),
            right: ExprRef(1),
        },
        K::Call {
            function: ExprRef(3),
            arguments: vec![ExprRef(2)],
        },
    ]);
    assert_eq!(rendered(kinds)?.matches(r"\left(").count(), 2);
    let mut kinds = symbols();
    kinds.push(K::Fence {
        open: "(".into(),
        close: ")".into(),
        value: ExprRef(0),
    });
    assert_eq!(rendered(kinds)?.matches(r"\left(").count(), 1);
    Ok(())
}
#[test]
fn differential_closes_integral_scope_but_power_base_stays_grouped() -> Result<(), String> {
    for (parent, count) in [
        (
            K::Equal {
                left: ExprRef(0),
                right: ExprRef(3),
            },
            0,
        ),
        (K::Neg { value: ExprRef(3) }, 0),
        (
            K::Mul {
                left: ExprRef(3),
                right: ExprRef(0),
            },
            0,
        ),
        (
            K::Pow {
                left: ExprRef(3),
                right: ExprRef(0),
            },
            1,
        ),
    ] {
        let mut kinds = symbols();
        kinds.extend([
            K::Integral {
                index: "t".into(),
                lower: ExprRef(0),
                upper: ExprRef(1),
                body: ExprRef(2),
            },
            parent,
        ]);
        let tex = rendered(kinds)?;
        assert_eq!(tex.matches(r"\left(").count(), count, "{tex}");
    }
    let mut kinds = symbols();
    kinds.extend([
        K::Add {
            left: ExprRef(0),
            right: ExprRef(1),
        },
        K::Sum {
            index: "i".into(),
            lower: ExprRef(0),
            upper: ExprRef(1),
            body: ExprRef(3),
        },
    ]);
    assert_eq!(rendered(kinds)?.matches(r"\left(").count(), 1);
    Ok(())
}
