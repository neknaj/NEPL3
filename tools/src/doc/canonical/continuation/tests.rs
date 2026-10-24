use super::*;
use crate::testing::Fixture;
use serde_json::json;
use std::io::{Cursor, Read};
use std::path::PathBuf;
fn fixture() -> Result<Fixture> {
    let f = Fixture::new()?;
    f.json("doc/canonical.json",&json!({"version":1,"pages":[
        {"id":"old","source":"doc/old.nepld","projection":"doc/old.md","aliases":"doc/old.aliases.json","route":"docs/old.html","renderer":host::RENDERER},
        {"id":"new","source":"doc/new.nepld","projection":"doc/new.md","aliases":"doc/new.aliases.json","route":"docs/new.html","renderer":projection::RENDERER}],
        "files":[{"id":"ref","source":"doc/ref.md","route":"sources/ref.md"},{"id":"registry","source":"doc/canonical.json","route":"sources/canonical.json"}],
        "compatibility":[{"page":"new","route":"sources/new.md"}]}))?;
    f.write(
        "doc/old.nepld",
        r#"article en "Old" body cons section s "Section" body nil nil"#,
    )?;
    f.write("doc/new.nepld",r#"article en "New" body cons paragraph cons sentence cons link relative "old.md" some "s" text "Older section" cons text " " cons link relative "ref.md" none text "File" nil nil nil"#)?;
    f.write(
        "doc/old.aliases.json",
        r#"[{"section":"s","name":"old-section"}]"#,
    )?;
    f.write(
        "doc/new.aliases.json",
        r#"[{"section":null,"name":"old-new"}]"#,
    )?;
    f.write("doc/ref.md", "original passive bytes")?;
    f.write("doc/old.md", "stale old projection")?;
    f.write("doc/new.md", "stale new projection")?;
    Ok(f)
}
struct MutatingInput {
    input: Cursor<Vec<u8>>,
    root: PathBuf,
    changed: bool,
}
impl MutatingInput {
    fn change(&mut self) -> std::io::Result<()> {
        if !self.changed {
            assert!(!self.root.join("output").exists());
            for name in [
                "canonical.json",
                "old.nepld",
                "new.nepld",
                "old.aliases.json",
                "new.aliases.json",
                "ref.md",
                "old.md",
                "new.md",
            ] {
                fs::remove_file(self.root.join("doc").join(name))?;
            }
            self.changed = true;
        }
        Ok(())
    }
}
impl Read for MutatingInput {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.change()?;
        self.input.read(out)
    }
}
impl BufRead for MutatingInput {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.change()?;
        self.input.fill_buf()
    }
    fn consume(&mut self, n: usize) {
        self.input.consume(n)
    }
}
#[test]
fn canonical_html_retry_freezes_mixed_corpus_and_compatibility_before_pause() -> Result<()> {
    for stage in [
        "preparation",
        "render",
        "compatibility-parse",
        "compatibility-lower",
        "compatibility-output",
    ] {
        let f = fixture()?;
        let raw = fs::read(f.root().join("doc/canonical.json"))?;
        let ordinary = super::super::generate_html(f.root(), "doc/canonical.json")?;
        let full = HtmlRetryLimits::default();
        let mut initial = full;
        match stage {
            "preparation" => initial.html.parse_limits.work = 0,
            "render" => initial.html.output_limits.output_bytes = 1,
            "compatibility-parse" => initial.compatibility.parse_limits.work = 0,
            "compatibility-lower" => initial.compatibility.lower_limits.work = 0,
            _ => initial.compatibility.output_limits.work = 0,
        }
        f.json("initial.json", &serde_json::to_value(initial)?)?;
        f.json("full.json", &serde_json::to_value(full)?)?;
        let mut input = MutatingInput {
            input: Cursor::new(
                format!("resume {}\n", f.root().join("full.json").display()).into_bytes(),
            ),
            root: f.root().to_owned(),
            changed: false,
        };
        let mut messages = Vec::new();
        write_with_io(
            f.root(),
            "doc/canonical.json",
            &f.root().join("initial.json"),
            &f.root().join("output"),
            &mut input,
            &mut messages,
        )?;
        assert!(input.changed);
        let text = String::from_utf8(messages)?;
        if stage.starts_with("compatibility") {
            assert!(text.contains("Paused canonical compatibility"));
        } else {
            assert!(text.contains(&format!("Paused canonical HTML {stage}")));
        }
        for (name, bytes) in &ordinary.files {
            assert_eq!(
                &fs::read(f.root().join("output").join(name))?,
                bytes,
                "{name}"
            );
        }
        assert_eq!(
            fs::read(f.root().join("output/sources/canonical.json"))?,
            raw
        );
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(f.root().join("output/manifest.json"))?)?;
        assert_eq!(
            m["canonical_html_retry"]["compatibility_stage"],
            "completed-retry-job"
        );
        assert_eq!(
            m["canonical_html_retry"]["registry_sha256"],
            references::hash(&raw)
        );
        assert_eq!(
            m["job_execution"]["contract"],
            "nepl3.local-doc-pages.retry-alias-job/1"
        );
        assert_eq!(m["compatibility_markdown"]["copies"][0]["page"], "new");
    }
    Ok(())
}
#[test]
fn canonical_html_retry_compatibility_cancel_and_eof_are_atomic() -> Result<()> {
    for command in ["cancel\n", ""] {
        let f = fixture()?;
        let mut limits = HtmlRetryLimits::default();
        limits.compatibility.output_limits.work = 0;
        f.json("limits.json", &serde_json::to_value(limits)?)?;
        let mut messages = Vec::new();
        let error = write_with_io(
            f.root(),
            "doc/canonical.json",
            &f.root().join("limits.json"),
            &f.root().join("output"),
            &mut Cursor::new(command),
            &mut messages,
        )
        .err()
        .ok_or("must cancel")?;
        assert!(error.to_string().contains("Cancelled"));
        assert!(String::from_utf8(messages)?.contains("Paused canonical compatibility"));
        assert!(!f.root().join("output").exists());
    }
    Ok(())
}
#[test]
fn canonical_html_retry_cancel_and_eof_do_not_publish_completed_compatibility() -> Result<()> {
    for command in [b"cancel\n".as_slice(), b"".as_slice()] {
        let f = fixture()?;
        let mut limits = HtmlRetryLimits::default();
        limits.html.output_limits.work = 0;
        f.json("limits.json", &serde_json::to_value(limits)?)?;
        let error = write_with_io(
            f.root(),
            "doc/canonical.json",
            &f.root().join("limits.json"),
            &f.root().join("output"),
            &mut Cursor::new(command),
            &mut Vec::new(),
        )
        .err()
        .ok_or("cancel")?;
        assert!(error.to_string().contains("Cancelled"));
        assert!(!f.root().join("output").exists());
    }
    Ok(())
}
#[test]
fn canonical_html_retry_configuration_is_strict_and_freezes_compatibility() -> Result<()> {
    for value in [
        r#"{"unknown":1}"#,
        r#"{"html":null}"#,
        r#"{"compatibility":{"output_limits":{"work":1}}}"#,
    ] {
        assert!(serde_json::from_str::<HtmlRetryLimits>(value).is_err());
    }
    let mut old = serde_json::to_value(HtmlRetryLimits::default())?;
    old["compatibility"]
        .as_object_mut()
        .ok_or("compatibility")?
        .remove("job_limits");
    let accepted: HtmlRetryLimits = serde_json::from_value(old)?;
    assert_eq!(
        accepted.compatibility.job_limits,
        projection::retry::preparation::RetryLimits::default().job_limits
    );
    let mut changed = HtmlRetryLimits::default();
    let original = changed;
    changed.compatibility.output_limits.work += 1;
    assert!(!changed.same_compatibility(original));
    let f = fixture()?;
    f.write("bad.json", r#"{"html":{},"html":{}}"#)?;
    assert!(resources::read_configuration::<HtmlRetryLimits>(&f.root().join("bad.json")).is_err());
    Ok(())
}

#[test]
fn canonical_html_retry_rejects_changed_compatibility_and_shrunk_html_grants() -> Result<()> {
    let f = fixture()?;
    let full = HtmlRetryLimits::default();
    let mut initial = full;
    initial.html.parse_limits.work = 0;
    let mut wrong_compatibility = full;
    wrong_compatibility.compatibility.output_limits.work += 1;
    let mut shrunk = full;
    shrunk.html.job_limits.work -= 1;
    for (name, limits) in [
        ("initial.json", initial),
        ("compat.json", wrong_compatibility),
        ("shrunk.json", shrunk),
        ("full.json", full),
    ] {
        f.json(name, &serde_json::to_value(limits)?)?;
    }
    let commands = ["compat.json", "shrunk.json", "full.json"]
        .map(|n| format!("resume {}\n", f.root().join(n).display()))
        .concat();
    let mut messages = Vec::new();
    write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("initial.json"),
        &f.root().join("output"),
        &mut Cursor::new(commands.into_bytes()),
        &mut messages,
    )?;
    let text = String::from_utf8(messages)?;
    assert_eq!(text.matches("Rejected: compatibility limits must remain unchanged and all HTML limits must be nondecreasing.").count(),2);
    Ok(())
}
#[test]
fn canonical_html_retry_without_compatibility_skips_markdown_and_binds_receipt() -> Result<()> {
    let f = fixture()?;
    let mut r: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root().join("doc/canonical.json"))?)?;
    r["compatibility"] = json!([]);
    f.json("doc/canonical.json", &r)?;
    let mut selected = HtmlRetryLimits::default();
    selected.compatibility.output_limits.work = 0;
    f.json("limits.json", &serde_json::to_value(selected)?)?;
    write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("limits.json"),
        &f.root().join("output"),
        &mut Cursor::new(Vec::new()),
        &mut Vec::new(),
    )?;
    let m: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root().join("output/manifest.json"))?)?;
    assert_eq!(
        m["canonical_html_retry"]["compatibility_stage"],
        "not-required"
    );
    assert!(m["compatibility_markdown"].is_null());
    let mut snapshot = m["canonical_html_retry"].clone();
    snapshot
        .as_object_mut()
        .ok_or("snapshot")?
        .remove("identity");
    let mut encoded = b"nepl3.canonical-html.retry/2\0".to_vec();
    for value in [&snapshot, &m["job_execution"], &m["compatibility_markdown"]] {
        encoded.extend_from_slice(&serde_json::to_vec(value)?);
    }
    assert_eq!(
        m["canonical_html_retry"]["identity"],
        references::hash(&encoded)
    );
    assert!(!f.root().join("output/sources/new.md").exists());
    Ok(())
}
#[test]
fn canonical_html_retry_snapshot_alias_cap_applies_without_compatibility() -> Result<()> {
    let f = fixture()?;
    let mut r: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root().join("doc/canonical.json"))?)?;
    r["compatibility"] = json!([]);
    f.json("doc/canonical.json", &r)?;
    let mut raw = b"[]".to_vec();
    raw.resize(524_289, b' ');
    fs::write(f.root().join("doc/old.aliases.json"), &raw)?;
    fs::write(f.root().join("doc/new.aliases.json"), &raw)?;
    f.json(
        "limits.json",
        &serde_json::to_value(HtmlRetryLimits::default())?,
    )?;
    let error = write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("limits.json"),
        &f.root().join("output"),
        &mut Cursor::new(Vec::new()),
        &mut Vec::new(),
    )
    .err()
    .ok_or("snapshot limit")?;
    assert!(error.to_string().contains("SnapshotPreparation"));
    assert!(!f.root().join("output").exists());
    Ok(())
}

