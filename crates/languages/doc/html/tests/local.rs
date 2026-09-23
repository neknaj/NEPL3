use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceStore},
};
use nepl3_doc_core::model::*;
use nepl3_doc_html::*;
use nepl3_wire::foundation::FoundationCodec;
fn b() -> Budget {
    Budget::new(Limits {
        work: 1_000_000_000,
        allocation_units: 1_000_000_000,
        nodes: 1_000_000,
        depth: 100_000,
        output_bytes: 100_000_000,
        source_bytes: 1_000_000,
        ..Limits::default()
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
        nepl3_markup::schema::descriptor(&mut b()),
        schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn request() -> LocalHtmlRequest {
    let kinds = vec![
        DocKind::Article {
            language: "ja".into(),
            title: SentenceRef(1),
            body: BodyRef(3),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(2)],
        },
        DocKind::Text {
            text: "あ🙂<&\r\n".into(),
        },
        DocKind::Body { blocks: vec![] },
    ];
    LocalHtmlRequest {
        document: DocumentSyntax {
            value: DocValue {
                root: DocRoot::Article(ArticleRef(0)),
                nodes: kinds
                    .into_iter()
                    .map(|kind| DocNode {
                        kind,
                        locations: vec![],
                        origin: None,
                        span: None,
                    })
                    .collect(),
                embeds: vec![],
            },
            sources: vec![],
            origins: vec![],
            views: vec![],
            source_maps: vec![],
        },
        options: RenderOptions {
            parallel: ParallelMode::Rows,
        },
    }
}
#[test]
fn first_cbor_receiver_prepares_renders_and_rejects_stale_or_forged_results() -> Result<(), String>
{
    let r = registry()?;
    let req = request();
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    let p = prepare_local(&req.document, &req.options, &r, &mut c, &mut b()).map_err(err)?;
    let native = render(&p, &mut b()).map_err(err)?;
    let raw = portable::rendered_to_value(&native, &p, &r, &mut c, &mut b()).map_err(err)?;
    let request_bytes = nepl3_wire::encode(
        &portable::request_to_value(&req, &r, &mut c, &mut b()).map_err(err)?,
        &mut b(),
    )
    .map_err(err)?;
    let reply_bytes = nepl3_wire::encode(&raw, &mut b()).map_err(err)?;
    let mut fresh_a = SourceAdmission::default();
    let empty = SourceStore::default();
    let mut fresh = FoundationCodec::new(&r, &empty, &mut fresh_a).map_err(err)?;
    let received = portable::request_from_value(
        &nepl3_wire::decode(&request_bytes, &mut b()).map_err(err)?,
        &r,
        &mut fresh,
        &mut b(),
    )
    .map_err(err)?;
    let prepared = prepare_local(
        &received.document,
        &received.options,
        &r,
        &mut fresh,
        &mut b(),
    )
    .map_err(err)?;
    let value = nepl3_wire::decode(&reply_bytes, &mut b()).map_err(err)?;
    assert_eq!(
        portable::rendered_from_value(&value, &prepared, &r, &mut fresh, &mut b()).map_err(err)?,
        native
    );
    let html = nepl3_markup::html::serialize(
        &nepl3_markup::html::validate(
            &native.markup.fragment,
            native.markup.slot,
            &native.markup.policy,
            &mut b(),
        )
        .map_err(err)?,
        &mut b(),
    )
    .map_err(err)?;
    assert_eq!(
        html,
        "<article class=\"nepl-doc\" lang=\"ja\"><h1><span>あ🙂&lt;&amp;&#xD;\n</span></h1></article>"
    );
    assert_eq!(
        native.origins,
        vec![
            ElementOrigin {
                element: 0,
                node: 0
            },
            ElementOrigin {
                element: 1,
                node: 0
            },
            ElementOrigin {
                element: 2,
                node: 1
            },
            ElementOrigin {
                element: 3,
                node: 2
            }
        ]
    );
    for field in [0, 3] {
        let mut v = value.clone();
        let nepl3_core::value::NdfValue::Record(record) = &mut v else {
            return Err("fragment".into());
        };
        record.fields[field] = if field == 0 {
            nepl3_core::value::NdfValue::Bytes(vec![0; 32])
        } else {
            nepl3_core::value::NdfValue::List(vec![])
        };
        assert!(portable::rendered_from_value(&v, &prepared, &r, &mut fresh, &mut b()).is_err());
    }
    let mut stale = received.clone();
    if let DocKind::Text { text } = &mut stale.document.value.nodes[2].kind {
        *text = "changed".into();
    }
    let p =
        prepare_local(&stale.document, &stale.options, &r, &mut fresh, &mut b()).map_err(err)?;
    assert!(portable::rendered_from_value(&value, &p, &r, &mut fresh, &mut b()).is_err());
    // No Parallel node exists here: different options have identical HTML,
    // but a reply from a different generation request must still be rejected.
    let columns = RenderOptions {
        parallel: ParallelMode::Columns,
    };
    let p = prepare_local(&received.document, &columns, &r, &mut fresh, &mut b()).map_err(err)?;
    assert_eq!(render(&p, &mut b()).map_err(err)?.markup, native.markup);
    assert!(portable::rendered_from_value(&value, &p, &r, &mut fresh, &mut b()).is_err());
    Ok(())
}
#[test]
fn output_budget_and_browser_depth_never_silently_flatten_the_document() -> Result<(), String> {
    let r = registry()?;
    let mut req = request();
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    let p = prepare_local(&req.document, &req.options, &r, &mut c, &mut b()).map_err(err)?;
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
        assert_eq!(render(&p, &mut limited), Err(RenderError::Stopped(reason)));
        assert_eq!(limited.poll(), Err(reason));
    }
    let len = 800;
    if let DocKind::Sentence { inlines } = &mut req.document.value.nodes[1].kind {
        *inlines = vec![InlineRef(4)];
    }
    for i in 0..len {
        req.document.value.nodes.push(DocNode {
            kind: DocKind::Concat {
                inlines: vec![InlineRef(if i + 1 == len { 2 } else { 5 + i })],
            },
            locations: vec![],
            origin: None,
            span: None,
        });
    }
    let p = prepare_local(&req.document, &req.options, &r, &mut c, &mut b()).map_err(err)?;
    assert!(matches!(
        render(&p, &mut b()),
        Err(RenderError::OutputDepth { .. })
    ));
    assert_eq!(req.document.value.nodes.len(), 804);
    Ok(())
}

fn inline_request() -> LocalHtmlRequest {
    let mut req = request();
    req.document.value.root = DocRoot::Inline(InlineRef(0));
    req.document.value.nodes = vec![
        DocKind::Concat {
            inlines: vec![InlineRef(1), InlineRef(2)],
        },
        DocKind::Reference {
            target: "target".into(),
            label: InlineRef(3),
        },
        DocKind::Anchor {
            id: "target".into(),
            label: InlineRef(3),
        },
        DocKind::Ruby {
            base: InlineRef(4),
            reading: InlineRef(5),
        },
        DocKind::Text { text: "字".into() },
        DocKind::Text { text: "じ".into() },
    ]
    .into_iter()
    .map(|kind| DocNode {
        kind,
        locations: vec![],
        origin: None,
        span: None,
    })
    .collect();
    req
}

#[test]
fn inline_fragment_resolves_forward_labels_and_preserves_ruby_owners() -> Result<(), String> {
    use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode, HtmlSlot};
    let r = registry()?;
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    let req = inline_request();
    let checked = nepl3_doc_core::labels::check_inline(
        &req.document,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(checked.definitions().len(), 1);
    assert_eq!(checked.references().len(), 1);
    assert_eq!(
        checked.references()[0].target,
        nepl3_doc_core::labels::DocLabelId(0)
    );
    assert_eq!(checked.definitions()[0].node, 2);
    let p = prepare_local_inline(&req.document, &req.options, &r, &mut c, &mut b()).map_err(err)?;
    let rendered = render_inline(&p, &mut b()).map_err(err)?;
    assert_eq!(rendered.markup.slot, HtmlSlot::Phrasing);
    let mut ids = 0;
    let mut refs = 0;
    for node in &rendered.markup.fragment.nodes {
        if let HtmlNode::Element { attributes, .. } = node {
            for attr in attributes {
                match attr {
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
    // Both display occurrences retain the same semantic Text owners.
    for (owner, text) in [(4, "字"), (5, "じ")] {
        assert_eq!(rendered.origins.iter().filter(|origin| origin.node == owner
            && matches!(&rendered.markup.fragment.nodes[origin.element as usize], HtmlNode::Text { text: value } if value == text)).count(), 2);
    }
    assert_eq!(req.document.value.nodes.len(), 6);
    Ok(())
}

#[test]
fn inline_fragment_rejects_wrong_root_labels_dependencies_and_stops() -> Result<(), String> {
    use nepl3_doc_core::{
        labels::LabelError,
        prepare::{DocRequirement, PreparationError},
    };
    let r = registry()?;
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    let article = request();
    assert!(matches!(
        prepare_local_inline(&article.document, &article.options, &r, &mut c, &mut b()),
        Err(LocalPreparationError::Input(PreparationError::Label(
            LabelError::ExpectedInline
        )))
    ));
    let mut req = inline_request();
    req.document.value.nodes[2].kind = DocKind::Anchor {
        id: "other".into(),
        label: InlineRef(3),
    };
    assert!(matches!(
        prepare_local_inline(&req.document, &req.options, &r, &mut c, &mut b()),
        Err(LocalPreparationError::Input(PreparationError::Label(
            LabelError::Unresolved { .. }
        )))
    ));
    req.document.value.nodes[1].kind = DocKind::Anchor {
        id: "other".into(),
        label: InlineRef(3),
    };
    assert!(matches!(
        prepare_local_inline(&req.document, &req.options, &r, &mut c, &mut b()),
        Err(LocalPreparationError::Input(PreparationError::Label(
            LabelError::Duplicate { .. }
        )))
    ));
    req.document.value.nodes[1].kind = DocKind::Link {
        target: LinkTarget::External {
            uri: "https://example.test/".into(),
        },
        label: InlineRef(3),
    };
    let Err(LocalPreparationError::NeedsResolution(plan)) =
        prepare_local_inline(&req.document, &req.options, &r, &mut c, &mut b())
    else {
        return Err("external link must remain an explicit requirement".into());
    };
    assert!(
        matches!(plan.requirements.as_slice(), [DocRequirement::Link { node: 1, target: LinkTarget::External { uri } }] if uri == "https://example.test/")
    );
    let req = inline_request();
    let p = prepare_local_inline(&req.document, &req.options, &r, &mut c, &mut b()).map_err(err)?;
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
            _ => unreachable!("fixed test cases"),
        }
        let mut limited = Budget::new(limits);
        assert!(
            matches!(prepare_local_inline(&req.document, &req.options, &r, &mut c, &mut limited),
            Err(LocalPreparationError::Stopped(actual)) if actual == reason)
        );
        assert_eq!(limited.poll(), Err(reason));
        let mut limited = Budget::new(limits);
        assert_eq!(
            render_inline(&p, &mut limited),
            Err(RenderError::Stopped(reason))
        );
        assert_eq!(limited.poll(), Err(reason));
    }
    Ok(())
}
