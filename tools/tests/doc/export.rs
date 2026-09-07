use nepl3_tools::doc::{export, source::compiled};

#[test]
fn export_preserves_content_and_binds_script_free_files() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article ja "[文書/ぶんしょ]" body cons paragraph cons "{[本文/ほんぶん]/body} & <tag>" nil nil"#;
    let first = export::generate(&compiled, source)?;
    let second = export::generate(&compiled, source)?;
    assert_eq!(first.html, second.html);
    assert_eq!(first.manifest, second.manifest);
    assert!(first.html.contains("&amp; &lt;tag&gt;"));
    assert!(first.html.contains("class=\"nepl-ruby\""));
    assert!(first.html.contains("class=\"nepl-anno\""));
    assert!(!first.html.contains("<script"));
    assert!(first.html.contains("default-src 'none'"));
    let manifest: serde_json::Value = serde_json::from_str(&first.manifest).map_err(super::err)?;
    for (entry, bytes) in manifest["files"]
        .as_array()
        .ok_or("files")?
        .iter()
        .zip([first.html.as_bytes(), export::CSS.as_bytes()])
    {
        let expected = nepl3_core::source::Digest::of(bytes)
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(entry["sha256"].as_str(), Some(expected.as_str()));
    }
    assert_eq!(manifest["viewer_scripts"], false);
    assert_eq!(
        manifest["profile_sha256"].as_str().ok_or("profile")?.len(),
        64
    );
    assert!(export::generate(&compiled, &(source.to_owned() + " unexpected")).is_err());
    assert!(export::generate(&compiled, "article ja \"unclosed").is_err());
    assert!(
        export::generate(
            &compiled,
            &" ".repeat(export::MAX_SOURCE_BYTES as usize + 1)
        )
        .is_err_and(|e| e == "SourceLimit")
    );
    Ok(())
}

#[test]
fn export_reserves_depth_for_the_document_shell() -> Result<(), String> {
    let compiled = compiled()?;
    for count in [250, 251, 252] {
        let source = format!(
            "article ja sentence cons {}text \"x\" nil body nil",
            "strong ".repeat(count)
        );
        let result = export::generate(&compiled, &source);
        if count == 250 {
            result?;
        } else {
            assert!(result.is_err_and(|e| e.starts_with("OutputDepth")));
        }
    }
    Ok(())
}

#[test]
fn page_export_shares_script_free_shell_and_verifies_every_file() -> Result<(), String> {
    use nepl3_tools::doc::export::pages::{self, Entry};
    let compiled = compiled()?;
    let inputs = vec![
        (Entry { id: "intro".into(), source: "intro.nepld".into(), route: "docs/intro/index.html".into() },
            r#"article en "Intro" body cons paragraph cons sentence cons link page "guide" none text "Guide" nil nil nil"#.into()),
        (Entry { id: "guide".into(), source: "guide.nepld".into(), route: "docs/guide/index.html".into() },
            r#"article en "Guide" body cons paragraph cons sentence cons link page "intro" none text "Back" nil nil nil"#.into()),
    ];
    let first = pages::generate(&compiled, &inputs)?;
    let second = pages::generate(&compiled, &inputs)?;
    assert_eq!(first.files, second.files);
    assert_eq!(first.manifest, second.manifest);
    assert_eq!(first.files.len(), 4);
    let intro = std::str::from_utf8(&first.files["docs/intro/index.html"]).map_err(super::err)?;
    assert!(intro.contains("href=\"../guide/index.html\""));
    assert!(intro.contains("default-src 'none'"));
    assert!(intro.contains("href=\"assets/doc.css\""));
    assert!(!intro.contains("<script"));
    let manifest: serde_json::Value = serde_json::from_str(&first.manifest).map_err(super::err)?;
    for record in manifest["files"].as_array().ok_or("files")? {
        let path = record["path"].as_str().ok_or("path")?;
        let expected = nepl3_core::source::Digest::of(&first.files[path])
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(record["sha256"].as_str(), Some(expected.as_str()));
    }
    let mut inputs = inputs;
    for route in [
        "CON.html",
        "com1.html",
        "dir./page.html",
        "DOCS/guide.html",
        "docs/intro/INDEX.html",
    ] {
        inputs[1].0.route = route.into();
        assert!(pages::generate(&compiled, &inputs).is_err(), "{route}");
    }
    inputs[1].0.route = "docs/intro/assets/doc.css/nested.html".into();
    assert!(pages::generate(&compiled, &inputs).is_err_and(|e| e.contains("collision")));
    inputs[1].0.route = "../escape.html".into();
    assert!(pages::generate(&compiled, &inputs).is_err());
    inputs.pop();
    assert!(pages::generate(&compiled, &inputs).is_err_and(|e| e.contains("MissingPage")));
    assert!(pages::generate(&compiled, &[]).is_err());
    Ok(())
}
