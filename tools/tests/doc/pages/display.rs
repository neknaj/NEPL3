use std::{fs, process::Command};
fn root(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nepl3-display-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    Ok(root)
}
const SOURCE: &str = r#"article en Sentence "Title" body cons paragraph cons parallel cons variant en Sentence "English content" cons variant ja Sentence "Japanese content" nil nil nil"#;
fn manifest() -> serde_json::Value {
    serde_json::json!({"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"}]})
}
#[test]
fn cli_manifest_selects_display_and_preserves_default_rows()
-> Result<(), Box<dyn std::error::Error>> {
    let root = root("modes")?;
    fs::write(root.join("one.nepld"), SOURCE)?;
    let input = root.join("pages.json");
    let mut default = None;
    for (i, (selection, english, japanese, mode)) in [
        (None, true, true, "Rows"),
        (Some(serde_json::json!({"mode":"rows"})), true, true, "Rows"),
        (
            Some(serde_json::json!({"mode":"columns"})),
            true,
            true,
            "Columns",
        ),
        (
            Some(serde_json::json!({"mode":"single","language":"en"})),
            true,
            false,
            "Single",
        ),
        (
            Some(serde_json::json!({"mode":"single","language":"fr","fallbacks":["ja","en"]})),
            false,
            true,
            "Single",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut value = manifest();
        if let Some(selection) = selection {
            value["display"] = selection;
        }
        fs::write(&input, serde_json::to_vec(&value)?)?;
        let out = root.join(format!("out-{i}"));
        let result = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(["doc-html", "pages", "--math-renderer", "mathml-only"])
            .arg(&input)
            .arg(&out)
            .output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let html = fs::read_to_string(out.join("one.html"))?;
        assert_eq!(html.contains("English content"), english);
        assert_eq!(html.contains("Japanese content"), japanese);
        let generated: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
        assert_eq!(generated["options"]["parallel"], mode);
        if i == 0 {
            default = Some((html, generated));
        } else if i == 1 {
            let (old_html, old_manifest) = default.as_ref().ok_or("default")?;
            assert_eq!(&html, old_html);
            assert_eq!(
                generated["execution_identity"],
                old_manifest["execution_identity"]
            );
        }
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
#[test]
fn cli_manifest_rejects_bad_selection_before_publication() -> Result<(), Box<dyn std::error::Error>>
{
    let root = root("reject")?;
    fs::write(root.join("one.nepld"), SOURCE)?;
    let input = root.join("pages.json");
    for (i, selection) in [
        serde_json::Value::Null,
        serde_json::json!({"mode":"Rows"}),
        serde_json::json!({"mode":"rows","language":"en"}),
        serde_json::json!({"mode":"single"}),
        serde_json::json!({"mode":"single","language":"missing"}),
        serde_json::json!({"mode":"single","language":"fr","fallbacks":["ja","ja"]}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut value = manifest();
        value["display"] = selection;
        fs::write(&input, serde_json::to_vec(&value)?)?;
        let out = root.join(format!("bad-{i}"));
        let result = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(["doc-html", "pages", "--math-renderer", "mathml-only"])
            .arg(&input)
            .arg(&out)
            .output()?;
        assert!(!result.status.success());
        assert!(!out.exists());
    }
    fs::write(
        &input,
        r#"{"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"}],"display":{"mode":"rows","mode":"columns"}}"#,
    )?;
    let duplicate = root.join("duplicate");
    assert!(
        nepl3_tools::doc::export::pages::write_with_renderer(
            &input,
            &duplicate,
            nepl3_tools::doc::export::MathRenderer::MathMLOnly
        )
        .is_err()
    );
    assert!(!duplicate.exists());
    fs::write(
        root.join("one.nepld"),
        r#"article en Sentence "Hidden" body cons paragraph cons parallel cons variant en Sentence sentence cons document anchor target text "English target" nil cons variant ja Sentence sentence cons document ref target text "Japanese link" nil nil nil nil"#,
    )?;
    let mut value = manifest();
    value["display"] = serde_json::json!({"mode":"single","language":"ja"});
    fs::write(&input, serde_json::to_vec(&value)?)?;
    let out = root.join("hidden");
    let error = nepl3_tools::doc::export::pages::write_with_renderer(
        &input,
        &out,
        nepl3_tools::doc::export::MathRenderer::MathMLOnly,
    )
    .err()
    .ok_or("hidden target accepted")?;
    assert!(error.to_string().contains("MissingFragment"), "{error}");
    assert!(!out.exists());
    fs::remove_dir_all(root)?;
    Ok(())
}