#[test]
fn canonical_html_retry_explicit_settings_override_registry_phase_budgets() -> Result<()> {
    let f = fixture()?;
    let mut r: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root().join("doc/canonical.json"))?)?;
    let zero = resources::OperationLimits {
        work: 0,
        ..Default::default()
    };
    for name in [
        "parse_limits",
        "lower_limits",
        "output_limits",
        "html_output_limits",
    ] {
        r[name] = serde_json::to_value(zero)?;
    }
    f.json("doc/canonical.json", &r)?;
    f.json(
        "limits.json",
        &serde_json::to_value(HtmlRetryLimits::default())?,
    )?;
    assert!(super::super::generate_html(f.root(), "doc/canonical.json").is_err());
    write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("limits.json"),
        &f.root().join("output"),
        &mut Cursor::new(Vec::new()),
        &mut Vec::new(),
    )?;
    assert!(f.root().join("output/sources/new.md").exists());
    Ok(())
}

#[test]
fn compatibility_then_html_pause_retains_completed_markdown_and_binds_both_jobs() -> Result<()> {
    let f = fixture()?;
    let ordinary = super::super::generate_html(f.root(), "doc/canonical.json")?;
    let full = HtmlRetryLimits::default();
    let mut middle = full;
    middle.html.parse_limits.work = 0;
    let mut initial = middle;
    initial.compatibility.parse_limits.work = 0;
    for (name, limits) in [
        ("initial.json", initial),
        ("middle.json", middle),
        ("full.json", full),
    ] {
        f.json(name, &serde_json::to_value(limits)?)?;
    }
    let commands = ["middle.json", "full.json"]
        .map(|n| format!("resume {}\n", f.root().join(n).display()))
        .concat();
    let mut input = MutatingInput {
        input: Cursor::new(commands.into_bytes()),
        root: f.root().to_owned(),
        changed: false,
    };
    let mut messages = Vec::new();
    write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("initial.json"),
        &f.root().join("output"),
        &mut input,
        &mut messages,
    )?;
    for (name, bytes) in &ordinary.files {
        assert_eq!(&fs::read(f.root().join("output").join(name))?, bytes);
    }
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root().join("output/manifest.json"))?)?;
    let markdown = &receipt["compatibility_markdown"]["markdown_receipt"];
    assert_eq!(
        markdown["job_execution"]["contract"],
        "nepl3.canonical-markdown.retry-job/2"
    );
    assert_eq!(
        markdown["job_execution"]["preparation_attempts"]
            .as_array()
            .ok_or("attempts")?
            .len(),
        4
    );
    assert_eq!(
        markdown["job_execution"]["render_attempts"]
            .as_array()
            .ok_or("renders")?
            .len(),
        1
    );
    assert_eq!(
        receipt["canonical_html_retry"]["initial_html_limits"]["parse_limits"]["work"],
        0
    );
    assert_eq!(
        receipt["canonical_html_retry"]["initial_compatibility_limits"]["parse_limits"]["work"],
        0
    );
    assert_ne!(
        receipt["canonical_html_retry"]["compatibility_limits"]["parse_limits"]["work"],
        0
    );
    let text = String::from_utf8(messages)?;
    assert_eq!(text.matches("Paused canonical compatibility").count(), 1);
    assert_eq!(text.matches("Paused canonical HTML preparation").count(), 1);
    let mut snapshot = receipt["canonical_html_retry"].clone();
    snapshot
        .as_object_mut()
        .ok_or("snapshot")?
        .remove("identity");
    let mut encoded = b"nepl3.canonical-html.retry/2\0".to_vec();
    for value in [
        &snapshot,
        &receipt["job_execution"],
        &receipt["compatibility_markdown"],
    ] {
        encoded.extend_from_slice(&serde_json::to_vec(value)?);
    }
    assert_eq!(
        receipt["canonical_html_retry"]["identity"],
        references::hash(&encoded)
    );
    Ok(())
}

