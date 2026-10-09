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

fn fixed_assets() -> Result<nepl3_tools::doc::math::assets::PreparedAssets, String> {
    use nepl3_tools::doc::math::assets::{self, Input};
    let inventory: serde_json::Value =
        serde_json::from_str(include_str!("../../math/katex/assets.json")).map_err(err)?;
    let pins = inventory["files"].as_array().ok_or("asset files")?;
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("audit/math/node_modules/katex");
    let mut bytes = Vec::new();
    for pin in pins {
        bytes.push(std::fs::read(root.join(pin["source"].as_str().ok_or("source")?)).map_err(err)?);
    }
    let mut input = Vec::new();
    for (pin, bytes) in pins.iter().zip(&bytes) {
        input.push(Input {
            path: pin["path"].as_str().ok_or("path")?,
            mime: pin["mime"].as_str().ok_or("mime")?,
            bytes,
        });
    }
    assets::prepare(&input, 2_000_000, &mut budget()).map_err(err)
}

#[test]
#[ignore = "requires Node and npm ci --prefix tools/audit/math"]
fn pinned_katex_accepts_production_constructor_output() -> Result<(), String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let fixed_assets = fixed_assets()?;
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
    for text in [
        "e\u{301}",
        "日本--",
        "<script>\\input{x}$%_&#^~",
        "Ȁ",
        "Ж",
        "Ա",
        "क",
        "ა",
        "日",
        "한",
    ] {
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
        // Production authority comes from the fixed verified asset catalog,
        // never the parser's output-derived test inventory.
        let bound = fixed_assets
            .prepare_visual(result["visual"].take(), "nepl-math-corpus", &mut budget())
            .map_err(|e| format!("{source}: fixed visual admission {e:?}"))?;
        let rendered = bound.serialize(&mut budget()).map_err(err)?;
        assert_eq!(rendered.assets().identity(), fixed_assets.identity());
        let visual = rendered.visual();
        assert!(!visual.html().contains(" style="));
        assert!(
            visual
                .html()
                .starts_with("<span class=\"nepl-math-corpus\" aria-hidden=\"true\">")
        );
        assert!(!visual.stylesheet().contains("url("));
        // Test-only clone supplies an independent native-owned input. This
        // exercises real renderer trees without claiming retained host binding.
        let parts = bound.parts();
        let classes: Vec<_> = parts.classes().iter().map(String::as_str).collect();
        let policy = nepl3_markup::katex::fragment::Policy {
            classes: &classes,
            scope: parts.scope(),
        };
        let projected = nepl3_markup::katex::fragment::prepare_owned(
            parts.fragment().clone(),
            &policy,
            &mut budget(),
        )
        .map_err(err)?
        .into_html(&mut budget())
        .map_err(err)?;
        assert_eq!(projected.stylesheet(), visual.stylesheet(), "{source}");
        let request = projected.request();
        let checked = nepl3_markup::html::validate(
            &request.fragment,
            request.slot,
            &request.policy,
            &mut budget(),
        )
        .map_err(err)?;
        let typed_html = nepl3_markup::html::serialize(&checked, &mut budget()).map_err(err)?;
        assert!(!typed_html.contains(" style="), "{source}");
        assert!(
            typed_html.starts_with("<span aria-hidden=\"true\" class=\"nepl-math-corpus\">"),
            "{source}"
        );

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
        assert!(katex::prepare(value.clone(), &policy, &mut budget()).is_err());
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

#[test]
fn retained_visual_parts_own_policy_revalidate_depth_and_preserve_legacy_output()
-> Result<(), String> {
    use nepl3_core::budget::{Budget, Resource, StopReason};
    use nepl3_markup::katex::fragment::Policy;
    use nepl3_tools::doc::math::katex;
    let value = serde_json::json!({"kind":"parsed-unchecked","nodes":[
        {"kind":"text","text":"x<&"},
        {"kind":"span","classes":"katex","style":"height:1em;","aria_hidden":null,"children":[0]}
    ]});
    let mut scope = String::from("nepl-math-retained");
    let mut class = String::from("katex");
    let mut preparation_limits = budget().limits();
    preparation_limits.output_bytes = 0;
    let mut prepared_budget = Budget::new(preparation_limits);
    let prepared = {
        let policy = Policy {
            classes: &[class.as_str()],
            scope: &scope,
        };
        let expected = katex::render(value.clone(), &policy, &mut budget()).map_err(err)?;
        let prepared = katex::prepare(value.clone(), &policy, &mut prepared_budget).map_err(err)?;
        assert_eq!(prepared_budget.usage().output_bytes, 0);
        let actual = prepared.serialize(&mut budget()).map_err(err)?;
        assert_eq!(actual.html(), expected.html());
        assert_eq!(actual.stylesheet(), expected.stylesheet());
        prepared
    };
    // Fail at the final preparation copy charge, not only inside tree parsing.
    let preparation_used = prepared_budget.usage();
    let policy = Policy {
        classes: &[class.as_str()],
        scope: &scope,
    };
    for (allocation, amount, reason) in [
        (
            true,
            preparation_used.allocation_units,
            StopReason::AllocationLimit,
        ),
        (false, preparation_used.work, StopReason::WorkLimit),
    ] {
        for enough in [true, false] {
            let mut limits = budget().limits();
            limits.output_bytes = 0;
            if allocation {
                limits.allocation_units = amount - u64::from(!enough);
            } else {
                limits.work = amount - u64::from(!enough);
            }
            let mut b = Budget::new(limits);
            assert_eq!(
                katex::prepare(value.clone(), &policy, &mut b).is_ok(),
                enough
            );
            if !enough {
                assert_eq!(b.poll(), Err(reason));
                let used = b.usage();
                assert!(katex::prepare(value.clone(), &policy, &mut b).is_err());
                assert_eq!(b.usage(), used);
            }
        }
    }
    scope.clear();
    class.clear();
    assert_eq!(prepared.scope(), "nepl-math-retained");
    assert_eq!(prepared.classes(), &["katex"]);
    assert_eq!(prepared.fragment().nodes.len(), 2);
    let mut measured = budget();
    let first = prepared.serialize(&mut measured).map_err(err)?;
    let used = measured.usage();
    let second = prepared.serialize(&mut measured).map_err(err)?;
    assert_eq!(first.html(), second.html());
    assert_eq!(measured.usage().output_bytes, 2 * used.output_bytes);
    assert!(measured.usage().work > used.work);
    assert!(first.html().contains("x&lt;&amp;"));
    assert!(!first.html().contains(" style="));
    assert!(first.stylesheet().contains("!important"));
    // Retained validation is not permission to serialize in a deeper context.
    let mut limits = budget().limits();
    limits.depth = used.depth + 4;
    let mut nested = Budget::new(limits);
    nested
        .with_depth_at_least(4, |b| prepared.serialize(b))
        .map_err(err)?;
    assert_eq!(nested.usage().depth, used.depth + 4);
    limits.depth -= 1;
    let mut short = Budget::new(limits);
    assert!(nested.current_depth() == 0);
    assert!(
        short
            .with_depth_at_least(4, |b| prepared.serialize(b))
            .is_err()
    );
    assert_eq!(short.poll(), Err(StopReason::DepthLimit));
    for (resource, amount, reason) in [
        (Resource::Work, used.work, StopReason::WorkLimit),
        (
            Resource::AllocationUnits,
            used.allocation_units,
            StopReason::AllocationLimit,
        ),
        (
            Resource::OutputBytes,
            used.output_bytes,
            StopReason::OutputLimit,
        ),
    ] {
        for enough in [true, false] {
            let mut limits = budget().limits();
            let bound = amount - u64::from(!enough);
            match resource {
                Resource::Work => limits.work = bound,
                Resource::AllocationUnits => limits.allocation_units = bound,
                Resource::OutputBytes => limits.output_bytes = bound,
                _ => return Err("resource".into()),
            }
            let mut b = Budget::new(limits);
            let result = prepared.serialize(&mut b);
            assert_eq!(result.is_ok(), enough);
            if !enough {
                assert_eq!(b.poll(), Err(reason));
                let used = b.usage();
                assert!(prepared.serialize(&mut b).is_err());
                assert_eq!(b.usage(), used);
            }
        }
    }
    let original_nodes = prepared.fragment().nodes.clone();
    let mut cloned = prepared.fragment().clone();
    cloned.nodes.clear();
    assert_eq!(prepared.fragment().nodes, original_nodes);
    for invalid in [
        Policy {
            classes: &["other"],
            scope: "nepl-math-test",
        },
        Policy {
            classes: &["katex"],
            scope: "invalid scope",
        },
    ] {
        assert!(katex::prepare(value.clone(), &invalid, &mut budget()).is_err());
    }
    let mut cancelled = budget();
    cancelled.cancel();
    let before = cancelled.usage();
    assert!(prepared.serialize(&mut cancelled).is_err());
    assert_eq!(cancelled.usage(), before);
    assert!(prepared.serialize(&mut cancelled).is_err());
    assert_eq!(cancelled.usage(), before);
    assert_eq!(prepared.fragment().nodes, original_nodes);
    let policy = Policy {
        classes: &["katex"],
        scope: "nepl-math-retained",
    };
    let mut bad = value;
    bad["nodes"][1]["onclick"] = "x".into();
    assert!(katex::prepare(bad, &policy, &mut budget()).is_err());
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "requires npm ci --prefix tools/audit/math --ignore-scripts; real fixed package bytes"]
fn native_fixed_katex_asset_binding_checks_complete_content_and_budgets() -> Result<(), String> {
    use nepl3_core::{
        budget::{Budget, StopReason},
        source::Digest,
    };
    use nepl3_tools::doc::math::assets::{self, Error, Input};
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../math/katex/assets.json")).map_err(err)?;
    let pins = manifest["files"].as_array().ok_or("files")?;
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("audit/math/node_modules/katex");
    let mut owned = Vec::new();
    for pin in pins {
        owned.push(std::fs::read(root.join(pin["source"].as_str().ok_or("source")?)).map_err(err)?);
    }
    fn inputs<'a>(
        pins: &'a [serde_json::Value],
        values: &'a [Vec<u8>],
    ) -> Result<Vec<Input<'a>>, String> {
        pins.iter()
            .zip(values)
            .map(|(pin, bytes)| {
                Ok(Input {
                    path: pin["path"].as_str().ok_or("path")?,
                    mime: pin["mime"].as_str().ok_or("mime")?,
                    bytes,
                })
            })
            .collect()
    }
    let input = inputs(pins, &owned)?;
    let total = owned.iter().map(|v| v.len() as u64).sum::<u64>();
    assert_eq!(total, 1_102_467);
    assert_eq!(input.len(), 62);
    let mut limits = budget().limits();
    limits.output_bytes = 0;
    let mut measured = Budget::new(limits);
    let prepared = assets::prepare(&input, total, &mut measured).map_err(err)?;
    let usage = measured.usage();
    assert_eq!(usage.output_bytes, 0);
    assert_eq!(usage.source_bytes, 0);
    assert_eq!(
        prepared.version(),
        manifest["version"].as_str().ok_or("version")?
    );
    let mut identity = b"nepl3.katex.asset-parts/1\0".to_vec();
    identity.extend_from_slice(&(prepared.version().len() as u64).to_be_bytes());
    identity.extend_from_slice(prepared.version().as_bytes());
    identity.extend_from_slice(&(pins.len() as u64).to_be_bytes());
    for ((file, pin), bytes) in prepared.files().iter().zip(pins).zip(&owned) {
        assert_eq!(file.bytes(), bytes);
        assert_eq!(file.path(), pin["path"]);
        assert_eq!(file.mime(), pin["mime"]);
        let digest = Digest::of(bytes);
        assert_eq!(file.digest(), digest);
        let hex = digest
            .0
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        assert_eq!(hex, pin["sha256"]);
        for text in [file.path(), file.mime()] {
            identity.extend_from_slice(&(text.len() as u64).to_be_bytes());
            identity.extend_from_slice(text.as_bytes());
        }
        identity.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        identity.extend_from_slice(&digest.0);
    }
    assert_eq!(prepared.identity(), Digest::of(&identity));
    let mut extra = inputs(pins, &owned)?;
    extra.push(Input {
        path: input[0].path,
        mime: input[0].mime,
        bytes: input[0].bytes,
    });
    assert!(matches!(
        assets::prepare(&extra, total, &mut budget()),
        Err(Error::Count)
    ));
    assert!(matches!(
        assets::prepare(&input[..61], total, &mut budget()),
        Err(Error::Count)
    ));
    assert!(matches!(
        assets::prepare(&input, total - 1, &mut budget()),
        Err(Error::AssetByteLimit)
    ));
    for bad_path in [
        "/katex.min.css",
        "../katex.min.css",
        "./katex.min.css",
        "katex%2emin.css",
        "KATEX.min.css",
        "fonts",
        "fonts\\x",
    ] {
        let mut modified = inputs(pins, &owned)?;
        modified[0].path = bad_path;
        let mut b = budget();
        assert!(matches!(
            assets::prepare(&modified, total, &mut b),
            Err(Error::Metadata { index: 0 })
        ));
        assert_eq!(b.usage().allocation_units, 0);
    }
    for index in 0..input.len() {
        let mut changed = owned.clone();
        changed[index][0] ^= 1;
        assert!(
            matches!(assets::prepare(&inputs(pins, &changed)?,total,&mut budget()),Err(Error::Digest{index:i}) if i==index)
        );
    }
    let mut swapped = inputs(pins, &owned)?;
    swapped.swap(0, 1);
    assert!(matches!(
        assets::prepare(&swapped, total, &mut budget()),
        Err(Error::Metadata { .. })
    ));
    let mut duplicate = inputs(pins, &owned)?;
    duplicate[1] = Input {
        path: input[0].path,
        mime: input[0].mime,
        bytes: input[0].bytes,
    };
    assert!(matches!(
        assets::prepare(&duplicate, total, &mut budget()),
        Err(Error::Metadata { index: 1 })
    ));
    let mut mime = inputs(pins, &owned)?;
    mime[0].mime = "text/xxx";
    assert!(matches!(
        assets::prepare(&mime, total, &mut budget()),
        Err(Error::Metadata { index: 0 })
    ));
    for truncated in [true, false] {
        let mut bytes = owned.clone();
        if truncated {
            bytes[0].pop();
        } else {
            bytes[0].push(0);
        }
        assert!(matches!(
            assets::prepare(&inputs(pins, &bytes)?, total, &mut budget()),
            Err(Error::Metadata { index: 0 })
        ));
    }
    for (allocation, amount, reason) in [
        (true, usage.allocation_units, StopReason::AllocationLimit),
        (false, usage.work, StopReason::WorkLimit),
    ] {
        for enough in [true, false] {
            let mut limits = budget().limits();
            limits.output_bytes = 0;
            if allocation {
                limits.allocation_units = amount - u64::from(!enough);
            } else {
                limits.work = amount - u64::from(!enough);
            }
            let mut b = Budget::new(limits);
            assert_eq!(assets::prepare(&input, total, &mut b).is_ok(), enough);
            if !enough {
                assert_eq!(b.poll(), Err(reason));
                let used = b.usage();
                assert!(assets::prepare(&input, total, &mut b).is_err());
                assert_eq!(b.usage(), used);
            }
        }
    }
    let mut b = budget();
    let first = prepared.materialize(&mut b).map_err(err)?;
    let materialized = b.usage();
    assert_eq!(materialized.output_bytes, total);
    let second = prepared.materialize(&mut b).map_err(err)?;
    assert_eq!(b.usage().output_bytes, 2 * total);
    for (a, b) in first.iter().zip(&second) {
        assert_eq!(a.bytes(), b.bytes());
    }
    for (kind, amount, reason) in [
        (0, materialized.work, StopReason::WorkLimit),
        (
            1,
            materialized.allocation_units,
            StopReason::AllocationLimit,
        ),
        (2, materialized.output_bytes, StopReason::OutputLimit),
    ] {
        for enough in [true, false] {
            let mut limits = budget().limits();
            let bound = amount - u64::from(!enough);
            match kind {
                0 => limits.work = bound,
                1 => limits.allocation_units = bound,
                _ => limits.output_bytes = bound,
            }
            let mut limited = Budget::new(limits);
            assert_eq!(prepared.materialize(&mut limited).is_ok(), enough);
            if !enough {
                assert_eq!(limited.poll(), Err(reason));
                let used = limited.usage();
                assert!(prepared.materialize(&mut limited).is_err());
                assert_eq!(limited.usage(), used);
            }
        }
    }
    let mut limits = budget().limits();
    limits.output_bytes = total - 1;
    let mut b = Budget::new(limits);
    assert!(prepared.materialize(&mut b).is_err());
    assert_eq!(b.poll(), Err(StopReason::OutputLimit));
    let mut b = budget();
    b.cancel();
    let before = b.usage();
    assert!(assets::prepare(&input, total, &mut b).is_err());
    assert!(prepared.materialize(&mut b).is_err());
    assert_eq!(b.usage(), before);
    drop(extra);
    drop(input);
    for file in &mut owned {
        file.clear();
    }
    assert_eq!(
        prepared
            .files()
            .iter()
            .map(|v| v.bytes().len() as u64)
            .sum::<u64>(),
        total
    );
    Ok(())
}

