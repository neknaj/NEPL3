use super::*;
use nepl3_math_core::{check, lower};

#[test]
fn actual_math_forms_use_structural_tex_with_explicit_annotation_failure() -> Result<(), String> {
    let compiled = compiled()?;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/fixtures/math/lower.json"
    ))
    .map_err(err)?;
    for case in fixture["cases"].as_array().ok_or("cases")? {
        if case["entry"].as_str() != Some("Expr") {
            continue;
        }
        let source = case["source"].as_str().ok_or("source")?;
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
            let result = nepl3_math_tex::render(&checked, &mut budget());
            if case["kind"] == "Label" {
                assert!(
                    matches!(
                        result,
                        Err(nepl3_math_tex::Error::Unsupported {
                            reason: nepl3_math_tex::Unsupported::ForeignAnnotation,
                            ..
                        })
                    ),
                    "{source}"
                );
            } else {
                let out = result.map_err(|e| format!("{source}: {e:?}"))?;
                assert!(!out.tex().is_empty(), "{source}");
                assert!(std::ptr::eq(out.source(), &math.value));
                for r in out.occurrences() {
                    assert!(r.node < math.value.nodes.len() as u64);
                    assert!(out.tex().get(r.start as usize..r.end as usize).is_some());
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}
