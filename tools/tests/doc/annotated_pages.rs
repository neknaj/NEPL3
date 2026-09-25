use super::*;
#[path = "model_invariants.rs"]
mod model_invariants;
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
    page_with_lower_budget(c, id, source, route, text, &mut budget())
}

fn page_with_lower_budget(
    c: &Compiled,
    id: &str,
    source: &str,
    route: &str,
    text: &str,
    lower_budget: &mut Budget,
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
                lower_budget,
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
fn projection_profile_preserves_output_usage_and_completed_stage_order() -> Result<(), String> {
    use annotated::pages::{Stage, render_profiled};
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "a",
            "a.nepld",
            "a.md",
            "article en sentence \"Title\" body cons paragraph cons sentence \"Body\" nil nil",
        )?],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut ordinary_admission = SourceAdmission::default();
    let mut ordinary_codec =
        FoundationCodec::new(&c.doc.registry, &store, &mut ordinary_admission).map_err(err)?;
    let mut ordinary_budget = budget();
    let ordinary = render(
        &set,
        &c.doc.registry,
        &mut ordinary_codec,
        &mut ordinary_budget,
        &[&[]],
    )
    .map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut measured = budget();
    let mut stages = Vec::new();
    let profiled = render_profiled(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut measured,
        &[&[]],
        &mut |stage, usage| stages.push((stage, usage)),
    )
    .map_err(err)?;
    assert_eq!(ordinary.identity, profiled.identity);
    assert_eq!(ordinary.pages[0].markdown, profiled.pages[0].markdown);
    assert_eq!(
        ordinary.pages[0].document_digest,
        profiled.pages[0].document_digest
    );
    assert_eq!(ordinary_budget.usage(), measured.usage());
    assert_eq!(
        stages.iter().map(|(stage, _)| *stage).collect::<Vec<_>>(),
        [
            Stage::Discovery,
            Stage::Selection,
            Stage::Resolution,
            Stage::Projection
        ]
    );
    assert_eq!(
        stages.last().ok_or("missing final stage")?.1,
        measured.usage()
    );
    for pair in stages.windows(2) {
        assert!(pair[0].1.work <= pair[1].1.work);
        assert!(pair[0].1.allocation_units <= pair[1].1.allocation_units);
    }
    // Stop immediately after the completed discovery boundary. Its observation
    // remains available; no later stage is reported as completed.
    // Keep all non-Work resources equal to the normal test operation.
    let mut limited = Budget::new(Limits {
        work: stages[0].1.work,
        ..budget().limits()
    });
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut completed = Vec::new();
    assert!(
        render_profiled(
            &set,
            &c.doc.registry,
            &mut codec,
            &mut limited,
            &[&[]],
            &mut |stage, _| completed.push(stage)
        )
        .is_err()
    );
    assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
    assert_eq!(completed, [Stage::Discovery]);
    Ok(())
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
fn tutorial_chapters_project_independent_sentences_and_links() -> Result<(), String> {
    let c = compiled()?;
    let set = PageSet {
        pages: vec![
            page(
                &c,
                "tutorial-miniexpr",
                "doc/tutorial/miniexpr.nepld",
                "doc/tutorial/miniexpr.md",
                include_str!("../../../doc/tutorial/miniexpr.nepld"),
            )?,
            page(
                &c,
                "tutorial-composition",
                "doc/tutorial/composition.nepld",
                "doc/tutorial/composition.md",
                include_str!("../../../doc/tutorial/composition.nepld"),
            )?,
            page(
                &c,
                "tutorial-hello",
                "doc/tutorial/hello.nepld",
                "doc/tutorial/hello.md",
                include_str!("../../../doc/tutorial/hello.nepld"),
            )?,
        ],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let artifact = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[], &[], &[]],
    )
    .map_err(err)?;
    assert_eq!(artifact.pages.len(), 3);
    let markdown = &artifact.pages[0].markdown;
    assert_eq!(
        links(markdown),
        [
            "https://github.com/neknaj/NEPL3/blob/main/conformance/extensions/suite/README.md",
            "composition.md",
        ]
    );
    // Check the independent Sentence ruby and both original RawCode blocks.
    assert!(markdown.contains("<ruby>再帰的<rt>さいきてき</rt></ruby>"));
    assert_eq!(
        code_blocks(markdown)?,
        [
            "cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example miniexpr -- \"add 1 mul 2 3\"\n",
            "add\n├── 1\n└── mul\n    ├── 2\n    └── 3\n",
        ]
    );
    let composition = &artifact.pages[1].markdown;
    assert_eq!(
        links(composition),
        ["https://github.com/neknaj/NEPL3/blob/main/conformance/extensions/suite/README.md"]
    );
    assert!(composition.contains("<ruby>二言語<rt>にげんご</rt></ruby>"));
    assert_eq!(
        code_blocks(composition)?,
        [
            "cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example composition -- \"add framed frame neg 7 2\"\n",
            "MiniExpr: add\n├── MiniExpr: framed\n│   └── Frame: frame\n│       └── MiniExpr: neg 7\n└── MiniExpr: 2\n",
            "cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example composition -- \"framed unknown\"\ncargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example composition -- \"framed frame\"\n",
            "cargo test --locked --manifest-path conformance/extensions/hello/Cargo.toml\n",
        ]
    );
    let hello = &artifact.pages[2].markdown;
    assert_eq!(links(hello), ["miniexpr.md"]);
    assert!(hello.contains("<ruby>入力<rt>にゅうりょく</rt></ruby>"));
    assert_eq!(
        code_blocks(hello)?,
        [
            "cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example inspect -- \"hello 世界\"\n",
            "Complete; cursor=12\nroot: NodeRef(0)\nnode 0: org.example.hello::Greeting [Child(NodeRef(1))]\nnode 1: org.example.hello::Name []\n",
            "cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example inspect -- --partial \"hello \"\n",
        ]
    );
    Ok(())
}

