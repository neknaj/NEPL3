use super::*;
use nepl3_tools::doc::projection::{
    Error,
    annotated::{host::from_source_footnotes, render_footnotes},
};
use pulldown_cmark::{Event, Options, Parser, Tag};

#[test]
fn footnotes_preserve_ruby_order_nested_notes_and_literal_markers() -> Result<(), String> {
    let source = r#"article ja "[記録/きろく]" body cons paragraph cons sentence
      cons text "用語 " cons anno code "kernel" cons ruby text "核" text "かく"
        cons concat cons text "関連: " cons anno text "image" cons text "像" nil nil nil
      cons text " [^nepl3-anno-1]" nil nil nil"#;
    let compiled = compiled()?;
    let result = from_source_footnotes(&compiled, source, &[])?;
    assert!(result.markdown.contains("`kernel`[^nepl3-anno-1]"));
    assert!(
        result
            .markdown
            .contains("[^nepl3-anno-1]:\n    1. <ruby>核<rt>かく</rt></ruby>")
    );
    assert!(result.markdown.contains("image[^nepl3-anno-2]"));
    let mut references = Vec::new();
    let mut definitions = Vec::new();
    let mut literal = String::new();
    for event in Parser::new_ext(&result.markdown, Options::ENABLE_FOOTNOTES) {
        match event {
            Event::FootnoteReference(id) => references.push(id.into_string()),
            Event::Start(Tag::FootnoteDefinition(id)) => definitions.push(id.into_string()),
            Event::Text(text) => literal.push_str(&text),
            _ => {}
        }
    }
    assert_eq!(references, ["nepl3-anno-1", "nepl3-anno-2"]);
    assert_eq!(definitions, ["nepl3-anno-1", "nepl3-anno-2"]);
    assert!(literal.contains("[^nepl3-anno-1]"));
    assert_eq!(
        result.markdown,
        from_source_footnotes(&compiled, source, &[])?.markdown
    );
    let source = r#"article en "T" body cons paragraph cons sentence cons link external "https://example.com/" anno text "base" cons text "note" nil nil nil nil"#;
    assert!(from_source_footnotes(&compiled, source, &[]).is_err());
    Ok(())
}

#[test]
fn footnotes_keep_cumulative_limits_and_original_nesting_depth() -> Result<(), String> {
    use nepl3_doc_core::{check::Category, lower};
    let source = r#"article en "T" body cons paragraph cons sentence cons anno text "base" cons anno text "middle" cons anno text "inner" cons text "last" nil nil nil nil nil nil"#;
    let compiled = compiled()?;
    with_input(&compiled, source, "Article", |tree, profile, _, _| {
        let store = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &store, &mut a).map_err(err)?;
        let document = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let original = document.clone();
        let mut full = budget();
        full.charge(Resource::Work, 19).map_err(err)?;
        let expected = render_footnotes(&document, profile.registry(), &mut codec, &mut full, &[])
            .map_err(err)?;
        let usage = full.usage();
        for mode in 0..5 {
            let mut limits = full.limits();
            let reason = match mode {
                0 => {
                    limits.work = usage.work - 1;
                    StopReason::WorkLimit
                }
                1 => {
                    limits.allocation_units = usage.allocation_units - 1;
                    StopReason::AllocationLimit
                }
                2 => {
                    limits.output_bytes = usage.output_bytes - 1;
                    StopReason::OutputLimit
                }
                3 => {
                    limits.nodes = usage.nodes - 1;
                    StopReason::NodeLimit
                }
                _ => {
                    limits.depth = usage.depth - 1;
                    StopReason::DepthLimit
                }
            };
            let mut b = Budget::new(limits);
            b.charge(Resource::Work, 19).map_err(err)?;
            assert!(
                matches!(render_footnotes(&document, profile.registry(), &mut codec, &mut b, &[]), Err(Error::Stopped(actual)) if actual == reason)
            );
            assert!(b.poll().is_err());
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            render_footnotes(
                &document,
                profile.registry(),
                &mut codec,
                &mut cancelled,
                &[]
            ),
            Err(Error::Stopped(StopReason::Cancelled))
        ));
        assert_eq!(document, original);
        assert_eq!(expected.markdown.matches("]: ").count(), 3);
        Ok(())
    })
}

#[test]
fn legacy_footnotes_reject_unresolved_math_without_erasing_it() -> Result<(), String> {
    use nepl3_doc_core::{check::Category, lower};
    let compiled = compiled()?;
    // Display syntax is not code evaluation, and the legacy entry point has no
    // Math resolver. None of these positions may silently lose its guest.
    for source in [
        r#"article en "T" body cons paragraph cons sentence cons math Math frac 1 0 nil nil nil"#,
        r#"article en "T" body cons display Math frac 1 0 nil"#,
        r#"article en sentence cons math Math frac 1 0 nil body nil"#,
        r#"article en "T" body cons paragraph cons sentence cons anno text "base" cons math Math frac 1 0 nil nil nil nil"#,
        r#"article en "T" body cons code Math frac 1 0 nil"#,
    ] {
        with_input(&compiled, source, "Article", |tree, profile, _, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let document = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let original = document.clone();
            assert!(matches!(
                render_footnotes(
                    &document,
                    profile.registry(),
                    &mut codec,
                    &mut budget(),
                    &[]
                ),
                Err(Error::NeedsResolution)
            ));
            assert_eq!(document, original);
            Ok(())
        })?;
    }
    Ok(())
}
