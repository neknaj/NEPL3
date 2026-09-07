use super::*;
use nepl3_tools::doc::projection::from_source;

#[test]
fn markdown_projection_preserves_literal_punctuation_and_code_delimiters() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r##"article en "Title" body cons paragraph cons sentence cons text "[x] &amp; *y* " cons code "`a`" cons text " / " cons code " both " nil nil nil"##;
    let output = from_source(&compiled, source)?;
    let mut texts = String::new();
    let mut codes = Vec::new();
    for event in pulldown_cmark::Parser::new(&output) {
        match event {
            pulldown_cmark::Event::Text(t) => texts.push_str(&t),
            pulldown_cmark::Event::Code(t) => codes.push(t.into_string()),
            _ => {}
        }
    }
    assert_eq!(texts, "Title[x] &amp; *y*  / ");
    assert_eq!(codes, ["`a`", " both "]);
    Ok(())
}

#[test]
fn actual_contract_projects_to_equivalent_markdown_events() -> Result<(), String> {
    let compiled = compiled()?;
    let output = from_source(
        &compiled,
        include_str!("../../../doc/migration/00-contract.nepld"),
    )?;
    fn events(input: &str) -> Vec<String> {
        use pulldown_cmark::Event;
        let mut result = Vec::new();
        let mut text = String::new();
        for event in pulldown_cmark::Parser::new(input) {
            match event {
                Event::Text(value) => text.push_str(&value),
                Event::SoftBreak => text.push(' '),
                other => {
                    if !text.is_empty() {
                        result.push(format!("Text({text})"));
                        text.clear();
                    }
                    result.push(format!("{other:?}"));
                }
            }
        }
        result
    }
    assert_eq!(
        events(&output),
        events(include_str!("../../../doc/spec/00-contract.md"))
    );
    assert_eq!(
        output,
        from_source(
            &compiled,
            include_str!("../../../doc/migration/00-contract.nepld")
        )?
    );
    Ok(())
}

#[test]
fn projection_refuses_lossy_or_unsupported_doc_shapes() -> Result<(), String> {
    let compiled = compiled()?;
    for body in [
        r#"cons paragraph cons "" nil nil"#,
        r#"cons paragraph cons " leading" nil nil"#,
        r#"cons paragraph cons "trailing " nil nil"#,
        r#"cons paragraph cons "[base/reading]" nil nil"#,
        r#"cons paragraph cons sentence cons break nil nil nil"#,
        r#"cons paragraph cons "first" cons "second" nil nil"#,
        r#"cons paragraph cons sentence cons text "x\ny" nil nil nil"#,
        r#"cons paragraph cons sentence cons code "a" cons code "b" nil nil nil"#,
        r#"cons section s "Section" body cons paragraph cons "inside" nil nil cons paragraph cons "outside" nil nil"#,
        r#"cons section s "Section" body cons section nested "Nested" body nil nil nil"#,
        r#"cons list unordered cons item none body cons paragraph cons "x" nil nil nil cons list unordered cons item none body cons paragraph cons "y" nil nil nil nil"#,
    ] {
        let result = from_source(&compiled, &format!("article en \"Title\" body {body}"));
        assert!(
            result.is_err_and(|e| e.starts_with("Unsupported") || e.starts_with("Text")),
            "{body}"
        );
    }
    Ok(())
}

#[test]
fn projection_stops_without_returning_partial_markdown() -> Result<(), String> {
    use nepl3_doc_core::{check::Category, lower};
    use nepl3_tools::doc::projection::{self, Error};
    let compiled = compiled()?;
    with_input(
        &compiled,
        r#"article en "Title" body cons paragraph cons "Body" nil nil"#,
        "Article",
        |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let doc = lower::document(
                &checked,
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let original = doc.clone();
            // A budget sufficient only for the preparation digest must not also
            // authorize uncharged Markdown output. Measure the separate prerequisite,
            // then leave exactly that allowance to the combined operation.
            let mut preparation_budget = budget();
            nepl3_doc_core::prepare::inspect(
                &doc,
                profile.registry(),
                &mut codec,
                &mut preparation_budget,
            )
            .map_err(err)?;
            let mut limits = budget().limits();
            limits.output_bytes = preparation_budget.usage().output_bytes;
            let mut output_limited = Budget::new(limits);
            assert!(matches!(
                projection::markdown(&doc, profile.registry(), &mut codec, &mut output_limited),
                Err(Error::Stopped(StopReason::OutputLimit))
            ));
            for allocation in [false, true] {
                for cap in [0, 1, 100, 1000] {
                    let mut limits = budget().limits();
                    if allocation {
                        limits.allocation_units = cap;
                    } else {
                        limits.work = cap;
                    }
                    let mut bounded = Budget::new(limits);
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &store, &mut admission)
                            .map_err(err)?;
                    let result =
                        projection::markdown(&doc, profile.registry(), &mut codec, &mut bounded);
                    assert!(matches!(result, Err(Error::Stopped(_))));
                    assert!(matches!(
                        projection::markdown(&doc, profile.registry(), &mut codec, &mut bounded),
                        Err(Error::Stopped(_))
                    ));
                }
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                projection::markdown(&doc, profile.registry(), &mut codec, &mut cancelled),
                Err(Error::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(doc, original);
            Ok(())
        },
    )
}