#[test]
fn contract_chapter_projects_independent_sentences_and_invariants() -> Result<(), String> {
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "contract",
            "doc/spec/00-contract.nepld",
            "doc/spec/00-contract.md",
            include_str!("../../../doc/spec/00-contract.nepld"),
        )?],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let artifact = render(&set, &c.doc.registry, &mut codec, &mut budget(), &[&[]]).map_err(err)?;
    assert_eq!(artifact.pages.len(), 1);
    let markdown = &artifact.pages[0].markdown;
    assert!(links(markdown).is_empty());
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("<ruby>目的<rt>もくてき</rt></ruby>"));
    for annotation in [
        r"<ruby>契約<rt>けいやく</rt></ruby>\{contract\}",
        r"<ruby>拡張点<rt>かくちょうてん</rt></ruby>\{extension point\}",
        r"<ruby>束縛<rt>そくばく</rt></ruby>\{binding\}",
        r"<ruby>正本<rt>せいほん</rt></ruby>\{canonical source\}",
    ] {
        assert!(
            markdown.contains(annotation),
            "missing annotation: {annotation}"
        );
    }
    // The contract's three lists enumerate four languages, six semantic
    // boundaries and fourteen invariants. Inspect Markdown structure, retaining
    // the independently specified code operands and invariant order.
    let mut list_sizes = Vec::new();
    let mut current_list = None;
    let mut inline_codes = Vec::new();
    let mut invariants = Vec::new();
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::List(_)) => {
                assert!(current_list.is_none());
                current_list = Some(0);
            }
            Event::Start(Tag::Item) => *current_list.as_mut().ok_or("list item")? += 1,
            Event::End(pulldown_cmark::TagEnd::List(_)) => {
                list_sizes.push(current_list.take().ok_or("list end")?);
            }
            Event::Code(code) => inline_codes.push(code.into_string()),
            Event::Text(text) if text.starts_with("INV") && !text.starts_with("INV01〜") => {
                invariants.push(text.split_once('：').ok_or("invariant label")?.0.to_owned());
            }
            _ => {}
        }
    }
    assert_eq!(list_sizes, [4, 6, 14]);
    assert_eq!(
        inline_codes,
        [
            "cons",
            "nil",
            "v1",
            "Fn",
            "value",
            "splice",
            "call",
            "map",
            "then",
            "design/forms.json"
        ]
    );
    assert_eq!(
        invariants,
        [
            "INV01", "INV02", "INV03", "INV04", "INV05", "INV06", "INV07", "INV08", "INV09",
            "INV10", "INV11", "INV12", "INV13", "INV14"
        ]
    );
    Ok(())
}

