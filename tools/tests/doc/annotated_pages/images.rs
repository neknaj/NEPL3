use super::*;
use nepl3_tools::doc::projection::annotated::pages::render_footnotes_svg;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0 L10 10" stroke="#000" fill="none"/></svg>"##;

fn fixture(c: &Compiled, source: &str) -> Result<PageSet, String> {
    Ok(PageSet {
        pages: vec![page(c, "a", "a.nepld", "nested/a.md", source)?],
        files: vec![PageFile {
            registration: PageRegistration {
                id: "triangle".into(),
                source: "triangle.svg".into(),
                route: "assets/triangle.svg".into(),
            },
            content: FileBytes(SVG.as_bytes().to_vec()),
        }],
    })
}

fn render_svg(c: &Compiled, set: &PageSet) -> Result<annotated::pages::PagesArtifact, Error> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission)
        .map_err(|e| Error::Invalid(format!("{e:?}")))?;
    render_footnotes_svg(set, &c.doc.registry, &mut codec, &mut budget(), &[&[]])
}

#[test]
fn static_svg_uses_base_only_alt_relative_route_and_caption_footnotes() -> Result<(), String> {
    let c = compiled()?;
    let set = fixture(
        &c,
        r#"article ja "SVG" body cons image asset "triangle" none "{[三角形/さんかくけい]/triangle}" some "{図/caption note}" cons paragraph cons sentence cons image asset "triangle" none sentence cons text "[]|&<>\\" cons break cons text "next" nil nil nil nil"#,
    )?;
    let before = set.clone();
    let output = render_svg(&c, &set).map_err(err)?;
    let markdown = &output.pages[0].markdown;
    assert!(markdown.contains("![三角形](<../assets/triangle.svg>)"));
    assert!(!markdown.contains("さんかくけい"));
    assert!(!markdown.contains("{triangle}"));
    assert!(markdown.contains("caption note"));
    assert_eq!(output.image_dependencies[0].len(), 2);
    assert!(
        output.image_dependencies[0]
            .iter()
            .all(|d| d.digest == Digest::of(SVG.as_bytes()))
    );
    let mut destinations = Vec::new();
    let mut image_texts = Vec::new();
    let mut in_image = false;
    let mut footnote_references = 0;
    let mut footnote_definitions = 0;
    for event in Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_FOOTNOTES) {
        match event {
            Event::FootnoteReference(_) => footnote_references += 1,
            Event::Start(Tag::FootnoteDefinition(_)) => footnote_definitions += 1,
            Event::Start(Tag::Image { dest_url, .. }) => {
                destinations.push(dest_url.into_string());
                in_image = true;
            }
            Event::End(pulldown_cmark::TagEnd::Image) => in_image = false,
            Event::Text(s) if in_image => image_texts.push(s.into_string()),
            _ => {}
        }
    }
    assert_eq!(
        destinations,
        ["../assets/triangle.svg", "../assets/triangle.svg"]
    );
    assert_eq!(image_texts.concat(), "三角形[]|&<>\\\nnext");
    assert_eq!((footnote_references, footnote_definitions), (1, 1));
    assert_eq!(set, before);
    Ok(())
}

#[test]
fn static_svg_shared_asset_records_each_pages_actual_dependency() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"article en "SVG" body cons image asset "triangle" none "alt" none nil"#;
    let mut set = fixture(&c, source)?;
    set.pages.push(page(&c, "b", "b.nepld", "b.md", source)?);
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let output = render_footnotes_svg(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &[&[], &[]],
    )
    .map_err(err)?;
    for (index, page) in set.pages.iter().enumerate() {
        let node = page
            .document
            .value
            .nodes
            .iter()
            .position(|n| matches!(n.kind, nepl3_doc_core::model::DocKind::Image { .. }))
            .ok_or("image missing")?;
        assert_eq!(output.image_dependencies[index].len(), 1);
        let actual = &output.image_dependencies[index][0];
        assert_eq!(actual.node, node as u64);
        assert_eq!(actual.asset_id, "triangle");
        assert_eq!(actual.route, "assets/triangle.svg");
        assert_eq!(actual.digest, Digest::of(SVG.as_bytes()));
    }
    assert!(output.pages[0].markdown.contains("../assets/triangle.svg"));
    assert!(output.pages[1].markdown.contains("(<assets/triangle.svg>)"));
    Ok(())
}

