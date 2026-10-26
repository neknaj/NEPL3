//! Scoped command parity in an incomplete, non-authoritative migration draft.
use super::super::*;
use super::{blocks, markdown_blocks};
use nepl3_doc_core::{check::Category, model::DocKind, text};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

#[test]
fn development_draft_existing_commands_match_current_source() -> Result<(), String> {
    let original = include_str!("../../../../doc/development.md");
    let source = include_str!("../../../../doc/migration/authored/guide/development.nepld");
    let mut expected = Vec::new();
    let mut block = None;
    let mut wasi = None;
    for event in Parser::new(original) {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language))) => {
                block = Some((Some(language.into_string()), String::new()));
            }
            Event::Text(text) => {
                if let Some((_, body)) = &mut block {
                    body.push_str(&text);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(value) = block.take() {
                    expected.push(value);
                }
            }
            Event::Code(text)
                if text.starts_with("cargo test --locked -p nepl3-core ")
                    && text.contains("--target wasm32-wasip2") =>
            {
                assert!(wasi.is_none(), "one source WASI command");
                wasi = Some(text.into_string());
            }
            _ => {}
        }
    }
    let wasi = wasi.ok_or("source WASI command missing")?;
    let c = compiled()?;
    let registry: nepl3_tools::doc::canonical::Registry =
        serde_json::from_str(include_str!("../../../../doc/canonical.json")).map_err(err)?;
    nepl3_tools::doc::source::with_named_input_limits(
        true,
        &c,
        source,
        "development-draft",
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
            let mut actual = Vec::new();
            let mut actual_wasi = Vec::new();
            for node in &document.value.nodes {
                match &node.kind {
                    DocKind::RawCode {
                        language_hint,
                        text,
                    } => {
                        actual.push((language_hint.clone(), text.clone()));
                    }
                    DocKind::InlineCode { text }
                        if text.starts_with("cargo test --locked -p nepl3-core ")
                            && text.contains("--target wasm32-wasip2") =>
                    {
                        actual_wasi.push(text.clone());
                    }
                    _ => {}
                }
            }
            assert_eq!(actual_wasi, [wasi]);
            // Deliberately a subset: the missing three fenced blocks, prose,
            // headings and link closure still block a complete migration.
            assert_eq!((expected.len(), actual.len()), (13, 10));
            for value in actual {
                let index = expected
                    .iter()
                    .position(|item| item == &value)
                    .ok_or_else(|| format!("draft command differs from source: {value:?}"))?;
                expected.remove(index);
            }
            assert_eq!(expected.len(), 3);
            let mut b = budget();
            let prepared =
                text::prepare(&document, profile.registry(), &mut codec, &mut b).map_err(err)?;
            let mut project = |sentence| match prepared
                .project(sentence, text::AnnotationPolicy::BaseOnly, &[], &mut b)
                .map_err(err)?
                .outcome
            {
                text::PlainTextOutcome::Complete { text } => Ok(text),
                other => Err(format!("incomplete section text: {other:?}")),
            };
            // Ordered base text only: heading ranks, Ruby, URLs and sentence
            // boundaries additionally require independent content review.
            for (id, heading, end) in [
                (
                    "branchIntegration",
                    "## branchとPRの統合手順",
                    "\n## 実装と独立レビュー",
                ),
                ("distribution", "## CIと配布", "\n## branchとPRの統合手順"),
            ] {
                let node = document.value.nodes.iter().position(|node| {
                    matches!(&node.kind, DocKind::Section { id: actual, .. } if actual == id)
                }).ok_or_else(|| format!("draft section missing: {id}"))?;
                let (_, rest) = original.split_once(heading).ok_or("source heading")?;
                let (body, _) = rest.split_once(end).ok_or("source section end")?;
                let expected_section = markdown_blocks(&format!("{heading}{body}"));
                let mut actual_section = Vec::new();
                blocks(&document, node as u64, &mut project, &mut actual_section)?;
                assert_eq!(actual_section, expected_section, "section {id}");
            }
            Ok(())
        },
    )?;
    assert!(
        registry
            .pages
            .iter()
            .all(|p| p.projection != "doc/development.md")
    );
    assert!(
        registry
            .files
            .iter()
            .any(|p| p.source == "doc/development.md" && p.route == "sources/development.md")
    );
    Ok(())
}
