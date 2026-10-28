use super::*;
use nepl3_tools::doc::{export::body as host, export::pages::resources::ExecutionLimits};
#[test]
fn body_html_source_host_preserves_bytes_and_phase_limits() -> Result<(), String> {
    let c = compiled()?;
    let source = "body nil";
    assert_eq!(
        host::from_source(&c, source, Default::default())?.html,
        "<div class=\"nepl-doc\"></div>"
    );
    let oversized = " ".repeat(nepl3_tools::doc::export::MAX_SOURCE_BYTES as usize + 1);
    assert_eq!(
        host::from_source(&c, &oversized, Default::default())
            .err()
            .ok_or("expected source cap")?,
        "SourceLimit"
    );
    let mut limits = ExecutionLimits::default();
    // Parse admission includes provider/validation boundaries, not only file bytes.
    let admitted = nepl3_tools::doc::source::with_named_input_limits(
        true,
        &c,
        source,
        "doc-input",
        "Body",
        budget().limits(),
        |_, _, b, _| Ok(b.usage().source_bytes),
    )?;
    limits.parse_limits.source_bytes = admitted;
    host::from_source(&c, source, limits)?;
    limits.parse_limits.source_bytes -= 1;
    assert!(
        host::from_source(&c, source, limits)
            .err()
            .ok_or("expected error")?
            .contains("SourceLimit")
    );
    for phase in ["parse", "lower", "output"] {
        let mut limits = ExecutionLimits::default();
        match phase {
            "parse" => limits.parse_limits.work = 0,
            "lower" => limits.lower_limits.work = 0,
            _ => limits.output_limits.work = 0,
        }
        assert!(
            host::from_source(&c, source, limits)
                .err()
                .ok_or("expected error")?
                .contains("WorkLimit")
        );
    }
    assert!(host::from_source(&c, "article en \"T\" body nil", Default::default()).is_err());
    Ok(())
}

#[test]
fn body_html_source_keeps_html_specific_structure_and_local_anchors() -> Result<(), String> {
    let c = compiled()?;
    // Body permits paragraphs after a Section; unlike the Markdown profile,
    // inline anchors are valid destinations and headings deeper than H6 survive.
    let source = r#"body cons paragraph cons sentence cons ref here text "forward" cons anchor here ruby text "漢" text "かん" cons anno text "base" cons text "note" nil nil nil cons section first "First" body nil cons paragraph cons "after" nil cons section later "Later" body nil nil"#;
    let out = host::from_source(&c, source, Default::default())?.html;
    assert!(out.starts_with("<div class=\"nepl-doc\">"));
    assert!(!out.contains("<article"));
    assert!(!out.contains("<html"));
    assert!(!out.ends_with('\n'));
    assert!(out.contains("href=\"#n-68657265\""));
    assert!(out.contains("id=\"n-68657265\""));
    assert!(out.contains("class=\"nepl-ruby\""));
    assert!(out.contains("かん"));
    assert!(out.contains("note"));
    assert_eq!(out.matches("<h1").count(), 2);
    assert!(out.find("First").ok_or("First")? < out.find("after").ok_or("after")?);
    let mut deep = "body nil".to_owned();
    for n in (0..7).rev() {
        deep = format!("body cons section s{n} \"Heading\" {deep} nil");
    }
    let out = host::from_source(&c, &deep, Default::default())?.html;
    assert!(out.contains("aria-level=\"7\""));
    assert!(out.contains("role=\"heading\""));
    let rows = r#"body cons paragraph cons parallel cons variant en "English" cons variant ja "日本語" nil nil nil"#;
    let out = host::from_source(&c, rows, Default::default())?.html;
    assert!(out.contains("English") && out.contains("日本語"));
    assert!(out.contains("nepl-parallel-rows"));
    let escaped = host::from_source(
        &c,
        r#"body cons paragraph cons sentence cons text "<&" nil nil nil"#,
        Default::default(),
    )?
    .html;
    assert!(escaped.contains("&lt;&amp;"));
    assert!(!escaped.contains("<&"));
    Ok(())
}

#[test]
fn body_html_source_refuses_all_requirements_and_invalid_labels() -> Result<(), String> {
    let c = compiled()?;
    for inline in [
        r#"link relative "other.nepld" none text "other""#,
        r#"link page "other" none text "other""#,
        r#"link external "https://example.org" text "external""#,
        r#"image asset "logo" none "alt""#,
        r#"math Math frac 1 2"#,
    ] {
        let source = format!(
            "body cons paragraph cons parallel cons variant en \"safe\" cons variant ja sentence cons {inline} nil nil nil nil"
        );
        let error = host::from_source(&c, &source, Default::default())
            .err()
            .ok_or("requirement accepted")?;
        assert!(error.contains("NeedsResolution"), "{error}");
    }
    for source in [
        r#"body cons section a "A" body nil cons section a "B" body nil nil"#,
        r#"body cons paragraph cons sentence cons ref missing text "bad" nil nil nil"#,
        r#"body cons paragraph cons code Doc article en "Guest" body nil nil nil"#,
    ] {
        assert!(host::from_source(&c, source, Default::default()).is_err());
    }
    Ok(())
}
