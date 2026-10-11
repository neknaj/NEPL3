//! Exercise actual CLI dispatch and file sets rather than only in-memory export.
use std::{fs, process::Command};

#[test]
fn incomplete_code_input_publishes_no_files() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("nepl3-incomplete-code-{}", std::process::id()));
    fs::create_dir(&root)?;
    let source = root.join("incomplete.nepld");
    let incomplete = r#"article en "Host" body cons paragraph cons code Math frac 1 0 nil"#;
    fs::write(&source, incomplete)?;
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    for css in ["external", "inline"] {
        let output = root.join(css);
        let run = Command::new(binary)
            .args(["doc-html", "export", "--css", css])
            .arg(&source)
            .arg(&output)
            .output()?;
        assert!(!run.status.success());
        assert!(!output.exists(), "incomplete source published files");
        assert_eq!(fs::read_to_string(&source)?, incomplete);
    }
    let valid = r#"article en "Complete" body cons paragraph cons code Math frac 1 0 nil nil"#;
    fs::write(root.join("valid.nepld"), valid)?;
    let manifest = root.join("pages.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({"version":1,"pages":[
            {"id":"valid","source":"valid.nepld","route":"valid/index.html"},
            {"id":"incomplete","source":"incomplete.nepld","route":"incomplete/index.html"}
        ]}))?,
    )?;
    let output = root.join("pages");
    let run = Command::new(binary)
        .args(["doc-html", "pages"])
        .arg(&manifest)
        .arg(&output)
        .output()?;
    assert!(!run.status.success());
    assert!(
        !output.exists(),
        "a valid first page must not leak a partial batch"
    );
    assert_eq!(fs::read_to_string(&source)?, incomplete);
    assert_eq!(fs::read_to_string(root.join("valid.nepld"))?, valid);
    // Repair only the missing host tail: the same batch and output path now
    // succeed, so the negative case was not a path or CLI-dispatch failure.
    fs::write(&source, format!("{incomplete} nil"))?;
    let run = Command::new(binary)
        .args(["doc-html", "pages"])
        .arg(&manifest)
        .arg(&output)
        .output()?;
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(output.join("valid/index.html").is_file());
    assert!(output.join("incomplete/index.html").is_file());
    assert!(output.join("manifest.json").is_file());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn css_modes_cli_and_failure_boundaries() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("nepl3-css-cli-{}", std::process::id()));
    fs::create_dir(&root)?;
    let source = root.join("input.nepld");
    fs::write(
        &source,
        "article en \"Export\" body cons paragraph cons \"Hello.\" nil nil",
    )?;
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    for mode in ["external", "inline"] {
        let output = root.join(mode);
        let run = Command::new(binary)
            .args(["doc-html", "export", "--css", mode])
            .arg(&source)
            .arg(&output)
            .output()?;
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let html = fs::read_to_string(output.join("document.html"))?;
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
        assert_eq!(manifest["options"]["css"], mode);
        assert_eq!(output.join("assets").exists(), mode == "external");
        assert_eq!(html.contains("<style>"), mode == "inline");
        let again = Command::new(binary)
            .args(["doc-html", "export", "--css", mode])
            .arg(&source)
            .arg(&output)
            .output()?;
        assert!(!again.status.success());
        assert_eq!(fs::read_to_string(output.join("document.html"))?, html);
    }
    let default = root.join("default");
    assert!(
        Command::new(binary)
            .args(["doc-html", "export"])
            .arg(&source)
            .arg(&default)
            .status()?
            .success()
    );
    assert_eq!(
        fs::read(default.join("document.html"))?,
        fs::read(root.join("external/document.html"))?
    );
    for arguments in [
        vec!["--css", "unknown"],
        vec!["--css"],
        vec!["--css", "inline", "--css", "external"],
    ] {
        let output = root.join("invalid");
        let result = Command::new(binary)
            .args(["doc-html", "export"])
            .args(arguments)
            .arg(&source)
            .arg(&output)
            .output()?;
        assert!(!result.status.success());
        assert!(!output.exists());
    }
    fs::write(
        &source,
        vec![b' '; (nepl3_tools::doc::export::MAX_SOURCE_BYTES + 1) as usize],
    )?;
    let oversized = root.join("oversized");
    let result = Command::new(binary)
        .args(["doc-html", "export", "--css", "inline"])
        .arg(&source)
        .arg(&oversized)
        .output()?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("SourceLimit"));
    assert!(!oversized.exists());
    // HTML escaping expands this admitted source beyond the output allowance.
    // This exercises OutputLimit independently of the input-byte cap.
    let expanded = format!(
        "article en \"Limits\" body cons rawcode none \"{}\" nil",
        "&".repeat(2_000_000)
    );
    fs::write(&source, expanded)?;
    for mode in ["external", "inline"] {
        let output = root.join(format!("output-limit-{mode}"));
        let result = Command::new(binary)
            .args(["doc-html", "export", "--css", mode])
            .arg(&source)
            .arg(&output)
            .output()?;
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("OutputLimit"));
        assert!(!output.exists());
    }
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn ordinary_math_cli_modes_and_failed_batch_are_atomic() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("nepl3-math-cli-{}", std::process::id()));
    fs::create_dir(&root)?;
    let source = root.join("math.nepld");
    let good = r#"article ja "Math" body cons paragraph cons parallel cons variant en sentence cons math Math frac 1 0 nil cons variant ja sentence cons math Math frac 1 0 nil nil nil cons display Math label frac 2 3 Sentence "[字/じ]" nil"#;
    fs::write(&source, good)?;
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    for renderer in ["katex-preferred", "mathml-only"] {
        for css in ["external", "inline"] {
            let out = root.join(format!("{renderer}-{css}"));
            let run = Command::new(binary)
                .args([
                    "doc-html",
                    "export",
                    "--math-renderer",
                    renderer,
                    "--css",
                    css,
                ])
                .arg(&source)
                .arg(&out)
                .output()?;
            assert!(
                run.status.success(),
                "{}",
                String::from_utf8_lossy(&run.stderr)
            );
            let html = fs::read_to_string(out.join("document.html"))?;
            assert_eq!(html.matches("<mfrac>").count(), 3);
            assert!(html.contains("nepl-ruby"));
            assert!(!html.contains("<script"));
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
            assert_eq!(manifest["options"]["math_renderer"], renderer);
            assert_eq!(
                manifest["math_diagnostics"]
                    .as_array()
                    .ok_or("diagnostics")?
                    .is_empty(),
                renderer == "mathml-only"
            );
        }
    }
    let manifest = root.join("pages.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({"version":1,"pages":[
            {"id":"good","source":"math.nepld","route":"good/index.html"},
            {"id":"bad","source":"bad.nepld","route":"bad/index.html"}
        ]}))?,
    )?;
    fs::write(
        root.join("bad.nepld"),
        r#"article en "Bad" body cons display Math frac 1"#,
    )?;
    let out = root.join("pages");
    let run = Command::new(binary)
        .args(["doc-html", "pages", "--math-renderer", "mathml-only"])
        .arg(&manifest)
        .arg(&out)
        .output()?;
    assert!(!run.status.success());
    assert!(
        !out.exists(),
        "a valid first page must not leak partial output"
    );
    fs::write(root.join("bad.nepld"), good)?;
    let run = Command::new(binary)
        .args(["doc-html", "pages", "--math-renderer", "mathml-only"])
        .arg(&manifest)
        .arg(&out)
        .output()?;
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(out.join("good/index.html").exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn urls_export_from_the_cli_and_invalid_urls_publish_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("nepl3-url-export-{}", std::process::id()));
    fs::create_dir(&root)?;
    let source = root.join("source.nepld");
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    fs::write(
        &source,
        r#"article en "Links" body cons paragraph cons sentence cons link external "https://example.org/?a=1&b=2" anno text "Source" cons text "reference" nil cons text " " cons math Math frac 1 2 nil nil nil"#,
    )?;
    for css in ["external", "inline"] {
        let output = root.join(css);
        let run = Command::new(binary)
            .args(["doc-html", "export", "--css", css])
            .arg(&source)
            .arg(&output)
            .output()?;
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let html = fs::read_to_string(output.join("document.html"))?;
        assert!(html.contains("href=\"https://example.org/?a=1&amp;b=2\""));
        assert!(html.contains("<mfrac>"));
        assert!(html.contains("reference"));
    }
    fs::write(
        &source,
        r#"article en "Links" body cons paragraph cons sentence cons link external "javascript:alert(1)" text "label" nil nil nil"#,
    )?;
    let invalid = root.join("invalid");
    let run = Command::new(binary)
        .args(["doc-html", "export"])
        .arg(&source)
        .arg(&invalid)
        .output()?;
    assert!(!run.status.success());
    assert!(!invalid.exists());
    fs::remove_dir_all(root)?;
    Ok(())
}
