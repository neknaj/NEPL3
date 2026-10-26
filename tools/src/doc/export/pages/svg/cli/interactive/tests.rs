use super::*;
use crate::{
    doc::{
        export::pages::{
            resources::OperationLimits, svg::preparation::Progress as PreparedProgress,
        },
        source::{budget, compiled},
    },
    testing::Fixture,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Read},
    path::PathBuf,
};
const SVG: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const ONE: &str = r#"article en Sentence "One" body cons image asset "logo" none Sentence "[漢/かん]" none cons paragraph cons parallel cons variant en Sentence "English content" cons variant ja Sentence "日本語本文" nil cons Sentence sentence cons math frac 1 2 cons document math Math frac 3 4 cons document link page "two" some "target" text "Next" nil cons code Math frac 5 6 nil cons display Math frac 7 8 nil"#;
const TWO: &str = r#"article en "Two" body cons image asset "logo" none "Two image" none cons section target "Target" body cons paragraph cons sentence cons math Math frac 9 10 nil nil nil nil"#;
fn zero() -> OperationLimits {
    OperationLimits {
        source_bytes: 0,
        work: 0,
        depth: 0,
        nodes: 0,
        allocation_units: 0,
        output_bytes: 0,
        diagnostics: 0,
        events: 0,
    }
}
fn manifest(caps: OperationLimits) -> Value {
    json!({"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"},{"id":"two","source":"two.nepld","route":"two.html"}],"files":[{"id":"manual","source":"manual.txt","route":"manual.txt"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"one.svg","mime":"image/svg+xml"}]},{"page":"two","assets":[{"id":"logo","source":"two.svg","mime":"image/svg+xml"}]}],"display":{"mode":"single","language":"fr","fallbacks":["ja","en"]},"parse_limits":caps,"lower_limits":caps,"output_limits":caps})
}
fn fixture(caps: OperationLimits) -> crate::Result<Fixture> {
    let f = Fixture::new()?;
    f.write("one.nepld", ONE)?;
    f.write("two.nepld", TWO)?;
    f.write("one.svg", SVG)?;
    f.write("two.svg", SVG)?;
    f.write("manual.txt", "frozen passive bytes")?;
    f.json("manifest.json", &manifest(caps))?;
    Ok(f)
}
fn verify_mode(done: &Rendered, mode: SvgMode) -> crate::Result<Value> {
    let receipt: Value = serde_json::from_str(&done.manifest)?;
    assert_eq!(receipt["format"], "nepl3.local-doc-pages.svg-render-job/1");
    let html = String::from_utf8(done.files.get("one.html").ok_or("one")?.clone())?;
    assert!(html.contains("日本語本文"));
    assert!(!html.contains("English content"));
    assert!(html.contains("<code"));
    assert_eq!(
        html.contains("data:image/svg+xml;base64,"),
        mode == SvgMode::Embedded
    );
    if mode == SvgMode::External {
        assert_eq!(receipt["package"]["svg"], "external");
        let svg: Vec<_> = done
            .files
            .iter()
            .filter(|(path, _)| path.ends_with(".svg"))
            .collect();
        assert_eq!(svg.len(), 1);
        assert_eq!(svg[0].1, SVG.as_bytes());
        assert!(html.contains(svg[0].0));
    }
    assert_eq!(
        done.files.get("manual.txt").ok_or("manual")?,
        b"frozen passive bytes"
    );
    assert_eq!(receipt["package"]["display"]["language"], "fr");
    assert_eq!(
        receipt["package"]["display"]["fallbacks"],
        json!(["ja", "en"])
    );
    assert_eq!(
        receipt["package"]["source_input"]["math_renderer"],
        "mathml-only-explicit"
    );
    Ok(receipt)
}
#[test]
fn svg_interactive_explicit_limits_override_manifest_and_latest_output_reaches_render()
-> crate::Result<()> {
    for mode in [SvgMode::Embedded, SvgMode::External] {
        let c = compiled()?;
        let f = fixture(zero())?;
        let full = RetryLimits::default();
        let ordinary = (match mode {
            SvgMode::Embedded => Preparation::capture,
            SvgMode::External => Preparation::capture_external,
        })(
            &c,
            &f.root().join("manifest.json"),
            MathRenderer::MathMLOnly,
            budget().limits(),
            OperationLimits::default(),
        )?;
        assert_eq!(ordinary.frozen().output_limits(), zero());
        assert!(matches!(ordinary.run(), PreparedProgress::Paused { .. }));
        let done = run_mode(
            &c,
            &f.root().join("manifest.json"),
            full,
            MathRenderer::MathMLOnly,
            mode,
            |_, _, _, _| Err("unexpected pause under explicit full limits".into()),
        )?;
        let receipt = verify_mode(&done, mode)?;
        assert_eq!(
            receipt["package"]["source_input"]["sources"][0]["operations"][0]["requested"]["work"],
            full.parse_limits.work
        );
        f.json("manifest.json", &manifest(OperationLimits::default()))?;
        let mut initial = full;
        initial.parse_limits.work = 0;
        initial.output_limits.work = 0;
        let mut calls = 0;
        let done = run_mode(
            &c,
            &f.root().join("manifest.json"),
            initial,
            MathRenderer::MathMLOnly,
            mode,
            |old, stage, reason, _| {
                calls += 1;
                if calls != 1 {
                    return Err("stale output cap caused an extra pause".into());
                }
                assert_eq!(stage, "preparation");
                assert_eq!(reason, StopReason::WorkLimit);
                assert_eq!(old.output_limits.work, 0);
                Ok(Some(full))
            },
        )?;
        assert_eq!(calls, 1);
        assert_eq!(done.renders.len(), 1);
        let _ = verify_mode(&done, mode)?;
    }
    Ok(())
}
struct RemovedInputs {
    input: Cursor<Vec<u8>>,
    root: PathBuf,
    output: PathBuf,
    changed: bool,
}
impl RemovedInputs {
    fn change(&mut self) -> std::io::Result<()> {
        if !self.changed {
            if self.output.exists() {
                return Err(std::io::Error::other("premature publication"));
            }
            for name in [
                "manifest.json",
                "one.nepld",
                "two.nepld",
                "one.svg",
                "two.svg",
                "manual.txt",
                "initial.json",
            ] {
                fs::remove_file(self.root.join(name))?;
            }
            self.changed = true;
        }
        Ok(())
    }
}
impl Read for RemovedInputs {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.change()?;
        self.input.read(out)
    }
}
impl BufRead for RemovedInputs {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.change()?;
        self.input.fill_buf()
    }
    fn consume(&mut self, n: usize) {
        self.input.consume(n)
    }
}
#[test]
fn svg_interactive_captures_all_inputs_once_across_preparation_and_render_prompts()
-> crate::Result<()> {
    for mode in [SvgMode::Embedded, SvgMode::External] {
        for stage in ["preparation", "render"] {
            let full = RetryLimits::default();
            let f = fixture(OperationLimits::default())?;
            let mut initial = full;
            if stage == "preparation" {
                initial.parse_limits.work = 0;
            } else {
                initial.output_limits.work = 0;
            }
            f.json("initial.json", &serde_json::to_value(initial)?)?;
            f.json("full.json", &serde_json::to_value(full)?)?;
            let output = f.root().join("output");
            let mut input = RemovedInputs {
                input: Cursor::new(
                    format!("resume {}\n", f.root().join("full.json").display()).into_bytes(),
                ),
                root: f.root().to_path_buf(),
                output: output.clone(),
                changed: false,
            };
            let mut messages = Vec::new();
            write_with_io_mode(
                &f.root().join("manifest.json"),
                &f.root().join("initial.json"),
                &output,
                MathRenderer::MathMLOnly,
                mode,
                &mut input,
                &mut messages,
            )?;
            assert!(input.changed);
            assert!(String::from_utf8(messages)?.contains(&format!("Paused {stage}")));
            let receipt: Value = serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
            assert_eq!(
                receipt["job_execution"]["render_attempts"]
                    .as_array()
                    .ok_or("renders")?
                    .len(),
                if stage == "preparation" { 1 } else { 2 }
            );
            assert_eq!(
                receipt["job_execution"]["preparation"]["attempts"]
                    .as_array()
                    .ok_or("preparation")?
                    .len(),
                if stage == "preparation" { 3 } else { 2 }
            );
            let html = fs::read_to_string(output.join("one.html"))?;
            assert!(html.contains("日本語本文"));
            assert!(!html.contains("English content"));
            assert!(html.contains("<code"));
            assert_eq!(
                html.contains("data:image/svg+xml;base64,"),
                mode == SvgMode::Embedded
            );
            if mode == SvgMode::External {
                assert_eq!(receipt["package"]["svg"], "external");
            }
            assert_eq!(
                fs::read(output.join("manual.txt"))?,
                b"frozen passive bytes"
            );
        }
    }
    Ok(())
}
fn decrease(v: &mut OperationLimits, field: usize) -> Result<(), String> {
    let selected = match field {
        0 => &mut v.source_bytes,
        1 => &mut v.work,
        2 => &mut v.depth,
        3 => &mut v.nodes,
        4 => &mut v.allocation_units,
        5 => &mut v.output_bytes,
        6 => &mut v.diagnostics,
        7 => &mut v.events,
        _ => return Err("field".into()),
    };
    *selected -= 1;
    Ok(())
}
#[test]
fn svg_interactive_driver_rejects_all_five_groups_and_retains_equal_grants() -> crate::Result<()> {
    for mode in [SvgMode::Embedded, SvgMode::External] {
        let c = compiled()?;
        let f = fixture(OperationLimits::default())?;
        let full = RetryLimits::default();
        let baseline = run_mode(
            &c,
            &f.root().join("manifest.json"),
            full,
            MathRenderer::MathMLOnly,
            mode,
            |_, _, _, _| Err("baseline pause".into()),
        )?;
        let mut initial = full;
        initial.output_limits.work = baseline.renders[0].usage.work / 2;
        assert!(initial.output_limits.work > 0);
        let mut calls = 0;
        let mut first_usage = None;
        let done = run_mode(
            &c,
            &f.root().join("manifest.json"),
            initial,
            MathRenderer::MathMLOnly,
            mode,
            |previous, stage, reason, usage| {
                assert_eq!(stage, "render");
                assert_eq!(reason, StopReason::WorkLimit);
                if calls < 40 {
                    if let Some(old) = first_usage {
                        assert_eq!(usage, old);
                    } else {
                        first_usage = Some(usage);
                    }
                    let mut next = previous;
                    let group = calls / 8;
                    let field = calls % 8;
                    let selected = match group {
                        0 => &mut next.job_limits,
                        1 => &mut next.parse_limits,
                        2 => &mut next.lower_limits,
                        3 => &mut next.code_limits,
                        _ => &mut next.output_limits,
                    };
                    decrease(selected, field)?;
                    calls += 1;
                    return Ok(Some(next));
                }
                calls += 1;
                if calls == 41 {
                    return Ok(Some(previous));
                }
                if calls == 42 {
                    assert!(usage.work > first_usage.ok_or("first usage")?.work);
                    return Ok(Some(full));
                }
                Err("unexpected extra pause".into())
            },
        )?;
        assert_eq!(calls, 42);
        assert_eq!(done.renders.len(), 3);
        assert_eq!(done.files, baseline.files);
        let _ = verify_mode(&done, mode)?;
    }
    Ok(())
}
#[test]
fn svg_interactive_cancel_bad_input_and_io_errors_never_publish_before_completion()
-> crate::Result<()> {
    for mode in [SvgMode::Embedded, SvgMode::External] {
        let f = fixture(OperationLimits::default())?;
        let full = RetryLimits::default();
        for (index, stage) in ["preparation", "render"].into_iter().enumerate() {
            let mut initial = full;
            if stage == "preparation" {
                initial.parse_limits.work = 0;
            } else {
                initial.output_limits.work = 0;
            }
            f.json("initial.json", &serde_json::to_value(initial)?)?;
            for (case, bytes) in [
                Vec::new(),
                b"cancel\n".to_vec(),
                vec![b'x'; 4097],
                b"unknown\ncancel\n".to_vec(),
            ]
            .into_iter()
            .enumerate()
            {
                let out = f.root().join(format!("out-{index}-{case}"));
                let mut input = Cursor::new(bytes);
                let mut messages = Vec::new();
                let error = write_with_io_mode(
                    &f.root().join("manifest.json"),
                    &f.root().join("initial.json"),
                    &out,
                    MathRenderer::MathMLOnly,
                    mode,
                    &mut input,
                    &mut messages,
                )
                .err()
                .ok_or("must fail/cancel")?
                .to_string();
                assert!(!out.exists());
                assert!(
                    error.contains("usage")
                        || error.contains("accounting")
                        || error.contains("Cancelled")
                );
                assert!(error.contains(if stage == "preparation" {
                    "attempts"
                } else {
                    "renders"
                }));
            }
        }
        let mut initial = full;
        initial.parse_limits.work = 0;
        f.json("initial.json", &serde_json::to_value(initial)?)?;
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("message failure"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::other("flush failure"))
            }
        }
        let out = f.root().join("write-error");
        let error = write_with_io_mode(
            &f.root().join("manifest.json"),
            &f.root().join("initial.json"),
            &out,
            MathRenderer::MathMLOnly,
            mode,
            &mut Cursor::new(Vec::<u8>::new()),
            &mut Broken,
        )
        .err()
        .ok_or("write error")?
        .to_string();
        assert!(error.contains("usage") && error.contains("attempts"));
        assert!(!out.exists());
    }
    Ok(())
}
#[test]
fn svg_interactive_transition_rejects_internal_output_shrink_with_original_evidence()
-> crate::Result<()> {
    let c = compiled()?;
    let f = fixture(OperationLimits::default())?;
    let full = RetryLimits::default();
    let PreparedProgress::Prepared(ready) = Preparation::capture_selected(
        &c,
        &f.root().join("manifest.json"),
        MathRenderer::MathMLOnly,
        full.job_limits.budget().limits(),
        full.preparation(),
        full.output_limits,
    )?
    .run() else {
        return Err("ready".into());
    };
    let usage = ready.usage();
    let history = format!("{:?}", ready.attempts());
    let mut lower = full.output_limits;
    lower.work -= 1;
    let failure = ready
        .into_render_selected(lower)
        .err()
        .ok_or("shrinking transition")?;
    assert_eq!(failure.error, "ShrinkingInitialSvgRenderAllowance");
    assert_eq!(failure.usage, usage);
    assert_eq!(format!("{:?}", failure.preparation), history);
    assert!(failure.renders.is_empty());
    Ok(())
}
#[test]
fn svg_interactive_bad_and_shrinking_limit_files_remain_paused() -> crate::Result<()> {
    for mode in [SvgMode::Embedded, SvgMode::External] {
        let f = fixture(OperationLimits::default())?;
        let full = RetryLimits::default();
        let mut initial = full;
        initial.parse_limits.work = 0;
        f.json("initial.json", &serde_json::to_value(initial)?)?;
        f.json("full limits.json", &serde_json::to_value(full)?)?;
        f.write("unknown.json", "{\"unknown\":1}")?;
        f.write(
            "duplicate.json",
            "{\"parse_limits\":{},\"parse_limits\":{}}",
        )?;
        f.write("oversize.json", format!("{{}}{}", " ".repeat(65535)))?;
        let mut smaller = initial;
        smaller.code_limits.nodes -= 1;
        f.json("smaller.json", &serde_json::to_value(smaller)?)?;
        let commands = [
            "unknown.json",
            "duplicate.json",
            "oversize.json",
            "smaller.json",
            "full limits.json",
        ]
        .map(|name| format!("resume {}\n", f.root().join(name).display()))
        .join("");
        let out = f.root().join("valid-output");
        let mut messages = Vec::new();
        write_with_io_mode(
            &f.root().join("manifest.json"),
            &f.root().join("initial.json"),
            &out,
            MathRenderer::MathMLOnly,
            mode,
            &mut Cursor::new(commands.into_bytes()),
            &mut messages,
        )?;
        let messages = String::from_utf8(messages)?;
        assert_eq!(messages.matches("Paused preparation").count(), 1);
        assert!(messages.contains("Cannot read limits"));
        assert!(messages.contains("nondecreasing"));
        let receipt: Value = serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
        assert_eq!(
            receipt["job_execution"]["preparation"]["attempts"]
                .as_array()
                .ok_or("attempts")?
                .len(),
            3
        );
        assert_eq!(
            receipt["job_execution"]["render_attempts"]
                .as_array()
                .ok_or("renders")?
                .len(),
            1
        );
    }
    Ok(())
}
