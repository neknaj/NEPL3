use super::*;
use crate::testing::Fixture;
use serde_json::json;
const SVG: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const ONE: &str = r#"article en Sentence "One" body cons image asset "logo" none Sentence "[漢/かん]" none cons paragraph cons Sentence sentence cons math frac 1 2 cons document math Math frac 3 4 cons document link page "two" some "target" text "Next" nil cons code Math frac 5 6 nil cons display Math frac 7 8 nil"#;
const TWO: &str = r#"article en "Two" body cons image asset "logo" none "Two image" none cons section target "Target" body cons paragraph cons sentence cons math Math frac 9 10 nil nil nil nil"#;
fn manifest() -> serde_json::Value {
    json!({"version":1,"pages":[{"id":"one","source":"one.nepld","route":"deep/a.html"},{"id":"two","source":"two.nepld","route":"deep/b.html"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"one.svg","mime":"image/svg+xml"}]},{"page":"two","assets":[{"id":"logo","source":"two.svg","mime":"image/svg+xml"}]}]})
}
fn fixture() -> crate::Result<Fixture> {
    let f = Fixture::new()?;
    f.write("one.nepld", ONE)?;
    f.write("two.nepld", TWO)?;
    f.write("one.svg", SVG)?;
    f.write("two.svg", SVG)?;
    f.json("manifest.json", &manifest())?;
    Ok(f)
}
#[test]
fn svg_pages_cli_capture_is_immutable_and_records_explicit_math_selection() -> crate::Result<()> {
    let f = fixture()?;
    let captured = input::capture(&f.root().join("manifest.json"))?;
    f.write("one.nepld", "changed invalid Doc")?;
    f.write("one.svg", "changed invalid SVG")?;
    let c = crate::doc::source::compiled()?;
    let mut b = crate::doc::source::budget();
    let output = generate::run(&c, &captured, &mut b)?;
    assert_eq!(output.math_generation.len(), 2);
    assert_eq!(output.math_generation[0].len(), 3);
    assert_eq!(output.math_generation[1].len(), 1);
    let receipt: serde_json::Value = serde_json::from_str(&output.manifest)?;
    assert_eq!(
        receipt["source_input"]["math_renderer"],
        "mathml-only-explicit"
    );
    assert_eq!(receipt["katex_generation"], false);
    // Source order: title=owner 0, excluded alt=owner 1, body=owner 2.
    // Title and body consume callback placements 0 and 1; Code is placement 2.
    let math = &receipt["source_input"]["math"];
    assert_eq!(
        math[0][0]["owner_path"],
        json!([
            {"kind":"sentence","placement":1,"occurrence":2,"owner":2},
            {"kind":"sentence-foreign","occurrence":0,"embed":0}
        ])
    );
    assert_eq!(
        math[0][1]["owner_path"],
        json!([
            {"kind":"sentence","placement":1,"occurrence":2,"owner":2},
            {"kind":"sentence-foreign","occurrence":1,"embed":1},
            {"kind":"document-math","occurrence":0,"embed":0}
        ])
    );
    assert_eq!(
        math[0][2]["owner_path"],
        json!([{"kind":"doc","placement":3,"embed":4}])
    );
    // The second page has no Sentence/Code slots before its sole direct Math.
    assert_eq!(
        math[1][0]["owner_path"],
        json!([{"kind":"doc","placement":0,"embed":0}])
    );
    for page in receipt["source_input"]["math"].as_array().ok_or("pages")? {
        for record in page.as_array().ok_or("math")? {
            assert_eq!(record["selection_reason"], "ExplicitMathMLOnly");
            assert_eq!(record["fallback_reason"], serde_json::Value::Null);
            assert!(!record["owner_path"].as_array().ok_or("owner")?.is_empty());
        }
    }
    assert_eq!(
        receipt["source_input"]["svg_capture"][0]["sha256"],
        crate::doc::export::digest_hex(nepl3_core::source::Digest::of(SVG.as_bytes()))
    );
    let html = std::str::from_utf8(&output.files["deep/a.html"])?;
    assert!(html.contains("img-src data:;"));
    assert!(html.contains("data:image/svg+xml;base64,"));
    assert!(html.contains("b.html#n-746172676574"));
    assert!(!html.contains("katex-html"));
    let changed = input::capture(&f.root().join("manifest.json"))?;
    assert!(generate::run(&c, &changed, &mut crate::doc::source::budget()).is_err());
    Ok(())
}
#[test]
fn svg_pages_cli_manifest_rejects_null_unknown_duplicate_and_escape() -> crate::Result<()> {
    let f = fixture()?;
    let base = manifest();
    let mut cases = Vec::new();
    for field in [
        "pages",
        "assets",
        "files",
        "display",
        "output_limits",
        "parse_limits",
        "lower_limits",
    ] {
        let mut v = base.clone();
        v[field] = serde_json::Value::Null;
        cases.push(v);
    }
    for field in ["input", "id", "source", "route"] {
        let mut v = base.clone();
        v["pages"][0][field] = serde_json::Value::Null;
        cases.push(v);
    }
    let mut v = base.clone();
    v["unknown"] = json!(true);
    cases.push(v);
    let mut v = base.clone();
    v["assets"][0]["page"] = json!("missing");
    cases.push(v);
    let mut v = base.clone();
    v["assets"][1]["page"] = json!("one");
    cases.push(v);
    let mut v = base.clone();
    v["pages"][1]["id"] = json!("one");
    cases.push(v);
    for name in [
        "../one.svg",
        "/one.svg",
        "a//b",
        "a/./b",
        "C:/a.svg",
        "one.svg?x",
        "one\\svg",
    ] {
        let mut v = base.clone();
        v["assets"][0]["assets"][0]["source"] = json!(name);
        cases.push(v);
    }
    let mut v = base.clone();
    v["assets"][0]["assets"][0]["mime"] = json!("image/png");
    cases.push(v);
    let mut v = base.clone();
    let a = v["assets"][0]["assets"][0].clone();
    v["assets"][0]["assets"] = json!([a.clone(), a]);
    cases.push(v);
    for v in cases {
        f.json("manifest.json", &v)?;
        assert!(
            input::capture(&f.root().join("manifest.json")).is_err(),
            "{v}"
        );
    }
    f.write(
        "manifest.json",
        serde_json::to_string(&base)?.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
    )?;
    assert!(input::capture(&f.root().join("manifest.json")).is_err());
    f.write("manifest.json", vec![b' '; 65_537])?;
    assert!(input::capture(&f.root().join("manifest.json")).is_err());
    Ok(())
}
#[test]
fn svg_pages_cli_capture_bounds_total_svg_bytes_and_count() -> crate::Result<()> {
    let f = fixture()?;
    let limit = nepl3_markup::html::svg::MAX_BYTES;
    f.write("one.svg", vec![b' '; limit])?;
    let mut v = manifest();
    v["assets"] = json!([{"page":"one","assets":(0..4).map(|n|json!({"id":format!("a{n}"),"source":"one.svg","mime":"image/svg+xml"})).collect::<Vec<_>>()}]);
    f.json("manifest.json", &v)?;
    assert!(input::capture(&f.root().join("manifest.json")).is_ok());
    v["assets"][0]["assets"]
        .as_array_mut()
        .ok_or("assets")?
        .push(json!({"id":"over","source":"one.svg","mime":"image/svg+xml"}));
    f.json("manifest.json", &v)?;
    assert!(input::capture(&f.root().join("manifest.json")).is_err());
    f.write("one.svg", vec![b' '; limit + 1])?;
    f.json("manifest.json", &manifest())?;
    assert!(input::capture(&f.root().join("manifest.json")).is_err());
    f.write("one.svg", SVG)?;
    for count in [input::SVG_COUNT, input::SVG_COUNT + 1] {
        let mut v = manifest();
        v["assets"] = json!([{"page":"one","assets":(0..count).map(|n|json!({"id":format!("a{n}"),"source":"one.svg","mime":"image/svg+xml"})).collect::<Vec<_>>()}]);
        f.json("manifest.json", &v)?;
        assert_eq!(
            input::capture(&f.root().join("manifest.json")).is_ok(),
            count == input::SVG_COUNT
        );
    }
    Ok(())
}
#[cfg(unix)]
#[test]
fn svg_pages_cli_capture_rejects_symlink_escape() -> crate::Result<()> {
    let f = fixture()?;
    let outside = Fixture::new()?;
    outside.write("asset.svg", SVG)?;
    std::fs::remove_file(f.root().join("one.svg"))?;
    std::os::unix::fs::symlink(outside.root().join("asset.svg"), f.root().join("one.svg"))?;
    assert!(input::capture(&f.root().join("manifest.json")).is_err());
    Ok(())
}
#[test]
fn svg_pages_cli_failure_does_not_create_output_or_hide_stop() -> crate::Result<()> {
    let f = fixture()?;
    let output = f.root().join("output");
    let mut v = manifest();
    v["assets"][0]["assets"][0]["id"] = json!("unused");
    f.json("manifest.json", &v)?;
    assert!(
        write(
            &f.root().join("manifest.json"),
            &output,
            MathRenderer::MathMLOnly
        )
        .is_err()
    );
    assert!(!output.exists());
    f.json("manifest.json", &manifest())?;
    let captured = input::capture(&f.root().join("manifest.json"))?;
    let mut b = crate::doc::source::budget();
    b.cancel();
    assert!(generate::run(&crate::doc::source::compiled()?, &captured, &mut b).is_err());
    assert_eq!(b.poll(), Err(nepl3_core::budget::StopReason::Cancelled));
    Ok(())
}

