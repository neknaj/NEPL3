use super::*;
use nepl3_doc_core::labels::namespace as labels;
use nepl3_doc_html::assets::{AssetError, SvgInput, SvgMode};
use nepl3_doc_html::namespace::assets::{MemberAssets, PreparationError, prepare};
use nepl3_markup::html::{HtmlAttribute, HtmlNode};
const SVG: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><path d='M0 0L10 0L5 10Z'/></svg>";
const OTHER: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20'><path d='M0 0L20 0L5 10Z'/></svg>";
fn image_request() -> LocalHtmlRequest {
    let mut req = request();
    req.document.value.nodes[3].kind = DocKind::Body {
        blocks: vec![BlockRef(4)],
    };
    req.document.value.nodes.push(DocNode {
        kind: DocKind::Image {
            asset: AssetRef {
                id: "shape".into(),
                digest: None,
            },
            alt: SentenceRef(1),
            caption: None,
        },
        locations: vec![],
        origin: None,
        span: None,
    });
    req
}
#[test]
fn namespace_svg_inputs_are_occurrence_bound_and_render_through_display() -> Result<(), String> {
    let r = registry()?;
    let req = image_request();
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let member = labels::inspect(&req.document, &r, &mut b(), &mut admission).map_err(err)?;
    // Repeated identical owners still have distinct selected occurrences.
    let members = [&member, &member];
    let checked = labels::resolve(&members, &mut b()).map_err(err)?;
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let one = [SvgInput {
        id: "shape",
        svg: SVG,
    }];
    let two = [SvgInput {
        id: "shape",
        svg: OTHER,
    }];
    let inputs = [
        MemberAssets {
            member: labels::MemberId(0),
            inputs: &one,
            resolved_alt: &[],
        },
        MemberAssets {
            member: labels::MemberId(1),
            inputs: &two,
            resolved_alt: &[],
        },
    ];
    for mode in [SvgMode::External, SvgMode::Embedded] {
        let prepared = prepare(
            &checked,
            &req.options,
            &inputs,
            mode,
            &r,
            &mut codec,
            &mut b(),
        )
        .map_err(err)?;
        let mut results = Vec::new();
        for index in 0..2 {
            let output = namespace::display::render_article(
                &prepared,
                labels::MemberId(index),
                &mut |_, _, _| Err::<nepl3_markup::html::HtmlRequest, _>("unexpected guest"),
                &mut b(),
            )
            .map_err(err)?;
            let attrs: Vec<_> = output
                .fragment
                .markup
                .fragment
                .nodes
                .iter()
                .filter_map(|n| match n {
                    HtmlNode::Element { attributes, .. } => Some(attributes),
                    _ => None,
                })
                .flatten()
                .filter(|a| {
                    matches!(
                        a,
                        HtmlAttribute::Src { .. } | HtmlAttribute::EmbeddedSvg { .. }
                    )
                })
                .cloned()
                .collect();
            // Image deliberately emits preview and full-size images.
            assert_eq!(attrs.len(), 2);
            assert_eq!(attrs[0], attrs[1]);
            let baseline = assets::prepare_svg_display(
                &req.document,
                &req.options,
                if index == 0 { &one } else { &two },
                mode,
                &r,
                &mut codec,
                &mut b(),
            )
            .map_err(err)?;
            let baseline = assets::render_svg_display(
                &baseline,
                &mut |_, _, _| Err::<nepl3_markup::html::HtmlRequest, _>("unexpected guest"),
                &mut b(),
            )
            .map_err(err)?;
            assert_eq!(output.fragment.markup, baseline.fragment.markup);
            results.push(attrs);
        }
        assert_ne!(results[0], results[1]);
    }
    // Existing namespace API remains asset-free.
    assert!(matches!(
        namespace::display::prepare(&checked, &req.options, &r, &mut codec, &mut b()),
        Err(LocalPreparationError::NeedsResolution(_))
    ));
    assert!(matches!(
        prepare(
            &checked,
            &req.options,
            &inputs[..1],
            SvgMode::Embedded,
            &r,
            &mut codec,
            &mut b()
        ),
        Err(PreparationError::Member {
            member: labels::MemberId(1),
            error: AssetError::Missing
        })
    ));
    let duplicates = [
        MemberAssets {
            member: labels::MemberId(0),
            inputs: &one,
            resolved_alt: &[],
        },
        MemberAssets {
            member: labels::MemberId(0),
            inputs: &two,
            resolved_alt: &[],
        },
    ];
    assert!(matches!(
        prepare(
            &checked,
            &req.options,
            &duplicates,
            SvgMode::Embedded,
            &r,
            &mut codec,
            &mut b()
        ),
        Err(PreparationError::DuplicateMember(labels::MemberId(0)))
    ));
    let unknown = [MemberAssets {
        member: labels::MemberId(u64::MAX),
        inputs: &one,
        resolved_alt: &[],
    }];
    assert!(matches!(
        prepare(
            &checked,
            &req.options,
            &unknown,
            SvgMode::Embedded,
            &r,
            &mut codec,
            &mut b()
        ),
        Err(PreparationError::InvalidMember(_))
    ));
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
        let mut budget = Budget::new(limits);
        if reason == StopReason::Cancelled {
            budget.cancel();
        }
        assert!(
            prepare(
                &checked,
                &req.options,
                &inputs,
                SvgMode::Embedded,
                &r,
                &mut codec,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn namespace_svg_preserves_cross_member_pending_labels() -> Result<(), String> {
    let r = registry()?;
    let mut req = image_request();
    req.document.value.nodes[2].kind = DocKind::Reference {
        target: "elsewhere".into(),
        label: InlineRef(5),
    };
    req.document.value.nodes.push(DocNode {
        kind: DocKind::Text {
            text: "label".into(),
        },
        locations: vec![],
        origin: None,
        span: None,
    });
    // Alt must remain locally projectable and not inherit the title reference.
    req.document.value.nodes.push(DocNode {
        kind: DocKind::Sentence {
            inlines: vec![InlineRef(5)],
        },
        locations: vec![],
        origin: None,
        span: None,
    });
    if let DocKind::Image { alt, .. } = &mut req.document.value.nodes[4].kind {
        *alt = SentenceRef(6);
    }
    let mut target = req.document.clone();
    target.value.root = DocRoot::Inline(InlineRef(0));
    target.value.nodes = vec![
        DocNode {
            kind: DocKind::Anchor {
                id: "elsewhere".into(),
                label: InlineRef(1),
            },
            locations: vec![],
            origin: None,
            span: None,
        },
        DocNode {
            kind: DocKind::Text {
                text: "target".into(),
            },
            locations: vec![],
            origin: None,
            span: None,
        },
    ];
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let first = labels::inspect(&req.document, &r, &mut b(), &mut admission).map_err(err)?;
    let second = labels::inspect(&target, &r, &mut b(), &mut admission).map_err(err)?;
    let members = [&first, &second];
    let checked = labels::resolve(&members, &mut b()).map_err(err)?;
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let svg = [SvgInput {
        id: "shape",
        svg: SVG,
    }];
    let inputs = [MemberAssets {
        member: labels::MemberId(0),
        inputs: &svg,
        resolved_alt: &[],
    }];
    let prepared = prepare(
        &checked,
        &req.options,
        &inputs,
        SvgMode::Embedded,
        &r,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    // Both individual parts remain valid pending namespace assembly.
    for id in 0..2 {
        namespace::display::render_part(
            &prepared,
            labels::MemberId(id),
            &mut |_, _, _| Err::<nepl3_markup::html::HtmlRequest, _>("unexpected guest"),
            &mut b(),
        )
        .map_err(err)?;
    }
    // Omitted target cannot turn the pending Article into complete output.
    let error = namespace::display::render_article(
        &prepared,
        labels::MemberId(0),
        &mut |_, _, _| Err::<nepl3_markup::html::HtmlRequest, _>("unexpected guest"),
        &mut b(),
    );
    assert!(matches!(
        error,
        Err(namespace::display::ArticleError::Render(
            ForeignRenderError::Render(RenderError::Markup(
                nepl3_markup::html::HtmlError::MissingFragment(_)
            ))
        ))
    ));
    Ok(())
}

#[test]
fn namespace_svg_member_failures_are_not_silently_dropped() -> Result<(), String> {
    let r = registry()?;
    let mut req = image_request();
    let store = SourceStore::default();
    for case in 0..6 {
        if let DocKind::Image { asset, .. } = &mut req.document.value.nodes[4].kind {
            asset.digest = (case == 4).then_some(nepl3_core::source::Digest::of(b"wrong"));
        }
        let mut admission = SourceAdmission::default();
        let member = labels::inspect(&req.document, &r, &mut b(), &mut admission).map_err(err)?;
        let members = [&member, &member];
        let checked = labels::resolve(&members, &mut b()).map_err(err)?;
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let valid = [SvgInput {
            id: "shape",
            svg: SVG,
        }];
        let invalid = match case {
            0 => vec![],
            1 => vec![
                SvgInput {
                    id: "shape",
                    svg: SVG,
                },
                SvgInput {
                    id: "shape",
                    svg: SVG,
                },
            ],
            2 => vec![SvgInput {
                id: "unused",
                svg: SVG,
            }],
            3 => vec![SvgInput {
                id: "shape",
                svg: "<svg><script>alert(1)</script></svg>",
            }],
            _ => vec![SvgInput {
                id: "shape",
                svg: SVG,
            }],
        };
        let inputs = [
            MemberAssets {
                member: labels::MemberId(0),
                inputs: if case == 4 { &invalid } else { &valid },
                resolved_alt: &[],
            },
            MemberAssets {
                member: labels::MemberId(1),
                inputs: &invalid,
                resolved_alt: &[],
            },
        ];
        let result = prepare(
            &checked,
            &req.options,
            &inputs,
            SvgMode::Embedded,
            &r,
            &mut codec,
            &mut b(),
        );
        if case == 5 {
            result.map_err(err)?;
            continue;
        }
        let Err(PreparationError::Member { member, error }) = result else {
            return Err("expected located asset failure".into());
        };
        assert_eq!(member, labels::MemberId(if case == 4 { 0 } else { 1 }));
        assert!(matches!(
            (case, error),
            (0, AssetError::Missing)
                | (1, AssetError::Duplicate)
                | (2, AssetError::Unused)
                | (3, AssetError::InvalidSvg)
                | (4, AssetError::Digest)
        ));
    }
    Ok(())
}

#[test]
fn namespace_svg_shares_source_admission_but_charges_each_raw_asset() -> Result<(), String> {
    use nepl3_core::{
        budget::Resource,
        source::{SourceId, SourceSnapshot},
    };
    let r = registry()?;
    let mut req = image_request();
    let source = SourceSnapshot::new(
        SourceId("shared".into()),
        0,
        "memory:shared".into(),
        "共有".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    req.document.sources.push(source);
    let mut initial_admission = SourceAdmission::default();
    let member =
        labels::inspect(&req.document, &r, &mut b(), &mut initial_admission).map_err(err)?;
    let members = [&member, &member];
    let checked = labels::resolve(&members, &mut b()).map_err(err)?;
    let svg = [SvgInput {
        id: "shape",
        svg: SVG,
    }];
    let inputs = [
        MemberAssets {
            member: labels::MemberId(0),
            inputs: &svg,
            resolved_alt: &[],
        },
        MemberAssets {
            member: labels::MemberId(1),
            inputs: &svg,
            resolved_alt: &[],
        },
    ];
    let exact = 7 + 6 + 2 * SVG.len() as u64;
    for allowance in [exact, exact - 1] {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let mut limits = b().limits();
        limits.source_bytes = allowance;
        let mut budget = Budget::new(limits);
        budget.charge(Resource::SourceBytes, 7).map_err(err)?;
        let result = prepare(
            &checked,
            &req.options,
            &inputs,
            SvgMode::Embedded,
            &r,
            &mut codec,
            &mut budget,
        );
        if allowance == exact {
            result.map_err(err)?;
            assert_eq!(budget.usage().source_bytes, exact);
        } else {
            assert!(matches!(
                result,
                Err(PreparationError::Member {
                    member: labels::MemberId(1),
                    error: AssetError::Stopped(StopReason::SourceLimit)
                })
            ));
            assert_eq!(budget.poll(), Err(StopReason::SourceLimit));
        }
    }
    Ok(())
}

#[test]
fn namespace_svg_rechecks_sources_and_rejects_unresolved_links() -> Result<(), String> {
    use nepl3_core::source::{SourceId, SourceSnapshot};
    let r = registry()?;
    let mut left = image_request();
    let mut right = image_request();
    for (req, text) in [(&mut left, "one"), (&mut right, "two")] {
        req.document.sources.push(
            SourceSnapshot::new(
                SourceId("same".into()),
                0,
                "memory:same".into(),
                text.as_bytes().to_vec(),
                &mut b(),
            )
            .map_err(err)?,
        );
    }
    let first = labels::inspect(
        &left.document,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let second = labels::inspect(
        &right.document,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let members = [&first, &second];
    let checked = labels::resolve(&members, &mut b()).map_err(err)?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    assert!(matches!(
        prepare(
            &checked,
            &left.options,
            &[],
            SvgMode::Embedded,
            &r,
            &mut codec,
            &mut b()
        ),
        Err(PreparationError::Input(
            nepl3_doc_core::prepare::NamespacePreparationError {
                member: Some(labels::MemberId(1)),
                ..
            }
        ))
    ));
    for target in [
        LinkTarget::External {
            uri: "javascript:alert(1)".into(),
        },
        LinkTarget::Page {
            page: "other".into(),
            fragment: None,
        },
        LinkTarget::Relative {
            path: "other.html".into(),
            fragment: None,
        },
    ] {
        let mut req = image_request();
        // Title link plus a separate, locally projectable image alt.
        req.document.value.nodes[2].kind = DocKind::Link {
            target,
            label: InlineRef(5),
        };
        req.document.value.nodes.push(DocNode {
            kind: DocKind::Text {
                text: "label".into(),
            },
            locations: vec![],
            origin: None,
            span: None,
        });
        req.document.value.nodes.push(DocNode {
            kind: DocKind::Sentence {
                inlines: vec![InlineRef(5)],
            },
            locations: vec![],
            origin: None,
            span: None,
        });
        if let DocKind::Image { alt, .. } = &mut req.document.value.nodes[4].kind {
            *alt = SentenceRef(6);
        }
        let mut admission = SourceAdmission::default();
        let member = labels::inspect(&req.document, &r, &mut b(), &mut admission).map_err(err)?;
        let members = [&member];
        let checked = labels::resolve(&members, &mut b()).map_err(err)?;
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let svg = [SvgInput {
            id: "shape",
            svg: SVG,
        }];
        let inputs = [MemberAssets {
            member: labels::MemberId(0),
            inputs: &svg,
            resolved_alt: &[],
        }];
        assert!(matches!(
            prepare(
                &checked,
                &req.options,
                &inputs,
                SvgMode::Embedded,
                &r,
                &mut codec,
                &mut b()
            ),
            Err(PreparationError::Member {
                member: labels::MemberId(0),
                error: AssetError::Unsupported
                    | AssetError::Preparation(LocalPreparationError::InvalidExternalUri { .. })
            })
        ));
    }
    Ok(())
}
