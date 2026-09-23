//! Doc placements retain independently owned Sentence content in Markdown.
use nepl3_tools::doc::{projection, source::compiled};

#[test]
fn literal_sentences_preserve_order_and_author_spacing() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body cons paragraph
        cons sentence "First. " cons sentence "Second." nil nil"#;
    let markdown = projection::from_source(&compiled, source)?;
    assert_eq!(markdown, "# Title\n\nFirst\\. Second\\.\n\n");
    Ok(())
}

#[test]
fn sentence_ruby_annotation_and_external_link_keep_their_owners() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article ja sentence "Title" body cons paragraph
        cons sentence sentence
          cons ruby text "漢字" text "かんじ"
          cons anno text "語" cons text "説明" nil
          cons link "https://example.com/" text "Link"
          nil nil nil"#;
    let output = projection::annotated::host::from_source(&compiled, source, &[])?.markdown;
    assert_eq!(
        output,
        "# Title\n\n<ruby>漢字<rt>かんじ</rt></ruby>語\\{説明\\}[Link](<https\\:\\/\\/example\\.com\\/>)\n\n"
    );
    let links: Vec<_> = pulldown_cmark::Parser::new(&output)
        .filter_map(|event| match event {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, .. }) => {
                Some(dest_url.into_string())
            }
            _ => None,
        })
        .collect();
    assert_eq!(links, ["https://example.com/"]);
    Ok(())
}

#[test]
fn text_failure_identifies_the_second_sentence_arena() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body cons paragraph
        cons sentence "good" cons sentence "bad\ttext" nil nil"#;
    // Both Sentence arenas have a Text node 0. The failing one is Doc embed 2
    // (title is 0 and the first paragraph sentence is 1).
    let expected = "Sentence { embed: EmbedRef(2), node: 0, issue: Text }";
    let Err(plain) = projection::from_source(&compiled, source) else {
        return Err("control character accepted by plain projection".into());
    };
    let Err(annotated) = projection::annotated::host::from_source(&compiled, source, &[]) else {
        return Err("control character accepted by annotated projection".into());
    };
    assert_eq!(plain, expected);
    assert_eq!(annotated, expected);
    Ok(())
}

#[test]
fn projection_preserves_stop_reason_after_document_inspection() -> Result<(), String> {
    use nepl3_core::{
        budget::{Budget, StopReason},
        source::{SourceAdmission, SourceStore},
    };
    use nepl3_doc_core::{check::Category, lower, prepare};
    use nepl3_tools::doc::source::{budget, err, with_input_route};
    use nepl3_wire::foundation::FoundationCodec;
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body cons paragraph cons sentence "body" nil nil"#;
    with_input_route(true, &compiled, source, "Article", |tree, profile, _, _| {
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
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let mut measure = budget();
        let plan = prepare::inspect(&document, profile.registry(), &mut codec, &mut measure)
            .map_err(err)?;
        let mut limits = budget().limits();
        // check_pending charges one Work per requirement. With precisely that
        // allowance, the subsequent Sentence preparation shape check must stop.
        limits.work = measure.usage().work + plan.requirements.len() as u64;
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let actual = projection::annotated::render(
            &document,
            profile.registry(),
            &mut codec,
            &mut Budget::new(limits),
            &[],
        );
        assert!(
            matches!(
                actual,
                Err(projection::Error::Stopped(StopReason::WorkLimit))
            ),
            "{actual:?}"
        );
        Ok(())
    })
}

#[test]
fn typed_signature_sources_parse_and_project_with_sentence_owned_content() -> Result<(), String> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    let compiled = compiled()?;
    for (source, title) in [
        (
            include_str!("../../doc/migration/generated/doc-signatures.nepld"),
            "Doc：",
        ),
        (
            include_str!("../../doc/migration/generated/math-signatures.nepld"),
            "Math：",
        ),
        (
            include_str!("../../doc/migration/generated/grammar-signatures.nepld"),
            "Grammar：",
        ),
        (
            include_str!("../../doc/migration/generated/circuit-signatures.nepld"),
            "Circuit：",
        ),
    ] {
        let artifact = projection::annotated::host::from_source(&compiled, source, &[])?;
        let mut heading = false;
        let mut heading_text = String::new();
        let mut tables = 0;
        for event in Parser::new_ext(&artifact.markdown, Options::ENABLE_TABLES) {
            match event {
                Event::Start(Tag::Heading {
                    level: pulldown_cmark::HeadingLevel::H1,
                    ..
                }) => heading = true,
                Event::End(pulldown_cmark::TagEnd::Heading(pulldown_cmark::HeadingLevel::H1)) => {
                    heading = false
                }
                Event::Text(text) if heading => heading_text.push_str(&text),
                Event::Start(Tag::Table(_)) => tables += 1,
                _ => {}
            }
        }
        assert!(heading_text.starts_with(title), "{heading_text}");
        assert!(tables > 0, "{title}: no form tables");
    }
    Ok(())
}
