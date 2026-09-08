use super::*;
use nepl3_tools::doc::projection::annotated::{Alias, host::from_source};
use pulldown_cmark::{Event, Parser, Tag};

#[test]
fn annotated_view_preserves_typed_notes_code_and_nested_sections() -> Result<(), String> {
    let source = r##"article ja "[文/ぶん]"
    body cons paragraph cons "導入。" nil
      cons section outer "[外/そと]" body
        cons paragraph
          cons sentence cons strong concat cons ruby text "漢" text "かん" cons text "字" nil
            cons text " " cons anno text "base" cons code "a/b" cons text "n[2]&amp;" nil
            cons text " " cons link external "https://example.com/?q=&amp;" text "link" nil
          cons " Next." nil
        cons section inner "Inner" body cons paragraph cons "本文。" nil nil nil
      cons section last "Last" body cons paragraph cons "終わり。" nil nil nil"##;
    let aliases = [
        Alias {
            section: None,
            name: "old-title".into(),
        },
        Alias {
            section: Some("inner".into()),
            name: "旧見出し".into(),
        },
    ];
    let result = from_source(&compiled()?, source, &aliases)?;
    assert!(result.markdown.contains("<a name=\"旧見出し\"></a>"));
    assert!(result.markdown.contains("<a name=\"n-696e6e6572\"></a>"));
    let mut text = String::new();
    let mut codes = Vec::new();
    let mut headings = Vec::new();
    let mut links = Vec::new();
    let mut strong = 0;
    for event in Parser::new(&result.markdown) {
        match event {
            Event::Text(t) => text.push_str(&t),
            Event::Code(t) => codes.push(t.into_string()),
            Event::Start(Tag::Heading { level, .. }) => headings.push(level as u8),
            Event::Start(Tag::Link { dest_url, .. }) => links.push(dest_url.into_string()),
            Event::InlineHtml(t) if t.as_ref() == "<strong>" => strong += 1,
            _ => {}
        }
    }
    assert_eq!(
        text,
        "文[ぶん]導入。外[そと]漢[かん]字 base{/n[2]&amp;} link Next.Inner本文。Last終わり。"
    );
    assert_eq!(codes, ["a/b"]);
    assert_eq!(headings, [1, 2, 3, 2]);
    assert_eq!(links, ["https://example.com/?q=&amp;"]);
    assert_eq!(strong, 1);
    assert_eq!(
        result.markdown,
        from_source(&compiled()?, source, &aliases)?.markdown
    );
    Ok(())
}

#[test]
fn annotated_view_rejects_hidden_boundaries_and_unresolved_content() -> Result<(), String> {
    for inline in [
        r#"concat cons text " leading" nil"#,
        r#"concat cons text "trailing " nil"#,
        r#"concat cons code "a" cons concat nil cons code "b" nil"#,
        r#"concat cons code "a" cons strong code "b" nil"#,
        r#"concat cons em code "a" cons strong code "b" nil"#,
        r#"concat cons code "a" cons link external "https://example.com/" code "b" nil"#,
        r#"concat cons link external "https://example.com/" code "a" cons code "b" nil"#,
        r#"concat cons text "a " cons break cons text "b" nil"#,
        r#"concat cons break cons text "b" nil"#,
        r#"link external "javascript:alert(1)" text "bad""#,
        r#"link external "https://example.com/" concat cons text "a" cons break cons text "b" nil"#,
        r#"link relative "other.md" none text "missing""#,
    ] {
        let source =
            format!("article ja \"T\" body cons paragraph cons sentence cons {inline} nil nil nil");
        assert!(from_source(&compiled()?, &source, &[]).is_err(), "{source}");
    }
    let reparenting = r#"article en "T" body cons section a "A" body cons section b "B" body nil cons paragraph cons "outside B" nil nil nil"#;
    assert!(from_source(&compiled()?, reparenting, &[]).is_err());
    let source = r#"article en "T" body cons section a "A" body nil nil"#;
    for aliases in [
        vec![Alias {
            section: Some("missing".into()),
            name: "old".into(),
        }],
        vec![Alias {
            section: None,
            name: "bad\"anchor".into(),
        }],
        vec![
            Alias {
                section: None,
                name: "same".into(),
            },
            Alias {
                section: Some("a".into()),
                name: "same".into(),
            },
        ],
        vec![Alias {
            section: None,
            name: "n-61".into(),
        }],
    ] {
        assert!(from_source(&compiled()?, source, &aliases).is_err());
    }
    Ok(())
}

#[test]
fn authored_chapter_thirteen_generates_annotations_without_rewriting() -> Result<(), String> {
    let source = include_str!("../../../doc/spec/13-reproducibility.nepld");
    let aliases = [Alias {
        section: None,
        name: "13-再現性schema識別契約の判定".into(),
    }];
    let output = from_source(&compiled()?, source, &aliases)?.markdown;
    let mut headings = 0;
    let mut links = Vec::new();
    for event in Parser::new(&output) {
        match event {
            Event::Start(Tag::Heading { .. }) => headings += 1,
            Event::Start(Tag::Link { dest_url, .. }) => links.push(dest_url.into_string()),
            _ => {}
        }
    }
    assert_eq!(headings, 8);
    assert_eq!(
        links,
        [
            "https://www.rfc-editor.org/rfc/rfc3986.html#section-3.1",
            "https://www.rfc-editor.org/rfc/rfc3987.html"
        ]
    );
    assert!(output.contains("再現性\\[さいげんせい\\]"));
    Ok(())
}

