use super::*;
use nepl3_doc_core::{check::Category, lower, pages::*};
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
                files: vec![],
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
fn sentence_preflight_rejects_hidden_unsafe_links_and_preserves_asset_requirements()
-> Result<(), String> {
    let (compiled, mut request) = request(
        r#"article en sentence "Links" body cons paragraph cons parallel cons variant en sentence "Visible" cons variant ja sentence sentence cons link "https://example.org/" text "Hidden" nil nil nil nil"#,
    )?;
    request.options.parallel = ParallelMode::Single {
        language: "en".into(),
        fallbacks: vec![],
    };
    let registry = &compiled.doc.registry;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    use nepl3_sentence_core::model::Kind;
    use nepl3_suite::adapters::{
        document::{sentence, sentences},
        sentence::html,
    };
    let surface = registry
        .selected("nepl3.syntax.sentence", 1)
        .ok_or("Sentence surface")?;
    let document = &request.set.pages[0].document;
    let selected = sentences::collect(document, surface, &[], registry, &mut codec, &mut budget())
        .map_err(err)?;
    assert_eq!(selected.occurrences().len(), 3);
    let hidden = selected.occurrences()[2].embed;
    let original = selected.sentence(hidden).ok_or("hidden Sentence")?.clone();
    let mut measured = budget();
    let prepared = sentences::html::prepare(
        &selected,
        registry,
        &mut |_, _, embed, _| {
            Err::<nepl3_markup::html::HtmlRequest, _>(html::Error::ForeignAdapterRequired(embed))
        },
        &mut measured,
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert!(core::ptr::eq(prepared.input(), &selected));
    assert_eq!(
        prepared.get(hidden).ok_or("prepared hidden")?.input(),
        &original
    );
    assert!(
        prepared
            .get(nepl3_doc_core::model::EmbedRef(u64::MAX))
            .is_none()
    );
    assert_eq!(prepared.into_parts().iter().flatten().count(), 3);
    for (reason, amount) in [
        (StopReason::WorkLimit, measured.usage().work),
        (
            StopReason::AllocationLimit,
            measured.usage().allocation_units,
        ),
        (StopReason::DepthLimit, measured.usage().depth),
    ] {
        assert!(amount > 0);
        for limit in [amount - 1, amount] {
            let mut limits = measured.limits();
            match reason {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                StopReason::DepthLimit => limits.depth = limit,
                _ => unreachable!("fixed limits"),
            }
            let mut limited = Budget::new(limits);
            let result = sentences::html::prepare(
                &selected,
                registry,
                &mut |_, _, embed, _| {
                    Err::<nepl3_markup::html::HtmlRequest, _>(html::Error::ForeignAdapterRequired(
                        embed,
                    ))
                },
                &mut limited,
                &mut SourceAdmission::default(),
            );
            if limit == amount {
                assert!(result.is_ok());
            } else {
                assert!(
                    matches!(result, Err(sentences::html::Error::Stopped(actual)) if actual == reason)
                );
            }
        }
    }
    let node = original
        .value
        .nodes
        .iter()
        .position(|node| matches!(node, Kind::ExternalLink { .. }))
        .ok_or("external link")?;
    // Establish a valid positive control before mutating only the URI.
    html::render(
        &original,
        registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
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
        let mut changed = original.clone();
        if let Kind::ExternalLink { uri: value, .. } = &mut changed.value.nodes[node] {
            *value = uri.into();
        }
        let mut document = document.clone();
        document.value.embeds[hidden.0 as usize] =
            sentence::embed(&changed, registry, &mut codec, &mut budget()).map_err(err)?;
        let all = sentences::collect(&document, surface, &[], registry, &mut codec, &mut budget())
            .map_err(err)?;
        let result = sentences::html::prepare(
            &all,
            registry,
            &mut |_, _, embed, _| {
                Err::<nepl3_markup::html::HtmlRequest, _>(html::Error::ForeignAdapterRequired(
                    embed,
                ))
            },
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        assert!(
            matches!(result, Err(sentences::html::Error::Sentence { embed, error: html::RenderFailure::Sentence(html::Error::Markup(nepl3_markup::html::HtmlError::Attribute { .. })) }) if embed == hidden),
            "{uri}"
        );
    }
    let mut stopped = budget();
    stopped.cancel();
    assert!(matches!(
        sentences::collect(document, surface, &[], registry, &mut codec, &mut stopped),
        Err(sentences::Error::Stopped(StopReason::Cancelled))
    ));
    let (compiled, request) = self::request(
        r#"article en sentence "Image" body cons paragraph cons sentence sentence cons link "https://example.org/" text "Link" nil nil cons image asset "logo" none sentence "Logo" none nil"#,
    )?;
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&compiled.doc.registry, &empty, &mut admission).map_err(err)?;
    assert!(
        matches!(render_pages(&request, &compiled.doc.registry, &mut codec, &mut budget()), Err(PagesRenderError::NeedsResolution(plan)) if plan.remaining.iter().any(|item| matches!(item.requirement, nepl3_doc_core::prepare::DocRequirement::Asset { .. })))
    );
    Ok(())
}
