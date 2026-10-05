use super::*;
use nepl3_doc_core::{
    check::Category,
    lower,
    model::{DocEmbed, DocKind},
    pages::{PageDocument, PageRegistration, PageSet},
};
use nepl3_doc_html::{
    ParallelMode, RenderOptions,
    pages::{PagesCodeRenderError, PagesHtmlRequest, PagesRenderError, render_pages_with_code},
};
use nepl3_markup::html::*;

fn request(
    c: &Compiled,
    inputs: &[(&str, &str)],
    parallel: ParallelMode,
) -> Result<PagesHtmlRequest, String> {
    let mut pages = Vec::new();
    for (id, source) in inputs {
        let document = nepl3_tools::doc::source::with_named_input(
            true,
            c,
            source,
            id,
            "Article",
            |tree, profile, _, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
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
        pages.push(PageDocument {
            registration: PageRegistration {
                id: (*id).into(),
                source: format!("{id}.nepld"),
                route: format!("{id}/index.html"),
            },
            document,
        });
    }
    Ok(PagesHtmlRequest {
        set: PageSet {
            pages,
            files: vec![],
        },
        options: RenderOptions { parallel },
    })
}

fn echo(
    embed: &DocEmbed,
    attribute: Option<HtmlAttribute>,
    b: &mut Budget,
) -> Result<HtmlRequest, String> {
    let root = embed
        .closure
        .syntax
        .bundle
        .node(embed.closure.syntax.bundle.root)
        .map_err(err)?;
    let cover = root.cover.as_ref().ok_or("missing cover")?;
    let source = embed
        .closure
        .syntax
        .bundle
        .sources
        .iter()
        .find(|s| s.identity() == cover.snapshot_ref())
        .ok_or("missing source")?;
    let text = source.slice(cover).map_err(err)?;
    b.charge(Resource::Work, text.len() as u64 + 1)
        .map_err(err)?;
    b.charge(Resource::AllocationUnits, text.len() as u64 + 512)
        .map_err(err)?;
    b.charge(Resource::Nodes, 2).map_err(err)?;
    Ok(HtmlRequest {
        fragment: HtmlFragment {
            root: 1,
            nodes: vec![
                HtmlNode::Text { text: text.into() },
                HtmlNode::Element {
                    tag: HtmlTag::Span,
                    attributes: attribute.into_iter().collect(),
                    children: vec![0],
                },
            ],
        },
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes: vec![] },
    })
}

#[test]
fn native_code_callback_binds_exact_page_and_shared_embed_occurrences() -> Result<(), String> {
    let c = compiled()?;
    let one = r#"article en "A" body cons paragraph cons code Doc article en sentence nil body nil nil nil"#;
    let two = r#"article en "B" body cons paragraph cons code Doc article en "Guest" body nil cons code Doc article en "Guest" body nil nil nil"#;
    let mut request = request(&c, &[("a", one), ("b", two)], ParallelMode::Rows)?;
    let mut shared = None;
    for node in &mut request.set.pages[1].document.value.nodes {
        if let DocKind::Code { syntax } = &mut node.kind {
            if let Some(index) = shared {
                *syntax = index;
            } else {
                shared = Some(*syntax);
            }
        }
    }
    // Both Code nodes deliberately share the first closure; remove the now
    // unused second closure to keep the input shape valid.
    request.set.pages[1].document.value.embeds.truncate(1);
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut calls = Vec::new();
    let rendered = render_pages_with_code(
        &request,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &mut |page, embed, index, b| {
            assert!(core::ptr::eq(
                embed,
                &request.set.pages[page as usize].document.value.embeds[index.0 as usize]
            ));
            calls.push((page, index));
            echo(
                embed,
                Some(HtmlAttribute::DataId {
                    value: "pc-example".into(),
                }),
                b,
            )
        },
    )
    .map_err(err)?;
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].0, 0);
    assert_eq!(calls[1].0, 1);
    assert_eq!(calls[1], calls[2]);
    assert_eq!(rendered.foreign[0].len(), 1);
    assert_eq!(rendered.foreign[1].len(), 2);
    assert_eq!(rendered.foreign[1][0].embed, rendered.foreign[1][1].embed);
    assert_ne!(
        rendered.foreign[1][0].first_element,
        rendered.foreign[1][1].first_element
    );
    Ok(())
}

