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
        r"\frac{\left(\mathord{\textit{x}}\right)-\left(\mathord{\textit{x}}\right)}{\mathord{\textit{x}}}"
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