#[test]
fn svg_pages_cli_source_generation_preserves_exact_limits_and_preconsumption() -> crate::Result<()>
{
    use nepl3_core::budget::{Budget, Limits, Resource, StopReason, Usage};
    let f = fixture()?;
    let captured = input::capture(&f.root().join("manifest.json"))?;
    let c = crate::doc::source::compiled()?;
    let execute = |limits| {
        let mut b = Budget::new(limits);
        let result = (|| -> Result<_, String> {
            b.charge(Resource::Work, 17)
                .map_err(crate::doc::source::err)?;
            b.with_depth(|b| Ok::<_, StopReason>(generate::run(&c, &captured, b).map(|g| g.files)))
                .map_err(crate::doc::source::err)?
        })();
        (result, b)
    };
    let (baseline, full) = execute(crate::doc::source::budget().limits());
    let baseline = baseline?;
    for dimension in 0..6 {
        let amount = |u: Usage| match dimension {
            0 => u.work,
            1 => u.allocation_units,
            2 => u.nodes,
            3 => u.depth,
            4 => u.output_bytes,
            _ => u.source_bytes,
        };
        let select = |l: &mut Limits, n| match dimension {
            0 => l.work = n,
            1 => l.allocation_units = n,
            2 => l.nodes = n,
            3 => l.depth = n,
            4 => l.output_bytes = n,
            _ => l.source_bytes = n,
        };
        let mut cap = amount(full.usage());
        assert!(cap > 0);
        let mut settled = false;
        for _ in 0..32 {
            let mut limits = full.limits();
            select(&mut limits, cap);
            let (result, measured) = execute(limits);
            assert_eq!(result?, baseline);
            let next = amount(measured.usage());
            assert!(next <= cap);
            if next == cap {
                settled = true;
                break;
            }
            cap = next;
        }
        assert!(settled);
        let expected = match dimension {
            0 => StopReason::WorkLimit,
            1 => StopReason::AllocationLimit,
            2 => StopReason::NodeLimit,
            3 => StopReason::DepthLimit,
            4 => StopReason::OutputLimit,
            _ => StopReason::SourceLimit,
        };
        let mut limits = full.limits();
        select(&mut limits, cap - 1);
        let (result, mut b) = execute(limits);
        assert!(result.is_err());
        assert_eq!(b.poll(), Err(expected));
        let before = b.usage();
        assert_eq!(b.charge(Resource::Work, 1), Err(expected));
        assert_eq!(before, b.usage());
    }
    Ok(())
}

