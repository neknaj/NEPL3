use super::*;
use crate::doc::canonical::projection::retry::OutputRetryLimits;
use std::io::Cursor;
fn fixture() -> Result<Fixture> {
    let f = Fixture::new()?;
    f.write(
        "page.nepld",
        r#"article en "Page" body cons paragraph cons "Text" nil nil"#,
    )?;
    f.write("aliases.json", "[]")?;
    f.json(
        "pages.json",
        &json!({"version":1,"pages":[{
            "id":"page","source":"page.nepld","projection":"page.md","route":"page.md",
            "aliases":"aliases.json","renderer":"nepl3-tools.markdown-footnotes-pages/1"
        }]}),
    )?;
    let mut limits = OutputRetryLimits::default();
    limits.output_limits.work = 10;
    f.json("small.json", &serde_json::to_value(limits)?)?;
    f.json(
        "large.json",
        &serde_json::to_value(OutputRetryLimits::default())?,
    )?;
    Ok(f)
}
#[test]
fn interactive_output_cancellation_eof_and_long_command_never_publish() -> Result<()> {
    for command in ["cancel\n".to_string(), String::new(), "x".repeat(4097)] {
        let f = fixture()?;
        let output = f.root().join("output");
        let mut messages = Vec::new();
        let error = markdown_interactive_output_with_io(
            f.root(),
            "pages.json",
            &f.root().join("small.json"),
            &output,
            &mut Cursor::new(command.as_bytes()),
            &mut messages,
        )
        .err()
        .ok_or("unexpected completion")?;
        assert!(!output.exists());
        assert!(error.to_string().contains("attempts="));
        if command.len() > 4096 {
            assert!(error.to_string().contains("ResumeCommandLimit"));
        } else {
            assert!(error.to_string().contains("Cancelled"));
        }
    }
    Ok(())
}
#[test]
fn interactive_output_rejects_bad_or_shrinking_grants_then_resumes() -> Result<()> {
    let f = fixture()?;
    f.write("bad.json", "{bad}")?;
    let mut shrink = OutputRetryLimits::default();
    shrink.output_limits.work = 1;
    f.json("shrink.json", &serde_json::to_value(shrink)?)?;
    let commands = format!(
        "unknown\nresume {}\nresume {}\nresume {}\n",
        f.root().join("bad.json").display(),
        f.root().join("shrink.json").display(),
        f.root().join("large.json").display()
    );
    let output = f.root().join("output");
    let mut messages = Vec::new();
    markdown_interactive_output_with_io(
        f.root(),
        "pages.json",
        &f.root().join("small.json"),
        &output,
        &mut Cursor::new(commands),
        &mut messages,
    )?;
    let messages = String::from_utf8(messages)?;
    assert!(messages.contains("Expected resume"));
    assert!(messages.contains("Cannot read limits"));
    assert!(messages.contains("nondecreasing"));
    assert!(output.join("page.md").is_file());
    assert!(output.join("manifest.json").is_file());
    Ok(())
}
#[test]
fn semantic_failure_never_prompts_or_publishes() -> Result<()> {
    let f = fixture()?;
    f.write("page.nepld", r#"article en "Broken" body cons paragraph cons sentence cons link relative "missing.md" none text "Missing" nil nil nil"#)?;
    let output = f.root().join("output");
    let mut messages = Vec::new();
    let error = markdown_interactive_output_with_io(
        f.root(),
        "pages.json",
        &f.root().join("large.json"),
        &output,
        &mut Cursor::new("cancel\n"),
        &mut messages,
    )
    .err()
    .ok_or("invalid link accepted")?;
    assert!(!output.exists());
    assert!(messages.is_empty());
    assert!(error.to_string().contains("attempts="));
    Ok(())
}
#[test]
fn input_io_failure_keeps_stopped_attempt_evidence() -> Result<()> {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test input failure"))
        }
    }
    impl std::io::BufRead for Broken {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            Err(std::io::Error::other("test input failure"))
        }
        fn consume(&mut self, _: usize) {}
    }
    let f = fixture()?;
    let output = f.root().join("output");
    let error = markdown_interactive_output_with_io(
        f.root(),
        "pages.json",
        &f.root().join("small.json"),
        &output,
        &mut Broken,
        &mut Vec::new(),
    )
    .err()
    .ok_or("input failure accepted")?;
    let text = error.to_string();
    assert!(text.contains("test input failure"));
    assert!(text.contains("previous stop=WorkLimit"));
    assert!(text.contains("attempts="));
    assert!(!output.exists());
    Ok(())
}
#[test]
fn full_retry_preparation_cancel_eof_and_resume_do_not_publish_early() -> Result<()> {
    use crate::doc::canonical::projection::retry::preparation::RetryLimits;
    for kind in [0, 1, 2] {
        let cancelled = kind < 2;
        let f = fixture()?;
        let mut small = RetryLimits::default();
        small.parse_limits.work = 0;
        f.json("full-small.json", &serde_json::to_value(small)?)?;
        f.json(
            "full-large.json",
            &serde_json::to_value(RetryLimits::default())?,
        )?;
        let output = f.root().join("full-output");
        let command = if kind == 0 {
            String::new()
        } else if kind == 1 {
            "cancel\n".to_owned()
        } else {
            format!("resume {}\n", f.root().join("full-large.json").display())
        };
        let mut messages = Vec::new();
        let result = markdown_interactive_with_io(
            f.root(),
            "pages.json",
            &f.root().join("full-small.json"),
            &output,
            &mut Cursor::new(command),
            &mut messages,
        );
        assert!(String::from_utf8(messages)?.contains("preparation page 0"));
        if cancelled {
            let error = result.err().ok_or("EOF completed")?;
            assert!(error.to_string().contains("Cancelled"));
            assert!(error.to_string().contains("preparation="));
            assert!(!output.exists());
        } else {
            result?;
            let r: serde_json::Value =
                serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
            assert_eq!(
                r["job_execution"]["preparation_attempts"]
                    .as_array()
                    .ok_or("attempts")?
                    .len(),
                2
            );
            assert!(output.join("page.md").exists());
        }
    }
    Ok(())
}
#[test]
fn full_retry_terminal_source_and_cross_page_errors_never_prompt_or_publish() -> Result<()> {
    use crate::doc::canonical::projection::retry::preparation::RetryLimits;
    for malformed in [true, false] {
        let f = fixture()?;
        f.json("full.json", &serde_json::to_value(RetryLimits::default())?)?;
        if malformed {
            f.write("page.nepld", "not-a-document")?;
        } else {
            f.write("page.nepld",r#"article en "Page" body cons paragraph cons sentence cons link relative "other.md" some "missing" text "Other" nil nil nil"#)?;
            f.write("other.nepld", r#"article en "Other" body nil"#)?;
            let mut registry: serde_json::Value =
                serde_json::from_slice(&fs::read(f.root().join("pages.json"))?)?;
            registry["pages"].as_array_mut().ok_or("pages")?.push(json!({"id":"other","source":"other.nepld","projection":"other.md","route":"other.md","aliases":"aliases.json","renderer":"nepl3-tools.markdown-footnotes-pages/1"}));
            f.json("pages.json", &registry)?;
        }
        let output = f.root().join("failure-output");
        let mut messages = Vec::new();
        let error = markdown_interactive_with_io(
            f.root(),
            "pages.json",
            &f.root().join("full.json"),
            &output,
            &mut Cursor::new("cancel\n"),
            &mut messages,
        )
        .err()
        .ok_or("invalid source accepted")?;
        let text = error.to_string();
        assert!(messages.is_empty());
        assert!(!output.exists());
        assert!(text.contains("preparation="));
        if malformed {
            assert!(text.contains("stage: Parse"));
        } else {
            assert!(text.contains("Markdown page set"));
            assert!(text.contains("page_index: 1"));
        }
    }
    Ok(())
}

#[test]
fn legacy_output_cli_cancels_eof_and_semantic_failure_without_publication() -> Result<()> {
    for semantic in [false, true] {
        for command in ["cancel\n", ""] {
            let f = fixture()?;
            f.write(
                "doc/page.nepld",
                r#"article en "Page" body cons paragraph cons "Text" nil nil"#,
            )?;
            f.write("doc/aliases.json", "[]")?;
            f.json(
                "pages.json",
                &json!({"version":1,"pages":[{
                    "id":"legacy","source":"doc/page.nepld","projection":"doc/page.md","route":"page.html",
                    "aliases":"doc/aliases.json","renderer":host::RENDERER
                }]}),
            )?;
            if semantic {
                f.write(
                    "doc/aliases.json",
                    r#"[{"section":"absent","name":"bad-anchor"}]"#,
                )?;
            }
            let output = f.root().join("legacy-output");
            let mut messages = Vec::new();
            let error = markdown_interactive_output_with_io(
                f.root(),
                "pages.json",
                &f.root()
                    .join(if semantic { "large.json" } else { "small.json" }),
                &output,
                &mut Cursor::new(command),
                &mut messages,
            )
            .err()
            .ok_or("unexpected completion")?;
            assert!(!output.exists());
            if semantic {
                assert!(messages.is_empty());
                assert!(!error.to_string().contains("Cancelled"));
            } else {
                assert!(error.to_string().contains("Cancelled"), "{error}");
                assert!(String::from_utf8(messages)?.contains("Paused Markdown output"));
            }
        }
    }
    Ok(())
}

#[test]
fn legacy_preparation_cli_cancels_eof_and_semantic_failure_without_publication() -> Result<()> {
    for semantic in [false, true] {
        for command in ["cancel\n", ""] {
            let f = fixture()?;
            f.write(
                "doc/page.nepld",
                r#"article en "Page" body cons paragraph cons "Text" nil nil"#,
            )?;
            f.write("doc/aliases.json", "[]")?;
            f.json(
                "pages.json",
                &json!({"version":1,"pages":[{
                    "id":"legacy","source":"doc/page.nepld","projection":"doc/page.md","route":"page.html",
                    "aliases":"doc/aliases.json","renderer":host::RENDERER
                }]}),
            )?;
            if semantic {
                f.write(
                    "doc/aliases.json",
                    r#"[{"section":"absent","name":"bad-anchor"}]"#,
                )?;
            }
            let mut limits = projection::retry::preparation::RetryLimits::default();
            if !semantic {
                limits.parse_limits.work = 0;
            }
            f.json("legacy-limits.json", &serde_json::to_value(limits)?)?;
            let output = f.root().join("legacy-preparation");
            let mut messages = Vec::new();
            let error = markdown_interactive_with_io(
                f.root(),
                "pages.json",
                &f.root().join("legacy-limits.json"),
                &output,
                &mut Cursor::new(command),
                &mut messages,
            )
            .err()
            .ok_or("unexpected completion")?;
            assert!(!output.exists());
            if semantic {
                assert!(messages.is_empty());
                assert!(!error.to_string().contains("Cancelled"));
            } else {
                assert!(error.to_string().contains("Cancelled"), "{error}");
                assert!(String::from_utf8(messages)?.contains("representation Legacy"));
            }
        }
    }
    Ok(())
}

#[test]
fn legacy_preparation_cli_rejects_shrinking_grant_then_resumes() -> Result<()> {
    let f = fixture()?;
    f.write(
        "doc/page.nepld",
        r#"article en "Page" body cons paragraph cons "Text" nil nil"#,
    )?;
    f.write("doc/aliases.json", "[]")?;
    f.json(
        "pages.json",
        &json!({"version":1,"pages":[{
            "id":"legacy","source":"doc/page.nepld","projection":"doc/page.md","route":"page.html",
            "aliases":"doc/aliases.json","renderer":host::RENDERER
        }]}),
    )?;
    let mut initial = projection::retry::preparation::RetryLimits::default();
    initial.parse_limits.work = 0;
    f.json("initial.json", &serde_json::to_value(initial)?)?;
    let mut shrink = projection::retry::preparation::RetryLimits::default();
    shrink.output_limits.work = 0;
    f.json("shrink.json", &serde_json::to_value(shrink)?)?;
    f.json(
        "full.json",
        &serde_json::to_value(projection::retry::preparation::RetryLimits::default())?,
    )?;
    let commands = format!(
        "resume {}\nresume {}\n",
        f.root().join("shrink.json").display(),
        f.root().join("full.json").display()
    );
    let output = f.root().join("resumed");
    let mut messages = Vec::new();
    markdown_interactive_with_io(
        f.root(),
        "pages.json",
        &f.root().join("initial.json"),
        &output,
        &mut Cursor::new(commands),
        &mut messages,
    )?;
    assert!(output.join("doc/page.md").is_file());
    let messages = String::from_utf8(messages)?;
    assert!(messages.contains("representation Legacy"));
    assert!(
        messages.contains("Rejected: all selected limits must be nondecreasing."),
        "{messages}"
    );
    Ok(())
}
