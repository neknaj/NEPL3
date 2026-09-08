use super::*;
use nepl3_tools::doc::projection::annotated::host::from_source;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[test]
fn annotated_lists_preserve_start_checks_notes_and_break_continuations() -> Result<(), String> {
    let source = r##"article ja "例" body
      cons list ordered 7
        cons item checked body cons paragraph
          cons "[確認/かくにん]。"
          cons sentence cons text "次 " cons strong text "step" cons break
            cons anno ruby text "式" text "しき" cons code "a|b" nil nil
          nil nil
        cons item unchecked body cons paragraph cons "未了。" nil nil
        cons item none body cons paragraph cons sentence cons text "[x] literal" nil nil nil
        nil
      cons paragraph cons "Separator." nil
      cons list unordered cons item none body cons paragraph cons "項目。" nil nil nil
      nil"##;
    let output = from_source(&compiled()?, source, &[])?.markdown;
    let mut starts = Vec::new();
    let mut checks = Vec::new();
    let mut items = 0;
    let mut breaks = 0;
    let mut code = Vec::new();
    let mut visible = String::new();
    let mut list_depth = 0;
    for event in Parser::new_ext(&output, Options::ENABLE_TASKLISTS) {
        match event {
            Event::Start(Tag::List(start)) => {
                starts.push(start);
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => list_depth -= 1,
            Event::Start(Tag::Item) => items += 1,
            Event::TaskListMarker(checked) => checks.push(checked),
            Event::HardBreak => {
                assert_eq!(list_depth, 1, "break must remain within its list item");
                breaks += 1;
            }
            Event::Text(text) => visible.push_str(&text),
            Event::Code(text) => code.push(text.into_string()),
            _ => {}
        }
    }
    assert_eq!(starts, [Some(7), None]);
    assert_eq!(checks, [true, false]);
    assert_eq!(items, 4);
    assert_eq!(breaks, 1);
    assert_eq!(code, ["a|b"]);
    assert_eq!(
        visible,
        "例確認[かくにん]。次 step式[しき]{}未了。[x] literalSeparator.項目。"
    );
    Ok(())
}

#[test]
fn annotated_lists_reject_lossy_shapes_and_marker_overflow() -> Result<(), String> {
    let compiled = compiled()?;
    for body in [
        "cons list unordered nil nil",
        "cons list unordered cons item none body nil nil nil",
        "cons list unordered cons item checked body nil nil nil",
        r#"cons list ordered 1000000000 cons item none body cons paragraph cons "x" nil nil nil nil"#,
        r#"cons list unordered cons item none body cons paragraph cons "" nil nil nil nil"#,
        r#"cons list unordered cons item none body cons paragraph cons "a" nil cons paragraph cons "b" nil nil nil nil"#,
        r#"cons list unordered cons item none body cons list unordered cons item none body cons paragraph cons "x" nil nil nil nil nil nil"#,
        r#"cons list unordered cons item none body cons paragraph cons "x" nil nil nil cons list unordered cons item none body cons paragraph cons "y" nil nil nil nil"#,
    ] {
        let source = format!("article en \"T\" body {body}");
        let error = from_source(&compiled, &source, &[])
            .err()
            .ok_or_else(|| format!("accepted unsupported source: {source}"))?;
        assert!(
            error.starts_with("Unsupported") || error.starts_with("Text"),
            "{source}: {error}"
        );
    }
    for start in [0, 999_999_999] {
        let source = format!(
            "article en \"T\" body cons list ordered {start} cons item none body cons paragraph cons \"a\" nil nil cons item none body cons paragraph cons \"b\" nil nil nil nil"
        );
        let text = from_source(&compiled, &source, &[])?.markdown;
        let starts: Vec<_> = Parser::new(&text)
            .filter_map(|event| match event {
                Event::Start(Tag::List(start)) => Some(start),
                _ => None,
            })
            .collect();
        assert_eq!(starts, [Some(start)]);
    }
    Ok(())
}

#[test]
fn annotated_tables_preserve_cells_alignment_notes_and_literal_pipes() -> Result<(), String> {
    let source = r##"article ja "表" body
      cons table cons default cons left cons center cons right nil
        some row cons "A" cons "B" cons "C" cons "D" nil
        cons row cons "" cons "{[値/あたい]/value}"
          cons sentence cons code "a|b" nil
          cons sentence cons text "x\\|y" nil nil
        cons row cons sentence cons code "a\\|b" nil
          cons sentence cons code "a\\\\|b" nil
          cons sentence cons code "`|`" nil
          cons sentence cons link external "https://example.com/?q=%7C" text "a|b" nil nil
        nil
      cons paragraph cons "After." nil nil"##;
    let output = from_source(&compiled()?, source, &[])?.markdown;
    let mut alignments = Vec::new();
    let mut cells = Vec::new();
    let mut cell = None;
    let mut codes = Vec::new();
    let mut links = Vec::new();
    let mut rows = 0;
    for event in Parser::new_ext(&output, Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::Table(value)) => alignments.push(value),
            Event::Start(Tag::TableRow) => rows += 1,
            Event::Start(Tag::TableCell) => cell = Some(String::new()),
            Event::End(TagEnd::TableCell) => cells.push(cell.take().ok_or("missing cell start")?),
            Event::Text(text) => {
                if let Some(cell) = &mut cell {
                    cell.push_str(&text);
                }
            }
            Event::Code(text) => {
                cell.as_mut()
                    .ok_or("code escaped its cell")?
                    .push_str(&text);
                codes.push(text.into_string());
            }
            Event::Start(Tag::Link { dest_url, .. }) => links.push(dest_url.into_string()),
            Event::HardBreak | Event::SoftBreak => return Err("cell introduced a break".into()),
            _ => {}
        }
    }
    use pulldown_cmark::Alignment as A;
    assert_eq!(alignments, [vec![A::None, A::Left, A::Center, A::Right]]);
    assert_eq!(rows, 2);
    assert_eq!(
        cells,
        [
            "A",
            "B",
            "C",
            "D",
            "",
            "値[あたい]{value}",
            "a|b",
            "x\\|y",
            "a\\|b",
            "a\\\\|b",
            "`|`",
            "a|b"
        ]
    );
    assert_eq!(codes, ["a|b", "a\\|b", "a\\\\|b", "`|`"]);
    assert_eq!(links, ["https://example.com/?q=%7C"]);
    Ok(())
}

