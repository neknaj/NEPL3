//! Doc backend boundaries with explicitly selected independent guests.
mod support;
use nepl3_core::{
    budget::{Budget, StopReason},
    source::{SourceAdmission, SourceStore},
    value::NdfValue,
};
use nepl3_doc_core::{labels::namespace as labels, model::*, prepare::DocRequirement};
use nepl3_doc_html::*;
use nepl3_markup::html::*;
use nepl3_wire::foundation::FoundationCodec;
use support::*;

#[test]
fn first_cbor_receiver_prepares_renders_and_rejects_stale_or_forged_results() -> Result<(), String>
{
    let r = registry()?;
    let req = request(&r, "あ🙂<&\r\n")?;
    let native = render_request(&req, &r)?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let request_bytes = nepl3_wire::encode(
        &portable::request_to_value(&req, &r, &mut codec, &mut b()).map_err(err)?,
        &mut b(),
    )
    .map_err(err)?;
    let reply_bytes = nepl3_wire::encode(
        &portable::foreign::to_value(&native, &r, &mut codec, &mut b()).map_err(err)?,
        &mut b(),
    )
    .map_err(err)?;
    let mut fresh_admission = SourceAdmission::default();
    let mut fresh = FoundationCodec::new(&r, &store, &mut fresh_admission).map_err(err)?;
    let received = portable::request_from_value(
        &nepl3_wire::decode(&request_bytes, &mut b()).map_err(err)?,
        &r,
        &mut fresh,
        &mut b(),
    )
    .map_err(err)?;
    let expected = render_request(&received, &r)?;
    let value = nepl3_wire::decode(&reply_bytes, &mut b()).map_err(err)?;
    assert_eq!(
        portable::foreign::from_value(&value, &expected, &r, &mut fresh, &mut b()).map_err(err)?,
        native
    );
    let markup = &native.fragment.markup;
    let html = serialize(
        &validate(&markup.fragment, markup.slot, &markup.policy, &mut b()).map_err(err)?,
        &mut b(),
    )
    .map_err(err)?;
    assert_eq!(
        html,
        "<article class=\"nepl-doc\" lang=\"ja\"><h1><span>あ🙂&lt;&amp;&#xD;\n</span></h1></article>"
    );
    assert_eq!(
        native
            .fragment
            .origins
            .iter()
            .map(|origin| (origin.element, origin.node))
            .collect::<Vec<_>>(),
        [(0, 0), (1, 0), (2, 1), (3, 1)]
    );
    assert_eq!(
        native.foreign,
        [ForeignPlacement {
            embed: EmbedRef(0),
            first_element: 3,
            elements: 1
        }]
    );
    for change in 0..4 {
        let mut forged = native.clone();
        match change {
            0 => forged.fragment.document_digest = nepl3_core::source::Digest::of(b"wrong"),
            1 => forged.fragment.origins.clear(),
            2 => forged.foreign[0].first_element = 0,
            _ => forged.foreign.clear(),
        }
        let forged = portable::foreign::to_value(&forged, &r, &mut fresh, &mut b()).map_err(err)?;
        assert!(matches!(
            portable::foreign::from_value(&forged, &expected, &r, &mut fresh, &mut b()),
            Err(portable::PortableError::Mismatch)
        ));
    }
    let changed = render_request(&request(&r, "changed")?, &r)?;
    assert!(matches!(
        portable::foreign::from_value(&value, &changed, &r, &mut fresh, &mut b()),
        Err(portable::PortableError::Mismatch)
    ));
    let mut columns = received.clone();
    columns.options.parallel = ParallelMode::Columns;
    let columns = render_request(&columns, &r)?;
    assert_eq!(columns.fragment.markup, native.fragment.markup);
    assert!(matches!(
        portable::foreign::from_value(&value, &columns, &r, &mut fresh, &mut b()),
        Err(portable::PortableError::Mismatch)
    ));
    assert!(matches!(
        portable::foreign::from_value(&NdfValue::Unit, &expected, &r, &mut fresh, &mut b()),
        Err(portable::PortableError::Schema(_))
    ));
    Ok(())
}

