//! Scoped command parity in an incomplete, non-authoritative migration draft.
use super::super::*;
use super::{blocks, markdown_blocks};
use nepl3_doc_core::{
    check::Category,
    model::{DocKind, DocRoot},
    text,
};
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
            let DocRoot::Article(root) = document.value.root else {
                return Err("article root".into());
            };
            let DocKind::Article { body, .. } = &document.value.nodes[root.0 as usize].kind else {
                return Err("article node".into());
            };
            let DocKind::Body { blocks: children } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("article body".into());
            };
            let section_ids: Vec<_> = children
                .iter()
                .filter_map(|child| match &document.value.nodes[child.0 as usize].kind {
                    DocKind::Section { id, .. } => Some(id.as_str()),
                    _ => None,
                })
                .collect();
            // This checks the repaired first-H1 subsection ownership only;
            // the misplaced second-H1 Portable section remains a separate gap.
            assert!(
                section_ids
                    .windows(3)
                    .any(|ids| ids == ["local", "documentGeneration", "pages"])
            );
            let checked = nepl3_doc_core::labels::check(
                &document,
                profile.registry(),
                &mut budget(),
                codec.source_admission(),
            )
            .map_err(err)?;
            assert_eq!(checked.references().len(), 1);
            let target = checked.references()[0].target.0 as usize;
            assert_eq!(
                checked
                    .definitions()
                    .get(target)
                    .ok_or("reference definition")?
                    .name,
                "local"
            );
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
            // All ten first-H1 fenced blocks are present: their source order
            // matters too. The three remaining blocks belong to the second H1.
            assert_eq!(actual.as_slice(), &expected[..10]);
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
                (
                    "review",
                    "## 実装と独立レビュー",
                    "\n# 正本と観測記録の境界",
                ),
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
            for (source_prefix, plain_prefix) in [
                (
                    "providerのstream transportは",
                    "providerのstreamtransportは",
                ),
                ("Replyの受信から意味検査まで", "Replyの受信から意味検査まで"),
                (
                    "Replyの追加sourceを検査するhostは",
                    "Replyの追加sourceを検査するhostは",
                ),
                ("Doc inventoryの明示的なbaseline監査", "Docinventoryの"),
                ("Doc は", "Docは"),
                ("初期Pages公開は", "初期Pages公開は"),
                (
                    "`cargo run --locked -p nepl3-tools -- site build",
                    "cargorun--locked-pnepl3-tools--sitebuild",
                ),
                ("scope付き証拠は", "scope付き証拠は"),
                ("`--commit` は", "--commitは"),
                ("この操作は検査対象", "この操作は検査対象"),
                ("[最初の移行候補]", "最初の移行候補は"),
                (
                    "Readerの生成sourceを持つReportの境界は",
                    "Readerの生成sourceを持つReportの境界は",
                ),
                ("Sentenceの統合経路は", "Sentenceの統合経路は"),
                ("nativeの依存操作schedulerは", "nativeの依存操作schedulerは"),
                (
                    "nativeの `cargo test --locked -p nepl3-provider --test process_protocol`",
                    "nativeのcargotest--locked-pnepl3-provider--testprocess_protocol",
                ),
                (
                    "Readerのterminal応答については",
                    "Readerのterminal応答については",
                ),
                (
                    "`doc/canonical.json` に登録された仕様",
                    "doc/canonical.jsonに",
                ),
            ] {
                let source_paragraph = original
                    .split("\n\n")
                    .find(|paragraph| paragraph.starts_with(source_prefix))
                    .ok_or("source paragraph")?;
                let expected_paragraph = markdown_blocks(source_paragraph);
                let mut matches = 0;
                for (node_index, node) in document.value.nodes.iter().enumerate() {
                    let DocKind::Paragraph { items } = &node.kind else {
                        continue;
                    };
                    let Some(first) = items.first() else { continue };
                    if !matches!(
                        document.value.nodes.get(first.0 as usize).map(|n| &n.kind),
                        Some(DocKind::Sentence { .. })
                    ) {
                        continue;
                    }
                    if super::normalized(&project(nepl3_doc_core::model::SentenceRef(first.0))?)
                        .starts_with(plain_prefix)
                    {
                        let mut actual = Vec::new();
                        blocks(&document, node_index as u64, &mut project, &mut actual)?;
                        assert_eq!(actual, expected_paragraph, "paragraph {source_prefix}");
                        if source_prefix.starts_with("providerの")
                            || source_prefix.starts_with("Replyの")
                            || source_prefix.starts_with("Readerの")
                            || source_prefix.starts_with("Sentenceの")
                            || source_prefix.starts_with("nativeの")
                            || source_prefix.starts_with("Doc は")
                            || source_prefix.starts_with("初期Pages公開")
                            || source_prefix
                                .starts_with("`cargo run --locked -p nepl3-tools -- site build")
                            || source_prefix.starts_with("scope付き")
                            || source_prefix.starts_with("`--commit`")
                            || source_prefix.starts_with("この操作は")
                            || source_prefix.starts_with("[最初の移行候補]")
                        {
                            let expected_code: Vec<_> = Parser::new(source_paragraph)
                                .filter_map(|event| match event {
                                    Event::Code(value) => Some(value.into_string()),
                                    _ => None,
                                })
                                .collect();
                            let mut actual_code = Vec::new();
                            paragraph_inline_code(&document, node_index as u64, &mut actual_code)?;
                            assert_eq!(
                                actual_code, expected_code,
                                "exact inline code {source_prefix}"
                            );
                        }
                        matches += 1;
                    }
                }
                assert_eq!(matches, 1, "paragraph {source_prefix}");
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

// Only the scoped execution paragraphs above use this traversal. Reject new forms
// rather than silently overlooking code nested in an unsupported container.
fn paragraph_inline_code(
    document: &nepl3_doc_core::model::DocumentSyntax,
    node: u64,
    output: &mut Vec<String>,
) -> Result<(), String> {
    match &document
        .value
        .nodes
        .get(node as usize)
        .ok_or("inline node")?
        .kind
    {
        DocKind::Paragraph { items } => {
            for item in items {
                paragraph_inline_code(document, item.0, output)?;
            }
        }
        DocKind::Sentence { inlines } | DocKind::Concat { inlines } => {
            for inline in inlines {
                paragraph_inline_code(document, inline.0, output)?;
            }
        }
        DocKind::Ruby { base, reading } => {
            paragraph_inline_code(document, base.0, output)?;
            paragraph_inline_code(document, reading.0, output)?;
        }
        DocKind::Link { label, .. } => paragraph_inline_code(document, label.0, output)?,
        DocKind::InlineCode { text } => output.push(text.clone()),
        DocKind::Text { .. } => {}
        other => return Err(format!("unhandled scoped inline-code node: {other:?}")),
    }
    Ok(())
}
