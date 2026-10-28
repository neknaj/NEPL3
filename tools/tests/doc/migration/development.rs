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
    // Match the pre-existing policy for this same unadopted guide in
    // canonical::tests::synchronized_context_spec_drafts_parse_lower_and_check_labels.
    // This is content/structure review of a growing reference corpus, not
    // admission under production canonical registry limits. The first attempt
    // with the registry's 550M allocation cap stopped; that remains a cutover
    // limitation, not a reason to trim source or change ordinary defaults.
    let draft_limits = Limits {
        work: 600_000_000,
        allocation_units: 1_500_000_000,
        ..budget().limits()
    };
    nepl3_tools::doc::source::with_named_input_limits(
        true,
        &c,
        source,
        "development-draft",
        "Body",
        draft_limits,
        |tree, profile, parse_budget, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let mut lower_budget = Budget::new(draft_limits);
            let document = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &c.doc.package.schema,
                Category::Body,
                profile.registry(),
                &mut lower_budget,
                &mut codec,
            )
            .map_err(err)?;
            let DocRoot::Body(root) = document.value.root else {
                return Err("Body root".into());
            };
            let DocKind::Body { blocks: roots } = &document.value.nodes[root.0 as usize].kind
            else {
                return Err("root Body".into());
            };
            assert_eq!(roots.len(), 2, "the two source H1 headings remain peers");
            let DocKind::Section {
                id: first_id,
                title: first_title,
                body: first_body,
            } = &document.value.nodes[roots[0].0 as usize].kind
            else {
                return Err("first H1 section".into());
            };
            let DocKind::Section {
                id: second_id,
                title: second_title,
                body: second_body,
            } = &document.value.nodes[roots[1].0 as usize].kind
            else {
                return Err("second H1 section".into());
            };
            assert_eq!(
                (first_id.as_str(), second_id.as_str()),
                ("developmentGuide", "canonicalBoundary")
            );
            let DocKind::Body { blocks: children } =
                &document.value.nodes[first_body.0 as usize].kind
            else {
                return Err("first H1 body".into());
            };
            let DocKind::Body {
                blocks: boundary_children,
            } = &document.value.nodes[second_body.0 as usize].kind
            else {
                return Err("second H1 body".into());
            };
            let section_ids: Vec<_> = children
                .iter()
                .filter_map(|child| match &document.value.nodes[child.0 as usize].kind {
                    DocKind::Section { id, .. } => Some(id.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(
                section_ids,
                [
                    "local",
                    "documentGeneration",
                    "pages",
                    "distribution",
                    "branchIntegration",
                    "review"
                ]
            );
            let boundary_sections: Vec<_> = boundary_children
                .iter()
                .filter_map(|child| match &document.value.nodes[child.0 as usize].kind {
                    DocKind::Section { id, .. } => Some(id.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(
                boundary_sections,
                ["portable"],
                "Portable belongs under the second H1"
            );
            assert_eq!(
                boundary_children.len(),
                8,
                "seven restored paragraphs then Portable"
            );
            let mut label_budget = budget();
            let checked = nepl3_doc_core::labels::check_body(
                &document,
                profile.registry(),
                &mut label_budget,
                codec.source_admission(),
            )
            .map_err(err)?;
            eprintln!(
                "development draft policy={draft_limits:?}; parse={:?}; lower={:?}; labels limits={:?} usage={:?}",
                parse_budget.usage(),
                lower_budget.usage(),
                label_budget.limits(),
                label_budget.usage()
            );
            let targets = checked
                .references()
                .iter()
                .map(|reference| {
                    checked
                        .definitions()
                        .get(reference.target.0 as usize)
                        .map(|site| site.name)
                        .ok_or("reference definition")
                })
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(
                targets,
                [
                    "local",
                    "documentGeneration",
                    "pages",
                    "branchIntegration",
                    "review",
                    "portable",
                    "local"
                ]
            );
            // The restored navigation is a four-item unordered list, not prose
            // with bullets or flattened section headings.
            let DocKind::List { kind, items } = &document.value.nodes[children[1].0 as usize].kind
            else {
                return Err("navigation list".into());
            };
            assert_eq!(*kind, nepl3_doc_core::model::ListKind::Unordered);
            assert_eq!(items.len(), 4);
            let DocKind::Paragraph { items: intro } =
                &document.value.nodes[children[0].0 as usize].kind
            else {
                return Err("intro paragraph".into());
            };
            assert_eq!(intro.len(), 2);
            let mut actual_links = Vec::new();
            for sentence in intro {
                let DocKind::Sentence { inlines } = &document.value.nodes[sentence.0 as usize].kind
                else {
                    return Err("intro sentence".into());
                };
                for inline in inlines {
                    if let DocKind::Link { target, .. } =
                        &document.value.nodes[inline.0 as usize].kind
                    {
                        let nepl3_doc_core::model::LinkTarget::Relative { path, fragment } = target
                        else {
                            return Err("intro relative link".into());
                        };
                        assert!(fragment.is_none());
                        actual_links.push(path.as_str());
                    }
                }
            }
            let intro_source = original
                .split("\n## ローカル環境")
                .next()
                .ok_or("source introduction")?
                .split_once("\n\n")
                .ok_or("source title boundary")?
                .1;
            let expected_links: Vec<_> = Parser::new(intro_source)
                .filter_map(|event| match event {
                    Event::Start(Tag::Link { dest_url, .. }) if !dest_url.starts_with('#') => {
                        Some(dest_url.into_string())
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(actual_links, expected_links);
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
            // Root Body -> peer Section gives H1, their Section children give
            // H2. Compare source heading ranks/titles as well as ownership.
            let source_h1: Vec<_> = Parser::new(original)
                .filter_map(|event| match event {
                    Event::Start(Tag::Heading {
                        level: pulldown_cmark::HeadingLevel::H1,
                        ..
                    }) => Some(()),
                    _ => None,
                })
                .collect();
            assert_eq!(source_h1.len(), roots.len());
            assert_eq!(project(*first_title)?, "開発と検査");
            assert_eq!(project(*second_title)?, "正本と観測記録の境界");
            let boundary_source = original
                .split_once("# 正本と観測記録の境界\n\n")
                .ok_or("source second H1")?
                .1
                .split("\n## Portable execution CI")
                .next()
                .ok_or("source Portable boundary")?;
            let mut actual_boundary = Vec::new();
            for child in &boundary_children[..7] {
                blocks(&document, child.0, &mut project, &mut actual_boundary)?;
            }
            assert_eq!(actual_boundary, markdown_blocks(boundary_source));
            let expected_boundary_code: Vec<_> = Parser::new(boundary_source)
                .filter_map(|event| match event {
                    Event::Code(value) => Some(value.into_string()),
                    _ => None,
                })
                .collect();
            let mut actual_boundary_code = Vec::new();
            for child in &boundary_children[..7] {
                paragraph_inline_code(&document, child.0, &mut actual_boundary_code)?;
            }
            assert_eq!(actual_boundary_code, expected_boundary_code);
            let DocKind::Paragraph {
                items: evidence_items,
            } = &document.value.nodes[boundary_children[6].0 as usize].kind
            else {
                return Err("evidence paragraph".into());
            };
            let DocKind::Sentence {
                inlines: evidence_inlines,
            } = &document.value.nodes[evidence_items[0].0 as usize].kind
            else {
                return Err("evidence sentence".into());
            };
            let evidence_links: Vec<_> = evidence_inlines
                .iter()
                .filter_map(
                    |inline| match &document.value.nodes[inline.0 as usize].kind {
                        DocKind::Link { target, .. } => Some(target),
                        _ => None,
                    },
                )
                .collect();
            assert_eq!(
                evidence_links,
                [&nepl3_doc_core::model::LinkTarget::Relative {
                    path: "../tools/evidence/README.md".into(),
                    fragment: None
                }]
            );

            let portable_source = original
                .split_once("## Portable execution CI\n\n")
                .ok_or("source Portable")?
                .1
                .split("\n### Inline schema-validation frontier")
                .next()
                .ok_or("Portable end")?;
            let DocKind::Section {
                body: portable_body,
                title: portable_title,
                ..
            } = &document.value.nodes[boundary_children[7].0 as usize].kind
            else {
                return Err("Portable section".into());
            };
            assert_eq!(project(*portable_title)?, "Portable execution CI");
            let mut actual_portable = Vec::new();
            blocks(
                &document,
                portable_body.0,
                &mut project,
                &mut actual_portable,
            )?;
            assert_eq!(actual_portable, markdown_blocks(portable_source));
            let mut actual_intro = Vec::new();
            for child in &children[..3] {
                blocks(&document, child.0, &mut project, &mut actual_intro)?;
            }
            assert_eq!(actual_intro, markdown_blocks(intro_source));
            // Ordered base text only: heading ranks, Ruby, URLs and sentence
            // boundaries additionally require independent content review.
            for (id, heading, end) in [
                (
                    "documentGeneration",
                    "## 文書生成と初期Pages公開",
                    "\n## ページ集合と移行候補",
                ),
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
                    "`python tools/site/payload.py",
                    "pythontools/site/payload.py",
                ),
                ("復旧用の", "復旧用の"),
                ("`python tools/site/smoke.py", "pythontools/site/smoke.py"),
                ("受入群の文書検査は", "受入群の文書検査は"),
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
                            || source_prefix.starts_with("`python tools/site/")
                            || source_prefix.starts_with("復旧用の")
                            || source_prefix.starts_with("受入群の文書検査は")
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
