use super::*;
use annotated::pages::{MathSurfaces, render_footnotes_svg_math};

fn view(c: &Compiled, text: &str) -> Result<annotated::pages::PagesArtifact, String> {
    let set = PageSet {
        pages: vec![page(c, "math", "math.nepld", "math.md", text)?],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    render_footnotes_svg_math(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[]],
        MathSurfaces {
            math: &c.others[0].schema,
            sentence: Some(&c.others[3].schema),
            doc: Some(&c.doc.package.schema),
        },
    )
    .map_err(err)
}

#[test]
fn explicit_math_page_preserves_titles_body_and_footnote_order() -> Result<(), String> {
    let c = compiled()?;
    let result = view(
        &c,
        r#"article en sentence cons text "Formula " cons math Math frac 1 0 nil body
      cons paragraph cons sentence cons anno math Math frac 1 0 cons concat cons text "Explanation " cons math Math add x y nil nil
        cons text " then " cons math Math frac 2 3 nil nil
      cons display Math frac 1 0 nil"#,
    )?;
    let md = &result.pages[0].markdown;
    assert!(md.starts_with("# Formula $`\\frac{1}{0}`$\n\n"), "{md}");
    assert!(
        md.contains("$`\\frac{1}{0}`$[^nepl3-anno-1] then $`\\frac{2}{3}`$"),
        "{md}"
    );
    assert!(md.contains("```math\n\\frac{1}{0}\n```"), "{md}");
    assert!(md.contains("Explanation"), "{md}");
    assert_eq!(md.matches("[^nepl3-anno-1]:").count(), 1);
    let mut math_spans = 0;
    let mut references = 0;
    let mut definitions = 0;
    for event in Parser::new_ext(md, pulldown_cmark::Options::ENABLE_FOOTNOTES) {
        match event {
            Event::Code(_) => math_spans += 1,
            Event::FootnoteReference(_) => references += 1,
            Event::Start(Tag::FootnoteDefinition(_)) => definitions += 1,
            _ => {}
        }
    }
    assert_eq!((math_spans, references, definitions), (4, 1, 1));

    Ok(())
}

#[test]
fn explicit_math_page_rejects_unrepresentable_and_html_nested_math() -> Result<(), String> {
    let c = compiled()?;
    for body in [
        r#"cons display Math label x Sentence "[字/じ]" nil"#,
        r#"cons code Math frac 1 0 nil"#,
        r#"cons paragraph cons sentence cons ruby math Math frac 1 0 text "reading" nil nil nil"#,
        r#"cons paragraph cons sentence cons link external "https://example.com" math Math frac 1 0 nil nil nil"#,
        r#"cons display Math sequence nil nil"#,
    ] {
        assert!(
            view(&c, &format!("article en \"T\" body {body}")).is_err(),
            "{body}"
        );
    }
    Ok(())
}

