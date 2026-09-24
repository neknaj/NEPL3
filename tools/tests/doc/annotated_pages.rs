use super::*;
#[path = "namespace_projection.rs"]
mod namespace_projection;
use nepl3_doc_core::{check::Category, lower, pages::*};
use nepl3_tools::doc::projection::{
    Error,
    annotated::{
        self,
        pages::{render, render_observed},
    },
};
use pulldown_cmark::{Event, Parser, Tag};

fn page(
    c: &Compiled,
    id: &str,
    source: &str,
    route: &str,
    text: &str,
) -> Result<PageDocument, String> {
    let document = nepl3_tools::doc::source::with_named_input(
        true,
        c,
        text,
        id,
        "Article",
        |tree, profile, _, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            lower::document(
                tree.syntax(),
                &c.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(|error| format!("page {id} lower: {error:?}"))
        },
    )
    .map_err(|error| format!("page {id} input: {error}"))?;
    Ok(PageDocument {
        registration: PageRegistration {
            id: id.into(),
            source: source.into(),
            route: route.into(),
        },
        document,
    })
}

fn links(markdown: &str) -> Vec<String> {
    Parser::new(markdown)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn page_projection_reuses_validation_and_matches_standalone_digest() -> Result<(), String> {
    let c = compiled()?;
    let store = SourceStore::default();
    for count in [8, 16, 32] {
        let source = format!(
            "article en sentence \"A\" body {} nil",
            "cons paragraph cons sentence \"A structured document with repeated independent paragraphs.\" nil ".repeat(count)
        );
        let set = PageSet {
            pages: vec![page(&c, "a", "a.md", "a.md", &source)?],
            files: vec![],
        };
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let mut resolved = budget();
        let checked = resolve(&set, &c.doc.registry, &mut codec, &mut resolved).map_err(err)?;
        let digest = checked.document_digest(0).ok_or("missing digest")?;
        // This is the old redundant work, in the same warm admission state.
        let mut repeated = budget();
        let standalone = annotated::render(
            &set.pages[0].document,
            &c.doc.registry,
            &mut codec,
            &mut repeated,
            &[],
        )
        .map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let mut actual = budget();
        let mut prepared = None;
        let output = render_observed(
            &set,
            &c.doc.registry,
            &mut codec,
            &mut actual,
            &[&[]],
            &mut |usage| prepared = Some(usage),
        )
        .map_err(err)?;
        assert_eq!(output.pages[0].markdown, standalone.markdown);
        assert_eq!(output.pages[0].document_digest, standalone.document_digest);
        assert_eq!(output.pages[0].document_digest, digest);
        let extra = actual
            .usage()
            .work
            .checked_sub(prepared.ok_or("missing preparation measurement")?.work)
            .ok_or("missing resolution work")?;
        // The writer must cost less than repeating complete preparation plus
        // that same writer. Regressing to per-page inspect violates this bound.
        assert!(
            extra < repeated.usage().work / 2,
            "{count} paragraphs: extra={extra}, repeated={}",
            repeated.usage().work
        );
    }
    Ok(())
}

#[test]
fn page_projection_scales_with_pages_without_rescanning_all_links() -> Result<(), String> {
    let c = compiled()?;
    let store = SourceStore::default();
    // Vary page count independently of links per page. Internal links exercise
    // Doc namespace resolution; external links belong to Sentence. Empty pages between
    // populated pages exercise advancing across gaps without losing entries.
    for (internal, external) in [(0, 1), (1, 0), (3, 2)] {
        let mut pages = Vec::new();
        for index in 0..32 {
            let id = format!("p{index:02}");
            let source = if index % 4 == 1 {
                "article en sentence \"Empty\" body nil".to_owned()
            } else {
                format!(
                    "article en sentence \"A\" body cons paragraph cons sentence sentence {}{}nil nil nil",
                    "cons doc link page \"p00\" none text \"Local\" ".repeat(internal),
                    "cons link \"https://example.com/\" text \"External\" ".repeat(external)
                )
            };
            pages.push(page(
                &c,
                &id,
                &format!("{id}.nepld"),
                &format!("{id}.md"),
                &source,
            )?);
        }
        let mut work = Vec::new();
        for count in [8, 16, 32] {
            let set = PageSet {
                pages: pages[..count].to_vec(),
                files: vec![],
            };
            // Isolate projection work from resolver lookup costs. Each run
            // starts with a fresh codec/admission and the same immutable input.
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
            let mut resolved = budget();
            let proof = resolve(&set, &c.doc.registry, &mut codec, &mut resolved).map_err(err)?;
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
            let mut actual = budget();
            let aliases = vec![&[][..]; count];
            let mut prepared = None;
            let output = render_observed(
                &set,
                &c.doc.registry,
                &mut codec,
                &mut actual,
                &aliases,
                &mut |usage| prepared = Some(usage),
            )
            .map_err(err)?;
            assert_eq!(output.pages.len(), count);
            for (index, artifact) in output.pages.iter().enumerate() {
                let expected = if index % 4 == 1 {
                    vec![]
                } else {
                    let mut links = vec!["p00.md".to_owned(); internal];
                    links.extend(vec!["https://example.com/".to_owned(); external]);
                    links
                };
                assert_eq!(links(&artifact.markdown), expected);
                assert_eq!(
                    Some(artifact.document_digest),
                    proof.document_digest(index as u64)
                );
            }
            work.push(
                actual
                    .usage()
                    .work
                    .checked_sub(prepared.ok_or("missing preparation measurement")?.work)
                    .ok_or("resolution work mismatch")?,
            );
        }
        // Identical fixed-width pages are repeated in groups of four. Their
        // writer cost is linear; a small allowance covers terminal cursor
        // comparisons. Whole-plan filtering instead adds P*(L+R) work and
        // exceeds this bound even with just one link per populated page.
        assert!(
            work[1] <= 2 * work[0] + 16,
            "{internal}/{external}: {work:?}"
        );
        assert!(
            work[2] <= 2 * work[1] + 16,
            "{internal}/{external}: {work:?}"
        );
    }
    Ok(())
}

#[test]
fn html_pages_consume_ordered_links_once_across_empty_pages() -> Result<(), String> {
    use nepl3_doc_html::{ParallelMode, RenderOptions, pages::PagesHtmlRequest};
    use nepl3_markup::html::HtmlHref;
    let c = compiled()?;
    for (internal, external) in [(0, 1), (1, 0), (3, 2)] {
        let mut pages = Vec::new();
        for index in 0..32 {
            let id = format!("p{index:02}");
            let input = if index % 4 == 1 {
                "article en sentence \"Empty\" body nil".to_owned()
            } else {
                format!(
                    "article en sentence \"A\" body cons paragraph cons sentence sentence {}{}nil nil nil",
                    "cons doc link page \"p00\" none text \"Local\" ".repeat(internal),
                    "cons link \"https://example.com/\" text \"External\" ".repeat(external)
                )
            };
            pages.push(page(
                &c,
                &id,
                &format!("{id}.nepld"),
                &format!("{id}.html"),
                &input,
            )?);
        }
        let mut work = Vec::new();
        let mut validation = Vec::new();
        for count in [8, 16, 32] {
            let request = PagesHtmlRequest {
                set: PageSet {
                    pages: pages[..count].to_vec(),
                    files: vec![],
                },
                options: RenderOptions {
                    parallel: ParallelMode::Rows,
                },
            };
            let (hrefs, render_work) = namespace_projection::html_links(&c, &request, &pages)?;
            assert_eq!(hrefs.len(), count);
            for (index, actual) in hrefs.iter().enumerate() {
                let mut expected = Vec::new();
                if index % 4 != 1 {
                    expected.extend((0..internal).map(|_| HtmlHref::BetweenArtifacts {
                        source: format!("p{index:02}.html"),
                        target: "p00.html".into(),
                        fragment: None,
                    }));
                    expected.extend((0..external).map(|_| HtmlHref::External {
                        uri: "https://example.com/".into(),
                    }));
                }
                assert_eq!(actual, &expected);
            }
            work.push(render_work.composition);
            validation.push(render_work.output_validation);
        }
        // Repeat fixed-width groups of four pages with a fixed admitted source
        // pool. Measure composition only; preparation and final output checking
        // have separate costs. A small allowance covers terminal cursor checks.
        assert!(
            work[1] <= 2 * work[0] + 16,
            "{internal}/{external}: {work:?}"
        );
        assert!(
            work[2] <= 2 * work[1] + 16,
            "{internal}/{external}: {work:?}"
        );
        // Complete output checking builds a route index and performs binary
        // lookup. Doubling admits O(P log P), while composition stays linear.
        assert!(validation[1] < 3 * validation[0], "{validation:?}");
        assert!(validation[2] < 3 * validation[1], "{validation:?}");
    }
    Ok(())
}

#[test]
fn checked_pages_still_reject_unsafe_external_and_guest_requirements() -> Result<(), String> {
    let c = compiled()?;
    let store = SourceStore::default();
    for inline in [
        r#"link "javascript:alert(1)" text "bad""#,
        "doc math Math frac 1 0",
    ] {
        let source = format!(
            "article en sentence \"A\" body cons paragraph cons sentence sentence cons {inline} nil nil nil"
        );
        let set = PageSet {
            pages: vec![page(&c, "a", "a.md", "a.md", &source)?],
            files: vec![],
        };
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let failure = render(&set, &c.doc.registry, &mut codec, &mut budget(), &[&[]]);
        assert!(
            matches!(
                failure,
                Err(Error::Sentence {
                    issue: nepl3_tools::doc::projection::SentenceIssue::Text,
                    ..
                } | Error::NeedsResolution)
            ),
            "{failure:?}"
        );
    }
    Ok(())
}

#[test]
fn adjacent_lists_in_page_set_keep_links_and_separate_numbering() -> Result<(), String> {
    let c = compiled()?;
    let a = page(
        &c,
        "a",
        "source/a.md",
        "docs/a.md",
        r#"article en sentence "A" body
        cons list ordered 7 cons item none body cons paragraph cons sentence sentence
          cons doc link page "b" some "use" text "B" nil nil nil nil
        cons list ordered 42 cons item none body cons paragraph cons sentence "Second" nil nil nil nil"#,
    )?;
    let b = page(
        &c,
        "b",
        "source/b.md",
        "docs/nested/b.md",
        r#"article en sentence "B" body cons section use sentence "Use" body nil nil"#,
    )?;
    let set = PageSet {
        pages: vec![a, b],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let output = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[], &[]],
    )
    .map_err(err)?;
    let markdown = &output.pages[0].markdown;
    assert_eq!(links(markdown), ["nested/b.md#n-757365"]);
    let starts: Vec<_> = Parser::new(markdown)
        .filter_map(|e| match e {
            Event::Start(Tag::List(start)) => Some(start),
            _ => None,
        })
        .collect();
    assert_eq!(starts, [Some(7), Some(42)]);
    assert_eq!(markdown.matches("<!-- -->").count(), 1);
    Ok(())
}