#[test]
fn reproducibility_chapter_preserves_code_and_external_references() -> Result<(), String> {
    reproducibility_projection(budget(), budget(), false)
}

#[test]
fn external_extensions_chapter_preserves_layers_and_separation_conditions() -> Result<(), String> {
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "external",
            "doc/spec/22-external-extensions.nepld",
            "doc/spec/22-external-extensions.md",
            include_str!("../../../doc/spec/22-external-extensions.nepld"),
        )?],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let artifact = render(&set, &c.doc.registry, &mut codec, &mut budget(), &[&[]]).map_err(err)?;
    assert_eq!(artifact.pages.len(), 1);
    let markdown = &artifact.pages[0].markdown;
    assert!(links(markdown).is_empty());
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("<ruby>外部言語<rt>がいぶげんご</rt></ruby>"));
    let mut headings = Vec::new();
    let mut codes = Vec::new();
    let mut rows = 0;
    let mut cells = 0;
    let mut tables = 0;
    let mut lists = Vec::new();
    let mut items = 0;
    let mut cell_text = None;
    let mut table_cells = Vec::new();
    for event in Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => headings.push(level),
            Event::Code(code) => codes.push(code.into_string()),
            Event::Start(Tag::Table(alignments)) => {
                assert_eq!(alignments, [pulldown_cmark::Alignment::None; 2]);
                tables += 1;
            }
            Event::Start(Tag::TableRow) => rows += 1,
            Event::Start(Tag::TableCell) => {
                cells += 1;
                cell_text = Some(String::new());
            }
            Event::Text(text) if cell_text.is_some() => {
                cell_text.as_mut().ok_or("table cell")?.push_str(&text);
            }
            Event::End(pulldown_cmark::TagEnd::TableCell) => {
                table_cells.push(cell_text.take().ok_or("table cell end")?);
            }
            Event::Start(Tag::List(start)) => lists.push(start),
            Event::Start(Tag::Item) => items += 1,
            _ => {}
        }
    }
    // Original chapter: five architectural layers plus the header, six
    // ordered separation conditions, and four exact inline-code operands.
    assert_eq!((tables, rows, cells), (1, 5, 12));
    assert_eq!(lists, [Some(1)]);
    assert_eq!(items, 6);
    use pulldown_cmark::HeadingLevel::{H1, H2};
    assert_eq!(headings, [H1, H2, H2, H2, H2]);
    assert_eq!(
        codes,
        [
            "no_std + alloc",
            "conformance/extensions/hello/",
            "hello <name>",
            "python tools/extensions/run.py",
        ]
    );
    assert_eq!(
        table_cells
            .iter()
            .skip(2)
            .step_by(2)
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "Foundation",
            "Language infrastructure",
            "Domain / Language package",
            "Backend / Adapter",
            "Composition",
        ]
    );
    Ok(())
}

#[test]
fn html_delivery_chapter_preserves_sections_sentences_and_annotation() -> Result<(), String> {
    use nepl3_doc_core::model::DocKind;
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "html-delivery",
            "doc/spec/18-html-delivery.nepld",
            "doc/spec/18-html-delivery.md",
            include_str!("../../../doc/spec/18-html-delivery.nepld"),
        )?],
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
    let nepl3_doc_core::model::DocRoot::Article(root) = document.root else {
        return Err("article root category".into());
    };
    let DocKind::Article { body, .. } = node(root.0)? else {
        return Err("article root".into());
    };
    let DocKind::Body { blocks } = node(body.0)? else {
        return Err("article body".into());
    };
    let mut sections = Vec::new();
    let mut paragraph_sizes = Vec::new();
    for block in blocks {
        match node(block.0)? {
            DocKind::Paragraph { items } => paragraph_sizes.push(items.len()),
            DocKind::Section { id, body, .. } => {
                sections.push(id.as_str());
                let DocKind::Body { blocks } = node(body.0)? else {
                    return Err("section body".into());
                };
                for block in blocks {
                    let DocKind::Paragraph { items } = node(block.0)? else {
                        return Err("section paragraph".into());
                    };
                    paragraph_sizes.push(items.len());
                }
            }
            _ => return Err("chapter block".into()),
        }
    }
    // Original chapter: introduction, four sections, twelve paragraphs and
    // forty-two explicitly authored sentence boundaries.
    assert_eq!(
        sections,
        [
            "implementation_dependencies",
            "document_preparation",
            "math_and_browser",
            "documentation_migration"
        ]
    );
    assert_eq!(paragraph_sizes, [3, 3, 3, 3, 4, 3, 4, 4, 2, 3, 3, 7]);
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let artifact = render(&set, &c.doc.registry, &mut codec, &mut budget(), &[&[]]).map_err(err)?;
    assert_eq!(artifact.pages.len(), 1);
    let markdown = &artifact.pages[0].markdown;
    assert!(links(markdown).is_empty());
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains(r"<ruby>資源閉包<rt>しげんへいほう</rt></ruby>\{resource closure\}"));
    let headings: Vec<_> = Parser::new(markdown)
        .filter_map(|event| match event {
            Event::Start(Tag::Heading { level, .. }) => Some(level),
            _ => None,
        })
        .collect();
    use pulldown_cmark::HeadingLevel::{H1, H2};
    assert_eq!(headings, [H1, H2, H2, H2, H2]);
    Ok(())
}