#[test]
fn annotated_tables_keep_header_only_and_reject_unrepresentable_shapes() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en "T" body cons table cons default nil some row cons "" nil nil nil"#;
    let output = from_source(&compiled, source, &[])?.markdown;
    assert_eq!(
        Parser::new_ext(&output, Options::ENABLE_TABLES)
            .filter(|e| matches!(e, Event::Start(Tag::TableCell)))
            .count(),
        1
    );
    for body in [
        r#"cons table cons default nil none cons row cons "body" nil nil nil"#,
        "cons table nil some row nil nil nil",
        r#"cons table cons default nil some row cons "H" nil cons row cons sentence cons text "a" cons break cons text "b" nil nil nil nil"#,
        r#"cons table cons default nil some row cons "H" nil cons row cons " " nil nil nil"#,
        r#"cons table cons default nil some row cons "H" nil cons row cons sentence cons code "" nil nil nil nil"#,
        r#"cons section s "S" body nil cons table cons default nil some row cons "H" nil nil nil"#,
        r#"cons section s "S" body nil cons list unordered cons item none body cons paragraph cons "x" nil nil nil nil"#,
    ] {
        let source = format!("article en \"T\" body {body}");
        let error = from_source(&compiled, &source, &[])
            .err()
            .ok_or_else(|| format!("accepted unsupported source: {source}"))?;
        assert!(
            error.starts_with("Unsupported") || error.starts_with("Text"),
            "{source}: {error}"
        );
    }
    Ok(())
}

#[test]
fn annotated_blocks_preserve_consumed_budget_and_sticky_stop() -> Result<(), String> {
    use nepl3_doc_core::{check::Category, lower};
    use nepl3_tools::doc::projection::{Error, annotated::render};
    let source = r#"article en "T" body
      cons list ordered 999999999 cons item checked body cons paragraph cons "[A/a]" nil nil nil
      cons table cons default nil some row cons "H" nil cons row cons sentence cons code "a|b" nil nil nil nil"#;
    let compiled = compiled()?;
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
        let mut full = budget();
        // The caller's usage is part of this operation, not replaced on entry.
        full.charge(Resource::Work, 19).map_err(err)?;
        let artifact =
            render(&document, profile.registry(), &mut codec, &mut full, &[]).map_err(err)?;
        assert!(artifact.markdown.contains("999999999."));
        let usage = full.usage();
        for resource in ["work", "allocation", "output", "nodes"] {
            let mut limits = full.limits();
            match resource {
                "work" => limits.work = usage.work - 1,
                "allocation" => limits.allocation_units = usage.allocation_units - 1,
                "output" => limits.output_bytes = usage.output_bytes - 1,
                "nodes" => limits.nodes = usage.nodes - 1,
                _ => unreachable!(),
            }
            let mut bounded = Budget::new(limits);
            bounded.charge(Resource::Work, 19).map_err(err)?;
            let Err(Error::Stopped(reason)) =
                render(&document, profile.registry(), &mut codec, &mut bounded, &[])
            else {
                return Err(format!(
                    "{resource} exhaustion must not return a partial artifact"
                ));
            };
            let stopped_usage = bounded.usage();
            assert!(
                matches!(render(&document, profile.registry(), &mut codec, &mut bounded, &[]),
                Err(Error::Stopped(again)) if again == reason)
            );
            assert_eq!(bounded.usage(), stopped_usage);
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
    })
}