#[test]
fn native_code_errors_and_cancel_do_not_publish_partial_pages() -> Result<(), String> {
    let c = compiled()?;
    let code = r#"article en "Code" body cons paragraph cons code Doc article en "Guest" body nil nil nil"#;
    let request = request(&c, &[("a", code), ("b", code)], ParallelMode::Rows)?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut stopped = budget();
    let mut calls = 0;
    let result = render_pages_with_code(
        &request,
        &c.doc.registry,
        &mut codec,
        &mut stopped,
        &mut |_, _, _, b| {
            calls += 1;
            b.cancel();
            Err::<HtmlRequest, _>("host failed after cancellation")
        },
    );
    assert!(matches!(
        result,
        Err(PagesCodeRenderError::Pages(PagesRenderError::Stopped(
            StopReason::Cancelled
        )))
    ));
    assert_eq!(calls, 1);
    assert_eq!(stopped.poll(), Err(StopReason::Cancelled));
    let result = render_pages_with_code(
        &request,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &mut |_, embed, _, b| {
            let mut output = echo(embed, None, b)?;
            output.fragment.root = u64::MAX;
            Ok::<_, String>(output)
        },
    );
    assert!(result.is_err());
    Ok(())
}

#[test]
fn unsupported_requirements_prevent_all_native_code_callbacks() -> Result<(), String> {
    let c = compiled()?;
    for other in [
        r#"article en "Math" body cons display Math frac 1 0 nil"#,
        r#"article en "Image" body cons image asset "missing" none "Alt" none nil"#,
    ] {
        let code = r#"article en "Code" body cons paragraph cons code Doc article en "Guest" body nil nil nil"#;
        let request = request(&c, &[("a", code), ("b", other)], ParallelMode::Rows)?;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let mut calls = 0;
        let result = render_pages_with_code(
            &request,
            &c.doc.registry,
            &mut codec,
            &mut budget(),
            &mut |_, embed, _, b| {
                calls += 1;
                echo(embed, None, b)
            },
        );
        assert!(matches!(
            result,
            Err(PagesCodeRenderError::Pages(
                PagesRenderError::NeedsResolution(_)
            ))
        ));
        assert_eq!(calls, 0);
    }
    Ok(())
}

#[test]
fn code_decorations_cannot_supply_hidden_doc_anchors() -> Result<(), String> {
    let c = compiled()?;
    for local in [false, true] {
        let from = r#"article en "From" body cons paragraph cons sentence cons link page "to" some "hidden" text "go" nil nil nil"#;
        let to = if local {
            r#"article ja "To" body cons paragraph cons parallel cons variant ja sentence cons anchor hidden text "対象" nil cons variant en "Translation" nil cons sentence cons ref hidden text "local" nil cons code Doc article en "Guest" body nil nil nil"#
        } else {
            r#"article ja "To" body cons paragraph cons parallel cons variant ja sentence cons anchor hidden text "対象" nil cons variant en "Translation" nil cons code Doc article en "Guest" body nil nil nil"#
        };
        let request = request(
            &c,
            &[("from", from), ("to", to)],
            ParallelMode::Single {
                language: "en".into(),
                fallbacks: vec![],
            },
        )?;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let result = render_pages_with_code(
            &request,
            &c.doc.registry,
            &mut codec,
            &mut budget(),
            &mut |_, embed, _, b| {
                echo(
                    embed,
                    Some(HtmlAttribute::Id {
                        value: "n-68696464656e".into(),
                    }),
                    b,
                )
            },
        );
        assert!(
            matches!(
                result,
                Err(PagesCodeRenderError::CodeId { page: 1, node: 1 })
            ),
            "{result:?}"
        );
        let result = render_pages_with_code(
            &request,
            &c.doc.registry,
            &mut codec,
            &mut budget(),
            &mut |_, embed, _, b| echo(embed, None, b),
        );
        assert!(result.is_err(), "hidden Doc anchor was manufactured");
    }
    Ok(())
}

#[test]
fn explicit_code_api_keeps_plain_page_output_identical() -> Result<(), String> {
    let c = compiled()?;
    let request = request(
        &c,
        &[(
            "a",
            r#"article en "A" body cons paragraph cons "Plain text" nil nil"#,
        )],
        ParallelMode::Rows,
    )?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let old =
        nepl3_doc_html::pages::render_pages(&request, &c.doc.registry, &mut codec, &mut budget())
            .map_err(err)?;
    let mut calls = 0;
    let new = render_pages_with_code(
        &request,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &mut |_, _, _, _| {
            calls += 1;
            Err::<HtmlRequest, _>("unexpected Code callback")
        },
    )
    .map_err(err)?;
    assert_eq!(new.pages, old);
    assert_eq!(calls, 0);
    assert_eq!(new.foreign.len(), 1);
    assert!(new.foreign[0].is_empty());
    Ok(())
}