#[test]
fn static_svg_rejects_unregistered_unused_invalid_and_digest_mismatch() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"article en "SVG" body cons image asset "triangle" none "alt" none nil"#;
    let set = fixture(&c, source)?;
    let mut missing = set.clone();
    missing.files.clear();
    assert!(render_svg(&c, &missing).is_err());
    for bytes in [
        "<svg/>".as_bytes().to_vec(),
        vec![0xff],
        format!("<?xml-stylesheet href='https://example.invalid/x'?>{SVG}").into_bytes(),
        vec![b' '; 262145],
    ] {
        let mut invalid = set.clone();
        invalid.files[0].content = FileBytes(bytes);
        assert!(render_svg(&c, &invalid).is_err());
    }
    let unused = fixture(&c, r#"article en "Empty" body nil"#)?;
    assert!(render_svg(&c, &unused).is_err());
    let mut wrong_digest = set.clone();
    for node in &mut wrong_digest.pages[0].document.value.nodes {
        if let nepl3_doc_core::model::DocKind::Image { asset, .. } = &mut node.kind {
            asset.digest = Some(Digest::of(b"different"));
        }
    }
    assert!(
        matches!(render_svg(&c, &wrong_digest), Err(Error::Invalid(message)) if message == "SVG digest mismatch")
    );
    Ok(())
}

#[test]
fn static_svg_projection_preserves_operation_resource_boundaries() -> Result<(), String> {
    use nepl3_core::budget::{Limits, StopReason};
    let c = compiled()?;
    let set = fixture(
        &c,
        r#"article en "SVG" body cons image asset "triangle" none "alt" some "{caption/note}" nil"#,
    )?;
    let run = |limits: Limits| -> Result<(_, _), String> {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let mut b = Budget::new(limits);
        let result = render_footnotes_svg(&set, &c.doc.registry, &mut codec, &mut b, &[&[]]);
        Ok((result, b))
    };
    let (result, baseline) = run(budget().limits())?;
    result.map_err(err)?;
    let used = baseline.usage();
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
        StopReason::DepthLimit,
        StopReason::NodeLimit,
        StopReason::SourceLimit,
    ] {
        let mut exact = baseline.limits();
        let count = match reason {
            StopReason::WorkLimit => {
                exact.work = used.work;
                used.work
            }
            StopReason::AllocationLimit => {
                exact.allocation_units = used.allocation_units;
                used.allocation_units
            }
            StopReason::OutputLimit => {
                exact.output_bytes = used.output_bytes;
                used.output_bytes
            }
            StopReason::DepthLimit => {
                exact.depth = used.depth;
                used.depth
            }
            StopReason::NodeLimit => {
                exact.nodes = used.nodes;
                used.nodes
            }
            _ => {
                exact.source_bytes = used.source_bytes;
                used.source_bytes
            }
        };
        assert!(count > 0);
        run(exact)?.0.map_err(err)?;
        match reason {
            StopReason::WorkLimit => exact.work -= 1,
            StopReason::AllocationLimit => exact.allocation_units -= 1,
            StopReason::OutputLimit => exact.output_bytes -= 1,
            StopReason::DepthLimit => exact.depth -= 1,
            StopReason::NodeLimit => exact.nodes -= 1,
            _ => exact.source_bytes -= 1,
        }
        let (result, b) = run(exact)?;
        assert!(matches!(result, Err(Error::Stopped(actual)) if actual == reason));
        assert_eq!(b.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn static_svg_caps_precede_portable_admission_allocations() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"article en "SVG" body cons image asset "triangle" none "alt" none nil"#;
    let mut oversized = fixture(&c, source)?;
    oversized.files[0].content = FileBytes(vec![b' '; 262_145]);
    let mut excess = fixture(&c, source)?;
    excess.files = vec![excess.files[0].clone(); 129];
    for set in [oversized, excess] {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let mut limits = budget().limits();
        limits.allocation_units = 0;
        limits.work = 1;
        let mut b = Budget::new(limits);
        let result = render_footnotes_svg(&set, &c.doc.registry, &mut codec, &mut b, &[&[]]);
        assert!(matches!(result, Err(Error::Invalid(_))));
        assert_eq!(b.usage().allocation_units, 0);
        assert_eq!(b.usage().source_bytes, 0);
        assert!(b.poll().is_ok());
    }
    Ok(())
}

#[test]
fn image_free_pages_do_not_pay_unused_plain_text_preparation() -> Result<(), String> {
    let c = compiled()?;
    let store = SourceStore::default();
    for count in [4, 16, 32] {
        let source = format!("article en \"Text\" body {} nil",
            "cons paragraph cons \"A text-only page keeps its checked document and needs no image alt preparation.\" nil ".repeat(count));
        let set = PageSet {
            pages: vec![page(&c, "a", "a.nepld", "a.md", &source)?],
            files: vec![],
        };
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let mut plain_budget = budget();
        let plain =
            render(&set, &c.doc.registry, &mut codec, &mut plain_budget, &[&[]]).map_err(err)?;
        // Measure the discarded preparation in the same warm admission state.
        let mut redundant = budget();
        nepl3_doc_core::text::prepare(
            &set.pages[0].document,
            &c.doc.registry,
            &mut codec,
            &mut redundant,
        )
        .map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let mut actual = budget();
        let svg = render_footnotes_svg(&set, &c.doc.registry, &mut codec, &mut actual, &[&[]])
            .map_err(err)?;
        assert_eq!(svg.pages[0].markdown, plain.pages[0].markdown);
        assert_eq!(svg.pages[0].document_digest, plain.pages[0].document_digest);
        assert!(svg.image_dependencies[0].is_empty());
        let extra = actual
            .usage()
            .work
            .saturating_sub(plain_budget.usage().work);
        eprintln!(
            "IMAGE_PREPARATION_METRICS paragraphs={count} plain_work={} svg_work={} unused_prepare_work={} svg_allocation={} unused_prepare_allocation={}",
            plain_budget.usage().work,
            actual.usage().work,
            redundant.usage().work,
            actual.usage().allocation_units,
            redundant.usage().allocation_units
        );
        assert!(
            extra < redundant.usage().work / 2,
            "{count}: extra={extra}, unused={}",
            redundant.usage().work
        );
        for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
            let mut limits = actual.limits();
            if reason == StopReason::WorkLimit {
                limits.work = actual.usage().work - 1;
            } else {
                limits.allocation_units = actual.usage().allocation_units - 1;
            }
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
            let mut stopped = Budget::new(limits);
            assert!(
                matches!(render_footnotes_svg(&set, &c.doc.registry, &mut codec, &mut stopped, &[&[]]), Err(Error::Stopped(s)) if s == reason)
            );
            assert_eq!(stopped.poll(), Err(reason));
        }
    }
    Ok(())
}