#[test]
fn annotated_page_set_resolves_mutual_self_and_passive_file_links() -> Result<(), String> {
    let c = compiled()?;
    let a = page(
        &c,
        "a",
        "source/a.md",
        "docs/a.md",
        r#"article en sentence "A" body
      cons paragraph cons sentence sentence
        cons doc link page "b" some "use" text "B"
        cons text " and " cons doc link relative "a.md" none text "self"
        cons text " and " cons doc link relative "legacy.md" none text "legacy" nil nil nil"#,
    )?;
    let b = page(
        &c,
        "b",
        "source/b.md",
        "docs/nested/b.md",
        r#"article en sentence "B" body
      cons section use sentence "Use" body cons paragraph cons sentence sentence
        cons doc link page "a" none ruby text "戻" text "もど" nil nil nil nil"#,
    )?;
    let mut set = PageSet {
        pages: vec![a, b],
        files: vec![PageFile {
            registration: PageRegistration {
                id: "legacy".into(),
                source: "source/legacy.md".into(),
                route: "reference/legacy.md".into(),
            },
            content: FileBytes(b"# Legacy\n".to_vec()),
        }],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let output = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[], &[]],
    )
    .map_err(err)?;
    assert_eq!(
        links(&output.pages[0].markdown),
        ["nested/b.md#n-757365", "a.md", "../reference/legacy.md"]
    );
    assert_eq!(links(&output.pages[1].markdown), ["../a.md"]);
    assert!(
        output.pages[1]
            .markdown
            .contains("<a name=\"n-757365\"></a>")
    );
    assert!(
        output.pages[1]
            .markdown
            .contains("<ruby>戻<rt>もど</rt></ruby>")
    );
    // Passive bytes belong to context identity even though they are not parsed.
    set.files[0].content.0.push(b'!');
    let changed = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[], &[]],
    )
    .map_err(err)?;
    assert_ne!(output.identity, changed.identity);
    assert_eq!(output.pages[0].markdown, changed.pages[0].markdown);
    assert!(matches!(
        annotated::render(
            &set.pages[0].document,
            &c.doc.registry,
            &mut codec,
            &mut budget(),
            &[]
        ),
        Err(Error::Sentence {
            issue: nepl3_tools::doc::projection::SentenceIssue::Unsupported,
            ..
        })
    ));
    Ok(())
}