#[test]
fn compatibility_grants_compare_against_last_accepted_html_limits() -> Result<()> {
    let f = fixture()?;
    let full = HtmlRetryLimits::default();
    let mut initial = full;
    initial.compatibility.parse_limits.work = 0;
    initial.compatibility.output_limits.work = 0;
    let mut raised = full;
    raised.compatibility.output_limits.work = 0;
    raised.html.job_limits.work += 1;
    let mut final_limits = full;
    final_limits.html.job_limits.work += 1;
    for (name, limits) in [
        ("initial.json", initial),
        ("raised.json", raised),
        ("shrunk.json", full),
        ("final.json", final_limits),
    ] {
        f.json(name, &serde_json::to_value(limits)?)?;
    }
    let commands = ["raised.json", "shrunk.json", "final.json"]
        .map(|n| format!("resume {}\n", f.root().join(n).display()))
        .concat();
    let mut messages = Vec::new();
    write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("initial.json"),
        &f.root().join("output"),
        &mut Cursor::new(commands),
        &mut messages,
    )?;
    let text = String::from_utf8(messages)?;
    assert_eq!(
        text.matches("Rejected: compatibility and HTML limits must be nondecreasing.")
            .count(),
        1
    );
    let m: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root().join("output/manifest.json"))?)?;
    assert_eq!(
        m["canonical_html_retry"]["initial_html_limits"]["job_limits"]["work"],
        final_limits.html.job_limits.work
    );
    Ok(())
}