#[test]
fn math_tables_keep_cells_and_reject_pipe_fences() -> Result<(), String> {
    let c = compiled()?;
    let template = |expression: &str| {
        format!(
            r#"article en "Table" body cons table cons default nil some row cons "H" nil cons row cons sentence cons math Math {expression} nil nil nil nil"#
        )
    };
    let result = view(&c, &template("frac 1 0"))?;
    let md = &result.pages[0].markdown;
    assert!(md.contains("| $`\\frac{1}{0}`$ |"), "{md}");
    assert_eq!(
        Parser::new_ext(md, pulldown_cmark::Options::ENABLE_TABLES)
            .filter(|e| matches!(e, Event::Start(Tag::TableCell)))
            .count(),
        2
    );
    assert!(view(&c, &template(r#"fence "|" "|" x"#)).is_err());
    Ok(())
}

#[test]
fn math_page_rejects_orphans_and_preserves_cumulative_budget() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"article en "Math" body cons display Math frac 1 0 cons display Math add x y cons display Math mul x y nil"#;
    let set = PageSet {
        pages: vec![page(&c, "a", "a.nepld", "a.md", source)?],
        files: vec![],
    };
    let render = |set: &PageSet, b: &mut Budget| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission)
            .map_err(|e| Error::Invalid(format!("{e:?}")))?;
        render_footnotes_svg_math(
            set,
            &c.doc.registry,
            &mut codec,
            b,
            &[&[]],
            MathSurfaces {
                math: &c.others[0].schema,
                sentence: Some(&c.others[3].schema),
                doc: Some(&c.doc.package.schema),
            },
        )
    };
    let mut measured = budget();
    let expected = render(&set, &mut measured).map_err(err)?;
    let mut at_host_depth = budget();
    let elevated = at_host_depth
        .with_depth_at_least(5, |b| render(&set, b))
        .map_err(err)?;
    assert_eq!(elevated.pages[0].markdown, expected.pages[0].markdown);
    assert_eq!(at_host_depth.current_depth(), 0);
    assert!(at_host_depth.usage().depth >= measured.usage().depth + 5);
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
    ] {
        let mut limits = budget().limits();
        let usage = measured.usage();
        let cap = match reason {
            StopReason::WorkLimit => &mut limits.work,
            StopReason::AllocationLimit => &mut limits.allocation_units,
            _ => &mut limits.output_bytes,
        };
        *cap = match reason {
            StopReason::WorkLimit => usage.work,
            StopReason::AllocationLimit => usage.allocation_units,
            _ => usage.output_bytes,
        };
        let mut exact = Budget::new(limits);
        assert_eq!(
            render(&set, &mut exact).map_err(err)?.pages[0].markdown,
            expected.pages[0].markdown
        );
        assert_eq!(exact.usage(), usage);
        match reason {
            StopReason::WorkLimit => limits.work -= 1,
            StopReason::AllocationLimit => limits.allocation_units -= 1,
            _ => limits.output_bytes -= 1,
        }
        let mut limited = Budget::new(limits);
        assert!(matches!(render(&set, &mut limited), Err(Error::Stopped(s)) if s == reason));
        assert_eq!(limited.poll(), Err(reason));
        assert!(matches!(render(&set, &mut limited), Err(Error::Stopped(s)) if s == reason));
    }
    for reason in [
        StopReason::NodeLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = budget().limits();
        if reason == StopReason::NodeLimit {
            limits.nodes = 0;
        }
        if reason == StopReason::DepthLimit {
            limits.depth = 0;
        }
        let mut stopped = Budget::new(limits);
        if reason == StopReason::Cancelled {
            stopped.cancel();
        }
        assert!(matches!(render(&set, &mut stopped), Err(Error::Stopped(s)) if s == reason));
        assert_eq!(stopped.poll(), Err(reason));
    }
    let mut unused = set.clone();
    let duplicate = unused.pages[0].document.value.embeds[0].clone();
    unused.pages[0].document.value.embeds.push(duplicate);
    assert!(render(&unused, &mut budget()).is_err());
    let mut unreachable = set.clone();
    let math = unreachable.pages[0]
        .document
        .value
        .nodes
        .iter()
        .find(|n| matches!(n.kind, nepl3_doc_core::model::DocKind::DisplayMath { .. }))
        .ok_or("math node")?
        .clone();
    unreachable.pages[0].document.value.nodes.push(math);
    assert!(render(&unreachable, &mut budget()).is_err());
    let mut bad_origin = set.clone();
    bad_origin.pages[0].document.value.nodes[0].origin =
        Some(nepl3_core::origin::OriginId(u64::MAX));
    assert!(render(&bad_origin, &mut budget()).is_err());
    Ok(())
}