#[test]
fn annotated_page_set_rejects_missing_file_fragments_and_unsafe_targets() -> Result<(), String> {
    let c = compiled()?;
    let store = SourceStore::default();
    for (target, expected) in [
        ("relative \"missing.md\" none", "MissingPage"),
        ("relative \"legacy.md\" some \"heading\"", "FileFragment"),
        ("page \"legacy\" none", "MissingPage"),
        ("page \"a\" some \"missing\"", "MissingFragment"),
        ("relative \"../../escape.md\" none", "InvalidRelative"),
    ] {
        // Each source is a separate operation, not a conflicting revision in
        // the previous operation's source admission namespace.
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let input = format!(
            "article en sentence \"A\" body cons paragraph cons sentence sentence cons doc link {target} text \"link\" nil nil nil"
        );
        let set = PageSet {
            pages: vec![page(&c, "a", "source/a.md", "docs/a.md", &input)?],
            files: vec![PageFile {
                registration: PageRegistration {
                    id: "legacy".into(),
                    source: "source/legacy.md".into(),
                    route: "legacy.md".into(),
                },
                content: FileBytes(b"# heading".to_vec()),
            }],
        };
        let failure = render(&set, &c.doc.registry, &mut codec, &mut budget(), &[&[]])
            .err()
            .ok_or("unexpected success")?;
        assert!(
            matches!(failure, Error::Invalid(ref value) if value.contains(expected)),
            "{failure:?}"
        );
    }
    Ok(())
}

