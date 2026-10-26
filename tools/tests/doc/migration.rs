//! Ordered typed-model versus Markdown comparisons for migration regressions.
//! This deliberately is not a source parser, renderer or full acceptance gate.
use nepl3_doc_core::model::{DocKind, DocumentSyntax, SentenceRef};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
#[path = "migration/agents.rs"]
mod agents;
#[path = "migration/index.rs"]
mod index;

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
