use super::*;
fn run(
    c: &crate::doc::source::Compiled,
    captured: &input::Captured,
    b: &mut nepl3_core::budget::Budget,
) -> Result<generate::Generated, String> {
    generate::run_external_selected(c, captured, MathRenderer::MathMLOnly, b)
}
#[test]
fn svg_pages_external_cli_capture_is_immutable_and_records_explicit_math_selection()
-> crate::Result<()> {
    let f = fixture()?;
    let captured = input::capture(&f.root().join("manifest.json"))?;
    f.write("one.nepld", "changed invalid Doc")?;
    f.write("one.svg", "changed invalid SVG")?;
    let c = crate::doc::source::compiled()?;
    let mut b = crate::doc::source::budget();
    let output = run(&c, &captured, &mut b)?;
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
    assert_eq!(
        receipt["format"],
        "nepl3.local-doc-pages.root-svg-external-package/1"
    );
    assert_eq!(receipt["svg"], "external");
    assert_eq!(output.files.len(), 5);
    let svg_path = format!(
        "deep/assets/{}.svg",
        crate::doc::export::digest_hex(nepl3_core::source::Digest::of(SVG.as_bytes()))
    );
    assert_eq!(output.files.get(&svg_path).ok_or("SVG")?, SVG.as_bytes());
    let embedded = generate::run_selected(
        &c,
        &captured,
        MathRenderer::MathMLOnly,
        &mut crate::doc::source::budget(),
    )?;
    let embedded_receipt: serde_json::Value = serde_json::from_str(&embedded.manifest)?;
    assert_ne!(
        receipt["input_identity"],
        embedded_receipt["input_identity"]
    );
    assert_eq!(
        receipt["source_input"]["svg_capture"],
        embedded_receipt["source_input"]["svg_capture"]
    );
    let html = std::str::from_utf8(&output.files["deep/a.html"])?;
    assert!(html.contains("img-src 'self';"));
    assert!(!html.contains("data:image/svg+xml;base64,"));
    assert!(html.contains("b.html#n-746172676574"));
    assert!(!html.contains("katex-html"));
    let changed = input::capture(&f.root().join("manifest.json"))?;
    assert!(run(&c, &changed, &mut crate::doc::source::budget()).is_err());
    Ok(())
}
#[test]
fn svg_pages_external_cli_failure_does_not_create_output_or_hide_stop() -> crate::Result<()> {
    let f = fixture()?;
    let output = f.root().join("output");
    let mut v = manifest();
    v["assets"][0]["assets"][0]["id"] = json!("unused");
    f.json("manifest.json", &v)?;
    assert!(
        write_external(
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
    assert!(run(&crate::doc::source::compiled()?, &captured, &mut b).is_err());
    assert_eq!(b.poll(), Err(nepl3_core::budget::StopReason::Cancelled));
    Ok(())
}

#[test]
fn svg_pages_external_cli_source_generation_preserves_exact_limits_and_preconsumption()
-> crate::Result<()> {
    use nepl3_core::budget::{Budget, Limits, Resource, StopReason, Usage};
    let f = fixture()?;
    let captured = input::capture(&f.root().join("manifest.json"))?;
    let c = crate::doc::source::compiled()?;
    let execute = |limits| {
        let mut b = Budget::new(limits);
        let result = (|| -> Result<_, String> {
            b.charge(Resource::Work, 17)
                .map_err(crate::doc::source::err)?;
            b.with_depth(|b| Ok::<_, StopReason>(run(&c, &captured, b).map(|g| g.files)))
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
fn svg_pages_external_preferred_metadata_keeps_seven_resource_boundaries() -> crate::Result<()> {
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
                    generate::run_external_selected(&c, &captured, MathRenderer::KaTeXPreferred, b)
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
