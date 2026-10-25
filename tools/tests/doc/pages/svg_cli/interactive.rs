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
    let mut c = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"));
    c.env_remove("NEPL3_KATEX_NODE_MODULES")
        .args(["doc-html", "pages-svg", "--interactive-limits"])
        .arg(root.join("initial.json"))
        .args(["--math-renderer", "mathml-only"])
        .arg(root.join("pages.json"))
        .arg(out);
    c
}
#[test]
fn svg_pages_interactive_cli_retains_sources_across_real_preparation_and_render_prompts()
-> Result<()> {
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
            command(&root, &out)
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
        drop(child);
        fs::remove_dir_all(root)?;
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
    assert!(String::from_utf8_lossy(&help.stdout).contains("pages-svg [--interactive-limits"));
    fs::remove_dir_all(root)?;
    Ok(())
}