#[test]
fn html_fragment_chapter_preserves_paragraphs_codes_and_references() -> Result<(), String> {
    html_fragment_projection(budget())
}

#[test]
#[ignore = "explicit HTML fragment chapter projection measurement under corpus limits"]
fn measure_html_fragment_projection_under_corpus_limits() -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Policy {
        output_limits: nepl3_tools::doc::export::pages::resources::OutputLimits,
    }
    let policy: Policy =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    html_fragment_projection(policy.output_limits.budget())
}

fn html_fragment_projection(mut render_budget: Budget) -> Result<(), String> {
    use nepl3_doc_core::model::{DocKind, DocRoot};
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "html-fragment",
            "doc/spec/19-html-fragment.nepld",
            "doc/spec/19-html-fragment.md",
            include_str!("../../../doc/spec/19-html-fragment.nepld"),
        )?],
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
        return Err("article root category".into());
    };
    let DocKind::Article { body, .. } = node(root.0)? else {
        return Err("article root".into());
    };
    let DocKind::Body { blocks } = node(body.0)? else {
        return Err("article body".into());
    };
    let mut paragraph_sizes = Vec::new();
    for block in blocks {
        let DocKind::Paragraph { items } = node(block.0)? else {
            return Err("chapter paragraph".into());
        };
        paragraph_sizes.push(items.len());
    }
    // The source authors eighteen paragraphs and 102 separate sentences.
    assert_eq!(
        paragraph_sizes,
        [3, 12, 8, 9, 8, 7, 4, 7, 5, 6, 4, 5, 3, 2, 7, 2, 4, 6]
    );
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let started = std::time::Instant::now();
    let artifact = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut render_budget,
        &[&[]],
    )
    .map_err(|error| {
        format!(
            "HTML fragment projection: {error:?}; {:?}",
            render_budget.usage()
        )
    })?;
    let finished = started.elapsed();
    eprintln!(
        "html-fragment projection elapsed_ns={} usage={:?}",
        finished.as_nanos(),
        render_budget.usage()
    );
    assert_eq!(artifact.pages.len(), 1);
    let markdown = &artifact.pages[0].markdown;
    assert!(code_blocks(markdown)?.is_empty());
    assert_eq!(
        links(markdown),
        [
            "https://www.rfc-editor.org/rfc/rfc3986#section-5.2.3",
            "https://html.spec.whatwg.org/multipage/tables.html#the-table-element",
            "https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-a-element",
            "https://url.spec.whatwg.org/",
            "https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-ruby-element",
        ]
    );
    let mut headings = Vec::new();
    let mut codes = Vec::new();
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => headings.push(level),
            Event::Code(code) => codes.push(code.into_string()),
            _ => {}
        }
    }
    assert_eq!(headings, [pulldown_cmark::HeadingLevel::H1]);
    assert_eq!(
        codes,
        [
            "nepl3.safe-markup/2",
            "nepl3.markup",
            "interfaces/markup.json",
            "nepl3-markup",
            "[a-z][a-z0-9-]*",
            "[a-z][a-z0-9-]*",
            "-_.",
            ".",
            "..",
            "BetweenArtifacts(source,target,fragment)",
            "../",
            "#fragment",
            "docs/a/index.html",
            "docs/b/index.html",
            "../b/index.html",
            "../",
            "BetweenArtifacts",
        ]
    );
    assert!(markdown.contains("<ruby>構造契約<rt>こうぞうけいやく</rt></ruby>"));
    Ok(())
}

