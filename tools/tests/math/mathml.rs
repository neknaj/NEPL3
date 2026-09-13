use super::*;
use nepl3_markup::mathml::{Display, serialize, validate};
use nepl3_math_core::{check, lower};

#[test]
fn every_expression_form_and_display_mode_has_structural_output() -> Result<(), String> {
    let compiled = compiled()?;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/fixtures/math/lower.json"
    ))
    .map_err(err)?;
    let cases = fixture["cases"].as_array().ok_or("cases")?;
    for case in cases {
        if case["entry"].as_str() != Some("Expr") {
            continue;
        }
        let source = case["source"].as_str().ok_or("source")?;
        let kind = case["kind"].as_str().ok_or("kind")?;
        with_input(&compiled, source, "Expr", |tree, profile, b, a| {
            let syntax = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let math = lower::expression(
                &syntax,
                &compiled.others[0].schema,
                check::Category::Expr,
                profile.registry(),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            let checked = check::expression(&math.value, &mut budget()).map_err(err)?;
            for display in [Display::Inline, Display::Block] {
                let result = nepl3_math_mathml::render(&checked, display, &mut budget());
                if kind == "Label" {
                    assert!(matches!(
                        result,
                        Err(nepl3_math_mathml::Error::AnnotationRequiresPreparation(_))
                    ));
                    continue;
                }
                let output = result.map_err(err)?;
                let proof = validate(&output.fragment, &mut budget()).map_err(err)?;
                let text = serialize(&proof, &mut budget()).map_err(err)?;
                // The expected element comes from spec 06's constructor table.
                let tag = match kind {
                    "Number" => "mn",
                    "Symbol" | "SymbolName" => "mi",
                    "Text" => "mtext",
                    "Frac" => "mfrac",
                    "Sqrt" => "msqrt",
                    "Root" => "mroot",
                    "Subscript" => "msub",
                    "Superscript" | "Pow" | "Transpose" => "msup",
                    "Scripts" => "msubsup",
                    "Matrix" | "Vector" => "mtable",
                    "Sum" | "Integral" => {
                        if display == Display::Block {
                            "munderover"
                        } else {
                            "msubsup"
                        }
                    }
                    _ => "mrow",
                };
                assert!(text.contains(&format!("<{tag}>")), "{kind}: {text}");
                assert!(!text.contains("<script"));
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                nepl3_math_mathml::render(&checked, Display::Inline, &mut cancelled),
                Err(nepl3_math_mathml::Error::Stopped(StopReason::Cancelled))
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn real_math_structure_renders_without_evaluation() -> Result<(), String> {
    let compiled = compiled()?;
    for (source, expected) in [
        ("frac 1 0", "<mfrac><mn>1</mn><mn>0</mn></mfrac>"),
        (
            "mul add x y z",
            "<mrow><mrow><mo>(</mo><mrow><mi>x</mi><mo>+</mo><mi>y</mi></mrow><mo>)</mo></mrow><mo>·</mo><mi>z</mi></mrow>",
        ),
        (
            "sub x sub y z",
            "<mrow><mi>x</mi><mo>−</mo><mrow><mo>(</mo><mrow><mi>y</mi><mo>−</mo><mi>z</mi></mrow><mo>)</mo></mrow></mrow>",
        ),
        ("root 3 x", "<mroot><mi>x</mi><mn>3</mn></mroot>"),
        (
            "scripts x 1 2",
            "<msubsup><mi>x</mi><mn>1</mn><mn>2</mn></msubsup>",
        ),
        (
            "pow -1 2",
            "<msup><mrow><mo>(</mo><mn>-1</mn><mo>)</mo></mrow><mn>2</mn></msup>",
        ),
        (
            "let x 1 add x 2",
            "<mrow><mi>x</mi><mo>:=</mo><mn>1</mn><mo>;</mo><mrow><mi>x</mi><mo>+</mo><mn>2</mn></mrow></mrow>",
        ),
        (
            "sum i 1 2 i",
            "<mrow><msubsup><mo>∑</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mn>2</mn></msubsup><mi>i</mi></mrow>",
        ),
        (
            "integral x 0 1 x",
            "<mrow><msubsup><mo>∫</mo><mn>0</mn><mn>1</mn></msubsup><mi>x</mi><mi>d</mi><mi>x</mi></mrow>",
        ),
    ] {
        with_input(&compiled, source, "Expr", |tree, profile, b, a| {
            let syntax = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let math = lower::expression(
                &syntax,
                &compiled.others[0].schema,
                check::Category::Expr,
                profile.registry(),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            let original = math.value.clone();
            let checked = check::expression(&math.value, &mut budget()).map_err(err)?;
            let mut full = budget();
            let rendered =
                nepl3_math_mathml::render(&checked, Display::Inline, &mut full).map_err(err)?;
            let actual = serialize(
                &validate(&rendered.fragment, &mut budget()).map_err(err)?,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(
                actual,
                format!(
                    "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"inline\">{expected}</math>"
                ),
                "{source}"
            );
            assert_eq!(math.value, original);
            assert_eq!(rendered.node_roots.len(), math.value.nodes.len());
            for reason in [
                StopReason::WorkLimit,
                StopReason::AllocationLimit,
                StopReason::NodeLimit,
                StopReason::DepthLimit,
            ] {
                let mut limits = budget().limits();
                match reason {
                    StopReason::WorkLimit => limits.work = full.usage().work - 1,
                    StopReason::AllocationLimit => {
                        limits.allocation_units = full.usage().allocation_units - 1
                    }
                    StopReason::NodeLimit => limits.nodes = full.usage().nodes - 1,
                    _ => limits.depth = full.usage().depth - 1,
                }
                let mut limited = Budget::new(limits);
                assert!(
                    matches!(nepl3_math_mathml::render(&checked,Display::Inline,&mut limited),Err(nepl3_math_mathml::Error::Stopped(s)) if s==reason)
                );
                assert_eq!(limited.poll(), Err(reason));
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn shared_input_keeps_mapping_and_expands_without_losing_order() -> Result<(), String> {
    use nepl3_math_core::model::*;
    let value = MathValue {
        root: MathRoot::Expr(ExprRef(1)),
        embeds: vec![],
        nodes: vec![
            MathNode {
                kind: MathKind::Symbol { name: "x".into() },
                origin: None,
                span: None,
                locations: vec![],
            },
            MathNode {
                kind: MathKind::Vector {
                    values: vec![ExprRef(0), ExprRef(0)],
                },
                origin: None,
                span: None,
                locations: vec![],
            },
        ],
    };
    let checked = check::expression(&value, &mut budget()).map_err(err)?;
    let rendered =
        nepl3_math_mathml::render(&checked, Display::Inline, &mut budget()).map_err(err)?;
    assert_eq!(rendered.node_roots.len(), 2);
    assert!(matches!(
        rendered.fragment.nodes[rendered.node_roots[0] as usize],
        nepl3_markup::mathml::Node::Element {
            tag: nepl3_markup::mathml::Tag::Identifier,
            ..
        }
    ));
    let text = serialize(
        &validate(&rendered.fragment, &mut budget()).map_err(err)?,
        &mut budget(),
    )
    .map_err(err)?;
    assert_eq!(
        text,
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"inline\"><mrow><mo>(</mo><mtable><mtr><mtd><mi>x</mi></mtd></mtr><mtr><mtd><mi>x</mi></mtd></mtr></mtable><mo>)</mo></mrow></math>"
    );
    Ok(())
}
