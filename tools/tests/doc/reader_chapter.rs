use super::*;
use nepl3_doc_core::model::{DocKind, DocRoot, ListKind};

#[test]
fn reader_chapter_preserves_contracts_and_combinators() -> Result<(), String> {
    project(budget().limits(), budget(), budget())
}

#[test]
#[ignore = "explicit Reader chapter stage measurement under corpus limits"]
fn measure_reader_chapter_under_corpus_limits() -> Result<(), String> {
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
    let document = nepl3_tools::doc::source::with_named_validated_input_limits(
        true,
        &c,
        include_str!("../../../doc/spec/03-reader.nepld"),
        "reader",
        "Article",
        parse_limits,
        |tree, profile, parse_budget, _| {
            eprintln!("Reader chapter parse usage={:?}", parse_budget.usage());
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            lower::document(
                &tree.syntax(),
                &c.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut lower_budget,
                &mut codec,
            )
            .map_err(|error| {
                format!(
                    "Reader chapter lower: {error:?}; {:?}",
                    lower_budget.usage()
                )
            })
        },
    )
    .map_err(err)?;
    eprintln!("Reader chapter lower usage={:?}", lower_budget.usage());
    let set = PageSet {
        pages: vec![PageDocument {
            registration: PageRegistration {
                id: "reader".into(),
                source: "doc/spec/03-reader.nepld".into(),
                route: "doc/spec/03-reader.md".into(),
            },
            document,
        }],
        files: vec![],
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
                DocKind::List { kind, items } => {
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
                    lists.push((id.as_str(), position, sentences));
                }
                _ => return Err("chapter block".into()),
            }
        }
        pending.extend(children.into_iter().rev());
        paragraphs.push(counts);
    }
    // Authored boundaries: twenty distinct combinators and a synchronous-host
    // child section within the continuation contract. No printer-derived golden.
    assert_eq!(
        sections,
        [
            (None, "policy"),
            (None, "request"),
            (None, "combinators"),
            (None, "lexical"),
            (None, "modes"),
            (None, "sentences"),
            (None, "complex_tokens"),
            (None, "properties"),
            (None, "continuation"),
            (Some("continuation"), "synchronous"),
        ]
    );
    assert_eq!(
        paragraphs,
        [
            vec![3],
            vec![6, 8, 4],
            vec![1, 3, 7],
            vec![3, 6, 8, 8, 4, 8, 5],
            vec![7, 4],
            vec![5, 5],
            vec![3, 3],
            vec![4, 3],
            vec![
                4, 5, 8, 5, 4, 5, 3, 4, 5, 6, 7, 4, 5, 7, 5, 2, 6, 5, 5, 5, 4, 6, 4
            ],
            vec![8, 6, 4, 6, 4, 4, 6, 5, 8, 3, 5],
        ]
    );
    assert_eq!(
        lists,
        [(
            "combinators",
            1,
            vec![2, 1, 2, 5, 3, 2, 2, 3, 2, 2, 2, 1, 2, 1, 2, 3, 2, 1, 1, 2]
        )]
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
    .map_err(|error| {
        format!(
            "Reader chapter projection: {error:?}; {:?}",
            render_budget.usage()
        )
    })?;
    eprintln!(
        "Reader chapter projection usage={:?}",
        render_budget.usage()
    );
    let markdown = &artifact.pages[0].markdown;
    assert_eq!(
        links(markdown),
        [
            "https://github.com/dtolnay/unicode-ident/blob/1.0.18/src/tables.rs",
            "https://www.rfc-editor.org/rfc/rfc5646.html#section-2.2.9",
        ]
    );
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("<ruby>役割<rt>やくわり</rt></ruby>"));
    let mut codes = Vec::new();
    let mut list_sizes = Vec::new();
    let mut active_list = None;
    for event in Parser::new(markdown) {
        match event {
            Event::Code(text) => codes.push(text.into_string()),
            Event::Start(Tag::List(start)) => {
                assert!(start.is_none());
                assert!(active_list.is_none());
                active_list = Some(0);
            }
            Event::Start(Tag::Item) => *active_list.as_mut().ok_or("list")? += 1,
            Event::End(pulldown_cmark::TagEnd::List(_)) => {
                list_sizes.push(active_list.take().ok_or("list end")?)
            }
            _ => {}
        }
    }
    assert_eq!(list_sizes, [20]);
    assert_eq!(
        codes,
        [
            "ReadRequest = {snapshot, start, limit, finalInput, context, state, limits}",
            "ReadReply = Matched(value, end, newState, view, facts, diagnostics) | NoMatch(expected, furthest) | NeedMore(expected) | Failed(diagnostic, recovery) | Stopped(reason) | Await(externalRequest, continuation)",
            "start < end <= limit",
            "Await",
            "seq [literal quote, commit bodyAndClose]",
            "lexical",
            "unicode-ident = 1.0.18",
            "Name",
            "Nat",
            "0",
            "[1-9][0-9]*",
            "Number",
            "-",
            ".",
            "Text",
            "Lang",
            "\\\\",
            "\\\"",
            "\\n",
            "\\r",
            "\\t",
            "\\u{1〜6 hex}",
            "Option<SourceReservation>",
            "#",
            "letx",
            "let",
            "x",
            "\\n",
            "[",
            "\\u{5B}",
            "[",
            "<",
            ">",
            ">",
            "interfaces/reader.json",
            "nepl3.reader",
            "reader --write",
            "List<NdfValue>",
            "read_accepted_with_host",
            "read_with_accepted_recover",
            "read_accepted_with_host_recover",
        ]
    );
    Ok(())
}