#[test]
fn circuit_chapter_preserves_operations_lists_and_state_notation() -> Result<(), String> {
    circuit_projection(budget())
}

#[test]
#[ignore = "explicit Circuit chapter projection measurement under corpus limits"]
fn measure_circuit_projection_under_corpus_limits() -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Policy {
        output_limits: nepl3_tools::doc::export::pages::resources::OutputLimits,
    }
    let policy: Policy =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    circuit_projection(policy.output_limits.budget())
}

fn circuit_projection(mut render_budget: Budget) -> Result<(), String> {
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "circuit",
            "doc/spec/07-circuit.nepld",
            "doc/spec/07-circuit.md",
            include_str!("../../../doc/spec/07-circuit.nepld"),
        )?],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let started = std::time::Instant::now();
    let artifact = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut render_budget,
        &[&[]],
    )
    .map_err(|error| format!("Circuit projection: {error:?}; {:?}", render_budget.usage()))?;
    let finished = started.elapsed();
    eprintln!(
        "circuit projection elapsed_ns={} usage={:?}",
        finished.as_nanos(),
        render_budget.usage()
    );
    assert_eq!(artifact.pages.len(), 1);
    let markdown = &artifact.pages[0].markdown;
    assert!(links(markdown).is_empty());
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("<ruby>同期離散時間<rt>どうきりさんじかん</rt></ruby>"));
    let mut headings = Vec::new();
    let mut codes = Vec::new();
    let mut lists = Vec::new();
    let mut current_list = None;
    let mut visible = String::new();
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => headings.push(level),
            Event::Code(code) => codes.push(code.into_string()),
            Event::Text(text) => visible.push_str(&text),
            Event::Start(Tag::List(start)) => {
                assert!(current_list.is_none());
                current_list = Some((start, 0));
            }
            Event::Start(Tag::Item) => current_list.as_mut().ok_or("list item")?.1 += 1,
            Event::End(pulldown_cmark::TagEnd::List(_)) => {
                lists.push(current_list.take().ok_or("list end")?);
            }
            _ => {}
        }
    }
    // Declaration kinds are unordered; elaboration stages start at one.
    assert_eq!(lists, [(None, 5), (Some(1), 6)]);
    use pulldown_cmark::HeadingLevel::{H1, H2};
    assert_eq!(headings, [H1, H2, H2, H2, H2, H2, H2, H2, H2, H2]);
    assert_eq!(
        codes,
        [
            "(width, unsigned value)",
            "0 <= value < 2^width",
            "wire name expr",
            "state name width initial",
            "next name expr",
            "next q q",
            "inst name module arguments",
            "output name width expr",
            "[lo,lo+width)",
            "initial(PreparedNetlist)",
            "step",
            "observe",
        ]
    );
    for expression in [
        "not a = nor a a。",
        "or a b = not (nor a b)。",
        "and a b = nor (not a) (not b)。",
        "xor a b = nor (nor a b) (and a b)。",
        "mux = (select AND yes) OR ((NOT select) AND no)。",
        "(q_{k+1},y_k)",
    ] {
        assert!(
            visible.contains(expression),
            "missing expression: {expression}"
        );
    }
    Ok(())
}

/// The ordinary test retains desktop limits. This opt-in measurement locates
/// the expense and checks content under the pre-existing corpus policy.
#[test]
#[ignore = "explicit reproducibility projection measurement under corpus limits"]
fn measure_reproducibility_projection_under_corpus_limits() -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Policy {
        output_limits: nepl3_tools::doc::export::pages::resources::OutputLimits,
    }
    let policy: Policy =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    reproducibility_projection(
        policy.output_limits.budget(),
        policy.output_limits.budget(),
        true,
    )
}