#[test]
fn annotated_page_set_keeps_stops_sticky_and_returns_no_partial_output() -> Result<(), String> {
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "a",
            "a.md",
            "a.md",
            r#"article en sentence "A" body cons paragraph cons sentence sentence cons doc link page "a" none text "self" nil nil nil"#,
        )?],
        files: vec![],
    };
    let original = set.clone();
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut success = budget();
    render(&set, &c.doc.registry, &mut codec, &mut success, &[&[]]).map_err(err)?;
    for resource in [
        Resource::Work,
        Resource::AllocationUnits,
        Resource::OutputBytes,
        Resource::Nodes,
    ] {
        let mut limits = budget().limits();
        let used = success.usage();
        match resource {
            Resource::Work => limits.work = used.work - 1,
            Resource::AllocationUnits => limits.allocation_units = used.allocation_units - 1,
            Resource::OutputBytes => limits.output_bytes = used.output_bytes - 1,
            Resource::Nodes => limits.nodes = used.nodes - 1,
            _ => return Err("unexpected test resource".into()),
        }
        let mut limited = Budget::new(limits);
        // Match the cold admission state used by the reference execution.
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let failed = render(&set, &c.doc.registry, &mut codec, &mut limited, &[&[]]);
        assert!(
            matches!(failed, Err(Error::Stopped(_))),
            "{resource:?}: {failed:?}"
        );
        let usage = limited.usage();
        let again = render(&set, &c.doc.registry, &mut codec, &mut limited, &[&[]]);
        assert_eq!(format!("{failed:?}"), format!("{again:?}"));
        assert_eq!(usage, limited.usage());
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(matches!(
        render(&set, &c.doc.registry, &mut codec, &mut cancelled, &[&[]]),
        Err(Error::Stopped(StopReason::Cancelled))
    ));
    assert_eq!(set, original);
    Ok(())
}

