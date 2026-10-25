use super::*;
const SOURCE: &str = r#"article ja "T" body cons paragraph
cons "Before."
cons parallel
 cons variant en sentence cons strong text "English" cons text " " cons code "a|b" cons text " " cons link external "https://example.com/" text "link" cons break cons text "Next." nil
 cons variant ja "{[日本語/にほんご]/note}" nil
cons "After."
cons parallel cons variant en "Second." cons variant ja "二つ目。" nil
nil nil"#;
#[test]
fn annotated_parallel_keeps_groups_languages_and_markdown_content() -> Result<(), String> {
    let result = from_source(&compiled()?, SOURCE, &[])?;
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, Parser::new(&result.markdown));
    // Derived from the authored tree: paragraph owns prose/group/prose/group;
    // each group owns two separate language containers in declared order.
    assert_eq!(
        html,
        concat!(
            "<h1>T</h1>\n<div class=\"nepl-paragraph\">\n<p>Before.</p>\n",
            "<div class=\"nepl-parallel\">\n<div lang=\"en\">\n<p><strong>en</strong></p>\n",
            "<p><strong>English</strong> <code>a|b</code> <a href=\"https://example.com/\">link</a><br />\nNext.</p>\n</div>\n",
            "<div lang=\"ja\">\n<p><strong>ja</strong></p>\n<p><ruby>日本語<rt>にほんご</rt></ruby>{note}</p>\n</div>\n</div>\n",
            "<p>After.</p>\n<div class=\"nepl-parallel\">\n<div lang=\"en\">\n<p><strong>en</strong></p>\n<p>Second.</p>\n</div>\n",
            "<div lang=\"ja\">\n<p><strong>ja</strong></p>\n<p>二つ目。</p>\n</div>\n</div>\n</div>\n"
        )
    );
    assert_eq!(html.matches("class=\"nepl-paragraph\"").count(), 1);
    assert_eq!(html.matches("class=\"nepl-parallel\"").count(), 2);
    assert_eq!(html.matches("lang=\"en\"").count(), 2);
    assert_eq!(html.matches("lang=\"ja\"").count(), 2);
    assert!(html.contains("<strong>English</strong> <code>a|b</code> <a href=\"https://example.com/\">link</a><br />\nNext."),"{html}");
    assert!(
        html.contains("<ruby>日本語<rt>にほんご</rt></ruby>{note}"),
        "{html}"
    );
    for pair in [
        ["Before.", "English"],
        ["English", "日本語"],
        ["日本語", "After."],
        ["After.", "Second."],
        ["Second.", "二つ目。"],
    ] {
        assert!(html.find(pair[0]).ok_or("first")? < html.find(pair[1]).ok_or("second")?);
    }
    // The original all-notes single-paragraph behavior remains unchanged.
    assert_eq!(
        from_source(
            &compiled()?,
            r#"article en "T" body cons paragraph cons "One." cons " Two." nil nil"#,
            &[]
        )?
        .markdown,
        "# T\n\nOne\\. Two\\.\n\n"
    );
    Ok(())
}
#[test]
fn annotated_parallel_rejects_invalid_or_lossy_variant_content() -> Result<(), String> {
    let c = compiled()?;
    for source in [
        r#"article en "T" body cons paragraph cons parallel cons variant en "A" cons variant EN "B" nil nil nil"#,
        r#"article en "T" body cons paragraph cons parallel cons variant en "A" cons variant ja sentence cons link external "javascript:bad" text "B" nil nil nil nil"#,
        r#"article en "T" body cons paragraph cons parallel cons variant en "A" cons variant ja " B" nil nil nil"#,
        r#"article en "T" body cons paragraph cons parallel cons variant en "A" cons variant ja sentence cons code "a" cons code "b" nil nil nil nil"#,
    ] {
        assert!(from_source(&c, source, &[]).is_err(), "accepted {source}");
    }
    Ok(())
}

#[test]
fn annotated_parallel_retains_explicit_empty_variant() -> Result<(), String> {
    let result = from_source(
        &compiled()?,
        r#"article en "T" body cons paragraph cons parallel cons variant en "English" cons variant ja "" nil nil nil"#,
        &[],
    )?;
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, Parser::new(&result.markdown));
    assert_eq!(html.matches("lang=\"ja\"").count(), 1);
    assert!(html.contains("<strong>ja</strong>"));
    assert!(html.contains("English"));
    Ok(())
}

