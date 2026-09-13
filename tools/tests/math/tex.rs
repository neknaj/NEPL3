use super::*;
use nepl3_math_core::{check, lower};

#[test]
fn actual_math_forms_use_structural_tex_with_explicit_annotation_failure() -> Result<(), String> {
    for_each_tex(|_, _| Ok(()))
}

fn for_each_tex(mut inspect: impl FnMut(&str, &str) -> Result<(), String>) -> Result<(), String> {
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
                inspect(source, out.tex())?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
#[ignore = "requires Node and npm ci --prefix tools/audit/math"]
fn pinned_katex_accepts_production_constructor_output() -> Result<(), String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut inputs = Vec::new();
    let mut sources = Vec::new();
    for_each_tex(|source, tex| {
        for display in [false, true] {
            inputs.push(serde_json::json!({"tex": tex, "displayMode": display}));
            sources.push(source.to_owned());
        }
        Ok(())
    })?;
    // Regression inputs absent from the constructor corpus. These enter through
    // the public checked Math API; the adapter must not reinterpret literal text.
    for text in ["e\u{301}", "日本--", "<script>\\input{x}$%_&#^~"] {
        use nepl3_math_core::model::{ExprRef, MathKind, MathNode, MathRoot, MathValue};
        let value = MathValue {
            root: MathRoot::Expr(ExprRef(0)),
            embeds: vec![],
            nodes: vec![MathNode {
                kind: MathKind::Text { text: text.into() },
                origin: None,
                span: None,
                locations: vec![],
            }],
        };
        let checked = check::expression(&value, &mut budget()).map_err(err)?;
        let rendered = nepl3_math_tex::render(&checked, &mut budget()).map_err(err)?;
        inputs.push(serde_json::json!({"tex": rendered.tex(), "displayMode": false}));
        sources.push(text.into());
    }
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("audit/math/render.mjs");
    let mut child = Command::new("node")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(err)?;
    let input = serde_json::to_vec(&inputs).map_err(err)?;
    child
        .stdin
        .take()
        .ok_or("node stdin")?
        .write_all(&input)
        .map_err(err)?;
    let output = child.wait_with_output().map_err(err)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let results: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).map_err(err)?;
    assert_eq!(results.len(), inputs.len());
    for (source, mut result) in sources.iter().zip(results) {
        assert_eq!(result["version"], "0.18.7");
        let html = result["html"].as_str().ok_or("renderer html")?;
        assert!(html.contains("class=\"katex\""), "{source}");
        assert!(html.contains("<math"), "{source}");
        assert!(!html.contains("<script"), "{source}");
        // Probe the actual fixed renderer's declarations. This extraction is
        // test-only and does not claim to parse/admit untrusted HTML.
        for part in html.split(" style=\"").skip(1) {
            let style = part.split_once('"').ok_or("style closing quote")?.0;
            assert!(
                nepl3_markup::katex::computed_style(style, &mut budget()).map_err(err)?,
                "{source}: out-of-profile style {style}"
            );
        }
        for part in html.split(" d=\"").skip(1) {
            let path = part.split_once('"').ok_or("path closing quote")?.0;
            assert!(
                nepl3_markup::katex::path_data(path, &mut budget()).map_err(err)?,
                "{source}: out-of-profile SVG path {path}"
            );
        }
        for part in html.split(" viewBox=\"").skip(1) {
            let viewport = part.split_once('"').ok_or("viewBox closing quote")?.0;
            assert!(
                nepl3_markup::katex::view_box(viewport, &mut budget()).map_err(err)?,
                "{source}: out-of-profile SVG viewport {viewport}"
            );
        }
        if source == "日本--" {
            assert!(html.contains("日本--"));
        }
        if source.starts_with("<script>") {
            // Literal payload may span several mtext nodes. Inspect only MathML
            // text, excluding the TeX annotation and visual HTML duplicate.
            let math = html.split_once("<annotation").ok_or("MathML annotation")?.0;
            let mut text = String::new();
            for part in math.split("<mtext>").skip(1) {
                text.push_str(part.split_once("</mtext>").ok_or("mtext close")?.0);
            }
            let text = text
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&");
            assert_eq!(&text, source);
        }
        // Independently specified constructor shapes, not golden output copies.
        if source.starts_with("frac ") {
            assert!(html.contains("<mfrac>"), "{source}");
        }
        if source.starts_with("sqrt ") {
            assert!(html.contains("<msqrt>"), "{source}");
        }
        if source.starts_with("matrix ") || source.starts_with("vector ") {
            assert!(html.contains("<mtable"), "{source}");
        }
        let classes: Vec<String> = serde_json::from_value(result["classes"].take()).map_err(err)?;
        let classes: Vec<&str> = classes.iter().map(String::as_str).collect();
        let visual = nepl3_tools::doc::math::katex::render(
            result["visual"].take(),
            &nepl3_markup::katex::fragment::Policy {
                classes: &classes,
                scope: "nepl-math-corpus",
            },
            &mut budget(),
        )
        .map_err(|e| format!("{source}: visual admission {e:?}"))?;
        assert!(!visual.html().contains(" style="));
        assert!(
            visual
                .html()
                .starts_with("<span class=\"nepl-math-corpus\" aria-hidden=\"true\">")
        );
        assert!(!visual.stylesheet().contains("url("));
        // Browser runner consumes original and production-serialized output;
        // nothing is committed as a renderer golden or review source snapshot.
        println!(
            "MATH_VISUAL_CASE {}",
            serde_json::json!({
                "original": result["visualHtml"], "html": visual.html(), "css": visual.stylesheet()
            })
        );
    }
    Ok(())
}

#[test]
fn visual_host_boundary_rejects_forged_and_extra_fields() {
    use nepl3_tools::doc::math::katex;
    let valid = serde_json::json!({"kind":"parsed-unchecked","nodes":[
        {"kind":"span","classes":"katex","style":"","aria_hidden":null,"children":[]}
    ]});
    let policy = nepl3_markup::katex::fragment::Policy {
        classes: &["katex"],
        scope: "nepl-math-test",
    };
    assert!(katex::render(valid.clone(), &policy, &mut budget()).is_ok());
    let mut extra = valid.clone();
    extra["nodes"][0]["onclick"] = serde_json::json!("alert(1)");
    let mut bad_kind = valid.clone();
    bad_kind["nodes"][0]["kind"] = serde_json::json!("script");
    let mut bad_ref = valid.clone();
    bad_ref["nodes"][0]["children"] = serde_json::json!([0]);
    let mut extra_root = valid.clone();
    extra_root["trusted"] = serde_json::json!(true);
    let mut bad_style = valid.clone();
    bad_style["nodes"][0]["style"] = serde_json::json!("width:url(x)");
    for value in [extra, bad_kind, bad_ref, extra_root, bad_style] {
        assert!(katex::render(value, &policy, &mut budget()).is_err());
    }
    let mut stopped = budget();
    stopped.stop(nepl3_core::budget::StopReason::Cancelled);
    assert!(matches!(
        katex::render(valid, &policy, &mut stopped),
        Err(katex::Error::Stopped(
            nepl3_core::budget::StopReason::Cancelled
        ))
    ));
}
