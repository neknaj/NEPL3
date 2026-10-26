//! Native semantic regression against the preserved pre-cutover Markdown.
//! Page resolution, rendering and browser acceptance remain separate gates.
use super::*;
use nepl3_doc_core::{check::Category, model::*, text};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

fn normalized(value: &str) -> String {
    value.chars().filter(|c| !c.is_whitespace()).collect()
}

fn markdown_blocks(source: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut active: Option<(String, String)> = None;
    for event in Parser::new_ext(source, Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::Heading { .. }) => active = Some(("heading".into(), String::new())),
            Event::Start(Tag::Paragraph | Tag::Item) => {
                active = Some(("paragraph".into(), String::new()))
            }
            Event::Start(Tag::TableCell) => active = Some(("cell".into(), String::new())),
            Event::Text(value) | Event::Code(value) => {
                if let Some((_, text)) = &mut active {
                    text.push_str(&value);
                }
            }
            Event::End(
                TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::TableCell | TagEnd::Item,
            ) => {
                if let Some((kind, text)) = active.take() {
                    result.push((kind, normalized(&text)));
                }
            }
            _ => {}
        }
    }
    result
}

// Explicit ordered traversal of the checked typed model, not a source parser.
fn blocks(
    document: &DocumentSyntax,
    node: u64,
    project: &mut impl FnMut(SentenceRef) -> Result<String, String>,
    output: &mut Vec<(String, String)>,
) -> Result<(), String> {
    match &document.value.nodes.get(node as usize).ok_or("node")?.kind {
        DocKind::Article { title, body, .. } | DocKind::Section { title, body, .. } => {
            output.push(("heading".into(), normalized(&project(*title)?)));
            blocks(document, body.0, project, output)?;
        }
        DocKind::Body { blocks: children } => {
            for child in children {
                blocks(document, child.0, project, output)?;
            }
        }
        DocKind::Paragraph { items } => {
            let mut value = String::new();
            for item in items {
                value.push_str(&project(SentenceRef(item.0))?);
            }
            output.push(("paragraph".into(), normalized(&value)));
        }
        DocKind::Table { header, rows, .. } => {
            for row in header.iter().chain(rows) {
                blocks(document, row.0, project, output)?;
            }
        }
        DocKind::Row { cells } => {
            for cell in cells {
                output.push(("cell".into(), normalized(&project(*cell)?)));
            }
        }
        DocKind::List { items, .. } => {
            for item in items {
                blocks(document, item.0, project, output)?;
            }
        }
        DocKind::ListItem { body, .. } => blocks(document, body.0, project, output)?,
        other => return Err(format!("unhandled draft block: {other:?}")),
    }
    Ok(())
}

#[test]
fn documentation_index_preserves_original_prose_tables_and_links() -> Result<(), String> {
    let source = include_str!("../../../doc/README.nepld");
    let original = include_str!("../../migration/fixtures/documentation-index.md");
    // Original doc/README.md at merged main
    // 113e87da0449430b4c5422d77593116d91a72f0f, before index cutover.
    // Do not regenerate the expected original from the candidate Doc source.
    use sha2::{Digest, Sha256};
    assert_eq!(
        Sha256::digest(original.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "5120d92cff9839cec6afdd608d284f479759c4ffe67ea9ea2c3e8e0fc4fec3a6"
    );
    let c = compiled()?;
    let registry: nepl3_tools::doc::canonical::Registry =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    // Use the existing native corpus configuration, not portable defaults or
    // a silently enlarged global/test budget. This is native parity evidence.
    nepl3_tools::doc::source::with_named_input_limits(
        true,
        &c,
        source,
        "documentation-index",
        "Article",
        registry.parse_limits.budget().limits(),
        |tree, profile, _, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let document = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &c.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut registry.lower_limits.budget(),
                &mut codec,
            )
            .map_err(err)?;
            let mut b = budget();
            let prepared =
                text::prepare(&document, profile.registry(), &mut codec, &mut b).map_err(err)?;
            let mut project = |sentence| match prepared
                .project(sentence, text::AnnotationPolicy::BaseOnly, &[], &mut b)
                .map_err(err)?
                .outcome
            {
                text::PlainTextOutcome::Complete { text } => Ok(text),
                other => Err(format!("plain-text incomplete: {other:?}")),
            };
            let DocRoot::Article(root) = document.value.root else {
                return Err("article root".into());
            };
            let mut actual = Vec::new();
            blocks(&document, root.0, &mut project, &mut actual)?;
            assert_eq!(actual, markdown_blocks(original));
            let sections: Vec<_> = document
                .value
                .nodes
                .iter()
                .filter_map(|node| match &node.kind {
                    DocKind::Section { id, .. } => Some(id.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(sections, ["purpose", "rationale", "authority"]);
            let tables: Vec<_> = document
                .value
                .nodes
                .iter()
                .filter_map(|node| match &node.kind {
                    DocKind::Table {
                        columns,
                        header,
                        rows,
                    } => Some((columns.len(), header.is_some(), rows.len())),
                    _ => None,
                })
                .collect();
            assert_eq!(tables, [(3, true, 7), (2, true, 15)]);
            let mut expected_links: Vec<_> = Parser::new_ext(original, Options::ENABLE_TABLES)
                .filter_map(|event| match event {
                    Event::Start(Tag::Link { dest_url, .. }) => Some(match dest_url.as_ref() {
                        "spec/13-reproducibility.nepld" => "https://github.com/neknaj/NEPL3/blob/main/doc/spec/13-reproducibility.nepld".into(),
                        "../examples/" => {
                            "https://github.com/neknaj/NEPL3/tree/main/examples".into()
                        }
                        "../tasks/" => "https://github.com/neknaj/NEPL3/tree/main/tasks".into(),
                        _ => dest_url.into_string(),
                    }),
                    _ => None,
                })
                .collect();
            let mut actual_links = Vec::new();
            for node in &document.value.nodes {
                if let DocKind::Link { target, .. } = &node.kind {
                    match target {
                        LinkTarget::Relative {
                            path,
                            fragment: None,
                        } => actual_links.push(path.clone()),
                        LinkTarget::External { uri }
                            if matches!(
                                uri.as_str(),
                                "https://github.com/neknaj/NEPL3/blob/main/doc/spec/13-reproducibility.nepld"
                                    | "https://github.com/neknaj/NEPL3/tree/main/examples"
                                    | "https://github.com/neknaj/NEPL3/tree/main/tasks"
                            ) =>
                        {
                            actual_links.push(uri.clone())
                        }
                        _ => return Err("unexpected index link target".into()),
                    }
                }
            }
            expected_links.sort();
            actual_links.sort();
            assert_eq!(actual_links, expected_links);
            Ok(())
        },
    )?;
    // Exact registration and output-only compatibility, never a duplicate
    // semantic passive source. This assertion alone is not HTML acceptance.
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    assert!(
        registry["pages"]
            .as_array()
            .ok_or("pages")?
            .iter()
            .any(|p| p["source"] == "doc/README.nepld"
                && p["projection"] == "doc/README.md"
                && p["route"] == "docs/guide/index.html"
                && p["id"] == "documentation-index")
    );
    assert!(
        !registry["files"]
            .as_array()
            .ok_or("files")?
            .iter()
            .any(|p| p["source"] == "doc/README.md")
    );
    assert!(
        registry["compatibility"]
            .as_array()
            .ok_or("compatibility")?
            .iter()
            .any(|p| p["page"] == "documentation-index" && p["route"] == "sources/doc-index.md")
    );
    Ok(())
}
