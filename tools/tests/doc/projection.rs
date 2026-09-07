use super::*;
use nepl3_tools::doc::projection::from_source;

#[test]
fn raw_code_projection_preserves_bytes_and_distinct_blocks() -> Result<(), String> {
    use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};
    let compiled = compiled()?;
    let source = r##"article en "T" body
      cons rawcode some "Rust" "\tlet x = \"<script>\";\n```\n````\n  日本語  \n\n"
      cons rawcode none ""
      cons paragraph cons "after" nil nil"##;
    let output = from_source(&compiled, source)?;
    let mut blocks = Vec::new();
    let mut active = false;
    for event in Parser::new(&output) {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(hint))) => {
                blocks.push((hint.into_string(), String::new()));
                active = true;
            }
            Event::Text(text) if active => blocks
                .last_mut()
                .ok_or("code text without block")?
                .1
                .push_str(&text),
            Event::End(pulldown_cmark::TagEnd::CodeBlock) => active = false,
            Event::Html(_) | Event::InlineHtml(_) => return Err("code escaped its fence".into()),
            _ => {}
        }
    }
    assert_eq!(
        blocks,
        vec![
            (
                "Rust".into(),
                "\tlet x = \"<script>\";\n```\n````\n  日本語  \n\n".into()
            ),
            (String::new(), String::new())
        ]
    );
    Ok(())
}

#[test]
fn raw_code_projection_rejects_normalized_bytes_and_ambiguous_hints() -> Result<(), String> {
    let compiled = compiled()?;
    for raw in [
        r#"none "no final newline""#,
        r#"none "CRLF\r\n""#,
        r#"some "" "x\n""#,
        r#"some "two words" "x\n""#,
        r#"some "bad`hint" "x\n""#,
        r#"some "<tag>" "x\n""#,
    ] {
        let source = format!("article en \"T\" body cons rawcode {raw} nil");
        assert!(
            from_source(&compiled, &source).is_err_and(|e| e.starts_with("Text")),
            "{source}"
        );
    }
    Ok(())
}

#[test]
fn explicit_breaks_preserve_markdown_paragraph_and_list_structure() -> Result<(), String> {
    use pulldown_cmark::{Event, Parser, Tag};
    let compiled = compiled()?;
    let source = r##"article en "Title" body
      cons paragraph cons sentence cons text "first" cons break cons text "# second" nil nil
      cons list unordered
        cons item none body cons paragraph cons sentence cons code "one" cons break cons text "- two" nil nil nil
        cons item none body cons paragraph cons "third" nil nil
      nil nil"##;
    let output = from_source(&compiled, source)?;
    let mut observed = Vec::new();
    let mut lists = 0;
    let mut items = 0;
    let mut headings = 0;
    let mut paragraphs = 0;
    for event in Parser::new(&output) {
        match event {
            Event::Text(t) => observed.push(format!("text:{t}")),
            Event::Code(t) => observed.push(format!("code:{t}")),
            Event::HardBreak => observed.push("break".into()),
            Event::SoftBreak => return Err("unexpected soft break".into()),
            Event::Start(Tag::List(_)) => lists += 1,
            Event::Start(Tag::Item) => items += 1,
            Event::Start(Tag::Heading { .. }) => headings += 1,
            Event::Start(Tag::Paragraph) => paragraphs += 1,
            _ => {}
        }
    }
    // Independently specified semantic sequence, not generated output golden.
    assert_eq!(
        observed,
        [
            "text:Title",
            "text:first",
            "break",
            "text:# second",
            "code:one",
            "break",
            "text:- two",
            "text:third"
        ]
    );
    assert_eq!((lists, items, headings, paragraphs), (1, 2, 1, 1));
    Ok(())
}

#[test]
fn markdown_breaks_refuse_lossy_edges_and_heading_line_splitting() -> Result<(), String> {
    let compiled = compiled()?;
    for inlines in [
        "cons break cons text \"x\"",
        "cons text \"x\" cons break",
        "cons text \"x\" cons break cons break cons text \"y\"",
        "cons text \"x \" cons break cons text \"y\"",
        "cons text \"x\" cons break cons text \" y\"",
    ] {
        let source =
            format!("article en \"T\" body cons paragraph cons sentence {inlines} nil nil nil");
        assert!(from_source(&compiled, &source).is_err(), "{source}");
    }
    for source in [
        r#"article en sentence cons text "x" cons break cons text "y" nil body nil"#,
        r#"article en "T" body cons section s sentence cons text "x" cons break cons text "y" nil body nil nil"#,
    ] {
        assert!(from_source(&compiled, source).is_err(), "{source}");
    }
    Ok(())
}

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
        r#"article en "Title" body cons paragraph cons "Body" nil cons rawcode some "sh" "echo example\n```\n" nil"#,
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
