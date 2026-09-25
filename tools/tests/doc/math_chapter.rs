use super::*;
use nepl3_doc_core::model::{DocKind, DocRoot, ListKind};

#[test]
fn math_chapter_preserves_rules_annotation_and_link() -> Result<(), String> {
    project(budget())
}

#[test]
#[ignore = "explicit Math chapter projection measurement under corpus limits"]
fn measure_math_chapter_under_corpus_limits() -> Result<(), String> {
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
        "math",
        "doc/spec/06-math.nepld",
        "doc/spec/06-math.md",
        include_str!("../../../doc/spec/06-math.nepld"),
        &mut lower_budget,
    )?;
    eprintln!("Math chapter lower usage={:?}", lower_budget.usage());
    let set = PageSet {
        pages: vec![page],
        files: vec![PageFile {
            registration: PageRegistration {
                id: "math-html".into(),
                source: "doc/spec/17-math-html.md".into(),
                route: "doc/spec/17-math-html.md".into(),
            },
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
                    assert_eq!(id, "evaluate");
                    assert_eq!(position, 5);
                    assert_eq!(*kind, ListKind::Unordered);
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
    // These boundaries and the thirteen evaluation rules come from the authored
    // chapter. They are independent of the printer's serialization choices.
    assert_eq!(
        sections,
        [
            "policy",
            "expression",
            "structure",
            "evaluate",
            "mathml",
            "arena",
            "resources",
            "bindings",
            "freeinputs",
            "public"
        ]
    );
    assert_eq!(
        paragraphs,
        [
            vec![4],
            vec![2, 5, 7, 5, 4, 5],
            vec![4, 3, 2],
            vec![4, 10, 8, 4, 1, 3],
            vec![2, 5, 5, 5, 4],
            vec![6, 3, 5, 5, 4, 4],
            vec![4, 2],
            vec![3, 3, 4],
            vec![3, 4],
            vec![4, 4, 3, 6, 5, 9, 6, 3]
        ]
    );
    assert_eq!(lists, [vec![1, 1, 2, 1, 4, 2, 3, 2, 1, 2, 3, 6, 2]]);
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
            "Math chapter projection: {e:?}; {:?}",
            render_budget.usage()
        )
    })?;
    eprintln!("Math chapter projection usage={:?}", render_budget.usage());
    let markdown = &artifact.pages[0].markdown;
    assert_eq!(links(markdown), ["17-math-html.md"]);
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("Math<ruby>言語<rt>げんご</rt></ruby>"));
    assert!(markdown.contains(r"<ruby>自由記号<rt>じゆうきごう</rt></ruby>\{free symbol\}"));
    let mut codes = Vec::new();
    let mut items = Vec::new();
    let mut current = None;
    let mut list_count = 0;
    for event in Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_TABLES) {
        match event {
            Event::Code(text) => codes.push(text.into_string()),
            Event::Start(Tag::Table(_)) => return Err("unexpected table".into()),
            Event::Start(Tag::List(start)) => {
                assert!(start.is_none());
                list_count += 1;
            }
            Event::Start(Tag::Item) => {
                assert!(current.is_none());
                current = Some(String::new());
            }
            Event::Text(text) if current.is_some() => {
                current.as_mut().ok_or("item")?.push_str(&text)
            }
            Event::End(pulldown_cmark::TagEnd::Item) => {
                items.push(current.take().ok_or("item end")?)
            }
            _ => {}
        }
    }
    assert_eq!(list_count, 1);
    let prefixes = [
        "add/sub:",
        "neg:",
        "mul:",
        "frac:",
        "pow:",
        "sqrt:",
        "root:",
        "equal:",
        "lt/le:",
        "transpose:",
        "det:",
        "sum:",
        "fence/label:",
    ];
    assert_eq!(items.len(), prefixes.len());
    for (item, prefix) in items.iter().zip(prefixes) {
        assert!(item.starts_with(prefix), "expected {prefix}, got {item}");
    }
    assert_eq!(
        codes,
        [
            "expression_from_rational",
            "frac 1 2",
            "symbol \"...\"",
            "call",
            "Vector(List<Q>)",
            "Matrix(rows,cols,List<Q>)",
            "interfaces/model.json",
            "interfaces/math.json",
            "MathSyntax",
            "MathValue",
            "Option<Span>",
            "lower::expression"
        ]
    );
    Ok(())
}
