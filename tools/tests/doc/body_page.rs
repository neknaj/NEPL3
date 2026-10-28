use super::*;
use nepl3_tools::doc::export::body::{self, page};
#[test]
fn explicit_body_page_metadata_does_not_change_body_digest_or_variants() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"body cons paragraph cons parallel cons variant en "English" cons variant ja "日本語" nil nil nil"#;
    let raw = body::from_source(&c, source, Default::default())?;
    for language in ["en", "ja"] {
        let metadata = page::Metadata {
            title: "Explicit title".into(),
            language: language.into(),
        };
        let result = page::from_source(&c, source, &metadata, Default::default())?;
        assert_eq!(result.document_digest, raw.document_digest);
        assert!(result.html.contains(&format!("<body>{}</body>", raw.html)));
        assert!(result.html.contains("English") && result.html.contains("日本語"));
        assert!(!result.html.contains("<h1"));
    }
    Ok(())
}

#[test]
fn body_page_reserves_two_ancestor_levels_beyond_valid_raw_fragment() -> Result<(), String> {
    let c = compiled()?;
    let metadata = page::Metadata {
        title: "Depth".into(),
        language: "en".into(),
    };
    for count in [249, 250] {
        let source = format!(
            "body cons paragraph cons sentence cons {}text \"x\" nil nil nil",
            "strong ".repeat(count)
        );
        body::from_source(&c, &source, Default::default())?;
        let result = page::from_source(&c, &source, &metadata, Default::default());
        if count == 249 {
            result?;
        } else {
            assert!(result.is_err_and(|e| e.starts_with("OutputDepth")));
        }
    }
    Ok(())
}
