use super::*;
use nepl3_core::schema::SchemaRegistry;
use nepl3_doc_core::{check::Category, model::*, prepare};
use nepl3_tools::doc::projection::{Error, annotated};
use pulldown_cmark::{Event, Parser, Tag};

fn with_body<T>(
    compiled: &Compiled,
    source: &str,
    finish: impl FnOnce(
        &DocumentSyntax,
        &SchemaRegistry,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_input(compiled, source, "Body", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
        let doc = nepl3_suite::adapters::sentence::literal::lower(
            &checked,
            &compiled.doc.package.schema,
            Category::Body,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        finish(&doc, profile.registry(), b, codec.source_admission())
    })
}

fn render(
    compiled: &Compiled,
    source: &str,
) -> Result<Result<annotated::body::BodyArtifact, Error>, String> {
    with_body(compiled, source, |doc, r, b, a| {
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(r, &empty, a).map_err(err)?;
        Ok(annotated::body::render_footnotes(doc, r, &mut codec, b))
    })
}

#[test]
fn body_markdown_preserves_intro_peer_headings_and_forward_references() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"body cons paragraph cons sentence cons text "Intro " cons ref later text "forward" nil nil cons section first "First" body cons section nested "Nested" body nil nil cons section later "Later" body nil nil"#;
    let out = render(&c, source)?.map_err(err)?;
    let headings: Vec<_> = Parser::new(&out.markdown)
        .filter_map(|e| match e {
            Event::Start(Tag::Heading { level, .. }) => Some(level as u8),
            _ => None,
        })
        .collect();
    assert_eq!(headings, [1, 2, 1]);
    assert!(out.markdown.starts_with("Intro "));
    let links: Vec<_> = Parser::new(&out.markdown)
        .filter_map(|e| match e {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect();
    assert_eq!(links, ["#n-6c61746572"]);
    assert_eq!(out.markdown, render(&c, source)?.map_err(err)?.markdown);
    Ok(())
}

#[test]
fn body_markdown_empty_depth_and_order_boundaries() -> Result<(), String> {
    let c = compiled()?;
    assert_eq!(render(&c, "body nil")?.map_err(err)?.markdown, "");
    for depth in [6, 7] {
        let mut source = "body nil".to_owned();
        for n in (0..depth).rev() {
            source = format!("body cons section s{n} \"Heading\" {source} nil");
        }
        let result = render(&c, &source)?;
        assert_eq!(result.is_ok(), depth == 6);
    }
    assert!(
        render(
            &c,
            r#"body cons section s "S" body nil cons paragraph cons "after" nil nil"#
        )?
        .is_err()
    );
    Ok(())
}

#[test]
fn body_markdown_footnotes_and_ruby_preserve_content() -> Result<(), String> {
    let c = compiled()?;
    let out = render(&c, r#"body cons paragraph cons sentence cons ruby text "漢" text "かん" cons text " " cons anno text "base" cons text "note one" cons text "note two" nil nil nil nil"#)?.map_err(err)?;
    assert!(out.markdown.contains("<ruby>漢<rt>かん</rt></ruby>"));
    assert!(out.markdown.contains("base[^nepl3-anno-1]"));
    assert!(
        out.markdown
            .contains("[^nepl3-anno-1]:\n    1. note one\n    2. note two")
    );
    assert!(out.markdown.contains("note one"));
    assert!(out.markdown.contains("note two"));
    assert!(!out.markdown.contains("base{"));
    Ok(())
}

#[test]
fn body_markdown_rejects_requirements_and_bad_labels() -> Result<(), String> {
    let c = compiled()?;
    for inline in [
        r#"link relative "other.nepld" none text "other""#,
        r#"link page "other" none text "other""#,
        r#"link external "javascript:alert(1)" text "bad""#,
    ] {
        let source = format!("body cons paragraph cons sentence cons {inline} nil nil nil");
        assert!(render(&c, &source)?.is_err(), "{source}");
    }
    for source in [
        r#"body cons section a "A" body nil cons section a "B" body nil nil"#,
        r#"body cons paragraph cons sentence cons ref missing text "bad" nil nil nil"#,
    ] {
        assert!(render(&c, source)?.is_err());
    }
    Ok(())
}

#[test]
fn body_markdown_budget_stops_and_legacy_rejection() -> Result<(), String> {
    let c = compiled()?;
    with_body(
        &c,
        r#"body cons paragraph cons "hello" nil nil"#,
        |doc, r, b, a| {
            let before = doc.clone();
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(r, &empty, a).map_err(err)?;
            assert!(prepare::inspect(doc, r, &mut codec, b).is_err());
            assert!(annotated::render_footnotes(doc, r, &mut codec, b, &[]).is_err());
            for reason in [
                StopReason::WorkLimit,
                StopReason::AllocationLimit,
                StopReason::OutputLimit,
            ] {
                let mut limits = budget().limits();
                match reason {
                    StopReason::WorkLimit => limits.work = 0,
                    StopReason::AllocationLimit => limits.allocation_units = 0,
                    StopReason::OutputLimit => limits.output_bytes = 0,
                    _ => unreachable!(),
                }
                let mut limited = Budget::new(limits);
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
                assert!(
                    matches!(annotated::body::render_footnotes(doc,r,&mut codec,&mut limited),Err(Error::Stopped(actual)) if actual==reason)
                );
                assert!(limited.poll().is_err());
            }
            assert_eq!(&before, doc);
            Ok(())
        },
    )
}

#[test]
fn body_markdown_all_variant_dependencies_are_retained() -> Result<(), String> {
    let c = compiled()?;
    assert!(matches!(
        render(
            &c,
            r#"body cons paragraph cons code Doc article en "Guest" body nil nil nil"#
        )?,
        Err(Error::NeedsResolution)
    ));
    for (inline, resolution) in [
        (r#"link relative "other.nepld" none text "other""#, true),
        (r#"link page "other" none text "other""#, true),
        (r#"image asset "logo" none "alt""#, true),
        (r#"math Math frac 1 2"#, true),
        (r#"link external "javascript:alert(1)" text "bad""#, false),
    ] {
        let source = format!(
            "body cons paragraph cons parallel cons variant en \"safe\" cons variant ja sentence cons {inline} nil nil nil nil"
        );
        let result = render(&c, &source)?;
        if resolution {
            assert!(
                matches!(result, Err(Error::NeedsResolution)),
                "{source}: {result:?}"
            );
        } else {
            assert!(
                matches!(result, Err(Error::Text { .. })),
                "{source}: {result:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn body_markdown_cancellation_exact_output_and_wrong_root() -> Result<(), String> {
    let c = compiled()?;
    with_body(
        &c,
        r#"body cons paragraph cons "hello" nil nil"#,
        |doc, r, _, _| {
            let empty = SourceStore::default();
            let run = |b: &mut Budget| {
                let mut a = SourceAdmission::default();
                let mut codec = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
                Ok::<_, String>(annotated::body::render_footnotes(doc, r, &mut codec, b))
            };
            let mut baseline = budget();
            let expected = run(&mut baseline)?.map_err(err)?;
            let mut limits = budget().limits();
            limits.output_bytes = baseline.usage().output_bytes;
            assert_eq!(
                run(&mut Budget::new(limits))?.map_err(err)?.markdown,
                expected.markdown
            );
            limits.output_bytes -= 1;
            assert!(matches!(
                run(&mut Budget::new(limits))?,
                Err(Error::Stopped(StopReason::OutputLimit))
            ));
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                run(&mut cancelled)?,
                Err(Error::Stopped(StopReason::Cancelled))
            ));
            Ok(())
        },
    )?;
    for (category, source, root) in [
        (Category::Article, r#"article en "T" body nil"#, "Article"),
        (
            Category::Sentence,
            r#"sentence cons text "T" nil"#,
            "Sentence",
        ),
        (Category::Inline, r#"text "T""#, "Inline"),
    ] {
        with_input(&c, source, root, |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let doc = nepl3_suite::adapters::sentence::literal::lower(
                &checked,
                &c.doc.package.schema,
                category,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            assert!(matches!(
                prepare::inspect_body(&doc, profile.registry(), &mut codec, b),
                Err(prepare::PreparationError::Label(
                    nepl3_doc_core::labels::LabelError::ExpectedBody
                ))
            ));
            assert!(
                annotated::body::render_footnotes(&doc, profile.registry(), &mut codec, b).is_err()
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn body_preparation_keeps_order_digest_and_section_only_reference_contract() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"body cons paragraph cons sentence cons link external "https://example.com/" text "link" cons image asset "logo" none "alt" cons math Math frac 1 2 nil nil nil"#;
    with_body(&c, source, |doc, r, b, a| {
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(r, &empty, a).map_err(err)?;
        let plan = prepare::inspect_body(doc, r, &mut codec, b).map_err(err)?;
        let again = prepare::inspect_body(doc, r, &mut codec, b).map_err(err)?;
        assert_eq!(plan, again);
        let kinds: Vec<_> = plan
            .requirements
            .iter()
            .map(|item| match item {
                prepare::DocRequirement::Link { .. } => "link",
                prepare::DocRequirement::Asset { .. } => "asset",
                prepare::DocRequirement::Foreign { .. } => "foreign",
            })
            .collect();
        assert_eq!(kinds, ["link", "asset", "foreign"]);
        Ok(())
    })?;
    // Local label resolution is not proof that this profile emits an Anchor.
    assert!(matches!(
        render(
            &c,
            r#"body cons paragraph cons sentence cons ref a text "ref" cons anchor a text "target" nil nil nil"#
        )?,
        Err(Error::Unsupported { .. })
    ));
    Ok(())
}

#[test]
fn body_markdown_unicode_note_reference_and_nested_link_boundaries() -> Result<(), String> {
    let c = compiled()?;
    let out=render(&c,r#"body cons paragraph cons sentence cons anno text "base" cons ref 後 text "later" nil nil nil cons section 後 "Later" body nil nil"#)?.map_err(err)?;
    let options = pulldown_cmark::Options::ENABLE_FOOTNOTES;
    let links: Vec<_> = Parser::new_ext(&out.markdown, options)
        .filter_map(|e| match e {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect();
    assert_eq!(links, ["#n-e5be8c"]);
    assert!(out.markdown.contains("<a name=\"n-e5be8c\"></a>"));
    for inline in [
        r#"ref s ref s text "nested""#,
        r#"link external "https://example.com/" ref s text "nested""#,
    ] {
        let source = format!(
            "body cons paragraph cons sentence cons {inline} nil nil cons section s \"S\" body nil nil"
        );
        assert!(matches!(
            render(&c, &source)?,
            Err(Error::Unsupported { .. })
        ));
    }
    assert!(annotated::host::from_source(&c,r#"article en "T" body cons paragraph cons sentence cons ref s text "ref" nil nil cons section s "S" body nil nil"#,&[]).is_err());
    Ok(())
}