#[test]
fn compatibility_legacy_checkpoint_survives_pause_and_ledgers_are_separate() -> Result<()> {
    let f = fixture()?;
    let full = HtmlRetryLimits::default();
    f.json("full.json", &serde_json::to_value(full)?)?;
    write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("full.json"),
        &f.root().join("baseline"),
        &mut Cursor::new(""),
        &mut Vec::new(),
    )?;
    let baseline: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root().join("baseline/manifest.json"))?)?;
    let first = baseline["compatibility_markdown"]["markdown_receipt"]["job_execution"]["preparation_attempts"][0]["stages"].as_array().ok_or("stages")?;
    let work = first.iter().try_fold(0u64, |sum, s| {
        s["usage"]["work"].as_u64().map(|n| sum + n).ok_or("work")
    })?;
    let mut initial = full;
    initial.compatibility.job_limits.work = work + 1;
    f.json("initial.json", &serde_json::to_value(initial)?)?;
    let mut input = MutatingInput {
        input: Cursor::new(
            format!("resume {}\n", f.root().join("full.json").display()).into_bytes(),
        ),
        root: f.root().to_owned(),
        changed: false,
    };
    let mut messages = Vec::new();
    write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("initial.json"),
        &f.root().join("output"),
        &mut input,
        &mut messages,
    )?;
    let text = String::from_utf8(messages)?;
    assert!(text.contains("role: Legacy"), "{text}");
    let m: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root().join("output/manifest.json"))?)?;
    assert_eq!(m["files"], baseline["files"]);
    let compatibility = &m["compatibility_markdown"]["markdown_receipt"]["job_execution"];
    let attempts = compatibility["preparation_attempts"]
        .as_array()
        .ok_or("attempts")?;
    assert_eq!(attempts.len(), 4);
    assert_eq!(
        attempts
            .iter()
            .filter(|a| a["page_index"] == 0 && a["representation"] == "PageSet")
            .count(),
        1
    );
    for (attempts, renders, total) in [
        (
            attempts,
            compatibility["render_attempts"]
                .as_array()
                .ok_or("renders")?,
            &compatibility["usage"],
        ),
        (
            m["phase_execution"]["evidence"]["attempts"]
                .as_array()
                .ok_or("HTML attempts")?,
            m["job_execution"]["render_attempts"]
                .as_array()
                .ok_or("HTML renders")?,
            &m["job_execution"]["usage"],
        ),
    ] {
        for key in [
            "work",
            "allocation_units",
            "nodes",
            "source_bytes",
            "output_bytes",
            "events",
            "diagnostics",
            "depth",
        ] {
            let mut sum = 0u64;
            for attempt in attempts {
                for stage in attempt["stages"].as_array().ok_or("stages")? {
                    let value = stage["usage"][key].as_u64().ok_or("counter")?;
                    sum = if key == "depth" {
                        sum.max(value)
                    } else {
                        sum + value
                    };
                }
            }
            for render in renders {
                let value = render["usage"][key].as_u64().ok_or("render counter")?;
                sum = if key == "depth" {
                    sum.max(value)
                } else {
                    sum + value
                };
            }
            assert_eq!(total[key].as_u64(), Some(sum), "{key}");
        }
    }
    Ok(())
}

