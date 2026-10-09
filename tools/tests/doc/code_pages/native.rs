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

#[test]
fn native_math_pages_share_source_admission_and_keep_old_route_closed() -> Result<(), String> {
    use nepl3_doc_core::model::{BlockRef, EmbedKind};
    use nepl3_doc_html::pages::{render_pages, render_pages_with_guests};
    use nepl3_markup::mathml::Display;
    use nepl3_tools::doc::math::MathDisplayHost;
    let c = compiled()?;
    let source = r#"article en "Math" body cons display Math label frac 1 0 Sentence "{[字/じ]/character}" nil"#;
    let mut request = request(&c, &[("a", source), ("b", source)], ParallelMode::Rows)?;
    let doc = &mut request.set.pages[0].document;
    let display = doc
        .value
        .nodes
        .iter()
        .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
        .ok_or("display")? as u64;
    let body = doc
        .value
        .nodes
        .iter_mut()
        .find_map(|n| {
            if let DocKind::Body { blocks } = &mut n.kind {
                Some(blocks)
            } else {
                None
            }
        })
        .ok_or("body")?;
    body.push(BlockRef(display));
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    assert!(matches!(
        render_pages(&request, &c.doc.registry, &mut codec, &mut budget()),
        Err(PagesRenderError::NeedsResolution(_))
    ));
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut limits = budget().limits();
    limits.source_bytes = 2 * source.len() as u64;
    let mut output = Budget::new(limits);
    let mut calls = Vec::new();
    let rendered = render_pages_with_guests(
        &request,
        &c.doc.registry,
        &mut codec,
        &mut output,
        &mut |page, guest, index, codec, b| {
            assert_eq!(guest.kind, EmbedKind::DisplayMath);
            let before = b.usage();
            let mut host = MathDisplayHost {
                registry: &c.doc.registry,
                math_surface: &c.others[0].schema,
                sentence_surface: Some(&c.others[3].schema),
                doc_surface: Some(&c.doc.package.schema),
                codec,
            };
            let markup = host
                .render(&guest.closure, Display::Block, b)
                .map_err(err)?
                .into_html(b)
                .map_err(err)?
                .markup;
            assert_eq!(before.source_bytes, b.usage().source_bytes);
            assert!(b.usage().work > before.work);
            calls.push((page, index.0));
            Ok::<_, String>(markup)
        },
    )
    .map_err(err)?;
    assert_eq!(output.usage().source_bytes, 2 * source.len() as u64);
    assert_eq!(calls, vec![(0, 0), (0, 0), (1, 0)]);
    assert_eq!(rendered.foreign[0].len(), 2);
    assert_eq!(rendered.foreign[1].len(), 1);
    limits.source_bytes -= 1;
    let mut short = Budget::new(limits);
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut callback = false;
    assert!(
        render_pages_with_guests(
            &request,
            &c.doc.registry,
            &mut codec,
            &mut short,
            &mut |_, _, _, _, _| {
                callback = true;
                Err::<HtmlRequest, _>("unexpected callback")
            }
        )
        .is_err()
    );
    assert!(!callback);
    assert_eq!(short.poll(), Err(StopReason::SourceLimit));
    Ok(())
}

#[test]
fn native_math_pages_reject_later_assets_before_callbacks_and_preserve_cancel() -> Result<(), String>
{
    use nepl3_doc_html::pages::{PagesGuestRenderError, render_pages_with_guests};
    let c = compiled()?;
    let math = r#"article en "Math" body cons display Math 1 nil"#;
    let image = r#"article en "Image" body cons image asset "missing" none "Alt" none nil"#;
    let request = request(&c, &[("a", math), ("b", image)], ParallelMode::Rows)?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut calls = 0;
    let result = render_pages_with_guests(
        &request,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &mut |_, _, _, _, _| {
            calls += 1;
            Err::<HtmlRequest, _>("unexpected callback")
        },
    );
    assert!(matches!(
        result,
        Err(PagesGuestRenderError::Pages(
            PagesRenderError::NeedsResolution(_)
        ))
    ));
    assert_eq!(calls, 0);
    let mut request = request;
    request.set.pages.pop();
    let mut output = budget();
    let result = render_pages_with_guests(
        &request,
        &c.doc.registry,
        &mut codec,
        &mut output,
        &mut |_, _, _, _, b| {
            b.cancel();
            Err::<HtmlRequest, _>("host failure")
        },
    );
    assert!(matches!(
        result,
        Err(PagesGuestRenderError::Pages(PagesRenderError::Stopped(
            StopReason::Cancelled
        )))
    ));
    Ok(())
}

#[test]
fn native_math_annotation_cannot_inject_dom_identity() -> Result<(), String> {
    use nepl3_doc_html::pages::{PagesGuestRenderError, render_pages_with_guests};
    use nepl3_markup::mathml::Tag;
    let c = compiled()?;
    let request = request(
        &c,
        &[("a", r#"article en "Math" body cons display Math 1 nil"#)],
        ParallelMode::Rows,
    )?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let result = render_pages_with_guests(
        &request,
        &c.doc.registry,
        &mut codec,
        &mut budget(),
        &mut |_, guest, _, _, b| {
            let mut value = echo(
                guest,
                Some(HtmlAttribute::Id {
                    value: "forged".into(),
                }),
                b,
            )?;
            value.fragment.nodes.push(HtmlNode::MathElement {
                tag: Tag::Text,
                attributes: vec![],
                children: vec![1],
            });
            value.fragment.nodes.push(HtmlNode::MathElement {
                tag: Tag::Math,
                attributes: vec![],
                children: vec![2],
            });
            value.fragment.root = 3;
            Ok::<_, String>(value)
        },
    );
    assert!(
        matches!(
            result,
            Err(PagesGuestRenderError::GuestId { page: 0, node: 1 })
        ),
        "{result:?}"
    );
    Ok(())
}
