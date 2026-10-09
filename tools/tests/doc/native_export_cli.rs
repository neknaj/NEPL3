//! Trusted-host standalone CLI checks; no browser/portable identity claim.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("nepl-native-export-{}-{stamp}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(host: &Path, input: &Path, output: &Path, extra: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .args(["doc-html", "export", "--native-host"])
        .arg(host)
        .args(extra)
        .arg(input)
        .arg(output)
        .output()
}
fn sha(bytes: &[u8]) -> String {
    nepl3_core::source::Digest::of(bytes)
        .0
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}
#[test]
fn native_selection_preserves_independent_mathml_and_config_failures()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = Temp::new()?;
    let input = temp.0.join("input.nepld");
    fs::write(
        &input,
        "article en \"Independent\" body cons display Math sqrt x nil",
    )?;
    let absent = temp.0.join("absent-host.json");
    let plain = temp.0.join("mathml-default-css");
    assert!(
        run(&absent, &input, &plain, &["--math-renderer", "mathml-only"])?
            .status
            .success()
    );
    for css in ["external", "inline"] {
        let output = temp.0.join(css);
        let result = run(
            &absent,
            &input,
            &output,
            &["--css", css, "--math-renderer", "mathml-only"],
        )?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(fs::read_to_string(output.join("document.html"))?.contains("<msqrt>"));
        assert!(!output.join("assets/katex").exists());
    }
    for (name, config) in [("unknown", br#"{"unknown":true}"#.as_slice()), ("oversize", &[b' ';16385][..]),
        ("relative", br#"{"node":"node","bridge":"bridge","modules_url":"file:///modules/","katex_installation":"katex"}"#.as_slice())] {
        let host = temp.0.join(format!("{name}.json"));fs::write(&host, config)?;
        let output = temp.0.join(name);
        let result = run(&host, &input, &output, &[])?;
        assert!(!result.status.success());assert!(!output.exists());
    }
    let output = temp.0.join("unsupported-inline");
    let result = run(
        &absent,
        &input,
        &output,
        &["--css", "inline", "--math-renderer", "katex-preferred"],
    )?;
    assert!(!result.status.success());
    assert!(!output.exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("NativeMathRequiresExternalCss"));
    Ok(())
}
#[test]
#[ignore = "requires pinned Node and fixed Math packages"]
fn native_export_cli_packages_real_output_and_rejects_failed_attempts()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = Temp::new()?;
    let node = PathBuf::from(std::env::var("NEPL3_TEST_NODE")?).canonicalize()?;
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let modules = tools.join("audit/math/node_modules").canonicalize()?;
    let url = Command::new(&node)
        .args([
            "-p",
            "require('node:url').pathToFileURL(process.argv[1]+require('node:path').sep).href",
        ])
        .arg(&modules)
        .output()?;
    assert!(url.status.success());
    let url = String::from_utf8(url.stdout)?;
    let bridge = tools.join("math/katex/node/stdio.mjs").canonicalize()?;
    let original = json!({"node":node,"bridge":bridge,"modules_url":url.trim(),"katex_installation":modules.join("katex")});
    let host = temp.0.join("host.json");
    fs::write(&host, serde_json::to_vec(&original)?)?;
    let input = temp.0.join("mixed.nepld");
    fs::write(
        &input,
        r#"article en "Native" body cons code Math x cons paragraph cons sentence cons math Math add x y nil nil cons display Math sqrt y cons display Math label x Sentence "note" nil"#,
    )?;
    let good = temp.0.join("good");
    let result = run(&host, &input, &good, &[])?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let html = fs::read_to_string(good.join("document.html"))?;
    for required in [
        "font-src 'self'",
        "assets/math.css",
        "assets/katex/katex.min.css",
        "<msqrt>",
        "<code",
        "nepl-code-name",
        "note",
    ] {
        assert!(html.contains(required), "missing {required}");
    }
    assert!(!html.contains("<script"));
    assert!(!html.contains("fonts.googleapis.com"));
    let manifest: Value = serde_json::from_slice(&fs::read(good.join("manifest.json"))?)?;
    assert_eq!(manifest["format"], "nepl3.trusted-native-doc-export/1");
    let records = manifest["math"]["occurrences"]
        .as_array()
        .ok_or("records")?;
    assert_eq!(records.len(), 3);
    for (i, r) in records.iter().enumerate() {
        assert_eq!(r["guest_occurrence"], i + 1);
    }
    for r in &records[..2] {
        assert_eq!(r["representation"], "dual");
        assert_eq!(r["termination_failure"], false);
        assert!(!r["observations"].is_null());
    }
    assert_eq!(records[2]["fallback"]["kind"], "tex-unsupported");
    assert!(records[2]["termination_failure"].is_null());
    assert_eq!(manifest["files"].as_array().ok_or("files")?.len(), 65);
    for file in manifest["files"].as_array().ok_or("files")? {
        let bytes = fs::read(good.join(file["path"].as_str().ok_or("path")?))?;
        assert_eq!(file["sha256"], sha(&bytes));
    }
    assert!(good.join("assets/katex/LICENSE").is_file());
    let prior = fs::read(good.join("manifest.json"))?;
    assert!(!run(&host, &input, &good, &[])?.status.success());
    assert_eq!(fs::read(good.join("manifest.json"))?, prior);
    // Missing modules are an explicit ordinary capability fallback, with the
    // verified installed asset set still separate from module availability.
    let mut unavailable = original.clone();
    unavailable["modules_url"] = json!("file:///nepl-test-absent-modules/");
    fs::write(&host, serde_json::to_vec(&unavailable)?)?;
    let output = temp.0.join("unavailable");
    let result = run(&host, &input, &output, &[])?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
    assert_eq!(
        report["math"]["occurrences"][0]["fallback"]["kind"],
        "renderer-unavailable"
    );
    assert_eq!(
        report["math"]["occurrences"][0]["termination_failure"],
        false
    );
    assert!(!output.join("assets/katex").exists());
    let cleanup =
        super::math_bundle::cleanup_failure_bridge(&bridge).map_err(std::io::Error::other)?;
    for (name, script) in [
        (
            "invalid-json",
            "process.stdin.resume();process.stdout.write('{');",
        ),
        (
            "output-limit",
            "process.stdin.resume();process.stdout.write(Buffer.alloc(1000001,120));setInterval(()=>{},1000);",
        ),
        (
            "violation",
            "process.stdin.resume();process.stdout.write(JSON.stringify({version:1,result:{kind:'provider-violation',reason:'markup'}}));",
        ),
    ] {
        let path = temp.0.join(format!("{name}.cjs"));
        fs::write(&path, script)?;
        let mut config = original.clone();
        config["bridge"] = json!(path);
        fs::write(&host, serde_json::to_vec(&config)?)?;
        let output = temp.0.join(name);
        let result = run(&host, &input, &output, &[])?;
        assert!(!result.status.success(), "{name} unexpectedly succeeded");
        assert!(!output.exists());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("MathFallback"));
    }
    for (name, key, path) in [
        ("missing-node", "node", temp.0.join("absent-node")),
        (
            "missing-assets",
            "katex_installation",
            temp.0.join("absent-assets"),
        ),
        ("failed-cleanup", "bridge", cleanup.path().to_path_buf()),
    ] {
        let mut config = original.clone();
        config[key] = json!(path);
        fs::write(&host, serde_json::to_vec(&config)?)?;
        let output = temp.0.join(name);
        let result = run(&host, &input, &output, &[])?;
        assert!(!result.status.success(), "{name} unexpectedly succeeded");
        assert!(!output.exists());
    }
    Ok(())
}