#[test]
#[ignore = "requires npm ci --prefix tools/audit/math --ignore-scripts; fixed CSS catalog"]
fn fixed_math_catalog_rejects_output_invented_policy() -> Result<(), String> {
    let assets = fixed_assets()?;
    let input = |class: &str| serde_json::json!({"kind":"parsed-unchecked","nodes":[{"kind":"span","classes":class,"style":"","aria_hidden":null,"children":[]}]});
    for class in [
        "unknown_class",
        "unknown_fallback",
        "size12",
        "nepl-math-forged",
        "accentunder",
        "allowbreak",
        "nobreak",
        "textmd",
        "textup",
    ] {
        assert!(
            assets
                .prepare_visual(input(class), "nepl-math-fixed", &mut budget())
                .is_err(),
            "{class}"
        );
    }
    for class in [
        "katex",
        "mord",
        "mtight",
        "text",
        "armenian_fallback",
        "brahmic_fallback",
        "cjk_fallback",
        "cyrillic_fallback",
        "georgian_fallback",
        "hangul_fallback",
        "latin_fallback",
    ] {
        let bound = assets
            .prepare_visual(input(class), "nepl-math-fixed", &mut budget())
            .map_err(err)?;
        let output = bound.serialize(&mut budget()).map_err(err)?;
        assert_eq!(output.assets().identity(), assets.identity());
        assert_eq!(bound.catalog_identity(), output.catalog_identity());
    }
    let catalog = include_str!("../../math/katex/classes.json");
    let bound = assets
        .prepare_visual(input("katex"), "nepl-math-fixed", &mut budget())
        .map_err(err)?;
    assert_eq!(
        bound.catalog_identity(),
        nepl3_core::source::Digest::of(catalog.as_bytes())
    );
    let mut stopped = budget();
    stopped.cancel();
    let used = stopped.usage();
    assert!(
        assets
            .prepare_visual(input("katex"), "nepl-math-fixed", &mut stopped)
            .is_err()
    );
    assert!(bound.serialize(&mut stopped).is_err());
    assert_eq!(stopped.usage(), used);
    Ok(())
}