#[test]
fn output_budget_and_browser_depth_never_silently_flatten_the_document() -> Result<(), String> {
    let r = registry()?;
    let mut req = request(&r, "title")?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let p = prepare_article_with_foreign(&req.document, &req.options, &r, &mut codec, &mut b())
        .map_err(err)?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::NodeLimit => limits.nodes = 0,
            StopReason::DepthLimit => limits.depth = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            _ => {}
        }
        let mut limited = Budget::new(limits);
        if reason == StopReason::Cancelled {
            limited.cancel();
        }
        let mut calls = 0;
        let result = render_article_with_foreign(
            &p,
            &mut |slot, embed, b| {
                calls += 1;
                adapter(slot, embed, b)
            },
            &mut limited,
        );
        assert!(
            matches!(result, Err(ForeignRenderError::Render(RenderError::Stopped(actual))) if actual == reason)
        );
        assert_eq!(limited.poll(), Err(reason));
        assert_eq!(calls, 0);
    }
    // The actual Doc tree is deep; rejection must preserve every input node.
    for i in 0..800 {
        let body = 2 + i * 3;
        req.document.value.nodes[body as usize].kind = DocKind::Body {
            blocks: vec![BlockRef(body + 1)],
        };
        req.document.value.nodes.extend([
            node(DocKind::List {
                kind: ListKind::Unordered,
                items: vec![ListItemRef(body + 2)],
            }),
            node(DocKind::ListItem {
                checked: None,
                body: BodyRef(body + 3),
            }),
            node(DocKind::Body { blocks: vec![] }),
        ]);
    }
    let p = prepare_article_with_foreign(&req.document, &req.options, &r, &mut codec, &mut b())
        .map_err(err)?;
    assert!(matches!(
        render_article_with_foreign(&p, &mut adapter, &mut b()),
        Err(ForeignRenderError::Render(RenderError::OutputDepth { .. }))
    ));
    assert_eq!(req.document.value.nodes.len(), 2403);
    Ok(())
}

#[test]
fn namespace_checks_final_selected_markup_and_rejects_unresolved_dependencies() -> Result<(), String>
{
    let r = registry()?;
    let mut req = request(&r, "title")?;
    req.document.value.nodes[2].kind = DocKind::Body {
        blocks: vec![BlockRef(3)],
    };
    req.document.value.nodes.extend([
        node(DocKind::Paragraph {
            items: vec![FlowRef(4)],
        }),
        node(DocKind::Parallel {
            variants: vec![VariantRef(5), VariantRef(7)],
        }),
        node(DocKind::Variant {
            language: "en".into(),
            sentence: SentenceRef(6),
        }),
        node(DocKind::Sentence {
            syntax: EmbedRef(1),
        }),
        node(DocKind::Variant {
            language: "ja".into(),
            sentence: SentenceRef(8),
        }),
        node(DocKind::Sentence {
            syntax: EmbedRef(2),
        }),
    ]);
    req.document.value.embeds.extend([
        req.document.value.embeds[0].clone(),
        req.document.value.embeds[0].clone(),
    ]);
    let mut admission = SourceAdmission::default();
    let member = labels::inspect(&req.document, &r, &mut b(), &mut admission).map_err(err)?;
    let members = [&member];
    let checked = labels::resolve(&members, &mut b()).map_err(err)?;
    let empty = SourceStore::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut selected = |_, _: &DocEmbed, embed: EmbedRef, _: &mut Budget| {
        let attribute = if embed.0 == 1 {
            HtmlAttribute::Href {
                value: HtmlHref::Fragment {
                    id: "target".into(),
                },
            }
        } else {
            HtmlAttribute::Id {
                value: "target".into(),
            }
        };
        Ok::<_, String>(if embed.0 == 0 {
            text("title")
        } else {
            HtmlRequest {
                fragment: HtmlFragment {
                    root: 0,
                    nodes: vec![HtmlNode::Element {
                        tag: if embed.0 == 1 {
                            HtmlTag::A
                        } else {
                            HtmlTag::Span
                        },
                        attributes: vec![attribute],
                        children: vec![],
                    }],
                },
                slot: HtmlSlot::Phrasing,
                policy: HtmlPolicy { classes: vec![] },
            }
        })
    };
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        if reason == StopReason::WorkLimit {
            limits.work = 0;
        }
        if reason == StopReason::AllocationLimit {
            limits.allocation_units = 0;
        }
        let mut limited = Budget::new(limits);
        if reason == StopReason::Cancelled {
            limited.cancel();
        }
        assert!(
            matches!(namespace::prepare_with_foreign(&checked, &req.options, &r, &mut codec, &mut limited), Err(LocalPreparationError::Stopped(actual)) if actual == reason)
        );
        assert_eq!(limited.poll(), Err(reason));
    }
    let all = namespace::prepare_with_foreign(&checked, &req.options, &r, &mut codec, &mut b())
        .map_err(err)?;
    namespace::render_with_foreign(&all, &mut selected, &mut b()).map_err(err)?;
    let single = RenderOptions {
        parallel: ParallelMode::Single {
            language: "en".into(),
            fallbacks: vec![],
        },
    };
    let selected_plan =
        namespace::prepare_with_foreign(&checked, &single, &r, &mut codec, &mut b())
            .map_err(err)?;
    assert!(matches!(
        namespace::render_with_foreign(&selected_plan, &mut selected, &mut b()),
        Err(namespace::ForeignNamespaceError::Namespace(
            namespace::Error::Render(RenderError::Markup(HtmlError::MissingFragment(_)))
        ))
    ));
    let link = document(
        DocRoot::Inline(InlineRef(0)),
        vec![DocKind::Link {
            target: LinkTarget::Relative {
                path: "other.nepld".into(),
                fragment: None,
            },
            label: EmbedRef(0),
        }],
        vec![embed(&r, EmbedKind::SentenceInline, "label")?],
    );
    let member =
        labels::inspect(&link, &r, &mut b(), &mut SourceAdmission::default()).map_err(err)?;
    let members = [&member];
    let checked = labels::resolve(&members, &mut b()).map_err(err)?;
    assert!(matches!(
        namespace::prepare_with_foreign(&checked, &req.options, &r, &mut codec, &mut b()),
        Err(LocalPreparationError::NeedsResolution(_))
    ));
    Ok(())
}

