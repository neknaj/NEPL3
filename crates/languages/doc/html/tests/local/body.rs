use super::*;
use nepl3_markup::html::{HtmlAttribute, HtmlNode, HtmlTag};

fn document(kinds: Vec<DocKind>) -> DocumentSyntax {
    let mut document = request().document;
    document.value.root = DocRoot::Body(BodyRef(0));
    document.value.nodes = kinds
        .into_iter()
        .map(|kind| DocNode {
            kind,
            locations: vec![],
            origin: None,
            span: None,
        })
        .collect();
    document
}
fn output(document: &DocumentSyntax, options: &RenderOptions) -> Result<RenderedFragment, String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let prepared = prepare_local_body(document, options, &r, &mut codec, &mut b()).map_err(err)?;
    render_body(&prepared, &mut b()).map_err(err)
}
fn rows() -> RenderOptions {
    RenderOptions {
        parallel: ParallelMode::Rows,
    }
}

#[test]
fn body_empty_fragment_is_neutral_and_has_exact_origins() -> Result<(), String> {
    let doc = document(vec![DocKind::Body { blocks: vec![] }]);
    let result = output(&doc, &rows())?;
    assert_eq!(result.markup.fragment.nodes.len(), 1);
    assert_eq!(
        result.origins,
        vec![ElementOrigin {
            element: 0,
            node: 0
        }]
    );
    assert!(
        matches!(&result.markup.fragment.nodes[0], HtmlNode::Element { tag: HtmlTag::Div, attributes, children } if children.is_empty() && !attributes.iter().any(|a| matches!(a, HtmlAttribute::Lang { .. })))
    );
    Ok(())
}

#[test]
fn body_intro_peer_and_nested_headings_have_no_invented_title() -> Result<(), String> {
    let doc = document(vec![
        DocKind::Body {
            blocks: vec![BlockRef(1), BlockRef(4), BlockRef(9)],
        },
        DocKind::Paragraph {
            items: vec![FlowRef(2)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(3)],
        },
        DocKind::Text {
            text: "intro".into(),
        },
        DocKind::Section {
            id: "first".into(),
            title: SentenceRef(5),
            body: BodyRef(7),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(6)],
        },
        DocKind::Text {
            text: "title".into(),
        },
        DocKind::Body {
            blocks: vec![BlockRef(8)],
        },
        DocKind::Section {
            id: "nested".into(),
            title: SentenceRef(5),
            body: BodyRef(10),
        },
        DocKind::Section {
            id: "second".into(),
            title: SentenceRef(5),
            body: BodyRef(10),
        },
        DocKind::Body { blocks: vec![] },
    ]);
    let result = output(&doc, &rows())?;
    let headings: Vec<_> = result
        .markup
        .fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            HtmlNode::Element {
                tag: HtmlTag::H1, ..
            } => Some(1),
            HtmlNode::Element {
                tag: HtmlTag::H2, ..
            } => Some(2),
            _ => None,
        })
        .collect();
    assert_eq!(headings, vec![1, 2, 1]);
    assert_eq!(result.origins.len(), result.markup.fragment.nodes.len());
    assert!(
        result
            .origins
            .iter()
            .enumerate()
            .all(|(i, o)| o.element == i as u64 && o.node < doc.value.nodes.len() as u64)
    );
    assert!(!result.markup.fragment.nodes.iter().any(|n| matches!(
        n,
        HtmlNode::Element {
            tag: HtmlTag::Article,
            ..
        }
    )));
    Ok(())
}

#[test]
fn body_proof_rejects_article_and_keeps_budget_stops_sticky() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let article = request();
    assert!(
        prepare_local_body(
            &article.document,
            &article.options,
            &r,
            &mut codec,
            &mut b()
        )
        .is_err()
    );
    let doc = document(vec![DocKind::Body { blocks: vec![] }]);
    let options = rows();
    assert!(prepare_local(&doc, &options, &r, &mut codec, &mut b()).is_err());
    let prepared = prepare_local_body(&doc, &options, &r, &mut codec, &mut b()).map_err(err)?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::NodeLimit => limits.nodes = 0,
            _ => {}
        }
        let mut budget = Budget::new(limits);
        if reason == StopReason::Cancelled {
            budget.cancel();
        }
        assert!(
            matches!(render_body(&prepared, &mut budget), Err(RenderError::Stopped(actual)) if actual == reason)
        );
        assert_eq!(budget.poll(), Err(reason));
    }
    Ok(())
}

