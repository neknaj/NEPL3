use super::*;
use nepl3_doc_core::{check::Category, lower, pages::*};
use nepl3_tools::doc::projection::{
    Error,
    annotated::{self, pages::render},
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
            .map_err(err)
        },
    )?;
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
fn adjacent_lists_in_page_set_keep_links_and_separate_numbering() -> Result<(), String> {
    let c = compiled()?;
    let a = page(
        &c,
        "a",
        "source/a.md",
        "docs/a.md",
        r#"article en "A" body
        cons list ordered 7 cons item none body cons paragraph cons sentence
          cons link page "b" some "use" text "B" nil nil nil nil
        cons list ordered 42 cons item none body cons paragraph cons "Second" nil nil nil nil"#,
    )?;
    let b = page(
        &c,
        "b",
        "source/b.md",
        "docs/nested/b.md",
        r#"article en "B" body cons section use "Use" body nil nil"#,
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
        r#"article en "A" body
      cons paragraph cons sentence
        cons link page "b" some "use" text "B"
        cons text " and " cons link relative "a.md" none text "self"
        cons text " and " cons link relative "legacy.md" none text "legacy" nil nil nil"#,
    )?;
    let b = page(
        &c,
        "b",
        "source/b.md",
        "docs/nested/b.md",
        r#"article en "B" body
      cons section use "Use" body cons paragraph cons sentence
        cons link page "a" none ruby text "戻" text "もど" nil nil nil nil"#,
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
    assert!(output.pages[1].markdown.contains("戻\\[もど\\]"));
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
        Err(Error::NeedsResolution)
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
            "article en \"A\" body cons paragraph cons sentence cons link {target} text \"link\" nil nil nil"
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
            r#"article en "A" body cons paragraph cons sentence cons link page "a" none text "self" nil nil nil"#,
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
    let artifact = render(&set, &c.doc.registry, &mut codec, &mut budget(), &[&[]]).map_err(err)?;
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
    Ok(())
}

#[test]
fn annotated_page_set_rejects_partial_render_and_ambiguous_registration() -> Result<(), String> {
    let c = compiled()?;
    let a = page(&c, "a", "a.md", "a.md", r#"article en "A" body nil"#)?;
    let b = page(
        &c,
        "b",
        "b.md",
        "b.md",
        r#"article en "B" body cons paragraph cons parallel cons variant en "B" cons variant ja "B" nil nil nil"#,
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
