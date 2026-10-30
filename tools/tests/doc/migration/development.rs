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
    // Growing unadopted corpus stopped at 930M parse Work.
    // Finite 980M / 13.5M sizing measured 948,025,722 Work / 12,911,000 Nodes.
    // Select 955M / 13M for parse only; other phase policies stay separate.
    let parse_limits = Limits {
        work: 955_000_000,
        nodes: 13_000_000,
        ..draft_limits
    };
    nepl3_tools::doc::source::with_named_input_limits(
        true,
        &c,
        source,
        "development-draft",
        "Body",
        parse_limits,
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
                [
                    "portable",
                    "savedExecutionScope",
                    "resourceTransitions",
                    "budgetUnwinding"
                ],
                "restored H2 sections belong under the second H1"
            );
            assert_eq!(
                boundary_children.len(),
                11,
                "seven restored paragraphs then four peer H2 sections"
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
                "development draft parse policy={parse_limits:?}; lower policy={draft_limits:?}; parse={:?}; lower={:?}; labels limits={:?} usage={:?}",
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
            // All ordered command fences are restored; prose, headings and
            // link closure still block complete document migration.
            assert_eq!((expected.len(), actual.len()), (13, 13));
            assert_eq!(actual, expected);
            // The prior 255M allowance stopped at 255,000,000 Work.
            // Finite 290M sizing measured 261,977,458 for preparation and
            // 262,583,259 for all parity. Use one cumulative 265M budget;
            // no reset, memoization or production/default policy change.
            let mut b = Budget::new(Limits {
                work: 265_000_000,
                ..budget().limits()
            });
            let prepared = text::prepare(&document, profile.registry(), &mut codec, &mut b)
                .map_err(|error| {
                    format!("text preparation failed: {error:?}; usage={:?}", b.usage())
                })?;
            eprintln!(
                "development text preparation limits={:?} usage={:?}",
                b.limits(),
                b.usage()
            );
            let mut project = |sentence| match prepared
                .project(sentence, text::AnnotationPolicy::BaseOnly, &[], &mut b)
                .map_err(|error| {
                    format!(
                        "text projection {sentence:?} failed: {error:?}; usage={:?}",
                        b.usage()
                    )
                })?
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
            let DocKind::Body {
                blocks: portable_children,
            } = &document.value.nodes[portable_body.0 as usize].kind
            else {
                return Err("Portable body".into());
            };
            assert_eq!(portable_children.len(), 4, "three paragraphs then one H3");
            let mut actual_portable = Vec::new();
            for child in &portable_children[..3] {
                assert!(matches!(
                    document.value.nodes[child.0 as usize].kind,
                    DocKind::Paragraph { .. }
                ));
                blocks(&document, child.0, &mut project, &mut actual_portable)?;
            }
            assert_eq!(actual_portable, markdown_blocks(portable_source));
            // Ownership, not whitespace-normalized heading text, establishes H3:
            // Body -> canonicalBoundary H1 -> portable H2 -> inlineSchemaFrontier H3.
            let DocKind::Section {
                id: frontier_id,
                title: frontier_title,
                body: frontier_body,
            } = &document.value.nodes[portable_children[3].0 as usize].kind
            else {
                return Err("nested frontier H3".into());
            };
            assert_eq!(frontier_id.as_str(), "inlineSchemaFrontier");
            assert_eq!(
                project(*frontier_title)?,
                "Inline schema-validation frontier"
            );
            let DocKind::Body {
                blocks: frontier_paragraphs,
            } = &document.value.nodes[frontier_body.0 as usize].kind
            else {
                return Err("frontier body".into());
            };
            assert_eq!(frontier_paragraphs.len(), 2);
            let frontier_source = original
                .split_once("### Inline schema-validation frontier\n\n")
                .ok_or("source frontier")?
                .1
                .split_once("\n## Pure saved execution-scope derivation")
                .ok_or("frontier source end")?
                .0;
            let expected_frontier: Vec<_> = frontier_source
                .trim()
                .split("\n\n")
                .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect();
            assert_eq!(expected_frontier.len(), 2);
            for ((paragraph, expected), sentence_count) in frontier_paragraphs
                .iter()
                .zip(&expected_frontier)
                .zip([5, 6])
            {
                let DocKind::Paragraph { items } = &document.value.nodes[paragraph.0 as usize].kind
                else {
                    return Err("frontier paragraph".into());
                };
                assert_eq!(items.len(), sentence_count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                // Actual spacing is semantic here: stripping whitespace would
                // incorrectly accept "array.Each" or joined English words.
                assert_eq!(sentences.concat(), *expected);
                for (index, sentence) in sentences.iter().enumerate() {
                    assert!(!sentence.starts_with(' '));
                    assert!(sentence.ends_with(if index + 1 < sentences.len() {
                        ". "
                    } else {
                        "."
                    }));
                }
            }
            let DocKind::Section {
                id: scope_id,
                title: scope_title,
                body: scope_body,
            } = &document.value.nodes[boundary_children[8].0 as usize].kind
            else {
                return Err("saved scope H2".into());
            };
            assert_eq!(scope_id.as_str(), "savedExecutionScope");
            assert_eq!(
                project(*scope_title)?,
                "Pure saved execution-scope derivation"
            );
            let DocKind::Body {
                blocks: scope_blocks,
            } = &document.value.nodes[scope_body.0 as usize].kind
            else {
                return Err("saved scope body".into());
            };
            assert_eq!(scope_blocks.len(), 3);
            assert!(matches!(
                document.value.nodes[scope_blocks[1].0 as usize].kind,
                DocKind::RawCode { .. }
            ));
            let scope_source = original
                .split_once("## Pure saved execution-scope derivation\n\n")
                .ok_or("source saved scope")?
                .1
                .split_once("\n## Pure resource-transition verification")
                .ok_or("source saved scope end")?
                .0;
            let expected_paragraphs: Vec<_> = scope_source
                .trim()
                .split("\n\n")
                .filter(|part| !part.starts_with("```"))
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(expected_paragraphs.len(), 2);
            for ((index, count), expected) in [(0, 4), (2, 7)].into_iter().zip(&expected_paragraphs)
            {
                let DocKind::Paragraph { items } =
                    &document.value.nodes[scope_blocks[index].0 as usize].kind
                else {
                    return Err("saved scope paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
            }
            let mut actual_scope_code = Vec::new();
            paragraph_inline_code(&document, scope_blocks[0].0, &mut actual_scope_code)?;
            assert_eq!(
                actual_scope_code,
                [
                    "ExecutionScope::child",
                    "suspension/execution/admission.rs",
                    "ExecutionScope::run"
                ]
            );
            let DocKind::Section { id, title, body } =
                &document.value.nodes[boundary_children[9].0 as usize].kind
            else {
                return Err("resource transition H2".into());
            };
            assert_eq!(id.as_str(), "resourceTransitions");
            assert_eq!(project(*title)?, "Pure resource-transition verification");
            let DocKind::Body {
                blocks: resource_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("resource transition body".into());
            };
            assert_eq!(resource_blocks.len(), 24);
            assert!(matches!(
                document.value.nodes[resource_blocks[4].0 as usize].kind,
                DocKind::RawCode { .. }
            ));
            // This stage includes the caller/host boundary paragraphs;
            // their restored nested coverage sections are checked separately below.
            let resource_source = original
                .split_once("## Pure resource-transition verification\n\n")
                .ok_or("source resource transitions")?
                .1
                .split_once("\n### Source admission interruption coverage")
                .ok_or("resource stage boundary")?
                .0;
            let expected_paragraphs: Vec<_> = resource_source
                .trim()
                .split("\n\n")
                .filter(|part| !part.starts_with("```"))
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(expected_paragraphs.len(), 13);
            let mut actual_codes = Vec::new();
            for ((index, count), expected) in [
                (0, 4),
                (1, 5),
                (2, 5),
                (3, 3),
                (5, 6),
                (6, 4),
                (7, 6),
                (8, 2),
                (9, 7),
                (10, 10),
                (11, 6),
                (12, 5),
                (13, 8),
            ]
            .into_iter()
            .zip(&expected_paragraphs)
            {
                let node = resource_blocks[index];
                let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                else {
                    return Err("resource paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                paragraph_inline_code(&document, node.0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(resource_source)
                .filter_map(|e| match e {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(actual_codes, expected_codes);
            for (index, expected_id, heading, end, counts, code_count) in [
                (
                    14,
                    "sourceAdmissionInterruption",
                    "Source admission interruption coverage",
                    "Saved execution-scope re-entry coverage",
                    &[7usize, 5][..],
                    2,
                ),
                (
                    15,
                    "savedScopeReentry",
                    "Saved execution-scope re-entry coverage",
                    "Markdown batch validation reuse",
                    &[6usize, 2][..],
                    4,
                ),
                (
                    16,
                    "markdownBatchReuse",
                    "Markdown batch validation reuse",
                    "Checked PageSet preparation reuse",
                    &[8usize, 5, 4][..],
                    1,
                ),
                (
                    17,
                    "checkedPagesetReuse",
                    "Checked PageSet preparation reuse",
                    "Digest request lookup probes",
                    &[6usize, 5, 4][..],
                    0,
                ),
                (
                    18,
                    "digestRequestLookup",
                    "Digest request lookup probes",
                    "PageSet guest digest の mapping scope 回帰確認（2026-10-10）",
                    &[6usize][..],
                    0,
                ),
                (
                    19,
                    "guestMappingScope",
                    "PageSet guest digest の mapping scope 回帰確認（2026-10-10）",
                    "source position の独立期待値（2026-10-10）",
                    &[4usize, 3][..],
                    2,
                ),
                (
                    20,
                    "sourcePositionOracle",
                    "source position の独立期待値（2026-10-10）",
                    "Browser foundation source-position slice（2026-10-10）",
                    &[5usize, 2][..],
                    2,
                ),
                (
                    21,
                    "browserSourcePosition",
                    "Browser foundation source-position slice（2026-10-10）",
                    "Native / WASI source-position oracle（2026-10-10）",
                    &[4usize, 5, 2][..],
                    5,
                ),
                (
                    22,
                    "nativeWasiSourcePosition",
                    "Native / WASI source-position oracle（2026-10-10）",
                    "通常Doc HTML内の数式",
                    &[4usize, 4, 3][..],
                    5,
                ),
            ] {
                let DocKind::Section { id, title, body } =
                    &document.value.nodes[resource_blocks[index].0 as usize].kind
                else {
                    return Err("resource coverage H3".into());
                };
                assert_eq!(id.as_str(), expected_id);
                assert_eq!(project(*title)?, heading);
                let DocKind::Body {
                    blocks: coverage_blocks,
                } = &document.value.nodes[body.0 as usize].kind
                else {
                    return Err("resource coverage body".into());
                };
                assert_eq!(coverage_blocks.len(), counts.len());
                let coverage_source = original
                    .split_once(&format!("### {heading}\n\n"))
                    .ok_or("source coverage heading")?
                    .1
                    .split_once(&format!("\n### {end}"))
                    .ok_or("source coverage end")?
                    .0;
                let expected_paragraphs: Vec<_> = coverage_source
                    .trim()
                    .split("\n\n")
                    .map(|part| {
                        part.replace('`', "")
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect();
                assert_eq!(expected_paragraphs.len(), counts.len());
                let mut actual_codes = Vec::new();
                for ((node, count), expected) in coverage_blocks
                    .iter()
                    .zip(counts.iter().copied())
                    .zip(&expected_paragraphs)
                {
                    let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                    else {
                        return Err("resource coverage paragraph".into());
                    };
                    assert_eq!(items.len(), count);
                    let sentences = items
                        .iter()
                        .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                        .collect::<Result<Vec<_>, _>>()?;
                    assert_eq!(sentences.concat(), *expected);
                    paragraph_inline_code(&document, node.0, &mut actual_codes)?;
                }
                let expected_codes: Vec<_> = Parser::new(coverage_source)
                    .filter_map(|event| match event {
                        Event::Code(code) => Some(code.into_string()),
                        _ => None,
                    })
                    .collect();
                assert_eq!(expected_codes.len(), code_count);
                assert_eq!(actual_codes, expected_codes);
            }
            let DocKind::Section { id, title, body } =
                &document.value.nodes[resource_blocks[23].0 as usize].kind
            else {
                return Err("ordinary math H3".into());
            };
            assert_eq!(id.as_str(), "ordinaryDocHtmlMath");
            assert_eq!(project(*title)?, "通常Doc HTML内の数式");
            let DocKind::Body {
                blocks: math_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("ordinary math body".into());
            };
            assert_eq!(math_blocks.len(), 3);
            let math_source = original
                .split_once("### 通常Doc HTML内の数式\n\n")
                .ok_or("math source heading")?
                .1
                .split_once("\n## BudgetのscopeとRust unwinding")
                .ok_or("math source end")?
                .0;
            let paragraphs: Vec<_> = math_source
                .trim()
                .split("\n\n")
                .filter(|part| !part.starts_with("```"))
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(paragraphs.len(), 2);
            let mut actual_codes = Vec::new();
            for ((index, count), expected) in [(0, 3), (2, 7)].into_iter().zip(&paragraphs) {
                let DocKind::Paragraph { items } =
                    &document.value.nodes[math_blocks[index].0 as usize].kind
                else {
                    return Err("ordinary math paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                paragraph_inline_code(&document, math_blocks[index].0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(math_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 6);
            assert_eq!(actual_codes, expected_codes);
            let DocKind::RawCode {
                language_hint,
                text,
            } = &document.value.nodes[math_blocks[1].0 as usize].kind
            else {
                return Err("ordinary math command fence".into());
            };
            assert_eq!(language_hint.as_deref(), Some("sh"));
            let source_code = math_source
                .split_once("```sh\n")
                .ok_or("math fence")?
                .1
                .split_once("```\n")
                .ok_or("math fence end")?
                .0;
            assert_eq!(text, source_code);
            let DocKind::Section { id, title, body } =
                &document.value.nodes[boundary_children[10].0 as usize].kind
            else {
                return Err("budget unwinding H2".into());
            };
            assert_eq!(id.as_str(), "budgetUnwinding");
            assert_eq!(project(*title)?, "BudgetのscopeとRust unwinding");
            let DocKind::Body {
                blocks: unwind_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("budget unwinding body".into());
            };
            assert_eq!(unwind_blocks.len(), 9);
            let DocKind::Paragraph { items } =
                &document.value.nodes[unwind_blocks[0].0 as usize].kind
            else {
                return Err("budget unwinding paragraph".into());
            };
            assert_eq!(items.len(), 5);
            let unwind_source = original
                .split_once("## BudgetのscopeとRust unwinding\n\n")
                .ok_or("unwind source heading")?
                .1
                .split_once("\n### Session-bound canonical Reader echoes")
                .ok_or("unwind source end")?
                .0;
            assert_eq!(unwind_source.trim().split("\n\n").count(), 1);
            let expected = unwind_source
                .replace('`', "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let sentences = items
                .iter()
                .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(sentences.concat(), expected);
            let mut actual_codes = Vec::new();
            paragraph_inline_code(&document, unwind_blocks[0].0, &mut actual_codes)?;
            let expected_codes: Vec<_> = Parser::new(unwind_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 6);
            assert_eq!(actual_codes, expected_codes);
            let DocKind::Section { id, title, body } =
                &document.value.nodes[unwind_blocks[1].0 as usize].kind
            else {
                return Err("reader echoes H3".into());
            };
            assert_eq!(id.as_str(), "readerEchoes");
            assert_eq!(project(*title)?, "Session-bound canonical Reader echoes");
            let DocKind::Body {
                blocks: echo_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("reader echoes body".into());
            };
            assert_eq!(echo_blocks.len(), 3);
            let echo_source = original
                .split_once("### Session-bound canonical Reader echoes\n\n")
                .ok_or("reader echoes source")?
                .1
                .split_once("\n### Exact saved Invoke comparison before settlement")
                .ok_or("reader echoes end")?
                .0;
            let paragraphs: Vec<_> = echo_source
                .trim()
                .split("\n\n")
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(paragraphs.len(), 3);
            let mut actual_codes = Vec::new();
            for ((node, count), expected) in echo_blocks.iter().zip([4, 6, 2]).zip(&paragraphs) {
                let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                else {
                    return Err("reader echoes paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                for (index, sentence) in sentences.iter().enumerate() {
                    assert!(!sentence.starts_with(' '));
                    assert!(sentence.ends_with(if index + 1 < sentences.len() {
                        ". "
                    } else {
                        "."
                    }));
                }
                paragraph_inline_code(&document, node.0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(echo_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 8);
            assert_eq!(actual_codes, expected_codes);
            let DocKind::Section { id, title, body } =
                &document.value.nodes[unwind_blocks[2].0 as usize].kind
            else {
                return Err("saved Invoke H3".into());
            };
            assert_eq!(id.as_str(), "savedInvokeComparison");
            assert_eq!(
                project(*title)?,
                "Exact saved Invoke comparison before settlement"
            );
            let DocKind::Body {
                blocks: invoke_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("saved Invoke body".into());
            };
            assert_eq!(invoke_blocks.len(), 4);
            let invoke_source = original
                .split_once("### Exact saved Invoke comparison before settlement\n\n")
                .ok_or("saved Invoke source")?
                .1
                .split_once("\n### Trusted terminal local children")
                .ok_or("saved Invoke end")?
                .0;
            let paragraphs: Vec<_> = invoke_source
                .trim()
                .split("\n\n")
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(paragraphs.len(), 4);
            let mut actual_codes = Vec::new();
            for ((node, count), expected) in invoke_blocks.iter().zip([4, 6, 6, 3]).zip(&paragraphs)
            {
                let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                else {
                    return Err("saved Invoke paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                for (index, sentence) in sentences.iter().enumerate() {
                    assert!(!sentence.starts_with(' '));
                    assert!(sentence.ends_with(if index + 1 < sentences.len() {
                        ". "
                    } else {
                        "."
                    }));
                }
                paragraph_inline_code(&document, node.0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(invoke_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 8);
            assert_eq!(actual_codes, expected_codes);
            let DocKind::Section { id, title, body } =
                &document.value.nodes[unwind_blocks[3].0 as usize].kind
            else {
                return Err("trusted local children H3".into());
            };
            assert_eq!(id.as_str(), "trustedLocalChildren");
            assert_eq!(project(*title)?, "Trusted terminal local children");
            let DocKind::Body {
                blocks: trusted_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("trusted local children body".into());
            };
            assert_eq!(trusted_blocks.len(), 5);
            let trusted_source = original
                .split_once("### Trusted terminal local children\n\n")
                .ok_or("trusted local children source")?
                .1
                .split_once("\n### Saved Reader call through a measured builtin child")
                .ok_or("trusted local children end")?
                .0;
            let paragraphs: Vec<_> = trusted_source
                .trim()
                .split("\n\n")
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(paragraphs.len(), 5);
            let mut actual_codes = Vec::new();
            for ((node, count), expected) in
                trusted_blocks.iter().zip([4, 5, 5, 4, 6]).zip(&paragraphs)
            {
                let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                else {
                    return Err("trusted local children paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                for (index, sentence) in sentences.iter().enumerate() {
                    assert!(!sentence.starts_with(' '));
                    assert!(sentence.ends_with(if index + 1 < sentences.len() {
                        ". "
                    } else {
                        "."
                    }));
                }
                paragraph_inline_code(&document, node.0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(trusted_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 7);
            assert_eq!(actual_codes, expected_codes);
            let DocKind::Section { id, title, body } =
                &document.value.nodes[unwind_blocks[4].0 as usize].kind
            else {
                return Err("saved Reader measured child H3".into());
            };
            assert_eq!(id.as_str(), "savedReaderMeasuredChild");
            assert_eq!(
                project(*title)?,
                "Saved Reader call through a measured builtin child"
            );
            let DocKind::Body {
                blocks: reader_child_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("saved Reader measured child body".into());
            };
            assert_eq!(reader_child_blocks.len(), 2);
            let reader_child_source = original
                .split_once("### Saved Reader call through a measured builtin child\n\n")
                .ok_or("saved Reader measured child source")?
                .1
                .split_once("\n### Tokenizer canonical pending-value echoes")
                .ok_or("saved Reader measured child end")?
                .0;
            let paragraphs: Vec<_> = reader_child_source
                .trim()
                .split("\n\n")
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(paragraphs.len(), 2);
            let mut actual_codes = Vec::new();
            for ((node, count), expected) in reader_child_blocks.iter().zip([6, 3]).zip(&paragraphs)
            {
                let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                else {
                    return Err("saved Reader measured child paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                for (index, sentence) in sentences.iter().enumerate() {
                    assert!(!sentence.starts_with(' '));
                    assert!(sentence.ends_with(if index + 1 < sentences.len() {
                        ". "
                    } else {
                        "."
                    }));
                }
                paragraph_inline_code(&document, node.0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(reader_child_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 2);
            assert_eq!(actual_codes, expected_codes);
            let DocKind::Section { id, title, body } =
                &document.value.nodes[unwind_blocks[5].0 as usize].kind
            else {
                return Err("tokenizer echoes H3".into());
            };
            assert_eq!(id.as_str(), "tokenizerPendingEchoes");
            assert_eq!(project(*title)?, "Tokenizer canonical pending-value echoes");
            let DocKind::Body {
                blocks: tokenizer_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("tokenizer echoes body".into());
            };
            assert_eq!(tokenizer_blocks.len(), 3);
            let tokenizer_source = original
                .split_once("### Tokenizer canonical pending-value echoes\n\n")
                .ok_or("tokenizer echoes source")?
                .1
                .split_once("\n### Standalone builtinName sender and encoding retries")
                .ok_or("tokenizer echoes end")?
                .0;
            let paragraphs: Vec<_> = tokenizer_source
                .trim()
                .split("\n\n")
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(paragraphs.len(), 3);
            let mut actual_codes = Vec::new();
            for ((node, count), expected) in tokenizer_blocks.iter().zip([3, 3, 3]).zip(&paragraphs)
            {
                let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                else {
                    return Err("tokenizer echoes paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                for (index, sentence) in sentences.iter().enumerate() {
                    assert!(!sentence.starts_with(' '));
                    assert!(sentence.ends_with(if index + 1 < sentences.len() {
                        ". "
                    } else {
                        "."
                    }));
                }
                paragraph_inline_code(&document, node.0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(tokenizer_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 0);
            assert_eq!(actual_codes, expected_codes);
            let DocKind::Section { id, title, body } =
                &document.value.nodes[unwind_blocks[6].0 as usize].kind
            else {
                return Err("standalone name sender H3".into());
            };
            assert_eq!(id.as_str(), "standaloneNameSender");
            assert_eq!(
                project(*title)?,
                "Standalone builtinName sender and encoding retries"
            );
            let DocKind::Body {
                blocks: sender_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("standalone name sender body".into());
            };
            assert_eq!(sender_blocks.len(), 3);
            let sender_source = original
                .split_once("### Standalone builtinName sender and encoding retries\n\n")
                .ok_or("standalone name sender source")?
                .1
                .split_once("\n### Native saved builtinName on the existing operation ledger")
                .ok_or("standalone name sender end")?
                .0;
            let paragraphs: Vec<_> = sender_source
                .trim()
                .split("\n\n")
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(paragraphs.len(), 3);
            let mut actual_codes = Vec::new();
            for ((node, count), expected) in sender_blocks.iter().zip([3, 7, 3]).zip(&paragraphs) {
                let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                else {
                    return Err("standalone name sender paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                for (index, sentence) in sentences.iter().enumerate() {
                    assert!(!sentence.starts_with(' '));
                    assert!(sentence.ends_with(if index + 1 < sentences.len() {
                        ". "
                    } else {
                        "."
                    }));
                }
                paragraph_inline_code(&document, node.0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(sender_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 4);
            assert_eq!(actual_codes, expected_codes);
            let DocKind::Section { id, title, body } =
                &document.value.nodes[unwind_blocks[7].0 as usize].kind
            else {
                return Err("native saved name ledger H3".into());
            };
            assert_eq!(id.as_str(), "nativeSavedNameLedger");
            assert_eq!(
                project(*title)?,
                "Native saved builtinName on the existing operation ledger"
            );
            let DocKind::Body {
                blocks: ledger_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("native saved name ledger body".into());
            };
            assert_eq!(ledger_blocks.len(), 3);
            let ledger_source = original
                .split_once("### Native saved builtinName on the existing operation ledger\n\n")
                .ok_or("native saved name ledger source")?
                .1
                .split_once("\n### Identical syntax source-prefix checkpoints")
                .ok_or("native saved name ledger end")?
                .0;
            let paragraphs: Vec<_> = ledger_source
                .trim()
                .split("\n\n")
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(paragraphs.len(), 3);
            let mut actual_codes = Vec::new();
            for ((node, count), expected) in ledger_blocks.iter().zip([3, 7, 6]).zip(&paragraphs) {
                let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                else {
                    return Err("native saved name ledger paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                for (index, sentence) in sentences.iter().enumerate() {
                    assert!(!sentence.starts_with(' '));
                    assert!(sentence.ends_with(if index + 1 < sentences.len() {
                        ". "
                    } else {
                        "."
                    }));
                }
                paragraph_inline_code(&document, node.0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(ledger_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 4);
            assert_eq!(actual_codes, expected_codes);
            let DocKind::Section { id, title, body } =
                &document.value.nodes[unwind_blocks[8].0 as usize].kind
            else {
                return Err("syntax source prefixes H3".into());
            };
            assert_eq!(id.as_str(), "syntaxSourcePrefixes");
            assert_eq!(
                project(*title)?,
                "Identical syntax source-prefix checkpoints"
            );
            let DocKind::Body {
                blocks: prefix_blocks,
            } = &document.value.nodes[body.0 as usize].kind
            else {
                return Err("syntax source prefixes body".into());
            };
            assert_eq!(prefix_blocks.len(), 5);
            let prefix_source = original
                .split_once("### Identical syntax source-prefix checkpoints\n\n")
                .ok_or("syntax source prefixes source")?
                .1
                .split_once("\n### Metered request source union and canonical parse policy")
                .ok_or("syntax source prefixes end")?
                .0;
            let paragraphs: Vec<_> = prefix_source
                .trim()
                .split("\n\n")
                .map(|part| {
                    part.replace('`', "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            assert_eq!(paragraphs.len(), 5);
            let mut actual_codes = Vec::new();
            for ((node, count), expected) in
                prefix_blocks.iter().zip([6, 6, 2, 4, 6]).zip(&paragraphs)
            {
                let DocKind::Paragraph { items } = &document.value.nodes[node.0 as usize].kind
                else {
                    return Err("syntax source prefixes paragraph".into());
                };
                assert_eq!(items.len(), count);
                let sentences = items
                    .iter()
                    .map(|item| project(nepl3_doc_core::model::SentenceRef(item.0)))
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sentences.concat(), *expected);
                for (index, sentence) in sentences.iter().enumerate() {
                    assert!(!sentence.starts_with(' '));
                    assert!(sentence.ends_with(if index + 1 < sentences.len() {
                        ". "
                    } else {
                        "."
                    }));
                }
                paragraph_inline_code(&document, node.0, &mut actual_codes)?;
            }
            let expected_codes: Vec<_> = Parser::new(prefix_source)
                .filter_map(|event| match event {
                    Event::Code(code) => Some(code.into_string()),
                    _ => None,
                })
                .collect();
            assert_eq!(expected_codes.len(), 0);
            assert_eq!(actual_codes, expected_codes);
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
            eprintln!(
                "development full text parity limits={:?} usage={:?}",
                b.limits(),
                b.usage()
            );
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