fn reproducibility_projection(
    mut lower_budget: Budget,
    mut render_budget: Budget,
    measure_encoding: bool,
) -> Result<(), String> {
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page_with_lower_budget(
            &c,
            "reproducibility",
            "doc/spec/13-reproducibility.nepld",
            "doc/spec/13-reproducibility.md",
            include_str!("../../../doc/spec/13-reproducibility.nepld"),
            &mut lower_budget,
        )?],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    eprintln!("reproducibility lower usage={:?}", lower_budget.usage());
    let started = std::time::Instant::now();
    let mut prepared = None;
    let artifact = annotated::pages::render_profiled(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut render_budget,
        &[&[]],
        &mut |stage, usage| {
            let elapsed = started.elapsed();
            eprintln!(
                "reproducibility stage={stage:?} elapsed_ns={} usage={usage:?}",
                elapsed.as_nanos()
            );
            if stage == annotated::pages::Stage::Resolution {
                prepared = Some((elapsed, usage));
            }
        },
    )
    .map_err(err)?;
    let finished = started.elapsed();
    let (elapsed, usage) = prepared.ok_or("preparation observation")?;
    eprintln!(
        "reproducibility namespace elapsed_ns={} usage={usage:?}",
        elapsed.as_nanos()
    );
    eprintln!(
        "reproducibility projection elapsed_ns={} usage={:?}",
        finished.as_nanos(),
        render_budget.usage()
    );
    assert_eq!(artifact.pages.len(), 1);
    let markdown = &artifact.pages[0].markdown;
    assert_eq!(
        links(markdown),
        [
            "https://www.rfc-editor.org/rfc/rfc3986.html#section-3.1",
            "https://www.rfc-editor.org/rfc/rfc3987.html",
        ]
    );
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("<ruby>再現性<rt>さいげんせい</rt></ruby>"));
    // These are the independently transcribed code operands, including literal
    // Unicode escapes, quotation marks and HTML entities, in document order.
    let codes: Vec<_> = Parser::new(markdown)
        .filter_map(|event| match event {
            Event::Code(code) => Some(code.into_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        codes,
        [
            "[A-Za-z][A-Za-z0-9+.-]*:",
            "%",
            r"\u00xx",
            "NEPL3-SCHEMA-1",
            "NEPL3-PACKAGE-1",
            "CheckedLanguagePackage::semantic_json",
            "interfaces/engine.json",
            r#"["ListOf",cons,nil,element]"#,
            r#"["Builtin",reader,kind,tokenKind]"#,
            "[SchemaRef,localKind]",
            "[SchemaRef,name]",
            "[selector,SchemaRef,name,fallback]",
            "[name,skipReaders,takePairs]",
            "[category,spelling,kind,fields,binding,styles]",
            "[category,kind,tokenKind,payloadType,binding,styles]",
            "[defaultUnexpected,rules]",
            "[category,unexpected,synchronization]",
            "[ancestorCategory,kind,spellingOrNull]",
            "NEPL3-PACKAGE-EXECUTION-1",
            "NEPL3-ENVIRONMENT-1",
            "&",
            "<",
            ">",
            "&amp;",
            "&lt;",
            "&gt;",
            "]]>",
            "\"",
            "&quot;",
            "&#xD;",
            "&#x9;",
            "&#xA;",
            "&#xD;",
        ]
    );
    let headings: Vec<_> = Parser::new(markdown)
        .filter_map(|event| match event {
            Event::Start(Tag::Heading { level, .. }) => Some(level),
            _ => None,
        })
        .collect();
    use pulldown_cmark::HeadingLevel::{H1, H2, H3};
    assert_eq!(headings, [H1, H2, H2, H3, H3, H2, H2, H2]);
    if measure_encoding {
        measure_page_encoding("reproducibility", &set, &c, render_budget.limits())?;
    }
    Ok(())
}

fn code_blocks(markdown: &str) -> Result<Vec<String>, String> {
    let mut blocks = Vec::new();
    let mut code = None;
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code = Some(String::new()),
            Event::Text(text) if code.is_some() => {
                code.as_mut().ok_or("code block")?.push_str(&text);
            }
            Event::End(pulldown_cmark::TagEnd::CodeBlock) => {
                blocks.push(code.take().ok_or("code block end")?);
            }
            _ => {}
        }
    }
    Ok(blocks)
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
        measure_page_encoding("architecture", &set, &c, render_budget.limits())?;
    }
    Ok(())
}