#[test]
fn annotated_projection_keeps_sticky_limits_and_heading_depth() -> Result<(), String> {
    use nepl3_doc_core::{check::Category, lower};
    use nepl3_tools::doc::projection::{Error, annotated::render};
    let compiled = compiled()?;
    for count in [5, 6] {
        let mut body = "body cons paragraph cons \"last\" nil nil".to_string();
        for index in (0..count).rev() {
            body = format!("body cons section s{index} \"S{index}\" {body} nil");
        }
        let result = from_source(&compiled, &format!("article en \"T\" {body}"), &[]);
        assert_eq!(result.is_ok(), count == 5);
    }
    with_input(
        &compiled,
        r#"article ja "[題/だい]" body cons paragraph cons "{[本文/ほんぶん]/note}" nil nil"#,
        "Article",
        |tree, profile, _, _| {
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
            let mut preparation = budget();
            nepl3_doc_core::prepare::inspect(
                &document,
                profile.registry(),
                &mut codec,
                &mut preparation,
            )
            .map_err(err)?;
            let mut limits = budget().limits();
            limits.output_bytes = preparation.usage().output_bytes;
            let mut bounded = Budget::new(limits);
            assert!(matches!(
                render(&document, profile.registry(), &mut codec, &mut bounded, &[]),
                Err(Error::Stopped(StopReason::OutputLimit))
            ));
            assert!(matches!(
                render(&document, profile.registry(), &mut codec, &mut bounded, &[]),
                Err(Error::Stopped(StopReason::OutputLimit))
            ));
            for resource in ["work", "nodes", "allocation", "depth"] {
                let mut limits = budget().limits();
                match resource {
                    "work" => limits.work = 0,
                    "nodes" => limits.nodes = 0,
                    "allocation" => limits.allocation_units = 0,
                    "depth" => limits.depth = 0,
                    _ => return Err("invalid test resource".into()),
                }
                let mut bounded = Budget::new(limits);
                assert!(matches!(
                    render(&document, profile.registry(), &mut codec, &mut bounded, &[]),
                    Err(Error::Stopped(_))
                ));
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
                Err(Error::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(document, original);
            Ok(())
        },
    )
}

#[cfg(not(target_os = "wasi"))]
#[test]
fn annotated_host_rejects_invalid_aliases_before_creating_output()
-> Result<(), Box<dyn std::error::Error>> {
    use nepl3_tools::doc::projection::annotated::host::write;
    use std::fs;
    let root = std::env::temp_dir().join(format!("nepl3-annotated-host-{}", std::process::id()));
    // Own this newly created directory; never remove an existing caller path.
    fs::create_dir(&root)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let input = root.join("source--metadata.nepld");
        let aliases = root.join("aliases.json");
        let output = root.join("output.md");
        fs::write(
            &input,
            r#"article en "Title" body cons paragraph cons "Body." nil nil"#,
        )?;
        for invalid in [
            r#"[{"name":"a","name":"b"}]"#,
            r#"[{"name":"a","extra":true}]"#,
            r#"[{"section":"missing","name":"a"}]"#,
            r#"[{"name":"bad<a"}]"#,
            r#"[{"name":"same"},{"name":"same"}]"#,
            r#"[{"name":null}]"#,
        ] {
            fs::write(&aliases, invalid)?;
            assert!(write(&input, &aliases, &output).is_err());
            assert!(!output.exists());
        }
        fs::write(&aliases, vec![b' '; 1_048_577])?;
        assert!(write(&input, &aliases, &output).is_err());
        assert!(!output.exists());
        fs::write(&aliases, r#"[{"name":"old-title"}]"#)?;
        write(&input, &aliases, &output)?;
        let original = fs::read(&output)?;
        let text = std::str::from_utf8(&original)?;
        assert!(text.contains("source&#45;&#45;metadata.nepld"));
        assert_eq!(text.matches("<!--").count(), 1);
        assert_eq!(text.matches("-->").count(), 1);
        assert!(text.contains("renderer nepl3-tools.markdown-annotated/1"));
        assert!(text.contains("<a name=\"old-title\"></a>"));
        assert!(text.ends_with('\n'));
        assert!(!text.ends_with("\n\n"));
        assert!(write(&input, &aliases, &output).is_err());
        assert_eq!(fs::read(&output)?, original);
        let invalid_source_output = root.join("failed.md");
        fs::write(&input, "not an article")?;
        assert!(write(&input, &aliases, &invalid_source_output).is_err());
        assert!(!invalid_source_output.exists());
        Ok(())
    })();
    fs::remove_dir_all(&root)?;
    result
}
