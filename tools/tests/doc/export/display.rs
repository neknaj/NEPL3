use nepl3_core::source::Digest;
use nepl3_doc_html::{ParallelMode, RenderOptions};
use nepl3_tools::doc::{
    export::{self, CssMode, DisplayOptions, MathRenderer, pages::resources::ExecutionLimits},
    source::compiled,
};
use serde_json::{Value, json};
use std::{fs, process::Command};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const LEGACY: &str = r#"article en "Legacy" body cons paragraph cons parallel cons variant en "English" cons variant ja "Japanese" nil nil nil"#;
const TYPED: &str = r#"article en Sentence "Title" body cons paragraph cons parallel cons variant en Sentence sentence cons document anchor english text "English" cons math frac 1 2 nil cons variant ja Sentence sentence cons document anchor japanese text "Japanese" nil nil cons code Math frac 3 4 cons Sentence sentence cons document ref english text "tail" nil nil nil"#;
fn selected(language: &str, fallbacks: &[&str]) -> DisplayOptions {
    DisplayOptions {
        document: RenderOptions {
            parallel: ParallelMode::Single {
                language: language.into(),
                fallbacks: fallbacks.iter().map(|s| (*s).into()).collect(),
            },
        },
        math_renderer: MathRenderer::MathMLOnly,
    }
}
fn hex(d: Digest) -> String {
    d.0.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn standalone_display_rows_wrappers_and_legacy_selection() -> Result<()> {
    let c = compiled()?;
    for source in [LEGACY, TYPED] {
        for css in [CssMode::External, CssMode::Inline] {
            let old = export::generate_with_limits(
                &c,
                source,
                css,
                MathRenderer::MathMLOnly,
                ExecutionLimits::default(),
            )?;
            let new = export::generate_with_display(
                &c,
                source,
                css,
                DisplayOptions {
                    math_renderer: MathRenderer::MathMLOnly,
                    ..Default::default()
                },
                ExecutionLimits::default(),
            )?;
            assert_eq!(old.html, new.html);
            assert_eq!(old.manifest, new.manifest);
            let m: Value = serde_json::from_str(&new.manifest)?;
            assert_eq!(m["options"]["parallel"], "Rows");
            assert!(m.get("display_scope").is_none());
        }
    }
    for mode in [
        ParallelMode::Columns,
        ParallelMode::Single {
            language: "ja".into(),
            fallbacks: vec![],
        },
    ] {
        let single = matches!(mode, ParallelMode::Single { .. });
        let doc = export::generate_with_display(
            &c,
            LEGACY,
            CssMode::Inline,
            DisplayOptions {
                document: RenderOptions { parallel: mode },
                math_renderer: MathRenderer::MathMLOnly,
            },
            ExecutionLimits::default(),
        )?;
        assert!(doc.html.contains("Japanese"));
        assert_eq!(doc.html.contains("English"), !single);
        let m: Value = serde_json::from_str(&doc.manifest)?;
        assert_eq!(
            m["options"]["parallel"],
            if single { "Single" } else { "Columns" }
        );
        assert!(m["display_scope"]["identity"].is_string());
    }
    for source in [LEGACY, r#"article en "Plain" body nil"#] {
        assert!(
            export::generate_with_display(
                &c,
                source,
                CssMode::Inline,
                selected("", &[]),
                ExecutionLimits::default()
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn standalone_display_keeps_all_owners_and_original_selected_placements() -> Result<()> {
    let c = compiled()?;
    for (language, fallback, target, expected) in [
        ("en", vec![], "english", vec![0, 1, 3]),
        ("fr", vec!["en"], "english", vec![0, 1, 3]),
        ("ja", vec![], "japanese", vec![0, 2, 3]),
    ] {
        let source = TYPED.replace("ref english", &format!("ref {target}"));
        let doc = export::generate_with_display(
            &c,
            &source,
            CssMode::Inline,
            selected(language, &fallback),
            ExecutionLimits::default(),
        )?;
        let parts = doc
            .sentence_sources
            .as_ref()
            .ok_or("missing retained sources")?;
        assert_eq!(parts.sentences.len(), 4);
        assert_eq!(parts.owners.len(), 4);
        assert_eq!(
            doc.sentences
                .iter()
                .map(|s| s.occurrence.0)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            doc.sentences
                .iter()
                .map(|s| s.placement)
                .collect::<Vec<_>>(),
            vec![0, 1, 3]
        );
        for record in &doc.sentences {
            assert_eq!(record.owner, parts.sentences[record.occurrence.0].owner);
            for foreign in &record.foreign {
                if let nepl3_tools::doc::annotations::ForeignRecord::Document(fragment) = foreign {
                    for origin in &fragment.origins {
                        let node = fragment
                            .document
                            .value
                            .nodes
                            .get(origin.node as usize)
                            .ok_or("origin node")?;
                        if let nepl3_doc_core::model::DocKind::Text { text } = &node.kind {
                            assert!(doc.rendered.markup.fragment.nodes.get(origin.element as usize).is_some_and(|node| matches!(node, nepl3_markup::html::HtmlNode::Text { text: actual } if actual == text)));
                        }
                    }
                }
            }
        }
        assert_eq!(doc.html.contains("English"), target == "english");
        assert_eq!(doc.html.contains("Japanese"), target == "japanese");
    }
    let error = export::generate_with_display(
        &c,
        TYPED,
        CssMode::Inline,
        selected("ja", &[]),
        ExecutionLimits::default(),
    )
    .err()
    .ok_or("hidden target accepted")?;
    assert!(error.contains("MissingFragment"), "{error}");
    Ok(())
}

#[test]
fn standalone_display_scope_binds_exact_requests_and_uses_output_budget() -> Result<()> {
    let c = compiled()?;
    let mut scopes = Vec::new();
    let mut html = None;
    for (language, fallbacks) in [
        ("fr", vec!["en", "ja"]),
        ("FR", vec!["en", "ja"]),
        ("fr", vec!["en", "de"]),
    ] {
        let doc = export::generate_with_display(
            &c,
            LEGACY,
            CssMode::Inline,
            selected(language, &fallbacks),
            ExecutionLimits::default(),
        )?;
        if let Some(old) = &html {
            assert_eq!(old, &doc.html);
        } else {
            html = Some(doc.html.clone());
        }
        let m: Value = serde_json::from_str(&doc.manifest)?;
        assert_eq!(m["options"]["language"], language);
        assert_eq!(m["options"]["fallbacks"], json!(fallbacks));
        let base = format!(
            "{}:{}:{}",
            m["source_sha256"].as_str().ok_or("source")?,
            m["profile_sha256"].as_str().ok_or("profile")?,
            m["options"]["math_renderer"].as_str().ok_or("renderer")?
        );
        let mut bytes = b"nepl3.local-doc-export.display-scope/1\0".to_vec();
        bytes.extend_from_slice(&Digest::of(base.as_bytes()).0);
        bytes.push(2);
        bytes.extend_from_slice(&(language.len() as u64).to_le_bytes());
        bytes.extend_from_slice(language.as_bytes());
        bytes.extend_from_slice(&(fallbacks.len() as u64).to_le_bytes());
        for fallback in &fallbacks {
            bytes.extend_from_slice(&(fallback.len() as u64).to_le_bytes());
            bytes.extend_from_slice(fallback.as_bytes());
        }
        let scope = hex(Digest::of(&bytes));
        assert_eq!(m["display_scope"]["identity"], scope);
        assert!(!scopes.contains(&scope));
        scopes.push(scope);
        let work = m["operations"]["prepare_render_serialize"]["work"]
            .as_u64()
            .ok_or("work")?;
        let mut exact = ExecutionLimits::default();
        exact.output_limits.work = work;
        let equal = export::generate_with_display(
            &c,
            LEGACY,
            CssMode::Inline,
            selected(language, &fallbacks),
            exact,
        )?;
        assert_eq!(equal.html, doc.html);
        exact.output_limits.work = work.checked_sub(1).ok_or("empty work")?;
        let error = export::generate_with_display(
            &c,
            LEGACY,
            CssMode::Inline,
            selected(language, &fallbacks),
            exact,
        )
        .err()
        .ok_or("short budget accepted")?;
        assert!(error.contains("WorkLimit"), "{error}");
    }
    Ok(())
}

#[test]
fn standalone_display_cli_options_limits_and_no_publication() -> Result<()> {
    let root =
        std::env::temp_dir().join(format!("nepl3-standalone-display-{}", std::process::id()));
    fs::create_dir(&root)?;
    let source = root.join("input.nepld");
    fs::write(&source, LEGACY)?;
    let config = root.join("display.json");
    fs::write(
        &config,
        r#"{"mode":"single","language":"fr","fallbacks":["ja","en"]}"#,
    )?;
    let limits = root.join("limits.json");
    fs::write(&limits, serde_json::to_vec(&ExecutionLimits::default())?)?;
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    for (index, (css, renderer, order)) in [
        ("inline", "mathml-only", 0),
        ("external", "mathml-only", 1),
        ("inline", "katex-preferred", 2),
    ]
    .into_iter()
    .enumerate()
    {
        let output = root.join(format!("good-{index}"));
        let mut cmd = Command::new(binary);
        cmd.args(["doc-html", "export"]);
        if order == 1 {
            cmd.arg("--limits").arg(&limits);
        }
        cmd.arg("--display").arg(&config);
        if order == 2 {
            cmd.arg("--limits").arg(&limits);
        }
        if order == 2 {
            cmd.args(["--math-renderer", renderer, "--css", css]);
        } else {
            cmd.args(["--css", css, "--math-renderer", renderer]);
        }
        let run = cmd.arg(&source).arg(&output).output()?;
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let text = fs::read_to_string(output.join("document.html"))?;
        assert!(text.contains("Japanese"));
        assert!(!text.contains("English"));
        let m: Value = serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
        assert_eq!(
            m["options"],
            json!({"parallel":"Single","language":"fr","fallbacks":["ja","en"],"css":css,"math_renderer":renderer})
        );
    }
    for (index, text) in [
        r#"{"mode":"single"}"#,
        r#"{"mode":"single","language":""}"#,
        r#"{"mode":"rows","language":"en"}"#,
        r#"{"mode":"rows","mode":"columns"}"#,
        r#"{"mode":"rows","\u006dode":"columns"}"#,
        r#"{"mode":"single","language":"en","fallbacks":["ja","JA"]}"#,
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(&config, text)?;
        let output = root.join(format!("invalid-{index}"));
        let run = Command::new(binary)
            .args(["doc-html", "export", "--display"])
            .arg(&config)
            .arg(&source)
            .arg(&output)
            .output()?;
        assert!(!run.status.success(), "{text}");
        assert!(!output.exists());
    }
    fs::write(&config, " ".repeat(65_537))?;
    let output = root.join("oversized");
    let run = Command::new(binary)
        .args(["doc-html", "export", "--display"])
        .arg(&config)
        .arg(&source)
        .arg(&output)
        .output()?;
    assert!(!run.status.success());
    assert!(!output.exists());
    fs::write(&config, r#"{"mode":"single","language":"ja"}"#)?;
    for stage in ["parse", "lower", "output"] {
        let mut cap = ExecutionLimits::default();
        match stage {
            "parse" => cap.parse_limits.work = 0,
            "lower" => cap.lower_limits.work = 0,
            _ => cap.output_limits.work = 0,
        };
        fs::write(&limits, serde_json::to_vec(&cap)?)?;
        let output = root.join(stage);
        let run = Command::new(binary)
            .args(["doc-html", "export", "--display"])
            .arg(&config)
            .arg("--limits")
            .arg(&limits)
            .arg(&source)
            .arg(&output)
            .output()?;
        assert!(!run.status.success());
        assert!(!output.exists());
    }
    let existing = root.join("good-0");
    let before = fs::read(existing.join("manifest.json"))?;
    let run = Command::new(binary)
        .args(["doc-html", "export", "--display"])
        .arg(&config)
        .arg(&source)
        .arg(&existing)
        .output()?;
    assert!(!run.status.success());
    assert_eq!(before, fs::read(existing.join("manifest.json"))?);
    fs::write(root.join("--display"), LEGACY)?;
    let repeated_flag = Command::new(binary)
        .current_dir(&root)
        .args(["doc-html", "export", "--display"])
        .arg(&config)
        .args(["--display", "duplicate-output"])
        .output()?;
    assert!(!repeated_flag.status.success());
    assert!(!root.join("duplicate-output").exists());
    let missing = Command::new(binary)
        .args(["doc-html", "export", "--display"])
        .arg(&config)
        .output()?;
    assert!(!missing.status.success());
    let help = Command::new(binary).arg("--help").output()?;
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout)?.contains("--display <display.json>"));
    fs::remove_dir_all(root)?;
    Ok(())
}