#[test]
fn svg_pages_preferred_metadata_keeps_seven_resource_boundaries() -> crate::Result<()> {
    use nepl3_core::budget::{Budget, Limits, Resource, StopReason, Usage};
    let f = fixture()?;
    let unsupported = r#"article en "Unsupported" body cons image asset "logo" none "diagram" none cons display Math label add x y Sentence "[字/じ]" nil"#;
    f.write("one.nepld", unsupported)?;
    f.write("two.nepld", unsupported)?;
    let captured = input::capture(&f.root().join("manifest.json"))?;
    let c = crate::doc::source::compiled()?;
    // No Ready TeX is present, so even a configured host is not invoked.
    let execute = |limits| {
        let mut b = Budget::new(limits);
        let result = (|| -> Result<_, String> {
            b.charge(Resource::Work, 23)
                .map_err(crate::doc::source::err)?;
            b.with_depth(|b| {
                Ok::<_, StopReason>(
                    generate::run_selected(&c, &captured, MathRenderer::KaTeXPreferred, b)
                        .map(|g| g.files),
                )
            })
            .map_err(crate::doc::source::err)?
        })();
        (result, b)
    };
    let (baseline, full) = execute(crate::doc::source::budget().limits());
    let baseline = baseline?;
    for dimension in 0..7 {
        let amount = |u: Usage| match dimension {
            0 => u.work,
            1 => u.allocation_units,
            2 => u.nodes,
            3 => u.depth,
            4 => u.output_bytes,
            5 => u.source_bytes,
            _ => u.diagnostics,
        };
        let select = |l: &mut Limits, n| match dimension {
            0 => l.work = n,
            1 => l.allocation_units = n,
            2 => l.nodes = n,
            3 => l.depth = n,
            4 => l.output_bytes = n,
            5 => l.source_bytes = n,
            _ => l.diagnostics = n,
        };
        let mut cap = amount(full.usage());
        assert!(cap > 0);
        let mut settled = false;
        for _ in 0..32 {
            let mut limits = full.limits();
            select(&mut limits, cap);
            let (result, used) = execute(limits);
            assert_eq!(result?, baseline);
            let next = amount(used.usage());
            assert!(next <= cap);
            if next == cap {
                settled = true;
                break;
            }
            cap = next;
        }
        assert!(settled);
        let expected = match dimension {
            0 => StopReason::WorkLimit,
            1 => StopReason::AllocationLimit,
            2 => StopReason::NodeLimit,
            3 => StopReason::DepthLimit,
            4 => StopReason::OutputLimit,
            5 => StopReason::SourceLimit,
            _ => StopReason::DiagnosticLimit,
        };
        let mut limits = full.limits();
        select(&mut limits, cap - 1);
        let (result, mut b) = execute(limits);
        assert!(result.is_err());
        assert_eq!(b.poll(), Err(expected));
        let before = b.usage();
        assert_eq!(b.charge(Resource::Work, 1), Err(expected));
        assert_eq!(b.usage(), before);
    }
    Ok(())
}

mod external;