#[test]
fn architecture_draft_projects_with_explicit_current_markdown_dependency() -> Result<(), String> {
    architecture_projection(budget(), false)
}

/// Explicit measurement under the existing corpus policy. The ordinary test
/// above keeps the desktop allowance and continues to expose its Work failure.
#[test]
#[ignore = "explicit architecture projection measurement under corpus limits"]
fn measure_architecture_projection_under_corpus_limits() -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Policy {
        output_limits: nepl3_tools::doc::export::pages::resources::OutputLimits,
    }
    let policy: Policy =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    architecture_projection(policy.output_limits.budget(), true)
}

fn architecture_projection(
    mut render_budget: Budget,
    measure_encoding: bool,
) -> Result<(), String> {
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "architecture",
            "doc/spec/01-architecture.md",
            "doc/spec/01-architecture.md",
            include_str!("../../../doc/spec/01-architecture.nepld"),
        )?],
        files: vec![PageFile {
            registration: PageRegistration {
                id: "external".into(),
                source: "doc/spec/22-external-extensions.md".into(),
                route: "doc/spec/22-external-extensions.md".into(),
            },
            content: FileBytes(
                include_bytes!("../../../doc/spec/22-external-extensions.md").to_vec(),
            ),
        }],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut prepared = None;
    let started = std::time::Instant::now();
    let mut prepared_at = None;
    let artifact = render_observed(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut render_budget,
        &[&[]],
        &mut |usage| {
            prepared = Some(usage);
            prepared_at = Some(started.elapsed());
        },
    )
    .map_err(|error| {
        format!(
            "architecture projection: {error:?}; prepared={prepared:?}; {:?}",
            render_budget.usage()
        )
    })?;
    let finished = started.elapsed();
    println!(
        "architecture namespace elapsed_ns={} usage={:?}",
        prepared_at.ok_or("preparation time")?.as_nanos(),
        prepared.ok_or("preparation usage")?
    );
    println!(
        "architecture projection elapsed_ns={} usage={:?}",
        finished.as_nanos(),
        render_budget.usage()
    );
    assert_eq!(
        links(&artifact.pages[0].markdown),
        ["22-external-extensions.md"]
    );
    let visible: String = Parser::new(&artifact.pages[0].markdown)
        .filter_map(|event| match event {
            Event::Text(text) | Event::Code(text) => Some(text.into_string()),
            _ => None,
        })
        .collect();
    assert!(visible.contains("doc-coreはmath-coreをimportしない。"));
    // This is Markdown generation only: passive Markdown is never an HTML page.
    assert_eq!(artifact.pages.len(), 1);
    if measure_encoding {
        let mut sources_in_single_node_guest = 0;
        let mut maps_in_single_node_guest = 0;
        for embed in &set.pages[0].document.value.embeds {
            if let nepl3_doc_core::model::DocContent::Syntax { closure } = &embed.content {
                let bundle = &closure.syntax.bundle;
                if bundle.nodes.len() == 1 {
                    sources_in_single_node_guest =
                        sources_in_single_node_guest.max(bundle.sources.len());
                    maps_in_single_node_guest =
                        maps_in_single_node_guest.max(bundle.source_maps.len());
                }
            }
        }
        println!(
            "architecture single_node_guest max_sources={sources_in_single_node_guest} max_maps={maps_in_single_node_guest}"
        );
        // Isolated component run with fresh admission, not a subtraction from
        // the enclosing projection. Keep the same immutable input and limits.
        let mut b = Budget::new(render_budget.limits());
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let started = std::time::Instant::now();
        let encoded = nepl3_doc_core::portable::pages::set_to_value(
            &set,
            &c.doc.registry,
            &mut codec,
            &mut b,
        )
        .map_err(err)?;
        let elapsed = started.elapsed();
        println!(
            "architecture isolated_encoding elapsed_ns={} usage={:?}",
            elapsed.as_nanos(),
            b.usage()
        );
        use nepl3_core::value::NdfValue;
        use nepl3_core::value_codec::{CanonicalDigestInput, FoundationValueCodec};
        let NdfValue::Record(value) = &encoded else {
            return Err("PageSet".into());
        };
        let Some(NdfValue::List(pages)) = value.fields.first() else {
            return Err("pages".into());
        };
        let Some(NdfValue::Record(page)) = pages.first() else {
            return Err("PageDocument".into());
        };
        let document = page.fields.get(1).ok_or("DocumentSyntax")?;
        let NdfValue::Record(record) = document else {
            return Err("DocumentSyntax record".into());
        };
        let Some(NdfValue::Record(value)) = record.fields.first() else {
            return Err("DocValue".into());
        };
        let Some(NdfValue::List(embeds)) = value.fields.get(2) else {
            return Err("embeds".into());
        };
        for (index, field) in record.fields.iter().enumerate() {
            println!(
                "architecture document_field={index} nodes={}",
                value_nodes(field)
            );
        }
        for (index, field) in value.fields.iter().enumerate() {
            println!(
                "architecture doc_value_field={index} nodes={}",
                value_nodes(field)
            );
        }
        let mut inputs = vec![CanonicalDigestInput {
            domain: nepl3_doc_core::prepare::DOCUMENT_DOMAIN,
            value: document,
        }];
        inputs.extend(embeds.iter().map(|value| CanonicalDigestInput {
            domain: nepl3_doc_core::prepare::GUEST_DOMAIN,
            value,
        }));
        let mut digest_budget = Budget::new(render_budget.limits());
        let started = std::time::Instant::now();
        let digests = codec
            .canonical_value_digests(&inputs, &mut digest_budget)
            .map_err(err)?;
        let elapsed = started.elapsed();
        println!(
            "architecture isolated_digests elapsed_ns={} requests={} usage={:?}",
            elapsed.as_nanos(),
            inputs.len(),
            digest_budget.usage()
        );
        assert_eq!(digests.len(), inputs.len());
    }
    Ok(())
}