#[test]
fn image_preparation_follows_current_page_requirements_across_gaps() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"article en "Image" body cons paragraph cons sentence cons link external "https://example.com/" text "External" nil nil cons image asset "triangle" none "alt" some "caption" nil"#;
    let mut set = fixture(&c, source)?;
    let image_only = render_svg(&c, &set).map_err(err)?;
    set.pages.insert(
        0,
        page(
            &c,
            "empty",
            "empty.nepld",
            "empty.md",
            "article en \"Empty\" body nil",
        )?,
    );
    set.pages.insert(1, page(&c, "links", "links.nepld", "links.md", "article en \"Links\" body cons paragraph cons sentence cons link external \"https://example.com/\" text \"External\" nil nil nil")?);
    set.pages.push(page(
        &c,
        "gap",
        "gap.nepld",
        "gap.md",
        "article en \"Gap\" body nil",
    )?);
    set.pages
        .push(page(&c, "other", "other.nepld", "other.md", source)?);
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let aliases: [&[annotated::Alias]; 5] = [&[]; 5];
    let result = render_footnotes_svg(&set, &c.doc.registry, &mut codec, &mut budget(), &aliases)
        .map_err(err)?;
    assert_eq!(result.pages[2].markdown, image_only.pages[0].markdown);
    for (i, deps) in result.image_dependencies.iter().enumerate() {
        assert_eq!(deps.len(), usize::from(i == 2 || i == 4));
    }
    assert!(result.pages[1].markdown.contains("https"));
    assert!(result.pages[4].markdown.contains("assets/triangle.svg"));
    Ok(())
}
