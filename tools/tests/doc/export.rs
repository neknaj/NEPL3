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
