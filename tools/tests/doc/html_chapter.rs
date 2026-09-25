use super::*;
use nepl3_doc_core::model::{DocKind, DocRoot};

#[test]
fn html_chapter_preserves_structure_codes_and_external_links() -> Result<(), String> {
    let mut render_budget = budget();
    let c = compiled()?;
    let mut lower_budget = budget();
    let page = page_with_lower_budget(
        &c,
        "doc-html",
        "doc/spec/20-doc-html.nepld",
        "doc/spec/20-doc-html.md",
        include_str!("../../../doc/spec/20-doc-html.nepld"),
        &mut lower_budget,
    )?;
    eprintln!("HTML chapter lower usage={:?}", lower_budget.usage());
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
        return Err("Body".into());
    };
    let mut introduction = Vec::new();
    let mut sections = Vec::new();
    let mut paragraphs = Vec::new();
    for block in blocks {
        match node(block.0)? {
            DocKind::Paragraph { items } if sections.is_empty() => introduction.push(items.len()),
            DocKind::Section { id, body, .. } => {
                sections.push(id.as_str());
                let DocKind::Body { blocks } = node(body.0)? else {
                    return Err("Section Body".into());
                };
                let mut counts = Vec::new();
                for block in blocks {
                    let DocKind::Paragraph { items } = node(block.0)? else {
                        return Err("Paragraph".into());
                    };
                    counts.push(items.len());
                }
                paragraphs.push(counts);
            }
            _ => return Err("chapter block".into()),
        }
    }
    // Original prose boundaries: nine introductory paragraphs and two sections.
    assert_eq!(introduction, [8, 6, 6, 6, 8, 7, 4, 8, 2]);
    assert_eq!(sections, ["local_export", "baseline"]);
    // The host paragraph also records the composed export registration.
    assert_eq!(paragraphs, [vec![5, 6, 6], vec![6, 5, 5, 4]]);
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
            "HTML chapter projection: {e:?}; {:?}",
            render_budget.usage()
        )
    })?;
    eprintln!("HTML chapter projection usage={:?}", render_budget.usage());
    let markdown = &artifact.pages[0].markdown;
    assert_eq!(
        links(markdown),
        [
            "https://drafts.csswg.org/css-inline/#baseline-source",
            "https://www.w3.org/TR/css-grid-1/#grid-baselines",
            "https://www.w3.org/TR/CSS2/visudet.html#leading",
            "https://www.w3.org/TR/CSS2/tables.html#height-layout",
        ]
    );
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("DocのHTML<ruby>変換<rt>へんかん</rt></ruby>"));
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
            "nepl3-doc-html",
            "interfaces/doc-html.json",
            "LocalHtmlRequest",
            "prepare_local",
            "RenderedFragment",
            "RenderedFragment",
            "NEPL3.Doc.Html.Fragment.v1",
            "nepl3-tools doc-html export <input.nepld> <new-directory>",
            "document.html",
            "assets/doc.css",
            "manifest.json",
            "baseline-source:last",
            "baseline-source:first",
            "baseline-source:first/last",
        ]
    );
    Ok(())
}