#[test]
fn namespace_wrapper_is_included_in_the_output_depth_envelope() -> Result<(), String> {
    let r = registry()?;
    let doc = document(
        DocRoot::Inline(InlineRef(0)),
        vec![DocKind::InlineMath {
            syntax: EmbedRef(0),
        }],
        vec![embed(&r, EmbedKind::InlineMath, "guest")?],
    );
    let options = options();
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let member = labels::inspect(&doc, &r, &mut b(), &mut admission).map_err(err)?;
    let members = [&member];
    let checked = labels::resolve(&members, &mut b()).map_err(err)?;
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let local =
        prepare_inline_with_foreign(&doc, &options, &r, &mut codec, &mut b()).map_err(err)?;
    // Local wrapper + 255 guest nodes reaches the 256-element envelope exactly.
    render_inline_with_foreign(&local, &mut |_, _, _| Ok::<_, String>(deep(255)), &mut b())
        .map_err(err)?;
    let composed = namespace::prepare_with_foreign(&checked, &options, &r, &mut codec, &mut b())
        .map_err(err)?;
    assert!(matches!(
        namespace::render_with_foreign(
            &composed,
            &mut |_, _, _, _| Ok::<_, String>(deep(255)),
            &mut b()
        ),
        Err(namespace::ForeignNamespaceError::Namespace(
            namespace::Error::OutputDepth { .. }
        ))
    ));
    Ok(())
}

#[test]
fn namespace_resolves_forward_labels_and_preserves_guest_occurrence_owners() -> Result<(), String> {
    let r = registry()?;
    let label = embed(&r, EmbedKind::SentenceInline, "字")?;
    let reference = document(
        DocRoot::Inline(InlineRef(0)),
        vec![DocKind::Reference {
            target: "target".into(),
            label: EmbedRef(0),
        }],
        vec![label.clone()],
    );
    let anchor = document(
        DocRoot::Inline(InlineRef(0)),
        vec![DocKind::Anchor {
            id: "target".into(),
            label: EmbedRef(0),
        }],
        vec![label],
    );
    let mut admission = SourceAdmission::default();
    let left = labels::inspect(&reference, &r, &mut b(), &mut admission).map_err(err)?;
    let right = labels::inspect(&anchor, &r, &mut b(), &mut admission).map_err(err)?;
    let members = [&left, &right];
    let checked = labels::resolve(&members, &mut b()).map_err(err)?;
    let empty = SourceStore::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let options = options();
    let prepared = namespace::prepare_with_foreign(&checked, &options, &r, &mut codec, &mut b())
        .map_err(err)?;
    // The guest owns its internal text nodes. Doc owns the enclosing anchor/ref.
    let rendered = namespace::render_with_foreign(
        &prepared,
        &mut |_, _, _, _| {
            Ok::<_, String>(HtmlRequest {
                fragment: HtmlFragment {
                    root: 0,
                    nodes: vec![
                        HtmlNode::Element {
                            tag: HtmlTag::Span,
                            attributes: vec![],
                            children: vec![1, 2],
                        },
                        HtmlNode::Text { text: "字".into() },
                        HtmlNode::Text { text: "じ".into() },
                    ],
                },
                slot: HtmlSlot::Phrasing,
                policy: HtmlPolicy { classes: vec![] },
            })
        },
        &mut b(),
    )
    .map_err(err)?;
    let mut ids = 0;
    let mut refs = 0;
    for node in &rendered.namespace.markup.fragment.nodes {
        if let HtmlNode::Element { attributes, .. } = node {
            for attribute in attributes {
                match attribute {
                    HtmlAttribute::Id { value } if value == "n-746172676574" => ids += 1,
                    HtmlAttribute::Href {
                        value: HtmlHref::Fragment { id },
                    } if id == "n-746172676574" => refs += 1,
                    _ => {}
                }
            }
        }
    }
    assert_eq!((ids, refs), (1, 1));
    assert_eq!(rendered.foreign.len(), 2);
    for (index, placement) in rendered.foreign.iter().enumerate() {
        assert_eq!(placement.member, labels::MemberId(index as u64));
        assert_eq!(placement.embed, EmbedRef(0));
        assert_eq!(placement.elements, 3);
        for element in placement.first_element..placement.first_element + placement.elements {
            assert_eq!(
                rendered
                    .namespace
                    .origins
                    .iter()
                    .filter(|origin| origin.element == element)
                    .map(|origin| (origin.member, origin.node))
                    .collect::<Vec<_>>(),
                [(placement.member, 0)]
            );
        }
    }
    for text in ["字", "じ"] {
        assert_eq!(
            rendered
                .namespace
                .markup
                .fragment
                .nodes
                .iter()
                .filter(|node| matches!(node, HtmlNode::Text { text: value } if value == text))
                .count(),
            2
        );
    }
    Ok(())
}

