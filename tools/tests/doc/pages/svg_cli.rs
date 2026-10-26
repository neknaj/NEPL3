//! Actual native CLI tests; no process-spawning assertion runs on WASI.
use std::{fs, process::Command};
fn root(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!(
        "nepl-svg-pages-cli-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&path)?;
    Ok(path)
}
const A: &str = r#"article en Sentence "One" body cons image asset "logo" none Sentence "[漢/かん]" none cons paragraph cons parallel cons variant en Sentence "English display" cons variant ja Sentence "Japanese display" nil cons Sentence sentence cons math frac 1 2 cons document math Math frac 3 4 cons document link page "two" some "target" text "Next" nil cons code Math frac 5 6 nil cons display Math frac 7 8 nil"#;
const B: &str = r#"article en "Two" body cons image asset "logo" none "Two image" none cons section target "Target" body cons paragraph cons sentence cons math Math frac 9 10 nil nil nil nil"#;
#[test]
fn svg_pages_cli_exports_actual_sources_with_explicit_selection()
-> Result<(), Box<dyn std::error::Error>> {
    let root = root("modes")?;
    fs::write(root.join("a.nepld"), A)?;
    fs::write(root.join("b.nepld"), B)?;
    for (name, width) in [("a", 100), ("b", 400)] {
        fs::write(
            root.join(format!("{name}.svg")),
            format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>"
            ),
        )?;
    }
    let manifest = root.join("pages.json");
    for (n, (display, en, ja)) in [
        (serde_json::json!({"mode":"rows"}), true, true),
        (serde_json::json!({"mode":"columns"}), true, true),
        (
            serde_json::json!({"mode":"single","language":"fr","fallbacks":["ja"]}),
            false,
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let value = serde_json::json!({"version":1,"display":display,"pages":[{"id":"one","source":"a.nepld","route":"deep/a.html"},{"id":"two","source":"b.nepld","route":"deep/b.html"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"a.svg","mime":"image/svg+xml"}]},{"page":"two","assets":[{"id":"logo","source":"b.svg","mime":"image/svg+xml"}]}]});
        fs::write(&manifest, serde_json::to_vec(&value)?)?;
        let out = root.join(format!("out-{n}"));
        let run = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(["doc-html", "pages-svg", "--math-renderer", "mathml-only"])
            .arg(&manifest)
            .arg(&out)
            .output()?;
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let html = fs::read_to_string(out.join("deep/a.html"))?;
        assert!(html.contains("data:image/svg+xml;base64,"));
        assert!(html.contains("img-src data:;"));
        assert!(html.contains("<math"));
        assert!(html.contains("nepl-code-marker"));
        assert_eq!(html.contains("English display"), en);
        assert_eq!(html.contains("Japanese display"), ja);
        let bytes = fs::read(out.join("manifest.json"))?;
        let receipt: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            receipt["source_input"]["math_renderer"],
            "mathml-only-explicit"
        );
        assert_eq!(receipt["katex_generation"], false);
        let math = receipt["source_input"]["math"].as_array().ok_or("math")?;
        assert_eq!(math.len(), 2);
        assert_eq!(math[0].as_array().ok_or("first")?.len(), 3);
        assert_eq!(math[1].as_array().ok_or("second")?.len(), 1);
        for page in math {
            for record in page.as_array().ok_or("page")? {
                assert_eq!(record["selection_reason"], "ExplicitMathMLOnly");
                assert!(record["fallback_reason"].is_null());
            }
        }
        let again = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(["doc-html", "pages-svg", "--math-renderer", "mathml-only"])
            .arg(&manifest)
            .arg(&out)
            .output()?;
        assert!(!again.status.success());
        assert_eq!(fs::read(out.join("manifest.json"))?, bytes);
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
#[test]
fn svg_pages_cli_rejects_invalid_order_and_durable_restore()
-> Result<(), Box<dyn std::error::Error>> {
    let root = root("flags")?;
    fs::write(root.join("one.nepld"), "article en \"No math\" body nil")?;
    let manifest = root.join("pages.json");
    fs::write(
        &manifest,
        serde_json::to_vec(
            &serde_json::json!({"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"}],"assets":[]}),
        )?,
    )?;
    for (n, args) in [
        vec!["doc-html", "pages-svg", "--restore", "snapshot.json"],
        vec![
            "doc-html",
            "pages-svg",
            "--svg",
            "external",
            "--interactive-limits",
            "limits.json",
        ],
    ]
    .into_iter()
    .enumerate()
    {
        let out = root.join(format!("bad-{n}"));
        let run = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(args)
            .arg(&manifest)
            .arg(&out)
            .output()?;
        assert!(!run.status.success());
        assert!(!out.exists());
        assert!(String::from_utf8_lossy(&run.stderr).contains("not implemented"));
    }
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn svg_pages_katex_default_is_requested_without_inventing_host_invocation()
-> Result<(), Box<dyn std::error::Error>> {
    let root = root("preferred-absent")?;
    let path = root.join("pages.json");
    fs::write(root.join("a.nepld"), A)?;
    fs::write(root.join("b.nepld"), B)?;
    for name in ["a", "b"] {
        fs::write(
            root.join(format!("{name}.svg")),
            "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>",
        )?;
    }
    let value = serde_json::json!({"version":1,"pages":[{"id":"one","source":"a.nepld","route":"a.html"},{"id":"two","source":"b.nepld","route":"b.html"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"a.svg","mime":"image/svg+xml"}]},{"page":"two","assets":[{"id":"logo","source":"b.svg","mime":"image/svg+xml"}]}]});
    fs::write(&path, serde_json::to_vec(&value)?)?;
    let mut original = None;
    for (n, args) in [
        vec!["doc-html", "pages-svg"],
        vec![
            "doc-html",
            "pages-svg",
            "--math-renderer",
            "katex-preferred",
        ],
    ]
    .into_iter()
    .enumerate()
    {
        let out = root.join(format!("out-{n}"));
        let run = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .env_remove("NEPL3_KATEX_NODE_MODULES")
            .args(args)
            .arg(&path)
            .arg(&out)
            .output()?;
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let bytes = fs::read(out.join("manifest.json"))?;
        let receipt: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            receipt["format"],
            "nepl3.local-doc-pages.root-svg-package/2"
        );
        assert_eq!(receipt["katex_generation"]["successful_visuals"], 0);
        assert_eq!(
            receipt["katex_generation"]["host_invocation_recorded"],
            false
        );
        assert_eq!(
            receipt["katex_generation"]["host_internal_work"],
            "unobserved"
        );
        assert_eq!(receipt["source_input"]["math_renderer"], "katex-preferred");
        for page in receipt["source_input"]["math"].as_array().ok_or("pages")? {
            for m in page.as_array().ok_or("math")? {
                assert_eq!(m["renderer"], "MathML");
                assert_eq!(m["selection_reason"], "KaTeXPreferred");
                assert_eq!(m["fallback_reason"], "KaTeXHostCapabilityUnavailable");
            }
        }
        if let Some(old) = &original {
            assert_eq!(old, &bytes);
        } else {
            original = Some(bytes);
        }
        assert!(!fs::read_to_string(out.join("a.html"))?.contains("katex-html"));
    }
    // Even with no formulas, the default request remains explicit provenance,
    // rather than pretending a successful native invocation took place.
    fs::write(root.join("a.nepld"), "article en \"No math\" body nil")?;
    fs::write(
        &path,
        serde_json::to_vec(
            &serde_json::json!({"version":1,"pages":[{"id":"one","source":"a.nepld","route":"a.html"}],"assets":[]}),
        )?,
    )?;
    let out = root.join("empty");
    let run = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .env_remove("NEPL3_KATEX_NODE_MODULES")
        .args(["doc-html", "pages-svg"])
        .arg(&path)
        .arg(&out)
        .output()?;
    assert!(run.status.success());
    let receipt: serde_json::Value = serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
    assert_eq!(receipt["katex_generation"]["successful_visual_pages"], 0);
    assert_eq!(
        receipt["source_input"]["native_generation"]["pages"][0]["node_version"],
        serde_json::Value::Null
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
#[ignore = "requires Node >=24 and locked tools/audit/math npm packages"]
fn pinned_native_katex_doc_and_pages_svg_cli() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeSet;
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repo")?;
    let packages = repo.join("tools/audit/math/node_modules").canonicalize()?;
    let root = root("preferred-pinned")?;
    let path = root.join("pages.json");
    fs::write(root.join("a.nepld"), A)?;
    fs::write(root.join("b.nepld"), B)?;
    for name in ["a", "b"] {
        fs::write(
            root.join(format!("{name}.svg")),
            "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>",
        )?;
    }
    let mut value = serde_json::json!({"version":1,"pages":[{"id":"one","source":"a.nepld","route":"a.html"},{"id":"two","source":"b.nepld","route":"b.html"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"a.svg","mime":"image/svg+xml"}]},{"page":"two","assets":[{"id":"logo","source":"b.svg","mime":"image/svg+xml"}]}]});
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    let run = |name: &str,
               renderer: Option<&str>,
               modules: Option<&std::path::Path>|
     -> Result<(std::path::PathBuf, serde_json::Value), Box<dyn std::error::Error>> {
        let out = root.join(name);
        let mut cmd = Command::new(binary);
        cmd.args(["doc-html", "pages-svg"]);
        if let Some(renderer) = renderer {
            cmd.args(["--math-renderer", renderer]);
        }
        if let Some(modules) = modules {
            cmd.env("NEPL3_KATEX_NODE_MODULES", modules);
        } else {
            cmd.env_remove("NEPL3_KATEX_NODE_MODULES");
        }
        let result = cmd.arg(&path).arg(&out).output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let receipt = serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
        Ok((out, receipt))
    };
    for (i, display) in [
        serde_json::json!({"mode":"rows"}),
        serde_json::json!({"mode":"columns"}),
        serde_json::json!({"mode":"single","language":"fr","fallbacks":["ja"]}),
    ]
    .into_iter()
    .enumerate()
    {
        value["display"] = display;
        fs::write(&path, serde_json::to_vec(&value)?)?;
        let (out, receipt) = run(
            &format!("native-{i}"),
            if i == 0 {
                None
            } else {
                Some("katex-preferred")
            },
            Some(&packages),
        )?;
        assert_eq!(
            receipt["format"],
            "nepl3.local-doc-pages.root-svg-package/2"
        );
        assert_eq!(receipt["katex_generation"]["successful_visual_pages"], 2);
        assert_eq!(receipt["katex_generation"]["successful_visuals"], 4);
        assert_eq!(
            receipt["katex_generation"]["host_invocation_recorded"],
            false
        );
        let pages = receipt["source_input"]["native_generation"]["pages"]
            .as_array()
            .ok_or("native pages")?;
        assert_eq!(pages.len(), 2);
        assert_ne!(pages[0]["request_context"], pages[1]["request_context"]);
        assert!(
            !pages[0]["files"]
                .as_array()
                .ok_or("native files")?
                .is_empty()
        );
        assert_eq!(pages[0]["files"], pages[1]["files"]);
        let mut scopes = BTreeSet::new();
        for page in pages {
            assert!(
                page["node_version"]
                    .as_str()
                    .ok_or("version")?
                    .split('.')
                    .next()
                    .ok_or("major")?
                    .parse::<u64>()?
                    >= 24
            );
            for visual in page["visuals"].as_array().ok_or("visuals")? {
                assert!(scopes.insert(visual["scope"].as_str().ok_or("scope")?));
            }
        }
        assert_eq!(scopes.len(), 4);
        for page in receipt["source_input"]["math"].as_array().ok_or("math")? {
            for m in page.as_array().ok_or("formula")? {
                assert_eq!(m["renderer"], "KaTeX+MathML");
                assert!(m["fallback_reason"].is_null());
            }
        }
        for name in ["a", "b"] {
            let html = fs::read_to_string(out.join(format!("{name}.html")))?;
            assert!(
                html.contains("katex-html")
                    && html.contains("<math")
                    && html.contains("data:image/svg+xml;base64,")
            );
            assert!(html.contains("style-src-attr 'none'") && html.contains("font-src data:"));
        }
        for record in receipt["files"].as_array().ok_or("files")? {
            let name = record["path"].as_str().ok_or("path")?;
            if name.ends_with(".css") {
                let css = fs::read_to_string(out.join(name))?;
                assert!(
                    css.contains("data:font/woff2;base64,")
                        && css.contains("Permission is hereby granted")
                );
                assert!(!css.contains("url(fonts/"));
            }
        }
    }
    // The same explicit MathML operation must not inspect or invoke the host.
    let (with_host, with_receipt) = run("mathml-configured", Some("mathml-only"), Some(&packages))?;
    let (without_host, without_receipt) = run("mathml-absent", Some("mathml-only"), None)?;
    assert_eq!(with_receipt, without_receipt);
    for record in with_receipt["files"].as_array().ok_or("files")? {
        let name = record["path"].as_str().ok_or("path")?;
        assert_eq!(
            fs::read(with_host.join(name))?,
            fs::read(without_host.join(name))?
        );
    }
    assert_eq!(
        with_receipt["format"],
        "nepl3.local-doc-pages.root-svg-package/1"
    );
    assert_eq!(with_receipt["katex_generation"], false);
    // Mixed and wholly unsupported formulas retain faithful fallback detail.
    value = serde_json::json!({"version":1,"pages":[{"id":"one","source":"a.nepld","route":"a.html"}],"assets":[]});
    fs::write(&path, serde_json::to_vec(&value)?)?;
    for (name, source, visuals) in [
        (
            "mixed",
            r#"article en "Mixed" body cons display Math frac 1 2 cons display Math label add x y Sentence "[字/じ]" nil"#,
            1,
        ),
        (
            "unsupported",
            r#"article en "Unsupported" body cons display Math label add x y Sentence "[字/じ]" nil"#,
            0,
        ),
    ] {
        fs::write(root.join("a.nepld"), source)?;
        let (_, receipt) = run(name, None, Some(&packages))?;
        assert_eq!(receipt["katex_generation"]["successful_visuals"], visuals);
        assert!(
            receipt["source_input"]["math"][0]
                .as_array()
                .ok_or("math")?
                .iter()
                .any(|m| m["fallback_reason"] == "FaithfulTexUnsupported")
        );
        assert!(
            receipt["source_input"]["native_generation"]["pages"][0]["formulas"]
                .as_array()
                .ok_or("formulas")?
                .iter()
                .any(|m| m["unsupported"]["reason"] == "ForeignAnnotation")
        );
    }
    fs::write(root.join("a.nepld"), super::export::sentence::NESTED)?;
    let (_, receipt) = run("nested", None, Some(&packages))?;
    assert_eq!(
        receipt["source_input"]["math"][0]
            .as_array()
            .ok_or("math")?
            .len(),
        5
    );
    assert_eq!(receipt["katex_generation"]["successful_visuals"], 4);
    let basic = receipt["source_input"]["math"][0]
        .as_array()
        .ok_or("basic math")?;
    let detail = receipt["source_input"]["native_generation"]["pages"][0]["formulas"]
        .as_array()
        .ok_or("detail math")?;
    assert_eq!(basic.len(), detail.len());
    for (basic, detail) in basic.iter().zip(detail) {
        assert_eq!(basic["root"], detail["root"]);
        assert_eq!(basic["owner_path"], detail["owner_path"]);
    }
    let paths = basic
        .iter()
        .map(|m| {
            m["owner_path"]
                .as_array()
                .ok_or("owner path")
                .and_then(|p| {
                    p.iter()
                        .map(|step| step["kind"].as_str().ok_or("kind"))
                        .collect::<Result<Vec<_>, _>>()
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert!(paths.iter().any(|p| p
        == &[
            "sentence",
            "sentence-foreign",
            "math-annotation",
            "sentence-foreign"
        ]));
    assert!(
        paths
            .iter()
            .any(|p| p == &["sentence", "sentence-foreign", "document-math"])
    );
    fs::write(
        root.join("a.nepld"),
        "article en \"Missing host package\" body cons display Math frac 1 2 nil",
    )?;
    let (_, receipt) = run(
        "missing-packages",
        None,
        Some(&root.join("missing-node-modules")),
    )?;
    assert_eq!(receipt["katex_generation"]["successful_visuals"], 0);
    assert_eq!(
        receipt["source_input"]["math"][0][0]["fallback_reason"],
        "KaTeXResourceUnavailable"
    );
    assert!(
        !receipt["source_input"]["native_generation"]["pages"][0]["formulas"][0]["fallback_detail"]
            .as_str()
            .ok_or("detail")?
            .is_empty()
    );
    // A failed middle page must not leak fonts/CSP and must not discard the
    // most recent successful immutable asset cache for the third page.
    let mut pages = Vec::new();
    let mut assets = Vec::new();
    for (i, math) in ["frac 1 2", "text \"🙂\"", "frac 3 4"]
        .into_iter()
        .enumerate()
    {
        let name = format!("gap{i}");
        fs::write(
            root.join(format!("{name}.nepld")),
            format!(
                "article en \"{name}\" body cons image asset \"logo\" none \"diagram\" none cons display Math {math} nil"
            ),
        )?;
        pages.push(serde_json::json!({"id":name,"source":format!("{name}.nepld"),"route":format!("{name}.html")}));
        assets.push(serde_json::json!({"page":name,"assets":[{"id":"logo","source":"a.svg","mime":"image/svg+xml"}]}));
    }
    fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({"version":1,"pages":pages,"assets":assets}))?,
    )?;
    let (out, receipt) = run("cache-gap", None, Some(&packages))?;
    assert_eq!(receipt["katex_generation"]["successful_visual_pages"], 2);
    let native = receipt["source_input"]["native_generation"]["pages"]
        .as_array()
        .ok_or("native pages")?;
    assert!(
        native[1]["visuals"]
            .as_array()
            .ok_or("gap visuals")?
            .is_empty()
    );
    assert!(native[1]["files"].as_array().ok_or("gap files")?.is_empty());
    assert_eq!(native[0]["files"], native[2]["files"]);
    assert_ne!(
        native[0]["visuals"][0]["scope"],
        native[2]["visuals"][0]["scope"]
    );
    assert_eq!(
        receipt["source_input"]["math"][1][0]["fallback_reason"],
        "KaTeXRendererParseError"
    );
    for (i, native) in [true, false, true].into_iter().enumerate() {
        let html = fs::read_to_string(out.join(format!("gap{i}.html")))?;
        assert_eq!(html.contains("katex-html"), native);
        assert_eq!(html.contains("font-src data:"), native);
        let route = format!("gap{i}.html");
        let css = out.join(format!(
            "assets/doc-{}.css",
            nepl3_core::source::Digest::of(route.as_bytes())
                .0
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ));
        assert_eq!(
            fs::read_to_string(css)?.contains("data:font/woff2;base64,"),
            native
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let executable = Command::new("node")
            .args(["-e", "process.stdout.write(process.execPath)"])
            .output()?;
        assert!(executable.status.success());
        let executable = String::from_utf8(executable.stdout)?;
        assert!(std::path::Path::new(&executable).is_absolute());
        let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        let mocks = root.join("mock-node");
        fs::create_dir(&mocks)?;
        let node = mocks.join("node");
        let mut paths = vec![mocks];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").ok_or("PATH")?,
        ));
        let mock_path = std::env::join_paths(paths)?;
        for (name, response, expected) in [
            (
                "violation",
                r#"{"kind":"provider-violation","reason":"controlled-provider"}"#,
                "KaTeXHostViolation",
            ),
            (
                "stopped",
                r#"{"kind":"stopped","reason":"controlled-stop"}"#,
                "Cancelled",
            ),
        ] {
            // Controlled authenticated protocol failures, not simulated success.
            let script = root.join(format!("mock-{name}.cjs"));
            fs::write(
                &script,
                format!(
                    "let s='';process.stdin.setEncoding('utf8');process.stdin.on('data',x=>s+=x);process.stdin.on('end',()=>{{const r=JSON.parse({});r.identity=JSON.parse(s).identity;process.stdout.write(JSON.stringify(r));}});",
                    serde_json::to_string(response)?
                ),
            )?;
            fs::write(
                &node,
                format!(
                    "#!/bin/sh\nexec {} {}\n",
                    quote(&executable),
                    quote(script.to_str().ok_or("script path")?)
                ),
            )?;
            fs::set_permissions(&node, fs::Permissions::from_mode(0o700))?;
            let out = root.join(format!("host-{name}"));
            let result = Command::new(binary)
                .env("PATH", &mock_path)
                .env("NEPL3_KATEX_NODE_MODULES", &packages)
                .args(["doc-html", "pages-svg"])
                .arg(&path)
                .arg(&out)
                .output()?;
            assert!(!result.status.success());
            assert!(!out.exists());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(expected),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
    // Native success includes font/visual evidence and CSS. Final counters
    // are the recorded pre-receipt snapshot plus the bounded Writer's bytes.
    fs::write(
        root.join("a.nepld"),
        "article en \"Bounds\" body cons image asset \"logo\" none \"bounded\" none cons display Math frac 1 2 nil",
    )?;
    let base = serde_json::json!({"version":1,"pages":[{"id":"one","source":"a.nepld","route":"a.html"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"a.svg","mime":"image/svg+xml"}]}]});
    fs::write(&path, serde_json::to_vec(&base)?)?;
    let (baseline_out, baseline) = run("bounds-baseline", None, Some(&packages))?;
    assert_eq!(baseline["katex_generation"]["successful_visual_pages"], 1);
    assert_eq!(baseline["katex_generation"]["successful_visuals"], 1);
    let native_pages = baseline["source_input"]["native_generation"]["pages"]
        .as_array()
        .ok_or("native bounds pages")?;
    assert_eq!(native_pages.len(), 1);
    assert!(
        !native_pages[0]["files"]
            .as_array()
            .ok_or("native bounds files")?
            .is_empty()
    );
    let total = |receipt: &serde_json::Value,
                 bytes: usize,
                 field: &str|
     -> Result<u64, Box<dyn std::error::Error>> {
        let before = receipt["pre_receipt_usage"][field]
            .as_u64()
            .ok_or("usage")?;
        Ok(before + (bytes as u64) * if field == "allocation_units" { 2 } else { 1 })
    };
    let baseline_bytes = fs::read(baseline_out.join("manifest.json"))?.len();
    for field in ["work", "allocation_units", "output_bytes"] {
        let mut cap = total(&baseline, baseline_bytes, field)?;
        let mut settled = false;
        for iteration in 0..16 {
            let mut value = base.clone();
            value["output_limits"] = baseline["limits"].clone();
            value["output_limits"][field] = serde_json::json!(cap);
            fs::write(&path, serde_json::to_vec(&value)?)?;
            let (out, receipt) = run(
                &format!("bounds-{field}-{iteration}"),
                None,
                Some(&packages),
            )?;
            assert_eq!(receipt["files"], baseline["files"]);
            let used = total(&receipt, fs::read(out.join("manifest.json"))?.len(), field)?;
            assert!(used <= cap);
            if used == cap {
                settled = true;
                break;
            }
            cap = used;
        }
        assert!(settled, "native receipt fixed point");
        let mut value = base.clone();
        value["output_limits"] = baseline["limits"].clone();
        value["output_limits"][field] = serde_json::json!(cap - 1);
        fs::write(&path, serde_json::to_vec(&value)?)?;
        let out = root.join(format!("bounds-{field}-short"));
        let result = Command::new(binary)
            .env("NEPL3_KATEX_NODE_MODULES", &packages)
            .args(["doc-html", "pages-svg"])
            .arg(&path)
            .arg(&out)
            .output()?;
        assert!(!result.status.success());
        assert!(!out.exists());
        let reason = match field {
            "work" => "WorkLimit",
            "allocation_units" => "AllocationLimit",
            _ => "OutputLimit",
        };
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(reason),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    // Exercise the new real interactive CLI with pinned generation. A grant
    // during preparation also raises output allowance before its first render.
    {
        use std::io::Write;
        use std::process::Stdio;
        let defaults = Command::new(binary)
            .args(["doc-html", "retry-limits"])
            .output()?;
        assert!(defaults.status.success());
        let full: serde_json::Value = serde_json::from_slice(&defaults.stdout)?;
        let mut initial = full.clone();
        initial["parse_limits"]["work"] = serde_json::json!(0);
        initial["output_limits"]["work"] = serde_json::json!(0);
        fs::write(
            root.join("interactive-initial.json"),
            serde_json::to_vec(&initial)?,
        )?;
        fs::write(
            root.join("interactive-full.json"),
            serde_json::to_vec(&full)?,
        )?;
        fs::write(&path, serde_json::to_vec(&base)?)?;
        let out = root.join("interactive-native");
        let mut child = Command::new(binary)
            .env("NEPL3_KATEX_NODE_MODULES", &packages)
            .args(["doc-html", "pages-svg", "--interactive-limits"])
            .arg(root.join("interactive-initial.json"))
            .arg(&path)
            .arg(&out)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut input = child.stdin.take().ok_or("interactive stdin")?;
        writeln!(
            input,
            "resume {}",
            root.join("interactive-full.json").display()
        )?;
        drop(input);
        let result = child.wait_with_output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let messages = String::from_utf8_lossy(&result.stderr);
        assert!(messages.contains("Paused preparation"));
        assert!(!messages.contains("Paused render"));
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
        assert_eq!(receipt["format"], "nepl3.local-doc-pages.svg-render-job/1");
        assert_eq!(
            receipt["package"]["source_input"]["math_renderer"],
            "katex-preferred"
        );
        assert_eq!(
            receipt["package"]["katex_generation"]["successful_visual_pages"],
            1
        );
        assert_eq!(
            receipt["package"]["katex_generation"]["successful_visuals"],
            1
        );
        assert_eq!(
            receipt["job_execution"]["preparation"]["attempts"]
                .as_array()
                .ok_or("preparation")?
                .len(),
            2
        );
        assert_eq!(
            receipt["job_execution"]["render_attempts"]
                .as_array()
                .ok_or("renders")?
                .len(),
            1
        );
    }
    external::pinned_external_cases(binary, &packages)?;
    interactive::pinned_external_interactive_cases(binary, &packages)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[path = "svg_cli/interactive.rs"]
mod interactive;

#[path = "svg_cli/external.rs"]
mod external;
