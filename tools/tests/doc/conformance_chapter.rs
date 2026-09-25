use super::*;
use nepl3_doc_core::model::{DocKind, DocRoot, ListKind};

#[test]
fn conformance_chapter_preserves_groups_sentences_and_link() -> Result<(), String> {
    project(budget())
}

#[test]
#[ignore = "explicit conformance chapter projection measurement under corpus limits"]
fn measure_conformance_chapter_under_corpus_limits() -> Result<(), String> {
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
        "conformance",
        "doc/spec/11-conformance.nepld",
        "doc/spec/11-conformance.md",
        include_str!("../../../doc/spec/11-conformance.nepld"),
        &mut lower_budget,
    )?;
    eprintln!("Conformance chapter lower usage={:?}", lower_budget.usage());
    let set = PageSet {
        pages: vec![page],
        files: vec![PageFile {
            registration: PageRegistration {
                id: "math-html".into(),
                source: "doc/spec/17-math-html.md".into(),
                route: "doc/spec/17-math-html.md".into(),
            },
            // Only the explicit route is needed; the referenced page is passive.
            content: FileBytes(include_bytes!("../../../doc/spec/17-math-html.md").to_vec()),
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
    let mut lists = Vec::new();
    let mut list_positions = Vec::new();
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
                DocKind::List { kind, items } => {
                    assert_eq!(id, "required");
                    assert_eq!(*kind, ListKind::Unordered);
                    list_positions.push(position);
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
                    lists.push(sentences);
                }
                _ => return Err("chapter block".into()),
            }
        }
        paragraphs.push(counts);
    }
    // The authored chapter has 57 acceptance groups plus nine S06 failure cases.
    // Preserve the thirteen separate lists and every item-level Sentence boundary.
    assert_eq!(
        sections,
        [
            "policy",
            "required",
            "properties",
            "ci",
            "stages",
            "evidence"
        ]
    );
    assert_eq!(
        paragraphs,
        [
            vec![3],
            vec![1, 2, 6],
            vec![7, 3],
            vec![1, 3],
            vec![3, 7, 3],
            vec![5, 4, 6, 4, 3]
        ]
    );
    assert_eq!(list_positions, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 14]);
    assert_eq!(
        lists,
        [
            vec![3, 3],
            vec![1; 5],
            vec![3, 1, 2, 1],
            vec![1, 1, 2, 1, 2],
            vec![1, 1, 1, 5],
            vec![1, 2, 1, 1, 2, 2],
            vec![1, 1, 1, 1, 1, 2],
            vec![2, 2, 1],
            vec![1, 1, 2, 2],
            vec![1, 1, 2, 1, 1, 2, 1, 1],
            vec![1, 2, 4, 2, 1, 3],
            vec![1; 9],
            vec![1, 2, 3, 1]
        ]
    );

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
            "Conformance chapter projection: {e:?}; {:?}",
            render_budget.usage()
        )
    })?;
    eprintln!(
        "Conformance chapter projection usage={:?}",
        render_budget.usage()
    );
    let markdown = &artifact.pages[0].markdown;
    assert_eq!(links(markdown), ["17-math-html.md"]);
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>"));
    let mut codes = Vec::new();
    let mut rendered_lists = Vec::new();
    let mut current_list = None;
    let mut current_item = None;
    for event in Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_TABLES) {
        match event {
            Event::Code(text) => codes.push(text.into_string()),
            Event::Start(Tag::Table(_)) => return Err("unexpected table".into()),
            Event::Start(Tag::List(start)) => {
                assert!(start.is_none());
                assert!(current_list.is_none());
                current_list = Some(Vec::new());
            }
            Event::Start(Tag::Item) => {
                assert!(current_item.is_none());
                current_item = Some(String::new());
            }
            Event::Text(text) if current_item.is_some() => {
                current_item.as_mut().ok_or("item")?.push_str(&text)
            }
            Event::End(pulldown_cmark::TagEnd::Item) => current_list
                .as_mut()
                .ok_or("list")?
                .push(current_item.take().ok_or("item end")?),
            Event::End(pulldown_cmark::TagEnd::List(_)) => {
                rendered_lists.push(current_list.take().ok_or("list end")?)
            }
            _ => {}
        }
    }
    let prefixes: &[&[&str]] = &[
        &["X01:", "X02:"],
        &["G01:", "G02:", "G03:", "G04:", "G05:"],
        &["P01:", "P02:", "P03:", "P04:"],
        &["D01:", "D02:", "D03:", "D04:", "D05:"],
        &["M01:", "M02:", "M03:", "M04:"],
        &["C01:", "C02:", "C03:", "C04:", "C05:", "C06:"],
        &["E01:", "E02:", "E03:", "E04:", "E05:", "E06:"],
        &["W01:", "W02:", "W03:"],
        &["A01:", "A02:", "A03:", "A04:"],
        &[
            "U01:", "U02:", "U03:", "U04:", "U05:", "U06:", "U07:", "U08:",
        ],
        &["S01:", "S02:", "S03:", "S04:", "S05:", "S06:"],
        &[
            "(a)", "(b)", "(c)", "(d)", "(e)", "(f)", "(g)", "(h)", "(i)",
        ],
        &["J01:", "J02:", "J03:", "J04:"],
    ];
    assert_eq!(rendered_lists.len(), prefixes.len());
    for (items, expected) in rendered_lists.iter().zip(prefixes) {
        assert_eq!(items.len(), expected.len());
        for (item, prefix) in items.iter().zip(*expected) {
            assert!(item.starts_with(prefix), "expected {prefix}, got {item}");
        }
    }
    assert_eq!(
        codes,
        [
            ">",
            "]]>",
            "design/acceptance.json",
            "cargo fmt --check",
            "design/tasks.json",
            "conformance/results/",
            "task_id",
            "checks",
            "commands",
            "targets",
            "result",
            "excluded_acceptance_portions",
            "interfaces/acceptance-evidence.schema.json",
            "conformance/results/",
            "schema",
            "acceptance_id",
            "design_revision",
            "identity",
            "result",
            "runs",
            "nepl3.repository-inputs/1",
            "kind: command",
            "kind: review",
            "doc/history/design-validation.json"
        ]
    );
    Ok(())
}