#[test]
fn math_capability_preserves_non_math_svg_output() -> Result<(), String> {
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(&c, "a", "a.nepld", "a.md", r#"article en "Image" body cons image asset "figure" none "Alt" some "{Caption/Note}" nil"#)?],
        files: vec![PageFile { registration: PageRegistration { id: "figure".into(), source: "f.svg".into(), route: "f.svg".into() }, content: FileBytes(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0 L10 10"/></svg>"#.to_vec()) }],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let old = annotated::pages::render_footnotes_svg(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[]],
    )
    .map_err(err)?;
    let new = render_footnotes_svg_math(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[]],
        MathSurfaces {
            math: &c.others[0].schema,
            sentence: Some(&c.others[3].schema),
            doc: Some(&c.doc.package.schema),
        },
    )
    .map_err(err)?;
    assert_eq!(old.pages[0].markdown, new.pages[0].markdown);
    assert_eq!(old.pages[0].document_digest, new.pages[0].document_digest);
    assert_eq!(old.identity, new.identity);
    assert!(new.math_dependencies[0].is_empty());
    assert_eq!(
        old.image_dependencies[0][0].digest,
        new.image_dependencies[0][0].digest
    );
    let hidden = PageSet {
        pages: vec![page(
            &c,
            "a",
            "a.nepld",
            "a.md",
            r#"article en "Hidden" body cons image asset "figure" none sentence cons anno text "Alt" cons math Math frac 1 0 nil nil none nil"#,
        )?],
        files: set.files.clone(),
    };
    let mut hidden_admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&c.doc.registry, &store, &mut hidden_admission).map_err(err)?;
    let hidden_node = hidden.pages[0]
        .document
        .value
        .nodes
        .iter()
        .position(|n| matches!(n.kind, nepl3_doc_core::model::DocKind::InlineMath { .. }))
        .ok_or("hidden math")? as u64;
    assert!(
        matches!(render_footnotes_svg_math(&hidden, &c.doc.registry, &mut codec, &mut budget(), &[&[]], MathSurfaces {
        math: &c.others[0].schema, sentence: Some(&c.others[3].schema), doc: Some(&c.doc.package.schema),
    }), Err(Error::Unsupported { node }) if node == hidden_node)
    );
    Ok(())
}

#[test]
fn page_guest_batch_preserves_standalone_guest_identities_and_stops() -> Result<(), String> {
    use nepl3_core::{budget::StopReason, value_codec::FoundationValueCodec};
    use nepl3_doc_core::{pages, prepare};
    let c = compiled()?;
    let set = PageSet {
        pages: vec![
            page(
                &c,
                "empty",
                "empty.nepld",
                "empty.md",
                r#"article en "Empty" body nil"#,
            )?,
            page(
                &c,
                "one",
                "one.nepld",
                "one.md",
                r#"article en "One" body cons display Math frac 5 6 nil"#,
            )?,
            page(
                &c,
                "first",
                "first.nepld",
                "first.md",
                r#"article en "First" body cons display Math frac 1 2 cons display Math add x y nil"#,
            )?,
            page(
                &c,
                "second",
                "second.nepld",
                "second.md",
                r#"article en "Second" body cons display Math mul x y cons display Math frac 3 4 nil"#,
            )?,
        ],
        files: vec![],
    };
    let run = |input: &PageSet, b: &mut Budget| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        pages::resolve(input, &c.doc.registry, &mut codec, b)
            .map(|checked| checked.into_plan())
            .map_err(err)
    };
    let mut measured = budget();
    let plan = run(&set, &mut measured)?;
    let mut actual = plan.remaining.iter();
    for (page_index, page) in set.pages.iter().enumerate() {
        let mut store = SourceStore::default();
        for snapshot in &page.document.sources {
            store
                .insert_with_budget(snapshot.clone(), &mut budget())
                .map_err(err)?;
        }
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        for (index, embed) in page.document.value.embeds.iter().enumerate() {
            assert!(!embed.closure.owner_sources.is_empty());
            let value = codec
                .encode_foreign_closure(&embed.closure, &mut budget())
                .map_err(err)?;
            let expected = codec
                .canonical_value_digest(prepare::GUEST_DOMAIN, &value, &mut budget())
                .map_err(err)?;
            let entry = actual.next().ok_or("missing guest requirement")?;
            assert_eq!(entry.page, page_index as u64);
            assert_eq!(
                entry.requirement,
                prepare::DocRequirement::Foreign {
                    embed: nepl3_doc_core::model::EmbedRef(index as u64),
                    kind: embed.kind,
                    guest_digest: expected,
                }
            );
        }
    }
    assert!(actual.next().is_none());
    for (resource, used) in [
        (Resource::Work, measured.usage().work),
        (Resource::AllocationUnits, measured.usage().allocation_units),
        (Resource::OutputBytes, measured.usage().output_bytes),
    ] {
        let mut limits = budget().limits();
        match resource {
            Resource::Work => limits.work = used,
            Resource::AllocationUnits => limits.allocation_units = used,
            Resource::OutputBytes => limits.output_bytes = used,
            _ => return Err("unexpected resource".into()),
        }
        assert_eq!(run(&set, &mut Budget::new(limits))?, plan);
        match resource {
            Resource::Work => limits.work -= 1,
            Resource::AllocationUnits => limits.allocation_units -= 1,
            Resource::OutputBytes => limits.output_bytes -= 1,
            _ => return Err("unexpected resource".into()),
        }
        let mut limited = Budget::new(limits);
        assert!(run(&set, &mut limited).is_err());
        let reason = match resource {
            Resource::Work => StopReason::WorkLimit,
            Resource::AllocationUnits => StopReason::AllocationLimit,
            Resource::OutputBytes => StopReason::OutputLimit,
            _ => return Err("unexpected resource".into()),
        };
        assert_eq!(limited.poll(), Err(reason));
        let usage = limited.usage();
        assert!(run(&set, &mut limited).is_err());
        assert_eq!(limited.usage(), usage);
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(run(&set, &mut cancelled).is_err());
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn checked_page_alt_text_reuses_admission_without_external_resolutions() -> Result<(), String> {
    use nepl3_core::budget::StopReason;
    use nepl3_doc_core::{model::DocKind, pages, text};
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "alt",
            "alt.nepld",
            "alt.md",
            r#"article en "Image" body cons image asset "figure" none sentence cons anno text "Alt" cons math Math frac 1 2 nil nil none nil"#,
        )?],
        files: vec![],
    };
    let original = set.clone();
    let document = &set.pages[0].document;
    let alt = document
        .value
        .nodes
        .iter()
        .find_map(|node| match node.kind {
            DocKind::Image { alt, .. } => Some(alt),
            _ => None,
        })
        .ok_or("image alt")?;
    for policy in [
        text::AnnotationPolicy::BaseOnly,
        text::AnnotationPolicy::WithReadings,
        text::AnnotationPolicy::WithAllNotes,
    ] {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let mut b = budget();
        let checked = pages::resolve(&set, &c.doc.registry, &mut codec, &mut b).map_err(err)?;
        assert!(core::ptr::eq(
            checked
                .document_structure(0, &mut b)
                .map_err(err)?
                .document(),
            document
        ));
        assert!(matches!(
            checked.document_structure(u64::MAX, &mut b),
            Err(pages::PageProjectionError::MissingPage)
        ));
        let mismatch = b.with_depth_at_least(1, |b| checked.document_structure(0, b).map(|_| ()));
        assert_eq!(mismatch, Err(pages::PageProjectionError::ScopeMismatch));
        let reply = checked
            .project_unresolved_text(0, alt, policy, &mut b)
            .map_err(err)?;
        let mut standalone = budget();
        let expected = text::plain_text_borrowed(
            document,
            alt,
            policy,
            &[],
            &c.doc.registry,
            &mut codec,
            &mut standalone,
        )
        .map_err(err)?;
        assert_eq!(reply.outcome, expected.outcome);
        assert_eq!(reply.report.usage, b.usage());
        let invalid_sentence = checked
            .project_unresolved_text(
                0,
                nepl3_doc_core::model::SentenceRef(u64::MAX),
                policy,
                &mut b,
            )
            .map_err(err)?;
        assert!(matches!(
            invalid_sentence.outcome,
            text::PlainTextOutcome::Invalid {
                error: text::PlainTextFailure::ExpectedSentence { .. }
            }
        ));
        if policy == text::AnnotationPolicy::WithAllNotes {
            assert!(matches!(
                reply.outcome,
                text::PlainTextOutcome::Invalid {
                    error: text::PlainTextFailure::UnresolvedEmbed { .. }
                }
            ));
        }
        if policy == text::AnnotationPolicy::BaseOnly {
            assert_eq!(
                reply.outcome,
                text::PlainTextOutcome::Complete { text: "Alt".into() }
            );
        }
        assert_eq!(
            checked.project_unresolved_text(u64::MAX, alt, policy, &mut b),
            Err(pages::PageProjectionError::MissingPage)
        );
        let mismatch =
            b.with_depth_at_least(1, |b| checked.project_unresolved_text(0, alt, policy, b));
        assert_eq!(mismatch, Err(pages::PageProjectionError::ScopeMismatch));
        let mut limits = b.limits();
        limits.work -= 1;
        let mismatch = b.with_ceiling(limits, |b| {
            checked.project_unresolved_text(0, alt, policy, b)
        });
        assert_eq!(mismatch, Err(pages::PageProjectionError::ScopeMismatch));
        b.cancel();
        assert_eq!(
            checked.project_unresolved_text(0, alt, policy, &mut b),
            Err(pages::PageProjectionError::Stopped(StopReason::Cancelled))
        );
    }
    let mut invalid = set.clone();
    invalid.pages[0].document.value.embeds[0]
        .closure
        .owner_environment
        .digest = nepl3_core::source::Digest([0; 32]);
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    assert!(pages::resolve(&invalid, &c.doc.registry, &mut codec, &mut budget()).is_err());
    let mut invalid = set.clone();
    invalid.pages[0].document.value.embeds[0]
        .closure
        .owner_sources
        .clear();
    assert!(pages::resolve(&invalid, &c.doc.registry, &mut codec, &mut budget()).is_err());
    assert_eq!(set, original);
    Ok(())
}
