use super::*;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> crate::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "nepl3-body-host-test-{}-{}",
            std::process::id(),
            NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn reject_before_writer(path: &Path) -> crate::Result<String> {
    let mut called = false;
    let result = publish(path, |_| {
        called = true;
        Ok(())
    });
    let error = result.err().ok_or("expected rejection")?;
    assert!(!called);
    Ok(error.to_string())
}

#[test]
fn staged_publication_is_complete_and_never_replaces() -> crate::Result<()> {
    let f = Fixture::new()?;
    let output = f.0.join("out.md");
    assert!(
        publish(&output, |file| {
            file.write_all(b"partial")?;
            Err(std::io::Error::other("injected write failure"))
        })
        .is_err()
    );
    assert!(!output.exists());
    assert_eq!(fs::read_dir(&f.0)?.count(), 0);
    assert!(
        publish(&output, |file| {
            file.write_all(b"candidate")?;
            fs::write(&output, b"winner")
        })
        .is_err()
    );
    assert_eq!(fs::read(&output)?, b"winner");
    assert_eq!(fs::read_dir(&f.0)?.count(), 1);
    fs::remove_file(&output)?;
    publish(&output, |file| file.write_all(b"complete"))?;
    assert_eq!(fs::read(&output)?, b"complete");
    assert_eq!(fs::read_dir(&f.0)?.count(), 1);
    reject_before_writer(&output)?;
    fs::remove_file(&output)?;
    publish(&output, |_| Ok(()))?;
    assert_eq!(fs::metadata(&output)?.len(), 0);
    Ok(())
}