#[test]
fn compatibility_semantic_failure_is_terminal_without_publication() -> Result<()> {
    let f = fixture()?;
    f.write(
        "doc/old.aliases.json",
        r#"[{"section":"missing","name":"alias"}]"#,
    )?;
    f.json(
        "full.json",
        &serde_json::to_value(HtmlRetryLimits::default())?,
    )?;
    let mut input = Cursor::new("resume ignored.json\n");
    let mut messages = Vec::new();
    let error = write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("full.json"),
        &f.root().join("output"),
        &mut input,
        &mut messages,
    )
    .err()
    .ok_or("semantic failure accepted")?;
    assert!(messages.is_empty(), "{error}");
    assert_eq!(input.position(), 0);
    assert!(!f.root().join("output").exists());
    Ok(())
}

#[test]
fn compatibility_append_size_failure_never_publishes_html() -> Result<()> {
    let f = fixture()?;
    // Markdown escapes every '*', so the output-only compatibility copy exceeds
    // its 262144-byte cap even though the original input remains bounded.
    f.write(
        "doc/new.nepld",
        format!(
            "article en \"Large\" body cons paragraph cons \"{}\" nil nil",
            "*".repeat(131_073)
        ),
    )?;
    let mut selected = HtmlRetryLimits::default();
    // This test targets the later copy-size boundary, not a phase budget stop.
    for limit in [
        &mut selected.compatibility.job_limits,
        &mut selected.compatibility.parse_limits,
        &mut selected.compatibility.lower_limits,
        &mut selected.compatibility.output_limits,
        &mut selected.html.job_limits,
        &mut selected.html.parse_limits,
        &mut selected.html.lower_limits,
        &mut selected.html.code_limits,
        &mut selected.html.output_limits,
    ] {
        limit.work = 20_000_000_000;
        limit.allocation_units = 40_000_000_000;
        limit.nodes = 500_000_000;
    }
    f.json("full.json", &serde_json::to_value(selected)?)?;
    let mut messages = Vec::new();
    let error = write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("full.json"),
        &f.root().join("output"),
        &mut Cursor::new(""),
        &mut messages,
    )
    .err()
    .ok_or("oversized copy accepted")?;
    assert!(
        error.to_string().contains("CompatibilityFileLimit"),
        "{error}"
    );
    assert!(messages.is_empty());
    assert!(!f.root().join("output").exists());
    Ok(())
}
