use super::*;
use std::{
    fs,
    io::{Cursor, Read},
    path::PathBuf,
};
struct ChangedResource {
    input: Cursor<Vec<u8>>,
    root: PathBuf,
    delete: bool,
    changed: bool,
}
impl ChangedResource {
    fn change(&mut self) -> std::io::Result<()> {
        if !self.changed {
            assert!(
                !self.root.join("output").exists(),
                "paused job must not publish partial output"
            );
            if self.delete {
                fs::remove_file(self.root.join("ref.txt"))?;
            } else {
                fs::write(self.root.join("ref.txt"), b"replaced resource")?;
            }
            fs::remove_file(self.root.join("pages.json"))?;
            self.changed = true;
        }
        Ok(())
    }
}
impl Read for ChangedResource {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.change()?;
        self.input.read(out)
    }
}
impl BufRead for ChangedResource {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.change()?;
        self.input.fill_buf()
    }
    fn consume(&mut self, n: usize) {
        self.input.consume(n)
    }
}
#[test]
fn passive_files_and_manifest_are_frozen_across_cli_resume() -> crate::Result<()> {
    for stage in ["preparation", "render"] {
        for delete in [false, true] {
            let root = std::env::temp_dir().join(format!(
                "nepl3-files-resume-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos()
            ));
            fs::create_dir(&root)?;
            fs::write(
                root.join("one.nepld"),
                r#"article en "Files" body cons paragraph cons sentence cons link relative "ref.txt" none text "Reference" nil nil nil"#,
            )?;
            fs::write(root.join("ref.txt"), b"original bytes\n")?;
            fs::write(
                root.join("pages.json"),
                r#"{"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"}],"files":[{"id":"ref","source":"ref.txt","route":"sources/ref.txt"}]}"#,
            )?;
            let full = RetryLimits::default();
            let mut initial = full;
            if stage == "preparation" {
                initial.parse_limits.work = 0;
            } else {
                initial.output_limits.output_bytes = 1;
            }
            fs::write(root.join("initial.json"), serde_json::to_vec(&initial)?)?;
            fs::write(root.join("full.json"), serde_json::to_vec(&full)?)?;
            let mut input = ChangedResource {
                input: Cursor::new(
                    format!("resume {}\n", root.join("full.json").display()).into_bytes(),
                ),
                root: root.clone(),
                delete,
                changed: false,
            };
            let mut messages = Vec::new();
            write_with_io(
                &root.join("pages.json"),
                &root.join("initial.json"),
                &root.join("output"),
                MathRenderer::MathMLOnly,
                &mut input,
                &mut messages,
            )?;
            assert!(input.changed);
            assert!(String::from_utf8(messages)?.contains(&format!("Paused {stage}")));
            assert_eq!(
                fs::read(root.join("output/sources/ref.txt"))?,
                b"original bytes\n"
            );
            let m: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join("output/manifest.json"))?)?;
            assert_eq!(
                m["registered_files"][0]["sha256"],
                super::super::digest_hex(nepl3_core::source::Digest::of(b"original bytes\n"))
            );
            assert_eq!(
                m["job_execution"]["render_attempts"]
                    .as_array()
                    .ok_or("attempts")?
                    .len(),
                if stage == "render" { 2 } else { 1 }
            );
            fs::remove_dir_all(root)?;
        }
    }
    Ok(())
}

#[test]
fn passive_file_cli_cancel_and_eof_publish_nothing() -> crate::Result<()> {
    for command in [b"cancel\n".as_slice(), b"".as_slice()] {
        let root = std::env::temp_dir().join(format!(
            "nepl3-files-cancel-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        fs::create_dir(&root)?;
        fs::write(root.join("one.nepld"), r#"article en "Files" body nil"#)?;
        fs::write(root.join("ref.txt"), b"original")?;
        fs::write(
            root.join("pages.json"),
            r#"{"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"}],"files":[{"id":"ref","source":"ref.txt","route":"ref.txt"}]}"#,
        )?;
        let mut limits = RetryLimits::default();
        limits.output_limits.work = 0;
        fs::write(root.join("limits.json"), serde_json::to_vec(&limits)?)?;
        let mut messages = Vec::new();
        let error = write_with_io(
            &root.join("pages.json"),
            &root.join("limits.json"),
            &root.join("output"),
            MathRenderer::MathMLOnly,
            &mut Cursor::new(command),
            &mut messages,
        )
        .err()
        .ok_or("must cancel")?;
        assert!(error.to_string().contains("Cancelled"));
        assert!(String::from_utf8(messages)?.contains("Paused render"));
        assert!(!root.join("output").exists());
        fs::remove_dir_all(root)?;
    }
    Ok(())
}
