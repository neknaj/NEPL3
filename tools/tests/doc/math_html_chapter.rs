use super::*;
use nepl3_doc_core::model::{DocKind, DocRoot, ListKind};

#[test]
fn math_html_chapter_preserves_tables_lists_and_links() -> Result<(), String> {
    project(budget())
}

#[test]
#[ignore = "explicit Math HTML chapter projection measurement under corpus limits"]
fn measure_math_html_chapter_under_corpus_limits() -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Policy {
        output_limits: nepl3_tools::doc::export::pages::resources::OutputLimits,
    }
    let policy: Policy =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    project(policy.output_limits.budget())
}

fn project(mut render_budget: Budget) -> Result<(), String> {
    let c = compiled()?;
    let mut lower_budget = budget();
    let page = page_with_lower_budget(
        &c,
        "math-html",
        "doc/spec/17-math-html.nepld",
        "doc/spec/17-math-html.md",
        include_str!("../../../doc/spec/17-math-html.nepld"),
        &mut lower_budget,
    )?;
    eprintln!("Math HTML chapter lower usage={:?}", lower_budget.usage());
    let set = PageSet {
        pages: vec![page],
        // Preserve the authored destination without parsing another chapter.
        files: vec![PageFile {
            registration: PageRegistration {
                id: "html-delivery".into(),
                source: "doc/spec/18-html-delivery.md".into(),
                route: "doc/spec/18-html-delivery.md".into(),
            },
            content: FileBytes(include_bytes!("../../../doc/spec/18-html-delivery.md").to_vec()),
        }],
    };
    let document = &set.pages[0].document.value;
    let node = |id: u64| -> Result<&DocKind, String> {
        document
            .nodes
            .get(usize::try_from(id).map_err(err)?)
            .map(|node| &node.kind)
            .ok_or_else(|| "document node".into())
    };
    let DocRoot::Article(root) = document.root else {
        return Err("Article root".into());
    };
    let DocKind::Article { body, .. } = node(root.0)? else {
        return Err("Article".into());
    };
    let DocKind::Body { blocks } = node(body.0)? else {
        return Err("Article Body".into());
    };
    let mut sections = Vec::new();
    let mut paragraphs = Vec::new();
    let mut tables = Vec::new();
    let mut lists = Vec::new();
    for section in blocks {
        let DocKind::Section { id, body, .. } = node(section.0)? else {
            return Err("Section".into());
        };
        sections.push(id.as_str());
        let DocKind::Body { blocks } = node(body.0)? else {
            return Err("Section Body".into());
        };
        let mut counts = Vec::new();
        for (position, block) in blocks.iter().enumerate() {
            match node(block.0)? {
                DocKind::Paragraph { items } => counts.push(items.len()),
                DocKind::Table {
                    columns,
                    header,
                    rows,
                } => {
                    let mut cells_per_row = Vec::new();
                    for row in
                        core::iter::once(header.ok_or("table header")?).chain(rows.iter().copied())
                    {
                        let DocKind::Row { cells } = node(row.0)? else {
                            return Err("Row".into());
                        };
                        cells_per_row.push(cells.len());
                    }
                    tables.push((id.as_str(), position, columns.len(), cells_per_row));
                }
                DocKind::List { kind, items } => {
                    assert_eq!(*kind, ListKind::Unordered);
                    let mut list_counts = Vec::new();
                    for item in items {
                        let DocKind::ListItem { checked, body } = node(item.0)? else {
                            return Err("ListItem".into());
                        };
                        assert!(checked.is_none());
                        let DocKind::Body { blocks } = node(body.0)? else {
                            return Err("ListItem Body".into());
                        };
                        assert_eq!(blocks.len(), 1);
                        let DocKind::Paragraph { items } = node(blocks[0].0)? else {
                            return Err("ListItem Paragraph".into());
                        };
                        list_counts.push(items.len());
                    }
                    lists.push((id.as_str(), position, list_counts));
                }
                _ => return Err("chapter block".into()),
            }
        }
        paragraphs.push(counts);
    }
    // Expectations follow the original authored sections, paragraph boundaries,
    // table columns/data rows, and the three references with one Sentence each.
    assert_eq!(
        sections,
        [
            "generation",
            "ownership",
            "failure_stop",
            "identity_resources",
            "checked_artifact",
            "acceptance",
            "references"
        ]
    );
    assert_eq!(
        paragraphs,
        [
            vec![4, 4, 4, 3],
            vec![5, 3, 3],
            vec![3, 4, 2],
            vec![3, 5, 5, 4, 5, 3],
            vec![4, 2, 4, 4, 6, 4, 4, 5],
            vec![3, 4],
            vec![1]
        ]
    );
    assert_eq!(
        tables,
        [
            ("generation", 1, 3, vec![3; 4]),
            ("failure_stop", 1, 2, vec![2; 9])
        ]
    );
    assert_eq!(lists, [("references", 0, vec![1, 1, 1])]);

    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let artifact = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut render_budget,
        &[&[]],
    )
    .map_err(|e| {
        format!(
            "Math HTML chapter projection: {e:?}; {:?}",
            render_budget.usage()
        )
    })?;
    eprintln!(
        "Math HTML chapter projection usage={:?}",
        render_budget.usage()
    );
    let markdown = &artifact.pages[0].markdown;
    assert_eq!(
        links(markdown),
        [
            "18-html-delivery.md",
            "https://katex.org/docs/node",
            "https://katex.org/docs/api",
            "https://katex.org/docs/options",
            "https://katex.org/docs/security",
            "https://katex.org/docs/browser"
        ]
    );
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("Doc・MathのHTML<ruby>生成<rt>せいせい</rt></ruby>"));
    let mut codes = Vec::new();
    let mut widths = Vec::new();
    let mut rows = 0;
    let mut cells = 0;
    let mut list_items = Vec::new();
    let mut item_links = None;
    for event in Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_TABLES) {
        match event {
            Event::Code(text) => codes.push(text.into_string()),
            Event::Start(Tag::Table(alignments)) => {
                assert!(
                    alignments
                        .iter()
                        .all(|a| *a == pulldown_cmark::Alignment::None)
                );
                widths.push(alignments.len());
            }
            Event::Start(Tag::TableRow) => rows += 1,
            Event::Start(Tag::TableCell) => cells += 1,
            Event::Start(Tag::List(start)) => assert!(start.is_none()),
            Event::Start(Tag::Item) => {
                assert!(item_links.is_none());
                item_links = Some(Vec::new());
            }
            Event::Start(Tag::Link { dest_url, .. }) if item_links.is_some() => {
                item_links
                    .as_mut()
                    .ok_or("item links")?
                    .push(dest_url.into_string());
            }
            Event::End(pulldown_cmark::TagEnd::Item) => {
                list_items.push(item_links.take().ok_or("item end")?)
            }
            _ => {}
        }
    }
    assert_eq!(widths, [3, 2]);
    assert_eq!(rows, 11);
    assert_eq!(cells, 30);
    assert_eq!(
        list_items,
        [
            vec!["https://katex.org/docs/node"],
            vec![
                "https://katex.org/docs/api",
                "https://katex.org/docs/options"
            ],
            vec![
                "https://katex.org/docs/security",
                "https://katex.org/docs/browser"
            ]
        ]
    );
    assert_eq!(
        codes,
        [
            "KaTeXPreferred",
            "renderToString",
            "render",
            "math-core",
            "nepl3-math-tex",
            "math-mathml",
            "no_std + alloc",
            "KaTeXPreferred",
            "MathMlOnly",
            "--math-renderer katex-preferred|mathml-only",
            "htmlAndMathml",
            "output: html",
            "throwOnError: true",
            "trust: false",
            "maxExpand",
            "maxSize",
            "maxExpand",
            "maxSize",
            "design/markup.json",
            "style-src-attr 'none'",
            "SaveFinished"
        ]
    );
    Ok(())
}