#[test]
fn inline_fragment_rejects_wrong_root_labels_dependencies_and_stops() -> Result<(), String> {
    use nepl3_doc_core::{labels::LabelError, prepare::PreparationError};
    let r = registry()?;
    let article = request(&r, "title")?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let options = options();
    assert!(matches!(
        prepare_inline_with_foreign(&article.document, &options, &r, &mut codec, &mut b()),
        Err(LocalPreparationError::Input(PreparationError::Label(
            LabelError::ExpectedInline
        )))
    ));
    let label = embed(&r, EmbedKind::SentenceInline, "label")?;
    let mut doc = document(
        DocRoot::Inline(InlineRef(0)),
        vec![DocKind::Reference {
            target: "missing".into(),
            label: EmbedRef(0),
        }],
        vec![label],
    );
    assert!(matches!(
        prepare_inline_with_foreign(&doc, &options, &r, &mut codec, &mut b()),
        Err(LocalPreparationError::Input(PreparationError::Label(
            LabelError::Unresolved { .. }
        )))
    ));
    doc.value.nodes[0].kind = DocKind::Anchor {
        id: "duplicate".into(),
        label: EmbedRef(0),
    };
    let first =
        labels::inspect(&doc, &r, &mut b(), &mut SourceAdmission::default()).map_err(err)?;
    let second =
        labels::inspect(&doc, &r, &mut b(), &mut SourceAdmission::default()).map_err(err)?;
    assert!(matches!(labels::resolve(&[&first, &second], &mut b()),
        Err(labels::Error::Duplicate { definition, previous })
        if definition.member == labels::MemberId(1)
            && previous.member == labels::MemberId(0)
            && definition.site.name == "duplicate"));
    doc.value.nodes[0].kind = DocKind::Link {
        target: LinkTarget::Relative {
            path: "other.nepld".into(),
            fragment: None,
        },
        label: EmbedRef(0),
    };
    let Err(LocalPreparationError::NeedsResolution(plan)) =
        prepare_inline_with_foreign(&doc, &options, &r, &mut codec, &mut b())
    else {
        return Err("missing link resolution".into());
    };
    let links: Vec<_> = plan
        .requirements
        .iter()
        .filter_map(|requirement| match requirement {
            DocRequirement::Link { node, target } => Some((*node, target.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        links,
        [(
            0,
            LinkTarget::Relative {
                path: "other.nepld".into(),
                fragment: None
            }
        )]
    );
    doc.value.nodes[0].kind = DocKind::Anchor {
        id: "target".into(),
        label: EmbedRef(0),
    };
    let prepared =
        prepare_inline_with_foreign(&doc, &options, &r, &mut codec, &mut b()).map_err(err)?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::DepthLimit => limits.depth = 0,
            _ => unreachable!("fixed cases"),
        }
        let mut limited = Budget::new(limits);
        assert!(
            matches!(prepare_inline_with_foreign(&doc, &options, &r, &mut codec, &mut limited), Err(LocalPreparationError::Stopped(actual)) if actual == reason)
        );
        assert_eq!(limited.poll(), Err(reason));
        let mut limited = Budget::new(limits);
        assert!(
            matches!(render_inline_with_foreign(&prepared, &mut adapter, &mut limited), Err(ForeignRenderError::Render(RenderError::Stopped(actual))) if actual == reason)
        );
        assert_eq!(limited.poll(), Err(reason));
    }
    Ok(())
}
