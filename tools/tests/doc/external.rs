use super::*;
use nepl3_doc_core::{check::Category, lower, model::*, pages::*};
use nepl3_doc_html::{ParallelMode, RenderOptions, pages::*};

fn request(source: &str) -> Result<(Compiled, PagesHtmlRequest), String> {
    let mut compiled = compiled()?;
    for descriptor in [
        nepl3_markup::schema::descriptor(&mut budget()),
        nepl3_doc_html::schema::descriptor(&mut budget()),
    ] {
        let descriptor = descriptor.map_err(err)?;
        compiled
            .doc
            .registry
            .register(
                descriptor.reference(&mut budget()).map_err(err)?,
                descriptor,
                &mut budget(),
            )
            .map_err(err)?;
    }
    compiled.doc.registry.finalize(&mut budget()).map_err(err)?;
    let document = with_input_route(true, &compiled, source, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)
    })?;
    Ok((
        compiled,
        PagesHtmlRequest {
            set: PageSet {
                pages: vec![PageDocument {
                    registration: PageRegistration {
                        id: "external".into(),
                        source: "external.nepld".into(),
                        route: "docs/links/index.html".into(),
                    },
                    document,
                }],
            },
            options: RenderOptions {
                parallel: ParallelMode::Rows,
            },
        },
    ))
}

#[test]
fn external_links_use_shared_markup_rules_and_portable_revalidation() -> Result<(), String> {
    use nepl3_doc_html::portable::pages as wire;
    use nepl3_markup::html::*;
    let source = r#"article en "Links" body cons paragraph cons sentence cons link external "https://example.org/docs?q=one&lang=en#part" text "Reference" cons text " / " cons link external "mailto:author@example.org" text "Mail" nil nil nil"#;
    let (compiled, request) = request(source)?;
    let registry = &compiled.doc.registry;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let result = render_pages(&request, registry, &mut codec, &mut budget()).map_err(err)?;
    let markup = &result.fragments[0].markup;
    let html = serialize(
        &validate(&markup.fragment, markup.slot, &markup.policy, &mut budget()).map_err(err)?,
        &mut budget(),
    )
    .map_err(err)?;
    assert!(html.contains("href=\"https://example.org/docs?q=one&amp;lang=en#part\""));
    assert!(html.contains("href=\"mailto:author@example.org\""));
    let value =
        wire::request_to_value(&request, registry, &mut codec, &mut budget()).map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
    let mut fresh_admission = SourceAdmission::default();
    let mut fresh = FoundationCodec::new(registry, &empty, &mut fresh_admission).map_err(err)?;
    let decoded = wire::request_from_value(
        &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
        registry,
        &mut fresh,
        &mut budget(),
    )
    .map_err(err)?;
    assert_eq!(
        render_pages(&decoded, registry, &mut fresh, &mut budget()).map_err(err)?,
        result
    );
    let value = wire::rendered_to_value(&result, &request, registry, &mut codec, &mut budget())
        .map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
    assert_eq!(
        wire::rendered_from_value(
            &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
            &decoded,
            registry,
            &mut fresh,
            &mut budget()
        )
        .map_err(err)?,
        result
    );
    let mut forged = result.clone();
    for node in &mut forged.fragments[0].markup.fragment.nodes {
        if let HtmlNode::Element { attributes, .. } = node {
            for attr in attributes {
                if let HtmlAttribute::Href {
                    value: HtmlHref::External { uri },
                } = attr
                {
                    *uri = "https://other.example.org/".into();
                }
            }
        }
    }
    assert!(
        wire::rendered_to_value(&forged, &request, registry, &mut codec, &mut budget()).is_err()
    );
    Ok(())
}

#[test]
fn hidden_unsafe_links_are_rejected_and_other_requirements_remain() -> Result<(), String> {
    let (compiled, mut request) = request(
        r#"article en "Links" body cons paragraph cons parallel cons variant en "Visible" cons variant ja sentence cons link external "https://example.org/" text "Hidden" nil nil nil nil"#,
    )?;
    request.options.parallel = ParallelMode::Single {
        language: "en".into(),
        fallbacks: vec![],
    };
    let registry = &compiled.doc.registry;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let node = request.set.pages[0]
        .document
        .value
        .nodes
        .iter()
        .position(|n| matches!(n.kind, DocKind::Link { .. }))
        .ok_or("link")?;
    for uri in [
        "javascript:alert(1)",
        "data:text/html,x",
        "//example.org/",
        "https://user@example.org/",
        "https://example.org/%",
        "https://example.org/a b",
        "HTTPS://example.org/",
        "https://example.org\\evil",
    ] {
        if let DocKind::Link {
            target: LinkTarget::External { uri: value },
            ..
        } = &mut request.set.pages[0].document.value.nodes[node].kind
        {
            *value = uri.into();
        }
        assert!(
            matches!(render_pages(&request, registry, &mut codec, &mut budget()), Err(PagesRenderError::InvalidExternalUri { page: 0, node: n }) if n == node as u64),
            "{uri}"
        );
    }
    if let DocKind::Link { target, .. } = &mut request.set.pages[0].document.value.nodes[node].kind
    {
        *target = LinkTarget::External {
            uri: "https://example.org/".into(),
        };
    }
    render_pages(&request, registry, &mut codec, &mut budget()).map_err(err)?;
    let mut stopped = budget();
    stopped.cancel();
    assert!(matches!(
        render_pages(&request, registry, &mut codec, &mut stopped),
        Err(PagesRenderError::Stopped(StopReason::Cancelled))
    ));
    let (compiled, request) = self::request(
        r#"article en "Image" body cons paragraph cons sentence cons link external "https://example.org/" text "Link" cons image asset "logo" none "Logo" nil nil nil"#,
    )?;
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&compiled.doc.registry, &empty, &mut admission).map_err(err)?;
    assert!(
        matches!(render_pages(&request, &compiled.doc.registry, &mut codec, &mut budget()), Err(PagesRenderError::NeedsResolution(plan)) if plan.remaining.len() == 2)
    );
    Ok(())
}
