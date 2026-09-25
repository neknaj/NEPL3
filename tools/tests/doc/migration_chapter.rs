use super::*;
use nepl3_doc_core::model::{DocKind, DocRoot, ListKind};

#[test]
fn migration_chapter_preserves_nested_sections_and_procedures() -> Result<(), String> {
    project(budget().limits(), budget(), budget())
}

#[test]
#[ignore = "explicit migration chapter stage measurement under corpus limits"]
fn measure_migration_chapter_under_corpus_limits() -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Policy {
        output_limits: nepl3_tools::doc::export::pages::resources::OutputLimits,
    }
    let policy: Policy =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    project(
        policy.output_limits.budget().limits(),
        policy.output_limits.budget(),
        policy.output_limits.budget(),
    )
}

fn project(
    parse_limits: nepl3_core::budget::Limits,
    mut lower_budget: Budget,
    mut render_budget: Budget,
) -> Result<(), String> {
    let c = compiled()?;
    let document = nepl3_tools::doc::source::with_named_input_limits(
        true,
        &c,
        include_str!("../../../doc/spec/16-doc-migration.nepld"),
        "migration",
        "Article",
        parse_limits,
        |tree, profile, parse_budget, _| {
            eprintln!(
                "Migration chapter parse and validation usage={:?}",
                parse_budget.usage()
            );
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            lower::document(
                tree.syntax(),
                &c.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut lower_budget,
                &mut codec,
            )
            .map_err(|error| {
                format!(
                    "Migration chapter lower: {error:?}; {:?}",
                    lower_budget.usage()
                )
            })
        },
    )
    .map_err(err)?;
    let page = PageDocument {
        registration: PageRegistration {
            id: "migration".into(),
            source: "doc/spec/16-doc-migration.nepld".into(),
            route: "doc/spec/16-doc-migration.md".into(),
        },
        document,
    };
    eprintln!("Migration chapter lower usage={:?}", lower_budget.usage());
    let set = PageSet {
        pages: vec![page],
        files: vec![
            PageFile {
                registration: PageRegistration {
                    id: "inventory".into(),
                    source: "doc/doc-inventory.md".into(),
                    route: "doc/doc-inventory.md".into(),
                },
                content: FileBytes(include_bytes!("../../../doc/doc-inventory.md").to_vec()),
            },
            PageFile {
                registration: PageRegistration {
                    id: "authoring".into(),
                    source: "doc/authoring.md".into(),
                    route: "doc/authoring.md".into(),
                },
                content: FileBytes(include_bytes!("../../../doc/authoring.md").to_vec()),
            },
        ],
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
    let mut pending: Vec<_> = blocks.iter().rev().map(|block| (None, *block)).collect();
    let mut sections = Vec::new();
    let mut paragraphs = Vec::new();
    let mut lists = Vec::new();
    let mut tables = Vec::new();
    while let Some((parent, section)) = pending.pop() {
        let DocKind::Section { id, body, .. } = node(section.0)? else {
            return Err("Section".into());
        };
        sections.push((parent, id.as_str()));
        let DocKind::Body { blocks } = node(body.0)? else {
            return Err("Section Body".into());
        };
        let mut counts = Vec::new();
        let mut children = Vec::new();
        for (position, block) in blocks.iter().enumerate() {
            match node(block.0)? {
                DocKind::Paragraph { items } => counts.push(items.len()),
                DocKind::Section { .. } => children.push((Some(id.as_str()), *block)),
                DocKind::Table {
                    columns,
                    header,
                    rows,
                } => {
                    let mut widths = Vec::new();
                    for row in core::iter::once(header.ok_or("header")?).chain(rows.iter().copied())
                    {
                        let DocKind::Row { cells } = node(row.0)? else {
                            return Err("Row".into());
                        };
                        widths.push(cells.len());
                    }
                    tables.push((id.as_str(), position, columns.len(), widths));
                }
                DocKind::List { kind, items } => {
                    assert_eq!(*kind, ListKind::Ordered { start: 1 });
                    let mut sentences = Vec::new();
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
                        sentences.push(items.len());
                    }
                    lists.push((id.as_str(), position, sentences));
                }
                _ => return Err("chapter block".into()),
            }
        }
        pending.extend(children.into_iter().rev());
        paragraphs.push(counts);
    }
    // The authored chapter owns two child sections under page_switch. Its
    // procedures and digest fields are ordered lists with distinct boundaries.
    assert_eq!(
        sections,
        [
            (None, "canonical_source"),
            (None, "expression_audit"),
            (None, "page_switch"),
            (Some("page_switch"), "registry"),
            (Some("page_switch"), "page_projection"),
            (None, "bootstrap"),
            (None, "completion"),
        ]
    );
    assert_eq!(
        paragraphs,
        [
            vec![4, 5, 3, 2],
            vec![3, 4, 4, 4],
            vec![5, 4, 3],
            vec![4, 5, 6, 4],
            vec![24, 5, 7, 3, 4, 5, 11, 8, 4, 4],
            vec![3, 4, 3],
            vec![2]
        ]
    );
    assert_eq!(
        lists,
        [
            ("page_switch", 1, vec![1, 1, 1, 2, 2]),
            ("page_projection", 4, vec![1, 1, 1, 1])
        ]
    );
    assert_eq!(tables, [("expression_audit", 3, 2, vec![2; 6])]);
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
            "Migration chapter projection: {e:?}; {:?}",
            render_budget.usage()
        )
    })?;
    eprintln!(
        "Migration chapter projection usage={:?}",
        render_budget.usage()
    );
    let markdown = &artifact.pages[0].markdown;
    assert_eq!(links(markdown), ["../doc-inventory.md", "../authoring.md"]);
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("<ruby>正式文書<rt>せいしきぶんしょ</rt></ruby>"));
    let mut codes = Vec::new();
    let mut list_sizes = Vec::new();
    let mut active_list = None;
    let mut rows = 0;
    let mut cells = 0;
    let mut widths = Vec::new();
    for event in Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_TABLES) {
        match event {
            Event::Code(text) => codes.push(text.into_string()),
            Event::Start(Tag::List(start)) => {
                assert_eq!(start, Some(1));
                assert!(active_list.is_none());
                active_list = Some(0);
            }
            Event::Start(Tag::Item) => *active_list.as_mut().ok_or("list")? += 1,
            Event::End(pulldown_cmark::TagEnd::List(_)) => {
                list_sizes.push(active_list.take().ok_or("list end")?)
            }
            Event::Start(Tag::Table(alignments)) => widths.push(alignments.len()),
            Event::Start(Tag::TableRow) => rows += 1,
            Event::Start(Tag::TableCell) => cells += 1,
            _ => {}
        }
    }
    assert_eq!(list_sizes, [5, 4]);
    assert_eq!(widths, [2]);
    assert_eq!((rows, cells), (5, 12));
    assert_eq!(
        codes,
        [
            "doc/canonical.json",
            "doc/history/",
            "doc/migration/generated/doc-inventory.json",
            "doc-inventory --check",
            "doc-inventory --check-current",
            "task.spec",
            "doc/canonical.json",
            "nepl3-tools doc-canonical --check",
            "nepl3-tools check",
            "nepl3-tools doc-canonical html <new-directory>",
            "nepl3-tools.markdown-annotated-pages/4",
            "files",
            "id",
            "source",
            "route",
            "doc/",
            ".md",
            "sources/",
            ".md",
            "nepl3-tools.markdown-annotated/4",
            "--check",
            "nepl3.canonical-page-context/1",
            "nepl3.canonical-input-context/1",
            "nepl3-tools doc-canonical markdown <new-directory>",
            "--check",
            "output_limits",
            "html_output_limits",
            "output_limits",
            "1-digest",
            "3-normal-formとartifact"
        ]
    );
    Ok(())
}
