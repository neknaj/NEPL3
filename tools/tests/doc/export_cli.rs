//! Exercise actual CLI dispatch and file sets rather than only in-memory export.
use std::{fs, process::Command};

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
