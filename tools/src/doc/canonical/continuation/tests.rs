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
    for stage in ["preparation", "render"] {
        let f = fixture()?;
        let raw = fs::read(f.root().join("doc/canonical.json"))?;
        let ordinary = super::super::generate_html(f.root(), "doc/canonical.json")?;
        let full = HtmlRetryLimits::default();
        let mut initial = full;
        if stage == "preparation" {
            initial.html.parse_limits.work = 0;
        } else {
            initial.html.output_limits.output_bytes = 1;
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
        assert!(String::from_utf8(messages)?.contains(&format!("Paused canonical HTML {stage}")));
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
            "completed-single-attempt"
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
fn canonical_html_retry_compatibility_stop_is_terminal_and_atomic() -> Result<()> {
    let f = fixture()?;
    let mut limits = HtmlRetryLimits::default();
    limits.compatibility.output_limits.work = 0;
    f.json("limits.json", &serde_json::to_value(limits)?)?;
    let mut input = Cursor::new(b"resume ignored.json\n".to_vec());
    let mut messages = Vec::new();
    let error = write_with_io(
        f.root(),
        "doc/canonical.json",
        &f.root().join("limits.json"),
        &f.root().join("output"),
        &mut input,
        &mut messages,
    )
    .err()
    .ok_or("must stop")?;
    assert!(
        error
            .to_string()
            .contains("CanonicalPreparationNonResumable")
    );
    assert_eq!(input.position(), 0);
    assert!(!f.root().join("output").exists());
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
    let mut encoded = b"nepl3.canonical-html.retry/1\0".to_vec();
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