fn value_nodes(value: &nepl3_core::value::NdfValue) -> usize {
    use nepl3_core::value::NdfValue;
    let mut pending = vec![value];
    let mut count = 0;
    while let Some(value) = pending.pop() {
        count += 1;
        match value {
            NdfValue::List(values) => pending.extend(values),
            NdfValue::Record(value) => pending.extend(&value.fields),
            NdfValue::Variant(value) => pending.extend(&value.fields),
            NdfValue::Some(value) => pending.push(value),
            NdfValue::Unit
            | NdfValue::Bool(_)
            | NdfValue::U64(_)
            | NdfValue::Integer(_)
            | NdfValue::Rational(_)
            | NdfValue::Text(_)
            | NdfValue::Bytes(_)
            | NdfValue::None => {}
        }
    }
    count
}

#[test]
fn annotated_page_set_rejects_partial_render_and_ambiguous_registration() -> Result<(), String> {
    let c = compiled()?;
    let a = page(
        &c,
        "a",
        "a.md",
        "a.md",
        r#"article en sentence "A" body nil"#,
    )?;
    let b = page(
        &c,
        "b",
        "b.md",
        "b.md",
        r#"article en sentence "B" body cons paragraph cons parallel cons variant en sentence "B" cons variant ja sentence "B" nil nil nil"#,
    )?;
    let mut set = PageSet {
        pages: vec![a, b],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    assert!(matches!(
        render(
            &set,
            &c.doc.registry,
            &mut codec,
            &mut budget(),
            &[&[], &[]]
        ),
        Err(Error::Unsupported { .. })
    ));
    assert!(matches!(
        render(&set, &c.doc.registry, &mut codec, &mut budget(), &[&[]]),
        Err(Error::Invalid(_))
    ));
    set.pages[1].registration.route = "a.md".into();
    let failure = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[], &[]],
    )
    .err()
    .ok_or("accepted colliding routes")?;
    assert!(
        matches!(failure, Error::Invalid(ref s) if s.contains("Collision")),
        "{failure:?}"
    );
    Ok(())
}