fn variants() -> DocumentSyntax {
    document(vec![
        DocKind::Body {
            blocks: vec![BlockRef(1)],
        },
        DocKind::Paragraph {
            items: vec![FlowRef(2), FlowRef(4)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(3)],
        },
        DocKind::Reference {
            target: "target".into(),
            label: InlineRef(10),
        },
        DocKind::Parallel {
            variants: vec![VariantRef(5), VariantRef(7)],
        },
        DocKind::Variant {
            language: "en".into(),
            sentence: SentenceRef(6),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(10)],
        },
        DocKind::Variant {
            language: "ja".into(),
            sentence: SentenceRef(8),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(9)],
        },
        DocKind::Anchor {
            id: "target".into(),
            label: InlineRef(10),
        },
        DocKind::Text {
            text: "label".into(),
        },
    ])
}
#[test]
fn body_forward_labels_respect_visible_variant_and_reject_duplicates() -> Result<(), String> {
    let doc = variants();
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    for parallel in [
        ParallelMode::Rows,
        ParallelMode::Columns,
        ParallelMode::Single {
            language: "ja".into(),
            fallbacks: vec![],
        },
    ] {
        output(&doc, &RenderOptions { parallel })?;
    }
    let options = RenderOptions {
        parallel: ParallelMode::Single {
            language: "en".into(),
            fallbacks: vec![],
        },
    };
    let prepared = prepare_local_body(&doc, &options, &r, &mut codec, &mut b()).map_err(err)?;
    assert!(matches!(
        render_body(&prepared, &mut b()),
        Err(RenderError::Markup(
            nepl3_markup::html::HtmlError::MissingFragment(_)
        ))
    ));
    let mut duplicate = variants();
    duplicate.value.nodes[6].kind = DocKind::Sentence {
        inlines: vec![InlineRef(9)],
    };
    assert!(prepare_local_body(&duplicate, &options, &r, &mut codec, &mut b()).is_err());
    let mut missing = variants();
    missing.value.nodes[3].kind = DocKind::Reference {
        target: "missing".into(),
        label: InlineRef(10),
    };
    assert!(prepare_local_body(&missing, &options, &r, &mut codec, &mut b()).is_err());
    Ok(())
}
#[test]
fn body_hidden_external_requirements_are_returned_intact() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    for target in [
        LinkTarget::External {
            uri: "https://example.com/".into(),
        },
        LinkTarget::Relative {
            path: "other.nepld".into(),
            fragment: None,
        },
        LinkTarget::Page {
            page: "other".into(),
            fragment: None,
        },
    ] {
        let mut doc = variants();
        doc.value.nodes[3].kind = DocKind::Text {
            text: "visible".into(),
        };
        doc.value.nodes[9].kind = DocKind::Link {
            target,
            label: InlineRef(10),
        };
        let options = RenderOptions {
            parallel: ParallelMode::Single {
                language: "en".into(),
                fallbacks: vec![],
            },
        };
        let expected =
            nepl3_doc_core::prepare::inspect_body(&doc, &r, &mut codec, &mut b()).map_err(err)?;
        assert!(!expected.requirements.is_empty());
        match prepare_local_body(&doc, &options, &r, &mut codec, &mut b()) {
            Err(LocalPreparationError::NeedsResolution(actual)) => assert_eq!(actual, expected),
            _ => return Err("hidden requirement must not disappear".into()),
        }
    }
    Ok(())
}
#[test]
fn body_deep_headings_use_aria_and_output_depth_is_bounded() -> Result<(), String> {
    fn nested(count: u64) -> DocumentSyntax {
        let mut kinds = vec![
            DocKind::Body { blocks: vec![] },
            DocKind::Sentence { inlines: vec![] },
        ];
        let mut body = 0;
        for i in 0..count {
            let section = kinds.len() as u64;
            kinds[body as usize] = DocKind::Body {
                blocks: vec![BlockRef(section)],
            };
            kinds.push(DocKind::Section {
                id: format!("section-{i}"),
                title: SentenceRef(1),
                body: BodyRef(section + 1),
            });
            kinds.push(DocKind::Body { blocks: vec![] });
            body = section + 1;
        }
        document(kinds)
    }
    let result = output(&nested(7), &rows())?;
    assert!(result.markup.fragment.nodes.iter().any(|node| matches!(node, HtmlNode::Element { tag: HtmlTag::Div, attributes, .. } if attributes.contains(&HtmlAttribute::AriaLevel { value: 7 }))));
    let doc = nested(260);
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let options = rows();
    let prepared = prepare_local_body(&doc, &options, &r, &mut codec, &mut b()).map_err(err)?;
    assert!(matches!(
        render_body(&prepared, &mut b()),
        Err(RenderError::OutputDepth { .. })
    ));
    Ok(())
}
#[test]
fn body_fragment_cannot_be_encoded_with_an_article_proof() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let article = request();
    let prepared = prepare_local(
        &article.document,
        &article.options,
        &r,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    let legitimate = render(&prepared, &mut b()).map_err(err)?;
    let mut fragment = output(&document(vec![DocKind::Body { blocks: vec![] }]), &rows())?;
    fragment.document_digest = legitimate.document_digest;
    fragment.options = legitimate.options;
    assert!(matches!(
        portable::rendered_to_value(&fragment, &prepared, &r, &mut codec, &mut b()),
        Err(portable::PortableError::Mismatch)
    ));
    Ok(())
}

