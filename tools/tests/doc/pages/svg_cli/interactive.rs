use super::*;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn fixture(name: &str) -> Result<PathBuf> {
    let root = root(name)?;
    fs::write(
        root.join("a.nepld"),
        A.replace("Japanese display", "日本語表示"),
    )?;
    fs::write(root.join("b.nepld"), B)?;
    for name in ["a", "b"] {
        fs::write(
            root.join(format!("{name}.svg")),
            "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>",
        )?;
    }
    fs::write(root.join("manual.txt"), "retained manual")?;
    fs::write(
        root.join("pages.json"),
        serde_json::to_vec(
            &json!({"version":1,"display":{"mode":"single","language":"fr","fallbacks":["ja"]},"pages":[{"id":"one","source":"a.nepld","route":"one.html"},{"id":"two","source":"b.nepld","route":"two.html"}],"files":[{"id":"manual","source":"manual.txt","route":"manual.txt"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"a.svg","mime":"image/svg+xml"}]},{"page":"two","assets":[{"id":"logo","source":"b.svg","mime":"image/svg+xml"}]}]}),
        )?,
    )?;
    Ok(root)
}
fn defaults() -> Result<Value> {
    let output = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .args(["doc-html", "retry-limits"])
        .output()?;
    assert!(output.status.success());
    Ok(serde_json::from_slice(&output.stdout)?)
}
fn command(root: &Path, out: &Path) -> Command {
    command_mode(root, out, None)
}
fn command_mode(root: &Path, out: &Path, mode: Option<&str>) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"));
    c.env_remove("NEPL3_KATEX_NODE_MODULES")
        .args(["doc-html", "pages-svg", "--interactive-limits"])
        .arg(root.join("initial.json"))
        .args(mode.into_iter().flat_map(|value| ["--svg", value]))
        .args(["--math-renderer", "mathml-only"])
        .arg(root.join("pages.json"))
        .arg(out);
    c
}
#[test]
fn svg_pages_interactive_cli_retains_sources_across_real_preparation_and_render_prompts()
-> Result<()> {
    for mode in [None, Some("external")] {
        for stage in ["preparation", "render"] {
            let root = fixture(stage)?;
            let full = defaults()?;
            let mut initial = full.clone();
            initial[if stage == "preparation" {
                "parse_limits"
            } else {
                "output_limits"
            }]["work"] = json!(0);
            if stage == "preparation" {
                initial["output_limits"]["work"] = json!(0);
            }
            fs::write(root.join("initial.json"), serde_json::to_vec(&initial)?)?;
            fs::write(root.join("full.json"), serde_json::to_vec(&full)?)?;
            let baseline = root.join("baseline");
            let result = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
                .args(["doc-html", "pages-svg", "--math-renderer", "mathml-only"])
                .args(mode.into_iter().flat_map(|value| ["--svg", value]))
                .arg(root.join("pages.json"))
                .arg(&baseline)
                .output()?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let baseline_receipt: Value =
                serde_json::from_slice(&fs::read(baseline.join("manifest.json"))?)?;
            let out = root.join("output");
            let mut child = ChildGuard(
                command_mode(&root, &out, mode)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()?,
            );
            let stderr = child.0.stderr.take().ok_or("stderr")?;
            let (send, receive) = mpsc::channel();
            let reader = thread::spawn(move || -> std::io::Result<String> {
                let mut source = BufReader::new(stderr);
                let mut text = String::new();
                loop {
                    let mut line = String::new();
                    if source.read_line(&mut line)? == 0 {
                        break;
                    }
                    if line.contains("resume <limits.json>") {
                        let _ = send.send(());
                    }
                    text.push_str(&line);
                }
                Ok(text)
            });
            if let Err(error) = receive.recv_timeout(Duration::from_secs(60)) {
                let _ = child.0.kill();
                let _ = child.0.wait();
                let text = reader.join().map_err(|_| "reader panic")??;
                return Err(format!("prompt {error}: {text}").into());
            }
            assert!(!out.exists());
            for name in [
                "pages.json",
                "a.nepld",
                "b.nepld",
                "a.svg",
                "b.svg",
                "manual.txt",
                "initial.json",
            ] {
                fs::remove_file(root.join(name))?;
            }
            let mut input = child.0.stdin.take().ok_or("stdin")?;
            writeln!(input, "resume {}", root.join("full.json").display())?;
            drop(input);
            let start = Instant::now();
            let status = loop {
                if let Some(status) = child.0.try_wait()? {
                    break status;
                }
                if start.elapsed() > Duration::from_secs(60) {
                    return Err("completion deadline".into());
                }
                thread::sleep(Duration::from_millis(10));
            };
            let messages = reader.join().map_err(|_| "reader panic")??;
            assert!(status.success(), "{messages}");
            assert!(messages.contains(&format!("Paused {stage}")));
            if stage == "preparation" {
                assert!(
                    !messages.contains("Paused render"),
                    "latest output grant must propagate"
                );
            }
            let receipt: Value = serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
            assert_eq!(receipt["format"], "nepl3.local-doc-pages.svg-render-job/1");
            assert_eq!(
                receipt["package"]["input_identity"],
                baseline_receipt["input_identity"]
            );
            for file in baseline_receipt["files"].as_array().ok_or("files")? {
                let path = file["path"].as_str().ok_or("path")?;
                assert_eq!(fs::read(out.join(path))?, fs::read(baseline.join(path))?);
            }
            assert_eq!(
                receipt["job_execution"]["render_attempts"]
                    .as_array()
                    .ok_or("renders")?
                    .len(),
                if stage == "preparation" { 1 } else { 2 }
            );
            let html = fs::read_to_string(out.join("one.html"))?;
            assert!(html.contains("日本語表示"));
            assert!(!html.contains("English display"));
            assert!(html.contains("<code"));
            assert_eq!(html.contains("data:image/svg+xml;base64,"), mode.is_none());
            if mode.is_some() {
                assert_eq!(receipt["package"]["svg"], "external");
            }
            drop(child);
            fs::remove_dir_all(root)?;
        }
    }
    Ok(())
}
#[test]
fn svg_pages_interactive_cli_rejects_bad_setup_and_protects_outputs() -> Result<()> {
    let root = fixture("interactive-errors")?;
    let full = defaults()?;
    let mut initial = full.clone();
    initial["parse_limits"]["work"] = json!(0);
    fs::write(root.join("initial.json"), serde_json::to_vec(&initial)?)?;
    let out = root.join("cancelled");
    let result = command(&root, &out).stdin(Stdio::null()).output()?;
    assert!(!result.status.success());
    assert!(!out.exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Cancelled"));
    let existing = root.join("existing");
    fs::create_dir(&existing)?;
    fs::write(existing.join("sentinel"), "keep")?;
    fs::remove_file(root.join("initial.json"))?;
    let result = command(&root, &existing).stdin(Stdio::null()).output()?;
    assert!(!result.status.success());
    assert_eq!(fs::read(existing.join("sentinel"))?, b"keep");
    assert!(!existing.join("manifest.json").exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("already exists"));
    let missing = root.join("missing-config");
    let result = command(&root, &missing).stdin(Stdio::null()).output()?;
    assert!(!result.status.success());
    assert!(!missing.exists());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("not implemented"));
    for (n, bytes) in [
        b"{bad".to_vec(),
        b"{\"unknown\":1}".to_vec(),
        b"{\"job_limits\":null}".to_vec(),
        format!("{{}}{}", " ".repeat(65535)).into_bytes(),
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(root.join("initial.json"), bytes)?;
        let out = root.join(format!("bad-{n}"));
        let result = command(&root, &out).stdin(Stdio::null()).output()?;
        assert!(!result.status.success());
        assert!(!out.exists());
    }
    fs::write(
        root.join("--interactive-limits"),
        fs::read(root.join("pages.json"))?,
    )?;
    let bad = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .current_dir(&root)
        .args([
            "doc-html",
            "pages-svg",
            "--interactive-limits",
            "accidental-output",
        ])
        .output()?;
    assert!(!bad.status.success());
    assert!(!root.join("accidental-output").exists());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("usage:"));
    let good = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .current_dir(&root)
        .args([
            "doc-html",
            "pages-svg",
            "--math-renderer",
            "mathml-only",
            "./--interactive-limits",
            "explicit-path",
        ])
        .output()?;
    assert!(
        good.status.success(),
        "{}",
        String::from_utf8_lossy(&good.stderr)
    );
    let help = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .arg("--help")
        .output()?;
    assert!(help.status.success());
    // Both grammars document SVG selection; interactive limits must come first.
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains("pages-svg [--svg embedded|external]"));
    assert!(text.contains("pages-svg --interactive-limits"));
    assert!(text.contains("--interactive-limits <limits.json> [--svg embedded|external]"));
    assert!(text.contains("durable restoration unsupported"));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn external_svg_interactive_cli_modes_aliases_and_errors() -> Result<()> {
    let root = fixture("interactive-external-modes")?;
    fs::write(root.join("initial.json"), serde_json::to_vec(&defaults()?)?)?;
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    let run = |name: &str, flags: &[&str]| -> Result<PathBuf> {
        let out = root.join(name);
        let result = Command::new(binary)
            .env_remove("NEPL3_KATEX_NODE_MODULES")
            .args(["doc-html", "pages-svg", "--interactive-limits"])
            .arg(root.join("initial.json"))
            .args(flags)
            .arg(root.join("pages.json"))
            .arg(&out)
            .stdin(Stdio::null())
            .output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.is_empty() && result.stderr.is_empty());
        Ok(out)
    };
    for (name, renderer) in [
        ("default", None),
        ("mathml", Some("mathml-only")),
        ("preferred", Some("katex-preferred")),
    ] {
        let renderer_flags: Vec<_> = renderer
            .into_iter()
            .flat_map(|r| ["--math-renderer", r])
            .collect();
        let legacy = run(&format!("{name}-legacy"), &renderer_flags)?;
        let mut alias_flags = vec!["--svg", "embedded"];
        alias_flags.extend_from_slice(&renderer_flags);
        let alias = run(&format!("{name}-alias"), &alias_flags)?;
        assert_eq!(
            super::external::tree(&legacy)?,
            super::external::tree(&alias)?
        );
        let mut external_flags = vec!["--svg", "external"];
        external_flags.extend_from_slice(&renderer_flags);
        let external = run(&format!("{name}-external"), &external_flags)?;
        let receipt: Value = serde_json::from_slice(&fs::read(external.join("manifest.json"))?)?;
        assert_eq!(receipt["package"]["svg"], "external");
        if renderer.is_some() {
            let mut reversed = renderer_flags.clone();
            reversed.extend_from_slice(&["--svg", "external"]);
            let other = run(&format!("{name}-reversed"), &reversed)?;
            assert_eq!(
                super::external::tree(&external)?,
                super::external::tree(&other)?
            );
        }
    }
    assert_eq!(
        super::external::tree(&root.join("default-external"))?,
        super::external::tree(&root.join("preferred-external"))?
    );
    for (n, (flags, expected)) in [
        (
            vec!["--svg", "bad"],
            "SVG mode must be embedded or external",
        ),
        (
            vec!["--svg", "bad", "--math-renderer", "bad"],
            "expected katex-preferred or mathml-only",
        ),
        (
            vec!["--math-renderer", "bad", "--svg", "bad"],
            "expected katex-preferred or mathml-only",
        ),
        (vec!["--svg", "external", "--svg", "embedded"], "usage:"),
        (
            vec![
                "--math-renderer",
                "mathml-only",
                "--math-renderer",
                "katex-preferred",
            ],
            "usage:",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let out = root.join(format!("invalid-{n}"));
        let result = Command::new(binary)
            .args(["doc-html", "pages-svg", "--interactive-limits"])
            .arg(root.join("missing-limits.json"))
            .args(flags)
            .arg(root.join("missing-manifest.json"))
            .arg(&out)
            .output()?;
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(expected));
        assert!(!out.exists());
    }
    let existing = root.join("protected");
    fs::create_dir(&existing)?;
    fs::write(existing.join("keep"), "original")?;
    let result = Command::new(binary)
        .args(["doc-html", "pages-svg", "--interactive-limits"])
        .arg(root.join("missing-limits.json"))
        .args(["--svg", "external"])
        .arg(root.join("missing-manifest.json"))
        .arg(&existing)
        .output()?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("already exists"));
    assert_eq!(fs::read(existing.join("keep"))?, b"original");
    assert!(!existing.join("manifest.json").exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

pub(super) fn pinned_external_interactive_cases(binary: &str, packages: &Path) -> Result<()> {
    let root = fixture("interactive-external-pinned")?;
    let full = defaults()?;
    fs::write(root.join("full.json"), serde_json::to_vec(&full)?)?;
    let ordinary = root.join("ordinary");
    let result = Command::new(binary)
        .env("NEPL3_KATEX_NODE_MODULES", packages)
        .args(["doc-html", "pages-svg", "--svg", "external"])
        .arg(root.join("pages.json"))
        .arg(&ordinary)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source: Value = serde_json::from_slice(&fs::read(ordinary.join("manifest.json"))?)?;
    let mut before_receipt = None;
    for stage in ["baseline", "preparation", "render", "receipt"] {
        let mut initial = full.clone();
        match stage {
            "preparation" => initial["parse_limits"]["work"] = json!(0),
            "render" => initial["output_limits"]["work"] = json!(0),
            "receipt" => {
                initial["output_limits"]["output_bytes"] =
                    json!(before_receipt.ok_or("baseline output usage")?)
            }
            _ => {}
        }
        fs::write(root.join("initial.json"), serde_json::to_vec(&initial)?)?;
        let out = root.join(stage);
        let mut child = Command::new(binary)
            .env("NEPL3_KATEX_NODE_MODULES", packages)
            .args(["doc-html", "pages-svg", "--interactive-limits"])
            .arg(root.join("initial.json"))
            .args(["--svg", "external"])
            .arg(root.join("pages.json"))
            .arg(&out)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut input = child.stdin.take().ok_or("stdin")?;
        if stage != "baseline" {
            writeln!(input, "resume {}", root.join("full.json").display())?;
        }
        drop(input);
        let result = child.wait_with_output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let messages = String::from_utf8_lossy(&result.stderr);
        if stage == "baseline" {
            assert!(messages.is_empty());
        } else {
            assert!(messages.contains(if stage == "preparation" {
                "Paused preparation"
            } else {
                "Paused render"
            }));
        }
        let receipt: Value = serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
        let package = &receipt["package"];
        assert_eq!(
            package["format"],
            "nepl3.local-doc-pages.root-svg-external-package/2"
        );
        assert_eq!(package["svg"], "external");
        assert_eq!(package["input_identity"], source["input_identity"]);
        assert_eq!(
            package["katex_generation"]["successful_visuals"],
            source["katex_generation"]["successful_visuals"]
        );
        assert!(
            package["katex_generation"]["successful_visuals"]
                .as_u64()
                .ok_or("visuals")?
                > 0
        );
        let native = package["source_input"]["native_generation"]["pages"]
            .as_array()
            .ok_or("native pages")?;
        let old_native = source["source_input"]["native_generation"]["pages"]
            .as_array()
            .ok_or("ordinary native pages")?;
        assert_eq!(native.len(), 2);
        assert_eq!(old_native.len(), 2);
        assert_eq!(package["katex_generation"]["successful_visual_pages"], 2);
        assert_eq!(package["katex_generation"]["successful_visuals"], 4);
        for (a, b) in native.iter().zip(old_native) {
            let actual = a["request_context"].as_str().ok_or("interactive context")?;
            let expected = b["request_context"].as_str().ok_or("ordinary context")?;
            assert_eq!(actual.len(), 64);
            assert!(actual.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert_eq!(actual, expected);
        }
        assert_eq!(package["files"], source["files"]);
        let mut actual_files = super::external::tree(&out)?;
        let mut expected_files = super::external::tree(&ordinary)?;
        assert!(actual_files.remove("manifest.json").is_some());
        assert!(expected_files.remove("manifest.json").is_some());
        assert_eq!(actual_files, expected_files);
        for row in package["files"].as_array().ok_or("files")? {
            let path = row["path"].as_str().ok_or("path")?;
            assert_eq!(fs::read(out.join(path))?, fs::read(ordinary.join(path))?);
        }
        let attempts = receipt["job_execution"]["render_attempts"]
            .as_array()
            .ok_or("render attempts")?;
        assert_eq!(
            attempts.len(),
            if matches!(stage, "render" | "receipt") {
                2
            } else {
                1
            }
        );
        if stage == "receipt" {
            assert_eq!(attempts[0]["stop"], "OutputLimit");
        }
        before_receipt = Some(
            package["pre_receipt_usage"]["output_bytes"]
                .as_u64()
                .ok_or("before receipt")?,
        );
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
