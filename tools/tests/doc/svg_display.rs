use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const SVG: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const MIXED: &str = r#"article en Sentence "Title" body cons image asset "logo" none Sentence "Alt" some Sentence "Caption" cons paragraph cons sentence cons image asset "logo" none Sentence "Inline" nil cons parallel cons variant en Sentence sentence cons document anchor english text "English" cons math frac 1 2 nil cons variant ja Sentence sentence cons document anchor japanese text "Japanese" cons math frac 3 4 nil nil cons code Math frac 5 6 cons sentence cons math Math frac 7 8 nil cons Sentence "Tail" nil nil"#;
const LEGACY: &str = r#"article en "Title" body cons image asset "logo" none "Alt" some "Caption" cons paragraph cons sentence cons image asset "logo" none "Inline" nil cons parallel cons variant en "English" cons variant ja "Japanese" nil cons code Math frac 5 6 cons sentence cons math Math frac 7 8 nil nil nil"#;
fn fixture(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!(
        "nepl3-svg-display-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&path)?;
    fs::write(path.join("image.svg"), SVG)?;
    fs::write(
        path.join("assets.json"),
        r#"{"version":1,"assets":[{"id":"logo","source":"image.svg","mime":"image/svg+xml"}]}"#,
    )?;
    Ok(path)
}
fn run(
    root: &Path,
    display: bool,
    css: &str,
    svg: &str,
    output: &str,
) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"));
    cmd.env_remove("NEPL3_KATEX_NODE_MODULES")
        .args(["doc-html", "svg"]);
    if display {
        cmd.arg("--display").arg(root.join("display.json"));
    }
    cmd.args(["--css", css, "--svg", svg])
        .arg(root.join("input.nepld"))
        .arg(root.join("assets.json"))
        .arg(root.join(output))
        .output()
}
fn files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, Box<dyn std::error::Error>> {
    fn walk(
        root: &Path,
        path: &Path,
        output: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(path)? {
            let p = entry?.path();
            if p.is_dir() {
                walk(root, &p, output)?;
            } else {
                output.insert(p.strip_prefix(root)?.into(), fs::read(&p)?);
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}
#[test]
fn svg_display_explicit_rows_preserves_all_default_paths_and_file_modes()
-> Result<(), Box<dyn std::error::Error>> {
    let root = fixture("rows")?;
    fs::write(root.join("display.json"), r#"{"mode":"rows"}"#)?;
    let alt = LEGACY
        .replace("\"Alt\"", "Sentence \"Alt\"")
        .replace("\"Inline\"", "Sentence \"Inline\"");
    for (index, source) in [LEGACY, alt.as_str(), MIXED].iter().enumerate() {
        fs::write(root.join("input.nepld"), source)?;
        for css in ["inline", "external"] {
            for svg in ["embedded", "external"] {
                let old = format!("default-{index}-{css}-{svg}");
                let new = format!("rows-{index}-{css}-{svg}");
                for (selected, out) in [(false, &old), (true, &new)] {
                    let result = run(&root, selected, css, svg, out)?;
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                }
                assert_eq!(files(&root.join(&old))?, files(&root.join(&new))?);
            }
        }
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
#[test]
fn svg_display_columns_single_and_fallback_use_real_cli_composition()
-> Result<(), Box<dyn std::error::Error>> {
    let root = fixture("selected")?;
    fs::write(root.join("input.nepld"), MIXED)?;
    for (index, config, english, japanese, count) in [
        (0, r#"{"mode":"columns"}"#, true, true, 3),
        (1, r#"{"mode":"single","language":"en"}"#, true, false, 2),
        (
            2,
            r#"{"mode":"single","language":"FR","fallbacks":["ja"]}"#,
            false,
            true,
            2,
        ),
    ] {
        fs::write(root.join("display.json"), config)?;
        for css in ["inline", "external"] {
            for svg in ["embedded", "external"] {
                let out = format!("selected-{index}-{css}-{svg}");
                let result = run(&root, true, css, svg, &out)?;
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                let html = fs::read_to_string(root.join(&out).join("document.html"))?;
                assert_eq!(html.contains("English"), english);
                assert_eq!(html.contains("Japanese"), japanese);
                assert_eq!(html.matches("<math").count(), count);
                assert_eq!(html.matches("<mfrac>").count(), count);
                assert!(
                    html.contains("Caption")
                        && html.contains("nepl-code-marker")
                        && html.contains("alt=\"Alt\"")
                );
                let m: serde_json::Value =
                    serde_json::from_slice(&fs::read(root.join(&out).join("manifest.json"))?)?;
                assert_eq!(
                    m["options"]["parallel"],
                    if index == 0 { "Columns" } else { "Single" }
                );
                assert_eq!(
                    m["display_scope"]["identity"]
                        .as_str()
                        .ok_or("scope")?
                        .len(),
                    64
                );
                assert_eq!(m["options"]["math_renderer"], "katex-preferred");
                if index == 2 {
                    assert_eq!(m["options"]["language"], "FR");
                    assert_eq!(m["options"]["fallbacks"], serde_json::json!(["ja"]));
                }
                for f in m["files"].as_array().ok_or("files")? {
                    let path = f["path"].as_str().ok_or("path")?;
                    let bytes = fs::read(root.join(&out).join(path))?;
                    let hash = nepl3_core::source::Digest::of(&bytes)
                        .0
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>();
                    assert_eq!(f["sha256"], hash);
                }
                let before = files(&root.join(&out))?;
                assert!(!run(&root, true, css, svg, &out)?.status.success());
                assert_eq!(before, files(&root.join(&out))?);
            }
        }
    }
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn svg_display_invalid_configuration_never_creates_output() -> Result<(), Box<dyn std::error::Error>>
{
    let root = fixture("invalid")?;
    fs::write(root.join("input.nepld"), MIXED)?;
    let mut cases = vec![
        "null".to_owned(),
        r#"{"mode":"invalid"}"#.to_owned(),
        r#"{"mode":"rows","unknown":true}"#.to_owned(),
        r#"{"mode":"rows","mode":"columns"}"#.to_owned(),
        r#"{"mode":"rows","\u006dode":"columns"}"#.to_owned(),
        r#"{"mode":"single"}"#.to_owned(),
        r#"{"mode":"single","language":""}"#.to_owned(),
        r#"{"mode":"single","language":"zz","fallbacks":[]}"#.to_owned(),
    ];
    cases.push(format!("{}{}", " ".repeat(65537), r#"{"mode":"rows"}"#));
    for (index, config) in cases.iter().enumerate() {
        fs::write(root.join("display.json"), config)?;
        let output = format!("invalid-{index}");
        let result = run(&root, true, "inline", "embedded", &output)?;
        assert!(!result.status.success(), "accepted {index}: {config}");
        assert!(
            !root.join(output).exists(),
            "published invalid case {index}"
        );
    }
    // An existing flag-named file must not turn a repeated option into a path.
    fs::write(root.join("--display"), r#"{"mode":"rows"}"#)?;
    let result = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .current_dir(&root)
        .args([
            "doc-html",
            "svg",
            "--display",
            "--display",
            "--css",
            "inline",
            "--svg",
            "embedded",
            "input.nepld",
            "assets.json",
            "repeated",
        ])
        .output()?;
    assert!(!result.status.success());
    assert!(!root.join("repeated").exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn svg_display_exact_request_identity_and_hidden_validation()
-> Result<(), Box<dyn std::error::Error>> {
    let root = fixture("identity-hidden")?;
    fs::write(root.join("input.nepld"), MIXED)?;
    let mut identities = std::collections::BTreeSet::new();
    let mut first_html = None;
    for (index, config) in [
        r#"{"mode":"single","language":"en"}"#,
        r#"{"mode":"single","language":"EN"}"#,
        r#"{"mode":"single","language":"en","fallbacks":["ja","fr"]}"#,
        r#"{"mode":"single","language":"en","fallbacks":["fr","ja"]}"#,
    ]
    .iter()
    .enumerate()
    {
        fs::write(root.join("display.json"), config)?;
        let out = format!("identity-{index}");
        let result = run(&root, true, "inline", "embedded", &out)?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let html = fs::read(root.join(&out).join("document.html"))?;
        if let Some(first) = &first_html {
            assert_eq!(first, &html);
        } else {
            first_html = Some(html);
        }
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(&out).join("manifest.json"))?)?;
        assert!(
            identities.insert(
                m["display_scope"]["identity"]
                    .as_str()
                    .ok_or("identity")?
                    .to_owned()
            )
        );
    }
    fs::write(
        root.join("display.json"),
        r#"{"mode":"single","language":"en"}"#,
    )?;
    // The omitted language's root SVG is still inspected. An asset manifest
    // for logo must not silently satisfy another asset identifier.
    let hidden = r#"article en "Title" body cons image asset "logo" none "root" none cons paragraph cons parallel cons variant en "English" cons variant ja sentence cons image asset "missing" none "Alt" nil nil nil nil"#;
    fs::write(root.join("input.nepld"), hidden)?;
    let result = run(&root, true, "inline", "embedded", "hidden")?;
    assert!(!result.status.success());
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(error.contains("Missing"), "{error}");
    assert!(!root.join("hidden").exists());
    // A root-owned SVG cannot authorize even an omitted guest-owned asset.
    let guest = r#"article en Sentence "Title" body cons image asset "logo" none "root" none cons paragraph cons parallel cons variant en Sentence "English" cons variant ja Sentence sentence cons document image asset "logo" none "Alt" nil nil nil nil"#;
    fs::write(root.join("input.nepld"), guest)?;
    let result = run(&root, true, "inline", "embedded", "guest")?;
    assert!(!result.status.success());
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(
        error.contains("MemberId(1)") && error.contains("Missing"),
        "{error}"
    );
    assert!(!root.join("guest").exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn svg_display_selection_keeps_legacy_and_alt_only_routes() -> Result<(), Box<dyn std::error::Error>>
{
    let root = fixture("legacy-selected")?;
    let alt = LEGACY
        .replace("\"Alt\"", "Sentence \"Alt\"")
        .replace("\"Inline\"", "Sentence \"Inline\"");
    for (kind, source) in [LEGACY, alt.as_str()].iter().enumerate() {
        fs::write(root.join("input.nepld"), source)?;
        for (mode, config) in [
            ("columns", r#"{"mode":"columns"}"#),
            (
                "single",
                r#"{"mode":"single","language":"fr","fallbacks":["ja"]}"#,
            ),
        ] {
            fs::write(root.join("display.json"), config)?;
            for css in ["inline", "external"] {
                for svg in ["embedded", "external"] {
                    let out = format!("legacy-{kind}-{mode}-{css}-{svg}");
                    let result = run(&root, true, css, svg, &out)?;
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    let html = fs::read_to_string(root.join(&out).join("document.html"))?;
                    assert_eq!(html.contains("English"), mode == "columns");
                    assert!(
                        html.contains("Japanese")
                            && html.contains("Caption")
                            && html.contains("alt=\"Alt\"")
                    );
                    assert_eq!(html.matches("<math").count(), 1);
                    let manifest: serde_json::Value =
                        serde_json::from_slice(&fs::read(root.join(&out).join("manifest.json"))?)?;
                    assert!(manifest.get("sentence_svg").is_none());
                    assert_eq!(manifest.get("sentence_alt").is_some(), kind == 1);
                }
            }
        }
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
