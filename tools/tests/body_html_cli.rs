#![cfg(not(target_family = "wasm"))]
use nepl3_tools::doc::{export::body as host, source::compiled};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let p =
            std::env::temp_dir().join(format!("nepl3-body-html-cli-{name}-{}", std::process::id()));
        fs::create_dir(&p)?;
        Ok(Self(p))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(input: &Path, output: &Path, limits: Option<&Path>) -> std::io::Result<Output> {
    let mut c = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"));
    c.args(["doc-html", "body-fragment"]);
    if let Some(p) = limits {
        c.arg("--limits").arg(p);
    }
    c.arg(input).arg(output).output()
}
#[test]
fn body_cli_exact_fragment_default_limits_and_empty_output()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new("bytes")?;
    let input = f.0.join("body.nepld");
    let output = f.0.join("out.html");
    let source = r#"body cons paragraph cons sentence cons text "Intro " cons ref later text "later" cons text " " cons anno ruby text "漢" text "かん" cons text "note" nil nil nil cons section first "First" body cons section inner "Inner" body nil nil cons section later "Later" body nil nil"#;
    fs::write(&input, source)?;
    let expected = host::from_source(&compiled()?, source, Default::default())?.html;
    let result = run(&input, &output, None)?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read_to_string(&output)?, expected);
    assert_eq!(fs::read_to_string(&input)?, source);
    let limits = f.0.join("limits.json");
    fs::write(&limits, b"{}")?;
    let explicit = f.0.join("explicit.html");
    assert!(run(&input, &explicit, Some(&limits))?.status.success());
    assert_eq!(fs::read(&output)?, fs::read(&explicit)?);
    fs::write(&input, b"body nil")?;
    let empty = f.0.join("empty.html");
    assert!(run(&input, &empty, None)?.status.success());
    assert_eq!(
        fs::read_to_string(&empty)?,
        "<div class=\"nepl-doc\"></div>"
    );
    assert_eq!(
        fs::read_dir(&f.0)?
            .collect::<std::io::Result<Vec<_>>>()?
            .len(),
        5
    );
    Ok(())
}
#[test]
fn body_cli_rejects_invalid_sources_options_and_existing_targets()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new("reject")?;
    let input = f.0.join("body.nepld");
    let output = f.0.join("out.html");
    for source in [
        r#"article en "T" body nil"#,
        r#"body cons paragraph cons "incomplete""#,
        r#"body cons paragraph cons sentence cons link relative "x.nepld" none text "x" nil nil nil"#,
    ] {
        fs::write(&input, source)?;
        assert!(!run(&input, &output, None)?.status.success());
        assert!(!output.exists());
    }
    fs::write(&input, [0xff])?;
    assert!(!run(&input, &output, None)?.status.success());
    assert!(!output.exists());
    fs::write(&input, b"body nil")?;
    let legacy = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .args(["doc-html", "export"])
        .arg(&input)
        .arg(&output)
        .output()?;
    assert!(!legacy.status.success());
    assert!(!output.exists());
    let mut limits = nepl3_tools::doc::export::pages::resources::ExecutionLimits::default();
    limits.output_limits.work = 0;
    let config = f.0.join("low.json");
    fs::write(&config, serde_json::to_vec(&limits)?)?;
    let stopped = run(&input, &output, Some(&config))?;
    assert!(!stopped.status.success());
    assert!(String::from_utf8_lossy(&stopped.stderr).contains("WorkLimit"));
    assert!(!output.exists());
    for invalid in [r#"{"unknown":1}"#, "{"] {
        fs::write(&config, invalid)?;
        assert!(!run(&input, &output, Some(&config))?.status.success());
        assert!(!output.exists());
    }
    let phase = serde_json::to_string(
        &nepl3_tools::doc::export::pages::resources::ExecutionLimits::default().parse_limits,
    )?;
    fs::write(
        &config,
        format!("{{\"parse_limits\":{phase},\"parse_limits\":{phase}}}"),
    )?;
    let duplicate = run(&input, &output, Some(&config))?;
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("duplicate object key"));
    assert!(!output.exists());
    fs::write(
        &input,
        vec![b' '; nepl3_tools::doc::export::MAX_SOURCE_BYTES as usize + 1],
    )?;
    let oversized = run(&input, &output, None)?;
    assert!(!oversized.status.success());
    assert!(String::from_utf8_lossy(&oversized.stderr).contains("input exceeds byte limit"));
    assert!(!output.exists());
    fs::write(&input, b"body nil")?;
    fs::write(&output, b"keep")?;
    assert!(!run(&input, &output, None)?.status.success());
    assert_eq!(fs::read(&output)?, b"keep");
    fs::remove_file(&output)?;
    // A malformed Body invocation must not fall through to the legacy
    // three-argument form, even if a file literally named body-fragment exists.
    fs::write(f.0.join("body-fragment"), b"article en \"T\" body nil")?;
    let result = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .current_dir(&f.0)
        .args(["doc-html", "body-fragment", "accidental.html"])
        .output()?;
    assert!(!result.status.success());
    assert!(!f.0.join("accidental.html").exists());
    let result = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .args(["doc-html", "body-fragment", "--unknown"])
        .arg(&output)
        .output()?;
    assert!(!result.status.success());
    assert!(!output.exists());
    Ok(())
}
