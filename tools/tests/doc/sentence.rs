//! Independent Sentence slots retain foreign ownership at the Doc boundary.
use super::*;
use nepl3_doc_core::{
    check::{Category, ShapeError},
    model::*,
    portable, text,
};

const INPUT: &str = r#"Sentence sentence cons text "guest" nil"#;

#[test]
fn independent_sentence_retains_closure_roundtrips_and_requires_selection() -> Result<(), String> {
    let compiled = compiled()?;
    for native in [false, true] {
        for input in [INPUT, r#"Sentence "guest [漢/かん]""#] {
            with_input_route(
                native,
                &compiled,
                input,
                "Sentence",
                |tree, profile, b, a| {
                    let registry = profile.registry();
                    let checked = tree
                        .tree()
                        .bundle
                        .validate_with_sources(registry, b, a)
                        .map_err(err)?;
                    let empty = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
                    let doc = nepl3_suite::adapters::sentence::literal::lower(
                        &checked,
                        &compiled.doc.package.schema,
                        Category::Sentence,
                        registry,
                        b,
                        &mut codec,
                    )
                    .map_err(err)?;
                    let DocRoot::Sentence(root) = doc.value.root else {
                        return Err("Sentence root".into());
                    };
                    assert_eq!(
                        doc.value.nodes.len(),
                        1,
                        "guest inlines must not be copied into Doc"
                    );
                    assert_eq!(
                        doc.value.nodes[0].kind,
                        DocKind::SentenceGuest {
                            syntax: EmbedRef(0)
                        }
                    );
                    assert_eq!(doc.value.embeds.kinds, vec![EmbedKind::Sentence]);
                    assert_eq!(
                        doc.value
                            .embeds
                            .get(0)
                            .ok_or("embed")?
                            .closure
                            .syntax()
                            .category,
                        "Sentence"
                    );
                    let shape = doc.value.validate_shape(b).map_err(err)?;
                    assert_eq!(
                        shape.annotation_content(),
                        None,
                        "foreign semantic content is not checked by Doc shape"
                    );
                    assert_eq!(
                        nepl3_doc_core::print::guest_depths(&shape, b).map_err(err)?,
                        vec![1]
                    );
                    let value = portable::to_value(&doc, registry, &mut codec, b).map_err(err)?;
                    let encoded = nepl3_wire::encode(&value, b).map_err(err)?;
                    let restored = portable::from_value(
                        &nepl3_wire::decode(&encoded, b).map_err(err)?,
                        registry,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    assert_doc_retention(&doc, &restored)?;
                    // Foreign graph and source-table ordering are canonicalized on wire.
                    assert_eq!(
                        portable::to_value(&restored, registry, &mut codec, b).map_err(err)?,
                        value
                    );
                    let fragment = doc
                        .fragment(doc.value.root, registry, b, codec.source_admission())
                        .map_err(err)?;
                    assert_eq!(fragment.value.embeds, doc.value.embeds);
                    assert_eq!(fragment.origins, doc.origins);
                    assert_eq!(fragment.sources, doc.sources);
                    let prepared = text::prepare(&doc, registry, &mut codec, b).map_err(err)?;
                    assert_eq!(prepared.identity().embeds.len(), 1);
                    let target = &prepared.identity().embeds[0];
                    let unresolved = prepared
                        .project(root, text::AnnotationPolicy::BaseOnly, &[], b)
                        .map_err(err)?;
                    assert!(matches!(
                        unresolved.outcome,
                        text::PlainTextOutcome::Invalid {
                            error: text::PlainTextFailure::UnresolvedEmbed { embed: EmbedRef(0) }
                        }
                    ));
                    let resolved = text::ResolvedInlineText {
                        document_digest: prepared.identity().document_digest,
                        guest_digest: target.guest_digest,
                        embed: target.embed,
                        text: "guest".into(),
                    };
                    let reply = prepared
                        .project(root, text::AnnotationPolicy::BaseOnly, &[resolved], b)
                        .map_err(err)?;
                    assert_eq!(
                        reply.outcome,
                        text::PlainTextOutcome::Complete {
                            text: "guest".into()
                        }
                    );
                    let options = nepl3_doc_html::RenderOptions {
                        parallel: nepl3_doc_html::ParallelMode::Rows,
                    };
                    let local = nepl3_doc_html::prepare_local_sentence(
                        &doc, &options, registry, &mut codec, b,
                    );
                    assert!(matches!(
                        local,
                        Err(nepl3_doc_html::LocalPreparationError::NeedsResolution(_))
                    ));
                    let mut wrong_category = doc.clone();
                    wrong_category.value.embeds.group.entries[0].syntax.category = "Inline".into();
                    assert!(
                        wrong_category
                            .validate_structure(registry, b, codec.source_admission())
                            .is_err()
                    );
                    let mut unused = doc.clone();
                    unused.value.embeds.kinds.push(EmbedKind::Sentence);
                    unused
                        .value
                        .embeds
                        .group
                        .entries
                        .push(unused.value.embeds.group.entries[0].clone());
                    assert_eq!(
                        unused.value.validate_shape(b).err(),
                        Some(ShapeError::UnusedEmbed(1))
                    );
                    // The same foreign entry occurs as the title and through two paragraphs.
                    // The deepest owner is five levels below the document entry.
                    let mut shared = doc.clone();
                    for kind in [
                        DocKind::Paragraph {
                            items: vec![FlowRef(0)],
                        },
                        DocKind::Paragraph {
                            items: vec![FlowRef(1), FlowRef(0)],
                        },
                        DocKind::Body {
                            blocks: vec![BlockRef(2)],
                        },
                        DocKind::Article {
                            language: "en".into(),
                            title: SentenceRef(0),
                            body: BodyRef(3),
                        },
                    ] {
                        shared.value.nodes.push(DocNode {
                            kind,
                            origin: None,
                            span: None,
                            locations: vec![],
                        });
                    }
                    shared.value.root = DocRoot::Article(ArticleRef(4));
                    let shape = shared.value.validate_shape(b).map_err(err)?;
                    assert_eq!(
                        nepl3_doc_core::print::guest_depths(&shape, b).map_err(err)?,
                        vec![5]
                    );
                    let mut measured = budget();
                    measured
                        .with_depth_at_least(7, |b| {
                            shared
                                .validate_structure(registry, b, &mut SourceAdmission::default())
                                .map(|_| ())
                        })
                        .map_err(err)?;
                    let mut limits = measured.limits();
                    limits.depth = measured.usage().depth - 1;
                    let mut limited = Budget::new(limits);
                    let stop = limited.with_depth_at_least(7, |b| {
                        shared
                            .validate_structure(registry, b, &mut SourceAdmission::default())
                            .map(|_| ())
                    });
                    assert!(matches!(
                        stop,
                        Err(nepl3_doc_core::check::StructureError::Stopped(
                            StopReason::DepthLimit
                        ))
                    ));
                    assert_eq!(limited.current_depth(), 0);
                    let mut wrong = doc.clone();
                    wrong.value.embeds.kinds[0] = EmbedKind::InlineMath;
                    assert_eq!(
                        wrong.value.validate_shape(b).err(),
                        Some(ShapeError::Embed(0))
                    );
                    let mut wrong = doc.clone();
                    wrong.value.root = DocRoot::Guest(GuestRef(0));
                    wrong.value.nodes[0].kind = DocKind::Guest {
                        language: GuestLanguage::Sentence,
                        syntax: EmbedRef(0),
                    };
                    wrong.value.embeds.kinds[0] = EmbedKind::Guest;
                    assert_eq!(
                        wrong.value.validate_shape(b).err(),
                        Some(ShapeError::Embed(0))
                    );
                    Ok(())
                },
            )?;
        }
    }
    Ok(())
}

#[test]
fn sentence_slot_shared_depth_and_closed_owner_are_not_skipped() -> Result<(), String> {
    let compiled = compiled()?;
    let input = r#"article en Sentence sentence cons text "Title" nil body cons paragraph nil nil"#;
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            input,
            "Article",
            |tree, profile, b, a| {
                let registry = profile.registry();
                let checked = tree
                    .tree()
                    .bundle
                    .validate_with_sources(registry, b, a)
                    .map_err(err)?;
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec =
                    FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
                let mut doc = nepl3_suite::adapters::sentence::literal::lower(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let sentence = doc
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::SentenceGuest { .. }))
                    .ok_or("sentence")? as u64;
                for node in &mut doc.value.nodes {
                    if let DocKind::Paragraph { items } = &mut node.kind {
                        *items = vec![FlowRef(sentence), FlowRef(sentence)];
                    }
                }
                let shape = doc.value.validate_shape(b).map_err(err)?;
                assert_eq!(
                    nepl3_doc_core::print::guest_depths(&shape, b).map_err(err)?,
                    vec![4]
                );
                let mut measured = budget();
                doc.validate_structure(registry, &mut measured, &mut SourceAdmission::default())
                    .map_err(err)?;
                let depth = measured.usage().depth;
                assert!(
                    depth > 4,
                    "foreign graph depth must be composed with its deepest Doc owner"
                );
                let mut limits = budget().limits();
                limits.depth = depth;
                doc.validate_structure(
                    registry,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
                limits.depth -= 1;
                let mut limited = Budget::new(limits);
                assert!(matches!(
                    doc.validate_structure(registry, &mut limited, &mut SourceAdmission::default()),
                    Err(nepl3_doc_core::check::StructureError::Stopped(
                        StopReason::DepthLimit
                    ))
                ));
                assert_eq!(limited.poll(), Err(StopReason::DepthLimit));
                let mut nested = Budget::new(budget().limits());
                nested
                    .with_depth_at_least(7, |b| {
                        doc.validate_structure(registry, b, &mut SourceAdmission::default())
                    })
                    .map_err(err)?;
                assert!(nested.usage().depth >= depth + 7);
                let fragment = doc
                    .fragment(
                        DocRoot::Sentence(SentenceRef(sentence)),
                        registry,
                        b,
                        codec.source_admission(),
                    )
                    .map_err(err)?;
                assert_eq!(fragment.value.nodes.len(), 1);
                assert_eq!(fragment.value.embeds.len(), 1);
                assert_eq!(
                    fragment.value.nodes[0].kind,
                    DocKind::SentenceGuest {
                        syntax: EmbedRef(0)
                    }
                );
                for mutation in 0..3 {
                    let mut bad = doc.clone();
                    match mutation {
                        0 => bad.value.embeds.group.entries[0].syntax.category = "Inline".into(),
                        1 => {
                            bad.value.embeds.group.provenances[0].sources.clear();
                        }
                        _ => {
                            bad.value
                                .embeds
                                .group
                                .entries
                                .push(bad.value.embeds.group.entries[0].clone());
                            bad.value.embeds.kinds.push(EmbedKind::Sentence);
                        }
                    }
                    assert!(
                        bad.validate_structure(
                            registry,
                            &mut budget(),
                            &mut SourceAdmission::default()
                        )
                        .is_err(),
                        "mutation {mutation}"
                    );
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn sentence_fragment_reindexes_nonzero_embed_and_checks_wire_depth() -> Result<(), String> {
    let compiled = compiled()?;
    let input =
        r#"article en Sentence "title" body cons paragraph cons Sentence "body [漢/かん]" nil nil"#;
    with_input_route(true, &compiled, input, "Article", |tree, profile, b, a| {
        let registry = profile.registry();
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(registry, b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
        let doc = nepl3_suite::adapters::sentence::literal::lower(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            registry,
            b,
            &mut codec,
        )
        .map_err(err)?;
        assert_eq!(doc.value.embeds.len(), 2);
        let selected = doc
            .value
            .nodes
            .iter()
            .position(|n| {
                matches!(
                    n.kind,
                    DocKind::SentenceGuest {
                        syntax: EmbedRef(1)
                    }
                )
            })
            .ok_or("second guest")?;
        let fragment = doc
            .fragment(
                DocRoot::Sentence(SentenceRef(selected as u64)),
                registry,
                b,
                codec.source_admission(),
            )
            .map_err(err)?;
        assert_eq!(
            fragment.value.nodes[0].kind,
            DocKind::SentenceGuest {
                syntax: EmbedRef(0)
            }
        );
        assert_eq!(
            fragment
                .value
                .embeds
                .get(0)
                .ok_or("selected")?
                .closure
                .syntax(),
            doc.value.embeds.get(1).ok_or("original")?.closure.syntax()
        );
        let mut measured = budget();
        let value =
            portable::to_value(&fragment, registry, &mut codec, &mut measured).map_err(err)?;
        let mut limits = budget().limits();
        limits.depth = measured.usage().depth;
        portable::to_value(&fragment, registry, &mut codec, &mut Budget::new(limits))
            .map_err(err)?;
        limits.depth -= 1;
        assert!(matches!(
            portable::to_value(&fragment, registry, &mut codec, &mut Budget::new(limits)),
            Err(portable::PortableError::Stopped(StopReason::DepthLimit))
        ));
        let mut incoming = budget();
        portable::from_value(&value, registry, &mut codec, &mut incoming).map_err(err)?;
        limits.depth = incoming.usage().depth;
        portable::from_value(&value, registry, &mut codec, &mut Budget::new(limits))
            .map_err(err)?;
        limits.depth -= 1;
        assert!(matches!(
            portable::from_value(&value, registry, &mut codec, &mut Budget::new(limits)),
            Err(portable::PortableError::Stopped(StopReason::DepthLimit))
        ));
        let mut missing = fragment.clone();
        missing.value.embeds.group.provenances[0].sources.clear();
        assert!(portable::to_value(&missing, registry, &mut codec, &mut budget()).is_err());
        Ok(())
    })
}

#[test]
fn article_sentence_guests_compose_one_namespace_and_reject_invalid_output() -> Result<(), String> {
    use nepl3_markup::html::*;
    let compiled = compiled()?;
    let source = r#"article en Sentence sentence cons text "Title" nil body cons paragraph cons Sentence sentence cons text "first" nil cons Sentence sentence cons text "second" nil nil nil"#;
    with_input_route(true, &compiled, source, "Article", |tree, profile, b, a| {
        let registry = profile.registry();
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(registry, b, a)
            .map_err(err)?;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let doc = nepl3_suite::adapters::sentence::literal::lower(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            registry,
            b,
            &mut codec,
        )
        .map_err(err)?;
        assert_eq!(doc.value.embeds.len(), 3);
        let options = nepl3_doc_html::RenderOptions {
            parallel: nepl3_doc_html::ParallelMode::Rows,
        };
        let prepared =
            nepl3_doc_html::display::prepare_display(&doc, &options, registry, &mut codec, b)
                .map_err(err)?;
        for scenario in 0..6 {
            let mut seen = Vec::new();
            let result = nepl3_doc_html::display::render_display(
                &prepared,
                &mut |view, embed, operation| {
                    assert_eq!(view.kind, EmbedKind::Sentence);
                    assert_eq!(view.closure.syntax().category, "Sentence");
                    assert_eq!(
                        view.closure.syntax(),
                        doc.value
                            .embeds
                            .get(embed.0 as usize)
                            .ok_or("missing owner")?
                            .closure
                            .syntax()
                    );
                    seen.push(embed);
                    if scenario == 4 {
                        return Err("adapter failed".to_string());
                    }
                    if scenario == 5 {
                        operation.cancel();
                    }
                    // A selected adapter supplies ordinary phrasing output.
                    // The first body guest references a later guest. The title
                    // and body share the complete Article's final namespace.
                    let (tag, attributes, text) = match embed.0 {
                        0 => (HtmlTag::Span, vec![], "Title"),
                        1 => (
                            HtmlTag::A,
                            vec![HtmlAttribute::Href {
                                value: HtmlHref::Fragment {
                                    id: "target".into(),
                                },
                            }],
                            "first",
                        ),
                        _ => (
                            if scenario == 3 {
                                HtmlTag::P
                            } else {
                                HtmlTag::Span
                            },
                            if scenario == 1 {
                                vec![]
                            } else {
                                vec![HtmlAttribute::Id {
                                    value: "target".into(),
                                }]
                            },
                            "second",
                        ),
                    };
                    let mut nodes = vec![
                        HtmlNode::Element {
                            tag,
                            attributes,
                            children: vec![1],
                        },
                        HtmlNode::Text { text: text.into() },
                    ];
                    if scenario == 2
                        && embed.0 == 0
                        && let HtmlNode::Element { attributes, .. } = &mut nodes[0]
                    {
                        attributes.push(HtmlAttribute::Id {
                            value: "target".into(),
                        });
                    }
                    Ok(HtmlRequest {
                        fragment: HtmlFragment { root: 0, nodes },
                        slot: HtmlSlot::Phrasing,
                        policy: HtmlPolicy { classes: vec![] },
                    })
                },
                &mut budget(),
            );
            if scenario == 0 {
                let rendered = result.map_err(err)?;
                assert_eq!(seen, vec![EmbedRef(0), EmbedRef(1), EmbedRef(2)]);
                assert_eq!(rendered.foreign.len(), 3);
                for (placement, embed) in rendered.foreign.iter().zip(&seen) {
                    assert_eq!(placement.embed, *embed);
                    assert_eq!(placement.elements, 2);
                    assert!(
                        placement.first_element + placement.elements
                            <= rendered.fragment.markup.fragment.nodes.len() as u64
                    );
                }
                let m = &rendered.fragment.markup;
                let valid = validate(&m.fragment, m.slot, &m.policy, &mut budget()).map_err(err)?;
                let html = serialize(&valid, &mut budget()).map_err(err)?;
                assert!(html.contains("href=\"#target\""));
                assert!(html.contains("id=\"target\""));
                assert!(html.contains("Title"));
                assert_eq!(
                    html.matches("<p>").count(),
                    1,
                    "consecutive Sentence guests share one paragraph"
                );
            } else {
                match (scenario, result) {
                    (
                        1,
                        Err(nepl3_doc_html::ForeignRenderError::Render(
                            nepl3_doc_html::RenderError::Markup(HtmlError::MissingFragment(_)),
                        )),
                    ) => {}
                    (
                        2,
                        Err(nepl3_doc_html::ForeignRenderError::Render(
                            nepl3_doc_html::RenderError::Markup(HtmlError::DuplicateId(_)),
                        )),
                    ) => {}
                    (
                        3,
                        Err(nepl3_doc_html::ForeignRenderError::Render(
                            nepl3_doc_html::RenderError::Markup(HtmlError::Content(_)),
                        )),
                    ) => {}
                    (4, Err(nepl3_doc_html::ForeignRenderError::Foreign(message))) => {
                        assert_eq!(message, "adapter failed")
                    }
                    (
                        5,
                        Err(nepl3_doc_html::ForeignRenderError::Render(
                            nepl3_doc_html::RenderError::Stopped(_),
                        )),
                    ) => {}
                    (_, Err(error)) => {
                        return Err(format!("scenario {scenario}: wrong error {error:?}"));
                    }
                    (_, Ok(_)) => {
                        return Err(format!("scenario {scenario}: invalid output accepted"));
                    }
                }
            }
        }
        let mut shared = doc.clone();
        let DocRoot::Article(root) = shared.value.root else {
            return Err("article root".into());
        };
        let title = match shared.value.nodes[root.0 as usize].kind {
            DocKind::Article { title, .. } => title,
            _ => return Err("article root".into()),
        };
        let paragraph = shared
            .value
            .nodes
            .iter_mut()
            .find_map(|node| match &mut node.kind {
                DocKind::Paragraph { items } => Some(items),
                _ => None,
            })
            .ok_or("paragraph")?;
        paragraph.push(FlowRef(title.0));
        let shared_prepared = nepl3_doc_html::display::prepare_display(
            &shared,
            &options,
            registry,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let text_guest = |_: &DocEmbedView<'_>, _: EmbedRef, operation: &mut Budget| {
            assert!(
                operation.current_depth() > 0,
                "insertion ancestors must be charged before callback"
            );
            Ok::<_, String>(HtmlRequest {
                fragment: HtmlFragment {
                    root: 0,
                    nodes: vec![HtmlNode::Text {
                        text: "text".into(),
                    }],
                },
                slot: HtmlSlot::Phrasing,
                policy: HtmlPolicy { classes: vec![] },
            })
        };
        let rendered = nepl3_doc_html::display::render_display(
            &shared_prepared,
            &mut { text_guest },
            &mut budget(),
        )
        .map_err(err)?;
        let occurrences: Vec<_> = rendered
            .foreign
            .iter()
            .filter(|p| p.embed == EmbedRef(0))
            .collect();
        assert_eq!(occurrences.len(), 2);
        assert_ne!(occurrences[0].first_element, occurrences[1].first_element);
        let mut shallow = Budget::new(Limits {
            depth: 1,
            work: u64::MAX,
            allocation_units: u64::MAX,
            nodes: u64::MAX,
            output_bytes: u64::MAX,
            source_bytes: u64::MAX,
            ..Limits::default()
        });
        assert!(matches!(
            nepl3_doc_html::display::render_display(
                &shared_prepared,
                &mut { text_guest },
                &mut shallow,
            ),
            Err(nepl3_doc_html::ForeignRenderError::Render(
                nepl3_doc_html::RenderError::Stopped(_)
            ))
        ));
        let mut invoked = false;
        let mut depth_limited = Budget::new(Limits {
            depth: 10,
            work: u64::MAX,
            allocation_units: u64::MAX,
            nodes: u64::MAX,
            output_bytes: u64::MAX,
            source_bytes: u64::MAX,
            ..Limits::default()
        });
        let too_deep = nepl3_doc_html::display::render_display(
            &shared_prepared,
            &mut |_, _, _| {
                invoked = true;
                let mut nodes = Vec::new();
                for index in 0..8 {
                    nodes.push(HtmlNode::Element {
                        tag: HtmlTag::Span,
                        attributes: vec![],
                        children: vec![index + 1],
                    });
                }
                nodes.push(HtmlNode::Text {
                    text: "deep".into(),
                });
                let markup = HtmlRequest {
                    fragment: HtmlFragment { root: 0, nodes },
                    slot: HtmlSlot::Phrasing,
                    policy: HtmlPolicy { classes: vec![] },
                };
                let mut standalone = Budget::new(Limits {
                    depth: 10,
                    work: u64::MAX,
                    allocation_units: u64::MAX,
                    nodes: u64::MAX,
                    output_bytes: u64::MAX,
                    source_bytes: u64::MAX,
                    ..Limits::default()
                });
                validate(
                    &markup.fragment,
                    markup.slot,
                    &markup.policy,
                    &mut standalone,
                )
                .map_err(err)?;
                Ok::<_, String>(markup)
            },
            &mut depth_limited,
        );
        assert!(
            invoked,
            "exercise guest validation, not only host ancestor creation"
        );
        assert!(matches!(
            too_deep,
            Err(nepl3_doc_html::ForeignRenderError::Render(
                nepl3_doc_html::RenderError::Stopped(_)
            ))
        ));
        Ok(())
    })
}

#[test]
fn article_foreign_preparation_rejects_unsupported_and_external_requirements() -> Result<(), String>
{
    let compiled = compiled()?;
    for (source, _circuit) in [
        (
            r#"article en sentence nil body cons circuit sentence nil Circuit design nil Main nil nil"#,
            true,
        ),
        (
            r#"article en sentence nil body cons image asset "missing" none sentence nil none nil"#,
            false,
        ),
    ] {
        with_input_route(true, &compiled, source, "Article", |tree, profile, b, a| {
            let registry = profile.registry();
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(registry, b, a)
                .map_err(err)?;
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
            let doc = nepl3_suite::adapters::sentence::literal::lower(
                &checked,
                &compiled.doc.package.schema,
                Category::Article,
                registry,
                b,
                &mut codec,
            )
            .map_err(err)?;
            let options = nepl3_doc_html::RenderOptions {
                parallel: nepl3_doc_html::ParallelMode::Rows,
            };
            let prepared =
                nepl3_doc_html::display::prepare_display(&doc, &options, registry, &mut codec, b);
            assert!(matches!(
                prepared,
                Err(nepl3_doc_html::LocalPreparationError::NeedsResolution(_))
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn article_selects_real_sentence_lowering_and_annotation_markup() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article ja Sentence "題[漢/かん]" body cons paragraph cons Sentence sentence cons anno text "word" cons text "note" nil nil nil nil"#;
    with_input_route(true, &compiled, source, "Article", |tree, profile, b, a| {
        let registry = profile.registry();
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(registry, b, a)
            .map_err(err)?;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let doc = nepl3_suite::adapters::sentence::literal::lower(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            registry,
            b,
            &mut codec,
        )
        .map_err(err)?;
        let options = nepl3_doc_html::RenderOptions {
            parallel: nepl3_doc_html::ParallelMode::Rows,
        };
        let prepared =
            nepl3_doc_html::display::prepare_display(&doc, &options, registry, &mut codec, b)
                .map_err(err)?;
        let rendered = nepl3_doc_html::display::render_display(
            &prepared,
            &mut |view, _, operation| {
                let input = view
                    .closure
                    .validate_with_bundle(registry, operation, codec.source_admission())
                    .map_err(err)?;
                let value = nepl3_sentence_core::lower::presentation::sentence(
                    &input,
                    &view.closure.syntax().schema,
                    registry,
                    &mut codec,
                    operation,
                )
                .map_err(err)?;
                let rendered = nepl3_suite::adapters::sentence::html::render(
                    &value,
                    registry,
                    operation,
                    codec.source_admission(),
                )
                .map_err(err)?;
                Ok::<_, String>(rendered.into_markup())
            },
            b,
        )
        .map_err(err)?;
        assert_eq!(rendered.foreign.len(), 2);
        let m = &rendered.fragment.markup;
        let valid = nepl3_markup::html::validate(&m.fragment, m.slot, &m.policy, b).map_err(err)?;
        let html = nepl3_markup::html::serialize(&valid, b).map_err(err)?;
        for expected in ["nepl-ruby", "nepl-anno", "漢", "かん", "word", "note"] {
            assert!(html.contains(expected), "{expected} missing from {html}");
        }
        let request = nepl3_doc_html::pages::PagesHtmlRequest {
            set: nepl3_doc_core::pages::PageSet {
                pages: ["one", "two"]
                    .into_iter()
                    .map(|id| nepl3_doc_core::pages::PageDocument {
                        registration: nepl3_doc_core::pages::PageRegistration {
                            id: id.into(),
                            source: format!("{id}.nepld"),
                            route: format!("{id}.html"),
                        },
                        document: doc.clone(),
                    })
                    .collect(),
                files: vec![],
            },
            options: options.clone(),
        };
        for fail_second in [false, true] {
            let mut visited = Vec::new();
            let result = nepl3_doc_html::pages::render_pages_with_display(
                &request,
                registry,
                &mut codec,
                &mut budget(),
                &mut |page, guest, _, _, _| {
                    visited.push(page);
                    assert_eq!(guest.kind, EmbedKind::Sentence);
                    if fail_second && page == 1 {
                        return Err("second page adapter failed".to_string());
                    }
                    Ok(nepl3_markup::html::HtmlRequest {
                        fragment: nepl3_markup::html::HtmlFragment {
                            root: 0,
                            nodes: vec![nepl3_markup::html::HtmlNode::Text {
                                text: "guest".into(),
                            }],
                        },
                        slot: nepl3_markup::html::HtmlSlot::Phrasing,
                        policy: nepl3_markup::html::HtmlPolicy { classes: vec![] },
                    })
                },
            );
            assert!(visited.contains(&0) && visited.contains(&1));
            if fail_second {
                assert!(
                    result.is_err(),
                    "no partial PageSet on a later guest failure"
                );
            } else {
                let output = result.map_err(err)?;
                assert_eq!(output.pages.fragments.len(), 2);
                assert_eq!(
                    output.foreign.iter().map(Vec::len).collect::<Vec<_>>(),
                    vec![2, 2]
                );
            }
        }
        Ok(())
    })
}
