use super::*;
use nepl3_doc_core::{
    labels::namespace as labels,
    pages::{PageDocument, PageRegistration, PageSet},
};
use nepl3_doc_html::namespace::pages as routed;

fn doc(target: LinkTarget) -> DocumentSyntax {
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
            text: "page".into(),
        },
        DocKind::Body {
            blocks: vec![BlockRef(4)],
        },
        DocKind::Paragraph {
            items: vec![FlowRef(5)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(6), InlineRef(7)],
        },
        DocKind::Anchor {
            id: "target".into(),
            label: InlineRef(2),
        },
        DocKind::Link {
            target,
            label: InlineRef(2),
        },
    ];
    DocumentSyntax {
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
            embeds: Default::default(),
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    }
}
fn fixture() -> PageSet {
    PageSet {
        pages: vec![
            PageDocument {
                registration: PageRegistration {
                    id: "one".into(),
                    source: "source/one.nepld".into(),
                    route: "first/index.html".into(),
                },
                document: doc(LinkTarget::Page {
                    page: "two".into(),
                    fragment: Some("target".into()),
                }),
            },
            PageDocument {
                registration: PageRegistration {
                    id: "two".into(),
                    source: "source/nested/two.nepld".into(),
                    route: "other/deep.html".into(),
                },
                document: doc(LinkTarget::Relative {
                    path: "../one.nepld".into(),
                    fragment: Some("target".into()),
                }),
            },
        ],
        files: vec![],
    }
}
fn with_pages<T>(
    input: &PageSet,
    run: impl FnOnce(&routed::PreparedPages<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let registry = registry()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let roots = input
        .pages
        .iter()
        .map(|p| labels::inspect(&p.document, &registry, &mut b(), &mut admission).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let members = roots.iter().map(|r| [r]).collect::<Vec<_>>();
    let namespaces = members
        .iter()
        .map(|m| labels::resolve(m, &mut b()).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = routed::prepare(
        input,
        &namespaces.iter().collect::<Vec<_>>(),
        &options,
        &registry,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    run(&prepared)
}
#[test]
fn resolved_routes_are_assembled_and_cross_page_anchors_checked() -> Result<(), String> {
    let input = fixture();
    with_pages(&input, |prepared| {
        let output = routed::render(
            prepared,
            &mut |_, _, _, _, _| Err::<_, String>("unexpected foreign".into()),
            &mut b(),
        )
        .map_err(err)?;
        assert_eq!(output.fragments.len(), 2);
        for (i, output) in output.fragments.iter().enumerate() {
            let mut links = 0;
            for node in &output.fragment.markup.fragment.nodes {
                if let nepl3_markup::html::HtmlNode::Element { attributes, .. } = node {
                    for a in attributes {
                        if let nepl3_markup::html::HtmlAttribute::Href {
                            value:
                                nepl3_markup::html::HtmlHref::BetweenArtifacts {
                                    source,
                                    target,
                                    fragment,
                                },
                        } = a
                        {
                            assert_eq!(source, &input.pages[i].registration.route);
                            assert_eq!(target, &input.pages[1 - i].registration.route);
                            assert!(fragment.is_some());
                            links += 1;
                        }
                    }
                }
            }
            assert_eq!(links, 1);
        }
        Ok(())
    })
}
#[test]
fn adopted_but_unemitted_target_does_not_satisfy_cross_page_link() -> Result<(), String> {
    let mut input = fixture();
    if let DocKind::Link {
        target: LinkTarget::Page { fragment, .. },
        ..
    } = &mut input.pages[0].document.value.nodes[7].kind
    {
        *fragment = Some("guest".into());
    }
    let guest = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Inline(InlineRef(0)),
            nodes: vec![
                DocKind::Anchor {
                    id: "guest".into(),
                    label: InlineRef(1),
                },
                DocKind::Text {
                    text: "hidden".into(),
                },
            ]
            .into_iter()
            .map(|kind| DocNode {
                kind,
                locations: vec![],
                origin: None,
                span: None,
            })
            .collect(),
            embeds: Default::default(),
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let registry = registry()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let first = labels::inspect(
        &input.pages[0].document,
        &registry,
        &mut b(),
        &mut admission,
    )
    .map_err(err)?;
    let second = labels::inspect(
        &input.pages[1].document,
        &registry,
        &mut b(),
        &mut admission,
    )
    .map_err(err)?;
    let extra = labels::inspect(&guest, &registry, &mut b(), &mut admission).map_err(err)?;
    let first_members = [&first];
    let second_members = [&second, &extra];
    let one = labels::resolve(&first_members, &mut b()).map_err(err)?;
    let two = labels::resolve(&second_members, &mut b()).map_err(err)?;
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = routed::prepare(
        &input,
        &[&one, &two],
        &options,
        &registry,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    let result = routed::render(
        &prepared,
        &mut |_, _, _, _, _| Err::<_, String>("unexpected foreign".into()),
        &mut b(),
    );
    assert!(matches!(
        result,
        Err(routed::RenderError::MissingAnchor {
            page: 0,
            target: 1,
            ..
        })
    ));
    Ok(())
}
#[test]
fn preexisting_stop_skips_all_callbacks() -> Result<(), String> {
    with_pages(&fixture(), |prepared| {
        let mut called = false;
        let mut budget = b();
        budget.stop(StopReason::Cancelled);
        let result = routed::render(
            prepared,
            &mut |_, _, _, _, _| {
                called = true;
                Err::<_, String>("unexpected".into())
            },
            &mut budget,
        );
        assert!(matches!(
            result,
            Err(routed::RenderError::Stopped(StopReason::Cancelled))
        ));
        assert!(!called);
        Ok(())
    })
}
#[test]
fn unsafe_later_external_link_is_rejected_during_preparation() -> Result<(), String> {
    let mut input = fixture();
    input.pages[1].document.value.nodes[7].kind = DocKind::Link {
        target: LinkTarget::External {
            uri: "javascript:bad".into(),
        },
        label: InlineRef(2),
    };
    let mut called = false;
    assert!(
        with_pages(&input, |_| {
            called = true;
            Ok(())
        })
        .is_err()
    );
    assert!(!called);
    Ok(())
}