#[test]
fn bounded_regular_inputs_and_limits_do_not_accept_invalid_data() -> crate::Result<()> {
    let f = Fixture::new()?;
    let p = f.0.join("input");
    fs::write(&p, b"abcd")?;
    assert_eq!(read_regular(&p, 4)?, "abcd");
    assert!(read_regular(&p, 3).is_err());
    fs::write(&p, [0xff])?;
    assert!(read_regular(&p, 4).is_err());
    assert!(read_regular(&f.0, 4).is_err());
    for config in [
        "{\"parse_limits\":{},\"parse_limits\":{}}",
        "{\"unknown\":1}",
    ] {
        fs::write(&p, config)?;
        assert!(read_limits(&p).is_err());
    }
    fs::write(&p, vec![b' '; 65_537])?;
    assert!(read_limits(&p).is_err());
    fs::write(&p, b"{}")?;
    read_limits(&p)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinks_and_static_special_files_are_not_clobbered_or_opened() -> crate::Result<()> {
    use std::os::unix::fs::symlink;
    let f = Fixture::new()?;
    let output = f.0.join("out.md");
    let target = f.0.join("missing");
    symlink(&target, &output)?;
    reject_before_writer(&output)?;
    assert_eq!(fs::read_link(&output)?, target);
    assert!(!target.exists());
    fs::write(&target, b"original")?;
    assert!(publish(&output, |_| Ok(())).is_err());
    assert_eq!(fs::read(&target)?, b"original");
    // Unix device file is a stable non-regular input; do not open/read it.
    assert!(read_regular(Path::new("/dev/null"), 10).is_err());
    Ok(())
}

#[test]
fn cleanup_errors_report_whether_complete_output_was_published() -> crate::Result<()> {
    for committed in [false, true] {
        let f = Fixture::new()?;
        let output = f.0.join("out.md");
        let error = publish_with_cleanup(
            &output,
            |file| {
                file.write_all(b"bytes")?;
                if committed {
                    Ok(())
                } else {
                    Err(std::io::Error::other("write failed"))
                }
            },
            |_| Err(std::io::Error::other("cleanup failed")),
        )
        .err()
        .ok_or("expected error")?
        .to_string();
        assert_eq!(output.exists(), committed);
        if committed {
            assert_eq!(fs::read(&output)?, b"bytes");
            assert!(error.contains("complete output published"));
        } else {
            assert!(error.contains("publication failed"));
        }
        assert!(error.contains("staging cleanup failed"));
    }
    Ok(())
}

#[test]
fn source_ceiling_and_exact_output_budget_are_enforced() -> crate::Result<()> {
    let c = crate::doc::source::compiled()?;
    let source = r#"body cons paragraph cons "hello" nil nil"#;
    let phases = PhaseLimits::default();
    let mut baseline = crate::doc::source::budget();
    let expected = from_source_profile(&c, source, phases, &mut baseline)?;
    let mut limits = baseline.limits();
    limits.output_bytes = baseline.usage().output_bytes;
    assert_eq!(
        from_source_profile(&c, source, phases, &mut Budget::new(limits))?.markdown,
        expected.markdown
    );
    limits.output_bytes -= 1;
    assert!(
        from_source_profile(&c, source, phases, &mut Budget::new(limits))
            .err()
            .ok_or("expected error")?
            .contains("OutputLimit")
    );
    let mut cancelled = crate::doc::source::budget();
    cancelled.cancel();
    assert!(
        from_source_profile(&c, source, phases, &mut cancelled)
            .err()
            .ok_or("expected error")?
            .contains("Cancelled")
    );
    let oversized = " ".repeat(crate::doc::export::MAX_SOURCE_BYTES as usize + 1);
    assert!(
        from_source(&c, &oversized, Default::default())
            .err()
            .ok_or("expected error")?
            .contains("SourceLimit")
    );
    let f = Fixture::new()?;
    let input = f.0.join("big.nepld");
    let output = f.0.join("out.md");
    fs::write(&input, oversized)?;
    assert!(write(&input, &output, Default::default()).is_err());
    assert!(!output.exists());
    Ok(())
}

#[test]
fn caller_selected_stage_spelling_never_becomes_the_staging_file() -> crate::Result<()> {
    for spelling in ["exact", "upper", "trailing"] {
        let f = Fixture::new()?;
        let mut name = Stage::name(0);
        assert_eq!(name.len(), 12);
        assert!(name[..8].bytes().all(|c| c.is_ascii_alphanumeric()));
        assert_eq!(&name[8..], ".tmp");
        if spelling == "upper" {
            name = name.to_ascii_uppercase();
        }
        if spelling == "trailing" {
            name.push_str(". ");
        }
        let output = f.0.join(name);
        let mut serial = 0;
        let mut stage = Stage::create_with_serial(&output, || {
            let value = serial;
            serial += 1;
            value
        })?;
        assert_eq!(serial, 2);
        stage.file.write_all(b"partial")?;
        assert!(!output.exists());
        drop(stage.file);
        fs::remove_file(stage.path)?;
        let directory = f.0.join("directory");
        fs::create_dir(&directory)?;
        reject_before_writer(&directory)?;
        assert!(directory.is_dir());
    }
    Ok(())
}

#[test]
fn stream_alias_spellings_are_rejected_before_staging() -> crate::Result<()> {
    let f = Fixture::new()?;
    for suffix in [":$DATA", "::$DATA", ":named"] {
        let output = f.0.join(format!("{}{suffix}", Stage::name(0)));
        let error = reject_before_writer(&output)?;
        assert!(error.contains("stream-qualified"));
        assert_eq!(fs::read_dir(&f.0)?.count(), 0);
    }
    #[cfg(windows)]
    assert!(validate_output(Path::new(r"C:\ordinary\out.md")).is_ok());
    Ok(())
}

#[cfg(windows)]
#[test]
fn windows_staging_short_name_has_no_distinct_basename() -> crate::Result<()> {
    let f = Fixture::new()?;
    let stage = Stage::create(&f.0.join("out.md"))?;
    let name = stage
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("staging name")?
        .to_owned();
    assert_eq!(name.len(), 12);
    assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.'));
    drop(stage.file);
    // Windows FOR's documented s/n/x modifiers query short name + extension.
    // Only the generated ASCII8.3 basename enters this command; the directory
    // is passed separately, never interpolated into shell text.
    // https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/for
    let result = std::process::Command::new("cmd.exe")
        .current_dir(&f.0)
        .args(["/D", "/E:ON", "/C"])
        .arg(format!("for %I in ({name}) do @echo %~snxI"))
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let short = std::str::from_utf8(&result.stdout)?.trim();
    assert!(short.eq_ignore_ascii_case(&name), "{short} != {name}");
    fs::remove_file(stage.path)?;
    Ok(())
}
