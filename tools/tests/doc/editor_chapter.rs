use super::*;
use nepl3_doc_core::model::{DocKind, DocRoot};

#[test]
fn editor_chapter_preserves_sections_sentences_and_codes() -> Result<(), String> {
    project(budget())
}

#[test]
#[ignore = "explicit editor chapter projection measurement under corpus limits"]
fn measure_editor_chapter_under_corpus_limits() -> Result<(), String> {
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
        "editor",
        "doc/spec/08-editor.nepld",
        "doc/spec/08-editor.md",
        include_str!("../../../doc/spec/08-editor.nepld"),
        &mut lower_budget,
    )?;
    eprintln!("editor lower usage={:?}", lower_budget.usage());
    let set = PageSet {
        pages: vec![page],
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
    let mut sections = Vec::new();
    let mut paragraphs = Vec::new();
    for block in blocks {
        let DocKind::Section { id, body, .. } = node(block.0)? else {
            return Err("Section".into());
        };
        sections.push(id.as_str());
        let DocKind::Body { blocks } = node(body.0)? else {
            return Err("Section Body".into());
        };
        let mut sentences = Vec::new();
        for block in blocks {
            let DocKind::Paragraph { items } = node(block.0)? else {
                return Err("Paragraph".into());
            };
            sentences.push(items.len());
        }
        paragraphs.push(sentences);
    }
    // Authored section order and Sentence boundaries, independent of the printer.
    assert_eq!(
        sections,
        [
            "policy",
            "snapshot",
            "derived",
            "region",
            "relations",
            "recovery",
            "incremental",
            "lsp",
            "diagnostics",
            "trust"
        ]
    );
    let expected: &[&[usize]] = &[
        &[3],
        &[2, 3, 7, 4, 7, 7, 4],
        &[4, 2, 3],
        &[3, 6, 7, 6, 5, 6, 10, 7, 3],
        &[
            5, 5, 5, 3, 6, 3, 6, 7, 5, 4, 3, 3, 2, 4, 9, 4, 5, 5, 7, 3, 6, 10, 9, 6,
        ],
        &[3, 4, 4, 3],
        &[4, 4, 4],
        &[2, 4, 3, 4],
        &[4, 2],
        &[5],
    ];
    assert_eq!(
        paragraphs.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        expected
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
    .map_err(|e| format!("editor projection: {e:?}; {:?}", render_budget.usage()))?;
    eprintln!("editor projection usage={:?}", render_budget.usage());
    let markdown = &artifact.pages[0].markdown;
    assert!(links(markdown).is_empty());
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains(
        "<ruby>共通<rt>きょうつう</rt></ruby>editor serviceと<ruby>診断<rt>しんだん</rt></ruby>"
    ));
    let mut codes = Vec::new();
    for event in Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_TABLES) {
        match event {
            Event::Code(text) => codes.push(text.into_string()),
            Event::Start(Tag::Table(_) | Tag::List(_)) => {
                return Err("unexpected table/list".into());
            }
            _ => {}
        }
    }
    assert_eq!(
        codes,
        [
            "SHA-256(domain || canonical NDF/1 CBOR(value))",
            "nepl3.analysis.tree/1",
            "nepl3.analysis.execution/1",
            "nepl3.analysis.request/1",
            "analysis::region::regions",
            "RegionKey",
            "RegionSidecar { facts: Option<List<ReaderFactBatch>> }",
            "SHA-256(\"nepl3.region.reader-facts/1\\0\" || bytes)",
            "analysis::query::query",
            "analysis::region::query::query",
            "RenameRequest(key, source, offset, newName, writable)",
        ]
    );
    Ok(())
}