#[test]
fn independent_guests_keep_namespace_links_and_owner_errors() -> Result<(), String> {
    let c = compiled()?;
    let text = r#"article en sentence "A" body cons paragraph cons sentence sentence
      cons doc anchor mark doc link page "b" some "use" text "First"
      cons text " / " cons doc link page "b" some "use" text "Second" nil nil nil"#;
    let target = r#"article en sentence "B" body cons section use sentence "Use" body nil nil"#;
    let make = |text: &str| -> Result<PageSet, String> {
        Ok(PageSet {
            pages: vec![
                page(&c, "a", "a.md", "a.md", text)?,
                page(&c, "b", "b.md", "b.md", target)?,
            ],
            files: vec![],
        })
    };
    let set = make(text)?;
    let store = SourceStore::default();
    let run = |b: &mut Budget| {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission)
            .map_err(|e| Error::Invalid(err(e)))?;
        render(&set, &c.doc.registry, &mut codec, b, &[&[], &[]])
    };
    let mut measured = budget();
    let output = run(&mut measured).map_err(err)?;
    let usage = measured.usage();
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
    ] {
        for deficit in [0, 1] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = usage.work - deficit,
                StopReason::AllocationLimit => {
                    limits.allocation_units = usage.allocation_units - deficit
                }
                StopReason::NodeLimit => limits.nodes = usage.nodes - deficit,
                StopReason::DepthLimit => limits.depth = usage.depth - deficit,
                _ => unreachable!(),
            }
            let mut limited = Budget::new(limits);
            let result = run(&mut limited);
            if deficit == 0 {
                assert_eq!(
                    result.map_err(err)?.pages[0].markdown,
                    output.pages[0].markdown
                );
            } else {
                assert!(
                    matches!(result, Err(Error::Stopped(actual)) if actual == reason),
                    "{reason:?}: {result:?}"
                );
                let used = limited.usage();
                assert!(
                    matches!(run(&mut limited), Err(Error::Stopped(actual)) if actual == reason)
                );
                assert_eq!(limited.usage(), used);
            }
        }
    }
    assert_eq!(
        links(&output.pages[0].markdown),
        ["b.md#n-757365", "b.md#n-757365"]
    );
    assert!(
        output.pages[0]
            .markdown
            .contains("<a name=\"n-6d61726b\"></a>")
    );
    let visible: String = Parser::new(&output.pages[0].markdown)
        .filter_map(|event| match event {
            Event::Text(value) => Some(value.into_string()),
            _ => None,
        })
        .collect();
    assert_eq!(visible, "AFirst / Second");
    // Root 0 -> Anchor 1 -> Link 2; the second root-level Link is member 3.
    assert_eq!(
        output.dependencies[0]
            .iter()
            .map(|d| (d.member, d.node))
            .collect::<Vec<_>>(),
        [(2, 0), (3, 0)]
    );
    for (label, depth) in [("First", 2), ("Second", 1)] {
        let invalid = text.replace(
            &format!("text \"{label}\""),
            &format!("text \"{label}\\t\""),
        );
        let invalid = make(&invalid)?;
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let error = render(
            &invalid,
            &c.doc.registry,
            &mut codec,
            &mut budget(),
            &[&[], &[]],
        )
        .err()
        .ok_or("control character in a selected guest label was accepted")?;
        let Error::Guest { owner, cause } = error else {
            return Err(format!("missing owner: {error:?}"));
        };
        assert!(matches!(*cause, Error::Sentence { node: 0, .. }));
        assert_eq!(owner.page, 0);
        assert_eq!(owner.slot.0, if depth == 2 { 0 } else { 1 });
        assert_eq!(owner.embed.0, if depth == 2 { 0 } else { 1 });
        let mut path = Some(owner.as_ref());
        let mut actual = 0;
        while let Some(owner) = path {
            actual += 1;
            path = owner.parent.as_deref();
        }
        assert_eq!(actual, depth);
        if let Some(root) = &owner.parent {
            assert_eq!((root.page, root.slot.0, root.embed.0), (0, 1, 0));
            assert!(root.parent.is_none());
        }
    }
    Ok(())
}