#[test]
fn body_preparation_and_rendering_preserve_depth_and_sticky_limits() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let doc = variants();
    let options = rows();
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::NodeLimit => limits.nodes = 0,
            StopReason::DepthLimit => limits.depth = 0,
            _ => {}
        }
        let mut budget = Budget::new(limits);
        if reason == StopReason::Cancelled {
            budget.cancel();
        }
        assert!(
            matches!(prepare_local_body(&doc, &options, &r, &mut codec, &mut budget), Err(LocalPreparationError::Stopped(actual)) if actual == reason)
        );
        assert_eq!(budget.poll(), Err(reason));
    }
    let prepared = prepare_local_body(&doc, &options, &r, &mut codec, &mut b()).map_err(err)?;
    let mut limits = b().limits();
    limits.depth = 1;
    let mut budget = Budget::new(limits);
    assert!(matches!(
        render_body(&prepared, &mut budget),
        Err(RenderError::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(budget.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

#[test]
fn body_assets_and_invalid_output_are_not_silently_omitted() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let options = rows();
    let doc = document(vec![
        DocKind::Body {
            blocks: vec![BlockRef(1)],
        },
        DocKind::Image {
            asset: AssetRef {
                id: "picture".into(),
                digest: None,
            },
            alt: SentenceRef(2),
            caption: None,
        },
        DocKind::Sentence { inlines: vec![] },
    ]);
    let expected =
        nepl3_doc_core::prepare::inspect_body(&doc, &r, &mut codec, &mut b()).map_err(err)?;
    assert!(!expected.requirements.is_empty());
    match prepare_local_body(&doc, &options, &r, &mut codec, &mut b()) {
        Err(LocalPreparationError::NeedsResolution(actual)) => assert_eq!(actual, expected),
        _ => return Err("asset requirement must be returned".into()),
    }
    let mut invalid = variants();
    invalid.value.nodes[10].kind = DocKind::Text {
        text: "a\0b".into(),
    };
    let prepared = prepare_local_body(&invalid, &options, &r, &mut codec, &mut b()).map_err(err)?;
    assert!(matches!(
        render_body(&prepared, &mut b()),
        Err(RenderError::Markup(nepl3_markup::html::HtmlError::Text {
            error: nepl3_markup::text::TextError::InvalidCharacter { byte: 1, scalar: 0 },
            ..
        }))
    ));
    let overflow = document(vec![
        DocKind::Body {
            blocks: vec![BlockRef(1)],
        },
        DocKind::List {
            kind: ListKind::Ordered {
                start: i32::MAX as u64 + 1,
            },
            items: vec![],
        },
    ]);
    assert!(matches!(
        prepare_local_body(&overflow, &options, &r, &mut codec, &mut b()),
        Err(LocalPreparationError::ListStart { node: 1, .. })
    ));
    Ok(())
}

#[test]
fn body_nonzero_root_owns_wrapper_and_empty_serialization_is_exact() -> Result<(), String> {
    let mut doc = document(vec![
        DocKind::Paragraph { items: vec![] },
        DocKind::Body {
            blocks: vec![BlockRef(0)],
        },
    ]);
    doc.value.root = DocRoot::Body(BodyRef(1));
    let result = output(&doc, &rows())?;
    assert_eq!(
        result.origins[0],
        ElementOrigin {
            element: 0,
            node: 1
        }
    );
    let empty = output(&document(vec![DocKind::Body { blocks: vec![] }]), &rows())?;
    assert_eq!(empty.markup.slot, nepl3_markup::html::HtmlSlot::Block);
    let checked = nepl3_markup::html::validate(
        &empty.markup.fragment,
        empty.markup.slot,
        &empty.markup.policy,
        &mut b(),
    )
    .map_err(err)?;
    let bytes = nepl3_markup::html::serialize(&checked, &mut b()).map_err(err)?;
    assert_eq!(bytes, "<div class=\"nepl-doc\"></div>");
    Ok(())
}

fn closure(r: &SchemaRegistry) -> Result<nepl3_core::syntax::ForeignClosure, String> {
    use nepl3_core::{
        origin::{Origin, OriginId},
        source::{SourceId, SourceSnapshot},
        syntax::{
            Environment, EnvironmentEntry, EnvironmentRef, ForeignClosure, ForeignSyntax, NodeRef,
            SyntaxBundle, SyntaxNode,
        },
    };
    let source = SourceSnapshot::new(
        SourceId("guest".into()),
        1,
        "memory:guest".into(),
        b"opaque guest spelling".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let span = source.span(0, source.text().len() as u64).map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = codec
        .environment_digest(&environment, &mut b())
        .map_err(err)?;
    let schema = r.selected("nepl3.doc", 1).ok_or("schema")?.clone();
    // This is deliberately only a common syntax graph, not a checked Article.
    // Code display cannot acquire a requirement to semantically lower this guest.
    let node = SyntaxNode {
        schema: schema.clone(),
        kind: "View:TextRun".into(),
        fields: vec![],
        head: Some(span.clone()),
        cover: Some(span.clone()),
        origin: OriginId(0),
        token: None,
    };
    Ok(ForeignClosure {
        syntax: ForeignSyntax {
            schema,
            category: "Article".into(),
            root: NodeRef(0),
            bundle: SyntaxBundle {
                sources: vec![source],
                nodes: vec![node],
                origins: vec![Origin::Direct(span)],
                root: NodeRef(0),
                environments: vec![],
                tokens: vec![],
                source_maps: vec![],
            },
            environment: EnvironmentRef { id: 17, digest },
        },
        owner_environment: EnvironmentEntry {
            id: 17,
            digest,
            value: environment,
        },
        owner_origins: vec![],
        owner_sources: vec![],
        owner_source_maps: vec![],
    })
}
fn embeds(kind: EmbedKind, closure: nepl3_core::syntax::ForeignClosure) -> DocEmbeds {
    use nepl3_core::syntax::group::*;
    DocEmbeds {
        kinds: vec![kind],
        group: ForeignClosureGroup {
            provenances: vec![ForeignProvenance {
                origins: closure.owner_origins,
                sources: closure.owner_sources,
                source_maps: closure.owner_source_maps,
            }],
            environments: vec![ForeignOwnerEnvironment {
                provenance: nepl3_core::syntax::group::ProvenanceRef(0),
                environment: closure.owner_environment,
            }],
            entries: vec![ForeignGroupEntry {
                owner: nepl3_core::syntax::group::OwnerEnvironmentRef(0),
                syntax: closure.syntax,
            }],
        },
    }
}

#[test]
fn body_hidden_foreign_and_asset_plan_preserves_identity() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let mut doc = variants();
    doc.value.nodes[3].kind = DocKind::Text {
        text: "visible".into(),
    };
    doc.value.nodes[8].kind = DocKind::Sentence {
        inlines: vec![InlineRef(9), InlineRef(11)],
    };
    doc.value.nodes[9].kind = DocKind::InlineMath {
        syntax: EmbedRef(0),
    };
    doc.value.nodes.push(DocNode {
        kind: DocKind::InlineImage {
            asset: AssetRef {
                id: "hidden".into(),
                digest: None,
            },
            alt: SentenceRef(6),
        },
        locations: vec![],
        origin: None,
        span: None,
    });
    doc.value.embeds = embeds(EmbedKind::InlineMath, closure(&r)?);
    let expected =
        nepl3_doc_core::prepare::inspect_body(&doc, &r, &mut codec, &mut b()).map_err(err)?;
    assert_eq!(expected.requirements.len(), 2);
    let options = RenderOptions {
        parallel: ParallelMode::Single {
            language: "en".into(),
            fallbacks: vec![],
        },
    };
    match prepare_local_body(&doc, &options, &r, &mut codec, &mut b()) {
        Err(LocalPreparationError::NeedsResolution(actual)) => assert_eq!(actual, expected),
        _ => return Err("hidden foreign and asset requirements must remain intact".into()),
    }
    Ok(())
}