#[test]
fn annotated_parallel_footnotes_preserve_variant_boundaries() -> Result<(), String> {
    let output = nepl3_tools::doc::projection::annotated::host::from_source_footnotes(
        &compiled()?,
        SOURCE,
        &[],
    )?
    .markdown;
    let mut html = String::new();
    pulldown_cmark::html::push_html(
        &mut html,
        Parser::new_ext(&output, pulldown_cmark::Options::ENABLE_FOOTNOTES),
    );
    assert_eq!(html.matches("class=\"nepl-parallel\"").count(), 2);
    assert!(html.contains("class=\"footnote-reference\""), "{html}");
    assert!(html.contains("class=\"footnote-definition\""), "{html}");
    assert!(html.contains("note"));
    Ok(())
}
#[test]
fn annotated_parallel_limits_are_cumulative_and_failure_sticky() -> Result<(), String> {
    use nepl3_doc_core::check::Category;
    use nepl3_tools::doc::projection::{self, annotated::render};
    let c = compiled()?;
    let source = r#"article en "T" body cons paragraph cons parallel cons variant en "English" cons variant ja "[日本語/にほんご]" nil nil nil"#;
    with_input(&c, source, "Article", |tree, profile, _, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let document = nepl3_suite::adapters::sentence::literal::lower(
            tree.syntax(),
            &c.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let execute = |limits: Limits| {
            let mut b = Budget::new(limits);
            b.charge(Resource::Work, 19).map_err(err)?;
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let result = render(&document, profile.registry(), &mut codec, &mut b, &[]);
            Ok::<_, String>((result, b))
        };
        let (baseline, full) = execute(budget().limits())?;
        let baseline = baseline.map_err(err)?;
        let usage = full.usage();
        for dim in 0..6 {
            for short in [false, true] {
                let less = u64::from(short);
                let mut limits = full.limits();
                match dim {
                    0 => limits.work = usage.work - less,
                    1 => limits.allocation_units = usage.allocation_units - less,
                    2 => limits.nodes = usage.nodes - less,
                    3 => limits.depth = usage.depth - less,
                    4 => limits.output_bytes = usage.output_bytes - less,
                    _ => limits.source_bytes = usage.source_bytes - less,
                }
                let (result, b) = execute(limits)?;
                if short {
                    assert!(result.is_err(), "dim {dim}");
                    assert!(b.poll().is_err());
                } else {
                    assert_eq!(result.map_err(err)?.markdown, baseline.markdown);
                }
            }
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            render(
                &document,
                profile.registry(),
                &mut codec,
                &mut cancelled,
                &[]
            ),
            Err(projection::Error::Stopped(StopReason::Cancelled))
        ));
        let before = cancelled.usage();
        assert!(
            render(
                &document,
                profile.registry(),
                &mut codec,
                &mut cancelled,
                &[]
            )
            .is_err()
        );
        assert_eq!(before, cancelled.usage());
        assert!(matches!(
            projection::markdown(&document, profile.registry(), &mut codec, &mut budget()),
            Err(projection::Error::Unsupported { .. })
        ));
        Ok(())
    })
}

#[test]
fn annotated_parallel_validates_typed_language_before_attribute_emission() -> Result<(), String> {
    use nepl3_doc_core::{check::Category, model::DocKind};
    use nepl3_tools::doc::projection::annotated::render;
    let c = compiled()?;
    let source = r#"article en "T" body cons paragraph cons parallel cons variant en "English" cons variant ja "日本語" nil nil nil"#;
    with_input(&c, source, "Article", |tree, profile, _, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let original = nepl3_suite::adapters::sentence::literal::lower(
            tree.syntax(),
            &c.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        for tag in ["", "en\" onclick=\"bad", "en\n", "x-"] {
            let mut doc = original.clone();
            let node = doc
                .value
                .nodes
                .iter_mut()
                .find(|node| matches!(node.kind, DocKind::Variant { .. }))
                .ok_or("variant")?;
            if let DocKind::Variant { language, .. } = &mut node.kind {
                *language = tag.into();
            }
            assert!(
                matches!(render(&doc,profile.registry(),&mut codec,&mut budget(),&[]),Err(nepl3_tools::doc::projection::Error::Invalid(message)) if message=="invalid parallel language")
            );
        }
        let mut doc = original;
        let node = doc
            .value
            .nodes
            .iter_mut()
            .find(|node| matches!(node.kind, DocKind::Variant { .. }))
            .ok_or("variant")?;
        if let DocKind::Variant { language, .. } = &mut node.kind {
            *language = "EN-us".into();
        }
        assert!(
            render(&doc, profile.registry(), &mut codec, &mut budget(), &[])
                .map_err(err)?
                .markdown
                .contains("lang=\"EN-us\"")
        );
        Ok(())
    })
}

#[test]
fn annotated_parallel_does_not_expand_flat_list_profile() -> Result<(), String> {
    let c = compiled()?;
    let list = r#"article en "T" body cons list unordered cons item none body cons paragraph cons parallel cons variant en "A" cons variant ja "B" nil nil nil nil nil"#;
    let error = from_source(&c, list, &[])
        .err()
        .ok_or("list Parallel accepted")?;
    assert!(error.contains("Unsupported"), "{error}");
    Ok(())
}
