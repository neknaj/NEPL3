use super::*;
#[test]
fn external_svg_cli_exports_actual_sources() -> Result<(), Box<dyn std::error::Error>> {
    let root = root("external-modes")?;
    fs::write(root.join("a.nepld"), A)?;
    fs::write(root.join("b.nepld"), B)?;
    for (name, width) in [("a", 100), ("b", 400)] {
        fs::write(
            root.join(format!("{name}.svg")),
            format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1L0 1Z'/></svg>"
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
            .args([
                "doc-html",
                "pages-svg",
                "--svg",
                "external",
                "--math-renderer",
                "mathml-only",
            ])
            .arg(&manifest)
            .arg(&out)
            .output()?;
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let html = fs::read_to_string(out.join("deep/a.html"))?;
        assert!(!html.contains("data:image/svg+xml;base64,"));
        assert!(html.contains("img-src 'self';"));
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
        assert_eq!(receipt["svg"], "external");
        assert_eq!(
            receipt["format"],
            "nepl3.local-doc-pages.root-svg-external-package/1"
        );
        assert_eq!(receipt["files"].as_array().ok_or("files")?.len(), 6);
        for record in receipt["files"].as_array().ok_or("files")? {
            let relative = record["path"].as_str().ok_or("path")?;
            let bytes = fs::read(out.join(relative))?;
            assert_eq!(
                record["sha256"],
                hex(nepl3_core::source::Digest::of(&bytes))
            );
        }

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
            .args([
                "doc-html",
                "pages-svg",
                "--svg",
                "external",
                "--math-renderer",
                "mathml-only",
            ])
            .arg(&manifest)
            .arg(&out)
            .output()?;
        assert!(!again.status.success());
        assert_eq!(fs::read(out.join("manifest.json"))?, bytes);
    }
    fs::remove_dir_all(root)?;
    Ok(())
}

fn hex(value: nepl3_core::source::Digest) -> String {
    value.0.iter().map(|byte| format!("{byte:02x}")).collect()
}
pub(super) fn tree(
    root: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
    fn visit(
        root: &std::path::Path,
        path: &std::path::Path,
        out: &mut std::collections::BTreeMap<String, Vec<u8>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                visit(root, &path, out)?;
            } else {
                out.insert(
                    path.strip_prefix(root)?
                        .to_str()
                        .ok_or("path")?
                        .replace('\\', "/"),
                    fs::read(path)?,
                );
            }
        }
        Ok(())
    }
    let mut out = std::collections::BTreeMap::new();
    visit(root, root, &mut out)?;
    Ok(out)
}
#[test]
fn external_svg_cli_aliases_rejections_and_precedence() -> Result<(), Box<dyn std::error::Error>> {
    let root = root("external-options")?;
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    fs::write(root.join("one.nepld"), "article en \"No math\" body nil")?;
    let manifest = root.join("pages.json");
    fs::write(
        &manifest,
        serde_json::to_vec(
            &serde_json::json!({"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"}],"assets":[]}),
        )?,
    )?;
    for (name, flags) in [
        ("default", vec![]),
        ("embedded", vec!["--svg", "embedded"]),
        (
            "external-first",
            vec!["--svg", "external", "--math-renderer", "mathml-only"],
        ),
        (
            "renderer-first",
            vec!["--math-renderer", "mathml-only", "--svg", "external"],
        ),
    ] {
        let result = Command::new(binary)
            .args(["doc-html", "pages-svg"])
            .args(flags)
            .env_remove("NEPL3_KATEX_NODE_MODULES")
            .arg(&manifest)
            .arg(root.join(name))
            .output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert_eq!(tree(&root.join("default"))?, tree(&root.join("embedded"))?);
    assert_eq!(
        tree(&root.join("external-first"))?,
        tree(&root.join("renderer-first"))?
    );
    let external: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("external-first/manifest.json"))?)?;
    assert_eq!(external["svg"], "external");
    assert_eq!(external["files"].as_array().ok_or("files")?.len(), 2);
    for (i, (flags, expected)) in [
        (
            vec!["--svg", "bogus"],
            "SVG mode must be embedded or external",
        ),
        (
            vec!["--svg", "bogus", "--math-renderer", "bogus"],
            "expected katex-preferred or mathml-only",
        ),
        (vec!["--svg", "external", "--svg", "embedded"], "usage:"),
        (
            vec!["--svg", "external", "--interactive-limits", "missing.json"],
            "--interactive-limits must precede",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let out = root.join(format!("bad-{i}"));
        let result = Command::new(binary)
            .args(["doc-html", "pages-svg"])
            .args(flags)
            .arg(root.join("missing-manifest.json"))
            .arg(&out)
            .output()?;
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(expected));
        assert!(!out.exists());
    }
    let existing = root.join("existing");
    fs::create_dir(&existing)?;
    fs::write(existing.join("keep"), "original")?;
    let result = Command::new(binary)
        .args(["doc-html", "pages-svg", "--svg", "external"])
        .arg(root.join("missing-manifest.json"))
        .arg(&existing)
        .output()?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("output directory already exists"));
    assert_eq!(fs::read(existing.join("keep"))?, b"original");
    fs::copy(&manifest, root.join("--pages.json"))?;
    let bare = Command::new(binary)
        .current_dir(&root)
        .args([
            "doc-html",
            "pages-svg",
            "--svg",
            "external",
            "--pages.json",
            "bare-out",
        ])
        .output()?;
    assert!(!bare.status.success());
    assert!(!root.join("bare-out").exists());
    let escaped = Command::new(binary)
        .current_dir(&root)
        .args([
            "doc-html",
            "pages-svg",
            "--svg",
            "external",
            "./--pages.json",
            "escaped-out",
        ])
        .output()?;
    assert!(
        escaped.status.success(),
        "{}",
        String::from_utf8_lossy(&escaped.stderr)
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

pub(super) fn pinned_external_cases(
    binary: &str,
    packages: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root("external-pinned")?;
    fs::write(root.join("a.nepld"), A)?;
    fs::write(root.join("b.nepld"), B)?;
    for (name, width) in [("a", 100), ("b", 400)] {
        fs::write(
            root.join(format!("{name}.svg")),
            format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1L0 1Z'/></svg>"
            ),
        )?;
    }
    let path = root.join("pages.json");
    fs::write(
        &path,
        serde_json::to_vec(
            &serde_json::json!({"version":1,"pages":[{"id":"one","source":"a.nepld","route":"deep/a.html"},{"id":"two","source":"b.nepld","route":"deep/b.html"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"a.svg","mime":"image/svg+xml"}]},{"page":"two","assets":[{"id":"logo","source":"b.svg","mime":"image/svg+xml"}]}]}),
        )?,
    )?;
    let run = |name: &str,
               flags: &[&str],
               configured: bool|
     -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut cmd = Command::new(binary);
        cmd.args(["doc-html", "pages-svg"]).args(flags);
        if configured {
            cmd.env("NEPL3_KATEX_NODE_MODULES", packages);
        } else {
            cmd.env_remove("NEPL3_KATEX_NODE_MODULES")
                .env("NEPL3_KATEX_NODE", root.join("missing-node"));
        }
        let result = cmd.arg(&path).arg(root.join(name)).output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = fs::read(root.join(name).join("manifest.json"))?;
        Ok(serde_json::from_slice(&bytes)?)
    };
    let default = run("default", &["--svg", "external"], true)?;
    let explicit = run(
        "explicit",
        &["--math-renderer", "katex-preferred", "--svg", "external"],
        true,
    )?;
    assert_eq!(tree(&root.join("default"))?, tree(&root.join("explicit"))?);
    assert_eq!(default, explicit);
    assert_eq!(
        default["format"],
        "nepl3.local-doc-pages.root-svg-external-package/2"
    );
    assert_eq!(default["svg"], "external");
    assert_eq!(default["source_input"]["math_renderer"], "katex-preferred");
    assert_eq!(default["katex_generation"]["successful_visual_pages"], 2);
    assert_eq!(default["katex_generation"]["successful_visuals"], 4);
    assert_eq!(
        default["katex_generation"]["host_invocation_recorded"],
        false
    );
    assert_eq!(default["files"].as_array().ok_or("files")?.len(), 6);
    let html = fs::read_to_string(root.join("default/deep/a.html"))?;
    assert!(html.contains("katex-html") && html.contains("<math"));
    assert!(html.contains("img-src 'self';") && !html.contains("data:image/svg+xml;base64,"));
    let embedded = run("embedded", &[], true)?;
    assert_ne!(default["input_identity"], embedded["input_identity"]);
    let external_pages = default["source_input"]["native_generation"]["pages"]
        .as_array()
        .ok_or("external pages")?;
    let embedded_pages = embedded["source_input"]["native_generation"]["pages"]
        .as_array()
        .ok_or("embedded pages")?;
    assert_eq!(external_pages.len(), 2);
    assert_eq!(embedded_pages.len(), 2);
    for (external, embedded) in external_pages.iter().zip(embedded_pages) {
        assert_ne!(
            external["request_context"]
                .as_str()
                .ok_or("external context")?,
            embedded["request_context"]
                .as_str()
                .ok_or("embedded context")?
        );
    }
    for name in ["a", "b"] {
        let bytes = fs::read(root.join(format!("{name}.svg")))?;
        let relative = format!(
            "default/deep/assets/{}.svg",
            hex(nepl3_core::source::Digest::of(&bytes))
        );
        assert_eq!(fs::read(root.join(relative))?, bytes);
    }
    let mathml = run(
        "mathml-bad-host",
        &["--svg", "external", "--math-renderer", "mathml-only"],
        false,
    )?;
    let _ = run(
        "mathml-configured",
        &["--svg", "external", "--math-renderer", "mathml-only"],
        true,
    )?;
    assert_eq!(
        tree(&root.join("mathml-bad-host"))?,
        tree(&root.join("mathml-configured"))?
    );
    assert_eq!(
        mathml["format"],
        "nepl3.local-doc-pages.root-svg-external-package/1"
    );
    assert_eq!(mathml["katex_generation"], false);
    assert!(mathml["source_input"]["native_generation"].is_null());
    let absent = run("preferred-absent", &["--svg", "external"], false)?;
    assert_eq!(
        absent["format"],
        "nepl3.local-doc-pages.root-svg-external-package/2"
    );
    assert_eq!(absent["katex_generation"]["successful_visuals"], 0);
    assert_eq!(
        absent["katex_generation"]["host_invocation_recorded"],
        false
    );
    fs::remove_dir_all(root)?;
    Ok(())
}