#[test]
fn shared_sentence_guests_keep_occurrences_and_resolve_forward_references() -> Result<(), String> {
    use nepl3_doc_core::model::DocKind;
    let c = compiled()?;
    let store = SourceStore::default();
    let project = |set: &PageSet| {
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        render(set, &c.doc.registry, &mut codec, &mut budget(), &[&[]]).map_err(err)
    };
    let mut set = PageSet {
        pages: vec![page(
            &c,
            "a",
            "a.md",
            "a.md",
            r#"article en sentence "A" body cons paragraph cons sentence sentence cons doc link page "a" none text "Again" nil nil nil"#,
        )?],
        files: vec![],
    };
    for node in &mut set.pages[0].document.value.nodes {
        if let DocKind::Paragraph { items } = &mut node.kind {
            items.push(items[0]);
        }
    }
    let output = project(&set)?;
    assert_eq!(links(&output.pages[0].markdown), ["a.md", "a.md"]);
    assert_eq!(
        output.dependencies[0]
            .iter()
            .map(|link| (link.member, link.node))
            .collect::<Vec<_>>(),
        [(1, 0), (2, 0)]
    );
    let source = r#"article en sentence "A" body
        cons paragraph cons sentence sentence cons doc ref mark text "Forward" nil nil
        cons paragraph cons sentence sentence cons doc anchor mark text "Target" nil nil nil"#;
    let set = PageSet {
        pages: vec![page(&c, "a", "a.md", "a.md", source)?],
        files: vec![],
    };
    let output = project(&set)?;
    assert_eq!(links(&output.pages[0].markdown), ["#n-6d61726b"]);
    assert!(
        output.pages[0]
            .markdown
            .contains("<a name=\"n-6d61726b\"></a>Target")
    );
    for (source, reason) in [
        (source.replace("ref mark", "ref missing"), "Unresolved"),
        (source.replace("ref mark", "anchor mark"), "Duplicate"),
    ] {
        let set = PageSet {
            pages: vec![page(&c, "a", "a.md", "a.md", &source)?],
            files: vec![],
        };
        let error = project(&set)
            .err()
            .ok_or("invalid namespace was accepted")?;
        assert!(
            error.starts_with(&format!("Invalid(\"{reason} {{")),
            "{error}"
        );
    }
    Ok(())
}

#[test]
fn guest_error_identifies_the_page_with_the_same_local_path() -> Result<(), String> {
    let c = compiled()?;
    let good = r#"article en sentence "A" body cons paragraph cons sentence sentence cons doc link page "a" none text "Label" nil nil nil"#;
    let bad = r#"article en sentence "A" body cons paragraph cons sentence sentence cons doc link page "a" none text "Label\t" nil nil nil"#;
    let set = PageSet {
        pages: vec![
            page(&c, "a", "a.md", "a.md", good)?,
            page(&c, "b", "b.md", "b.md", bad)?,
        ],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let error = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[], &[]],
    )
    .err()
    .ok_or("second page label was accepted")?;
    let Error::Guest { owner, cause } = error else {
        return Err(format!("missing owner: {error:?}"));
    };
    assert_eq!((owner.page, owner.slot.0, owner.embed.0), (1, 1, 0));
    assert!(owner.parent.is_none());
    assert!(matches!(
        *cause,
        Error::Sentence {
            embed: nepl3_doc_core::model::EmbedRef(0),
            node: 0,
            ..
        }
    ));
    Ok(())
}