/// Encoding starts with cold source admission. Hashing uses that encoded NDF
/// with an independent Budget. Neither run's usage is subtracted from the
/// enclosing projection's cumulative usage.
fn measure_page_encoding(
    label: &str,
    set: &PageSet,
    c: &Compiled,
    limits: nepl3_core::budget::Limits,
) -> Result<(), String> {
    assert_eq!(set.pages.len(), 1, "single-page measurement");
    let mut sources_in_single_node_guest = 0;
    let mut maps_in_single_node_guest = 0;
    let mut source_occurrences = 0;
    let mut source_text_bytes = 0;
    let mut distinct_sources = std::collections::BTreeMap::new();
    let mut mapping_occurrences = 0;
    for embed in &set.pages[0].document.value.embeds {
        if let nepl3_doc_core::model::DocContent::Syntax { closure } = &embed.content {
            let bundle = &closure.syntax.bundle;
            // Root guest bundles only: nested bundles are already represented
            // within each root's encoding and are not counted a second time.
            mapping_occurrences += bundle.source_maps.len();
            for source in &bundle.sources {
                source_occurrences += 1;
                source_text_bytes += source.text().len();
                if let Some(prior) = distinct_sources.insert(source.identity(), source) {
                    assert_eq!(prior.uri(), source.uri());
                    assert_eq!(prior.text(), source.text());
                }
            }
            if bundle.nodes.len() == 1 {
                sources_in_single_node_guest =
                    sources_in_single_node_guest.max(bundle.sources.len());
                maps_in_single_node_guest = maps_in_single_node_guest.max(bundle.source_maps.len());
            }
        }
    }
    println!(
        "{label} single_node_guest max_sources={sources_in_single_node_guest} max_maps={maps_in_single_node_guest}"
    );
    let unique_text_bytes: usize = distinct_sources.values().map(|s| s.text().len()).sum();
    println!(
        "{label} root_guest_sources occurrences={source_occurrences} distinct={} text_bytes={source_text_bytes} unique_text_bytes={unique_text_bytes} mappings={mapping_occurrences}",
        distinct_sources.len()
    );
    // Isolated component run with fresh admission, not a subtraction from
    // the enclosing projection. Keep the same immutable input and limits.
    let store = SourceStore::default();
    let mut b = Budget::new(limits);
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    // Measure the shared exchange independently. Native references are collected
    // outside the timed boundary; no bundle is cloned or source scope pruned.
    {
        use nepl3_core::value_codec::FoundationValueCodec;
        let bundles: Vec<_> = set.pages[0]
            .document
            .value
            .embeds
            .iter()
            .filter_map(|embed| match &embed.content {
                nepl3_doc_core::model::DocContent::Syntax { closure } => {
                    Some(&closure.syntax.bundle)
                }
                nepl3_doc_core::model::DocContent::Value { .. } => None,
            })
            .collect();
        let mut shared_admission = SourceAdmission::default();
        let mut validation_budget = Budget::new(limits);
        let started = std::time::Instant::now();
        for bundle in &bundles {
            bundle
                .validate_with_sources(
                    &c.doc.registry,
                    &mut validation_budget,
                    &mut shared_admission,
                )
                .map_err(err)?;
        }
        println!(
            "{label} shared_native_validation elapsed_ns={} usage={:?}",
            started.elapsed().as_nanos(),
            validation_budget.usage()
        );
        let mut shared_admission = SourceAdmission::default();
        let mut shared_codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut shared_admission).map_err(err)?;
        let mut shared_budget = Budget::new(limits);
        let started = std::time::Instant::now();
        let shared = shared_codec
            .encode_syntax_set(&bundles, &mut shared_budget)
            .map_err(err)?;
        let elapsed = started.elapsed();
        println!(
            "{label} shared_root_bundles count={} elapsed_ns={} nodes={} usage={:?}",
            bundles.len(),
            elapsed.as_nanos(),
            value_nodes(&shared),
            shared_budget.usage()
        );
    }
    let started = std::time::Instant::now();
    let encoded =
        nepl3_doc_core::portable::pages::set_to_value(set, &c.doc.registry, &mut codec, &mut b)
            .map_err(err)?;
    let elapsed = started.elapsed();
    println!(
        "{label} isolated_encoding elapsed_ns={} usage={:?}",
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
            "{label} document_field={index} nodes={}",
            value_nodes(field)
        );
    }
    for (index, field) in value.fields.iter().enumerate() {
        println!(
            "{label} doc_value_field={index} nodes={}",
            value_nodes(field)
        );
    }
    // Attribute member references and body fields separately. Shared content
    // tables are reported above as DocValue fields 4 and 5.
    let mut bundle_fields = [0_usize; 7];
    let mut token_fields = [0_usize; 5];
    let mut token_records = std::collections::BTreeMap::new();
    let mut bundles = 0;
    for embed in embeds {
        let NdfValue::Record(embed) = embed else {
            return Err("DocEmbed record".into());
        };
        let Some(NdfValue::Variant(content)) = embed.fields.get(1) else {
            return Err("DocContent variant".into());
        };
        if content.variant == "Value" {
            continue;
        }
        assert_eq!(content.variant, "Syntax");
        let Some(NdfValue::Record(closure)) = content.fields.first() else {
            return Err("DocClosure record".into());
        };
        let Some(NdfValue::Record(bundle)) = closure.fields.get(3) else {
            return Err("SyntaxBundle record".into());
        };
        assert_eq!(bundle.schema, *codec.foundation_schema());
        assert_eq!(bundle.kind, "SharedSyntaxBundle");
        assert_eq!(bundle.fields.len(), 3);
        let NdfValue::Record(body) = &bundle.fields[2] else {
            return Err("SyntaxBody".into());
        };
        assert_eq!(body.kind, "SyntaxBody");
        assert_eq!(body.fields.len(), 5);
        let fields = [
            &bundle.fields[0],
            &body.fields[0],
            &body.fields[1],
            &body.fields[2],
            &body.fields[3],
            &body.fields[4],
            &bundle.fields[1],
        ];
        bundles += 1;
        for (total, field) in bundle_fields.iter_mut().zip(fields) {
            *total += value_nodes(field);
        }
        let NdfValue::List(tokens) = &body.fields[4] else {
            return Err("tokens list".into());
        };
        for token in tokens {
            let NdfValue::Record(token) = token else {
                return Err("Token record".into());
            };
            assert_eq!(token.schema, *codec.foundation_schema());
            assert_eq!(token.kind, "Token");
            assert_eq!(token.fields.len(), token_fields.len());
            for (total, field) in token_fields.iter_mut().zip(&token.fields) {
                *total += value_nodes(field);
            }
            for (part, root) in [("payload", &token.fields[2]), ("views", &token.fields[3])] {
                let mut pending = vec![root];
                while let Some(value) = pending.pop() {
                    match value {
                        NdfValue::Record(record) => {
                            *token_records
                                .entry((part, record.schema.package.as_str(), record.kind.as_str()))
                                .or_insert(0_usize) += 1;
                            pending.extend(&record.fields);
                        }
                        NdfValue::Variant(variant) => pending.extend(&variant.fields),
                        NdfValue::List(values) => pending.extend(values),
                        NdfValue::Some(value) => pending.push(value),
                        _ => {}
                    }
                }
            }
        }
    }
    println!(
        "{label} root_guest_member_fields bundles={bundles} source_refs={} nodes={} origins={} root={} environments={} tokens={} mapping_refs={}",
        bundle_fields[0],
        bundle_fields[1],
        bundle_fields[2],
        bundle_fields[3],
        bundle_fields[4],
        bundle_fields[5],
        bundle_fields[6]
    );
    println!(
        "{label} root_guest_token_fields kind={} head={} payload={} views={} trivia={}",
        token_fields[0], token_fields[1], token_fields[2], token_fields[3], token_fields[4]
    );
    for ((part, package, kind), count) in token_records {
        println!("{label} token_records part={part} package={package} kind={kind} count={count}");
    }
    let mut inputs = vec![CanonicalDigestInput {
        domain: nepl3_doc_core::prepare::DOCUMENT_DOMAIN,
        value: document,
    }];
    inputs.extend(embeds.iter().map(|value| CanonicalDigestInput {
        domain: nepl3_doc_core::prepare::GUEST_DOMAIN,
        value,
    }));
    let mut digest_budget = Budget::new(limits);
    let started = std::time::Instant::now();
    let digests = codec
        .canonical_value_digests(&inputs, &mut digest_budget)
        .map_err(err)?;
    let elapsed = started.elapsed();
    println!(
        "{label} isolated_digests elapsed_ns={} requests={} usage={:?}",
        elapsed.as_nanos(),
        inputs.len(),
        digest_budget.usage()
    );
    assert_eq!(digests.len(), inputs.len());
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
