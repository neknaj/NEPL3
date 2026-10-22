use super::*;
use std::{
    fs,
    io::{Cursor, Read},
    path::PathBuf,
};
struct ChangedFiles {
    input: Cursor<Vec<u8>>,
    root: PathBuf,
    changed: bool,
}
impl ChangedFiles {
    fn change(&mut self) -> std::io::Result<()> {
        if !self.changed {
            fs::write(
                self.root.join("pages.json"),
                r#"{"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"}],"display":{"mode":"single","language":"en"}}"#,
            )?;
            fs::write(
                self.root.join("one.nepld"),
                r#"article en "Replaced" body cons paragraph cons "Changed source" nil nil"#,
            )?;
            self.changed = true;
        }
        Ok(())
    }
}
impl Read for ChangedFiles {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.change()?;
        self.input.read(out)
    }
}
impl BufRead for ChangedFiles {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.change()?;
        self.input.fill_buf()
    }
    fn consume(&mut self, n: usize) {
        self.input.consume(n)
    }
}
#[test]
fn interactive_manifest_selection_is_not_reread_on_resume() -> crate::Result<()> {
    for stage in ["preparation", "render"] {
        let root = std::env::temp_dir().join(format!(
            "nepl3-display-resume-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        fs::create_dir(&root)?;
        fs::write(
            root.join("one.nepld"),
            r#"article en Sentence "Title" body cons paragraph cons parallel cons variant en Sentence "English content" cons variant ja Sentence "Japanese content" nil nil nil"#,
        )?;
        fs::write(
            root.join("pages.json"),
            r#"{"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"}],"display":{"mode":"single","language":"fr","fallbacks":["ja","en"]}}"#,
        )?;
        let full = RetryLimits::default();
        let mut initial = full;
        if stage == "preparation" {
            initial.parse_limits.work = 0;
        } else {
            initial.output_limits.work = 0;
        }
        fs::write(root.join("initial.json"), serde_json::to_vec(&initial)?)?;
        fs::write(root.join("full.json"), serde_json::to_vec(&full)?)?;
        let command = format!("resume {}\n", root.join("full.json").display());
        let mut input = ChangedFiles {
            input: Cursor::new(command.into_bytes()),
            root: root.clone(),
            changed: false,
        };
        let mut messages = Vec::new();
        let output = root.join("output");
        write_with_io(
            &root.join("pages.json"),
            &root.join("initial.json"),
            &output,
            MathRenderer::MathMLOnly,
            &mut input,
            &mut messages,
        )?;
        assert!(input.changed);
        assert!(String::from_utf8(messages)?.contains(&format!("Paused {stage}")));
        let html = fs::read_to_string(output.join("one.html"))?;
        assert!(html.contains("Japanese content"));
        assert!(!html.contains("English content"));
        assert!(!html.contains("Changed source"));
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
        assert_eq!(m["options"]["language"], "fr");
        assert_eq!(m["options"]["fallbacks"], serde_json::json!(["ja", "en"]));
        assert_eq!(
            m["job_execution"]["render_attempts"]
                .as_array()
                .ok_or("attempts")?
                .len(),
            if stage == "preparation" { 1 } else { 2 }
        );
        fs::remove_dir_all(root)?;
    }
    Ok(())
}
