//! Repair a historical candidate without changing current AGENTS authority.
use super::super::*;
use super::{blocks, markdown_blocks};
use nepl3_doc_core::{check::Category, model::*, text};
use pulldown_cmark::{Event, Parser, Tag};

#[test]
fn agents_draft_preserves_current_instructions_without_cutover() -> Result<(), String> {
    let original = include_str!("../../../../AGENTS.md");
    let source = include_str!("../../../../doc/migration/authored/project/AGENTS.nepld");
    let c = compiled()?;
    let registry: nepl3_tools::doc::canonical::Registry =
        serde_json::from_str(include_str!("../../../../doc/canonical.json")).map_err(err)?;
    nepl3_tools::doc::source::with_named_input_limits(
        true,
        &c,
        source,
        "agents-draft",
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
                other => Err(format!("incomplete original comparison: {other:?}")),
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
                .filter_map(|n| match &n.kind {
                    DocKind::Section { id, .. } => Some(id.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(
                sections,
                [
                    "norms",
                    "quality",
                    "completion",
                    "sequence",
                    "repository",
                    "responsibilities"
                ]
            );
            let mut actual_links = Vec::new();
            let mut actual_code = Vec::new();
            let mut list_items = 0;
            let mut lists = 0;
            for node in &document.value.nodes {
                match &node.kind {
                    DocKind::Link { target, .. } => actual_links.push(match target {
                        LinkTarget::Relative { path, fragment } => match fragment {
                            Some(fragment) => format!("{path}#{fragment}"),
                            None => path.clone(),
                        },
                        LinkTarget::External { uri } => uri.clone(),
                        _ => return Err("unexpected AGENTS link target".into()),
                    }),
                    DocKind::InlineCode { text } => actual_code.push(text.clone()),
                    DocKind::List { kind, items } => {
                        assert_eq!(*kind, ListKind::Unordered);
                        lists += 1;
                        list_items += items.len();
                    }
                    _ => {}
                }
            }
            let mut expected_links = Vec::new();
            let mut expected_code = Vec::new();
            let mut expected_items = 0;
            let mut expected_lists = 0;
            for event in Parser::new(original) {
                match event {
                    Event::Start(Tag::Link { dest_url, .. }) => {
                        expected_links.push(dest_url.into_string())
                    }
                    Event::Code(text) => expected_code.push(text.into_string()),
                    Event::Start(Tag::Item) => expected_items += 1,
                    Event::Start(Tag::List(start)) => {
                        assert_eq!(start, None);
                        expected_lists += 1;
                    }
                    _ => {}
                }
            }
            actual_links.sort();
            expected_links.sort();
            actual_code.sort();
            expected_code.sort();
            assert_eq!(actual_links, expected_links);
            assert_eq!(actual_code, expected_code);
            assert_eq!((lists, list_items), (expected_lists, expected_items));
            assert!(actual_links.contains(&"doc/development.md#branchとprの統合手順".into()));
            Ok(())
        },
    )?;
    // This is text/model parity, not fragment resolution or canonical authority.
    assert!(registry.pages.iter().all(|p| p.projection != "AGENTS.md"));
    assert!(
        registry
            .files
            .iter()
            .any(|p| p.source == "AGENTS.md" && p.route == "sources/agents.md")
    );
    Ok(())
}
