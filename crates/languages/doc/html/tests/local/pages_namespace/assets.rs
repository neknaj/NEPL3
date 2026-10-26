use super::*;
use nepl3_doc_html::{
    assets::SvgInput,
    namespace::{
        assets::MemberAssets,
        pages::assets::{self, PageMemberAssets},
    },
};
const A: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const B: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 2 2'><path d='M0 0L2 2'/></svg>";
fn image_pages() -> PageSet {
    let mut set = fixture();
    for page in &mut set.pages {
        let id = page.document.value.nodes.len() as u64;
        page.document.value.nodes.push(DocNode {
            kind: DocKind::Image {
                asset: AssetRef {
                    id: "logo".into(),
                    digest: None,
                },
                alt: SentenceRef(1),
                caption: None,
            },
            locations: vec![],
            origin: None,
            span: None,
        });
        if let DocKind::Body { blocks } = &mut page.document.value.nodes[3].kind {
            blocks.push(BlockRef(id));
        }
    }
    set
}
fn input<'a>(page: u64, member: u64, svg: &'a [SvgInput<'a>]) -> PageMemberAssets<'a> {
    PageMemberAssets {
        page,
        assets: MemberAssets {
            member: labels::MemberId(member),
            inputs: svg,
            resolved_alt: &[],
        },
    }
}
fn run(
    set: &PageSet,
    inputs: &[PageMemberAssets<'_>],
    budget: &mut Budget,
) -> Result<routed::RenderedPages, String> {
    let registry = registry()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let roots = set
        .pages
        .iter()
        .map(|p| labels::inspect(&p.document, &registry, budget, &mut admission).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let members = roots.iter().map(|r| [r]).collect::<Vec<_>>();
    let namespaces = members
        .iter()
        .map(|m| labels::resolve(m, budget).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = assets::prepare_embedded(
        set,
        &namespaces.iter().collect::<Vec<_>>(),
        &options,
        inputs,
        &registry,
        &mut codec,
        budget,
    )
    .map_err(err)?;
    routed::render(
        &prepared,
        &mut |_, _, _, _, _| Err::<_, String>("unexpected guest".into()),
        budget,
    )
    .map_err(err)
}
#[test]
fn embedded_svg_page_routes_and_exact_page_owned_bytes() -> Result<(), String> {
    let set = image_pages();
    let a = [SvgInput { id: "logo", svg: A }];
    let bb = [SvgInput { id: "logo", svg: B }];
    let output = run(&set, &[input(0, 0, &a), input(1, 0, &bb)], &mut b())?;
    assert_eq!(output.fragments.len(), 2);
    for (index, fragment) in output.fragments.iter().enumerate() {
        let mut images = 0;
        let mut links = 0;
        for node in &fragment.fragment.markup.fragment.nodes {
            if let nepl3_markup::html::HtmlNode::Element { attributes, .. } = node {
                for attribute in attributes {
                    match attribute {
                        nepl3_markup::html::HtmlAttribute::EmbeddedSvg { svg } => {
                            assert_eq!(svg, if index == 0 { A } else { B });
                            images += 1;
                        }
                        nepl3_markup::html::HtmlAttribute::Href {
                            value:
                                nepl3_markup::html::HtmlHref::BetweenArtifacts {
                                    source,
                                    target,
                                    fragment,
                                },
                        } => {
                            assert_eq!(source, &set.pages[index].registration.route);
                            assert_eq!(target, &set.pages[1 - index].registration.route);
                            assert!(fragment.is_some());
                            links += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        assert_eq!((images, links), (2, 1));
    }
    Ok(())
}
#[test]
fn embedded_svg_page_input_failures_are_owned() -> Result<(), String> {
    let set = image_pages();
    let a = [SvgInput { id: "logo", svg: A }];
    for (inputs, expected) in [
        (
            vec![input(0, 0, &a)],
            "page: 1, member: MemberId(0), error: Missing",
        ),
        (vec![input(2, 0, &a)], "InvalidPage(2)"),
        (vec![input(0, 1, &a)], "InvalidMember"),
        (vec![input(0, 0, &a), input(0, 0, &a)], "DuplicateMember"),
    ] {
        let error = run(&set, &inputs, &mut b())
            .err()
            .ok_or("unexpected success")?;
        assert!(error.contains(expected), "{error}");
    }
    for (id, svg, expected) in [
        ("other", A, "Unused"),
        ("logo", "<svg><script>alert(1)</script></svg>", "InvalidSvg"),
    ] {
        let bad = [SvgInput { id, svg }];
        let error = run(&set, &[input(0, 0, &bad), input(1, 0, &a)], &mut b())
            .err()
            .ok_or("unexpected success")?;
        assert!(
            error.contains("page: 0") && error.contains(expected),
            "{error}"
        );
    }
    Ok(())
}
#[test]
fn embedded_svg_page_shared_limits_are_exact() -> Result<(), String> {
    let set = image_pages();
    let a = [SvgInput { id: "logo", svg: A }];
    let inputs = [input(0, 0, &a), input(1, 0, &a)];
    let mut budget = b();
    run(&set, &inputs, &mut budget)?;
    let usage = budget.usage();
    for dimension in 0..6 {
        for short in [false, true] {
            let mut limits = b().limits();
            let less = u64::from(short);
            match dimension {
                0 => limits.work = usage.work - less,
                1 => limits.allocation_units = usage.allocation_units - less,
                2 => limits.nodes = usage.nodes - less,
                3 => limits.depth = usage.depth - less,
                4 => limits.output_bytes = usage.output_bytes - less,
                _ => limits.source_bytes = usage.source_bytes - less,
            }
            let result = run(&set, &inputs, &mut Budget::new(limits));
            assert_eq!(
                result.is_ok(),
                !short,
                "dimension {dimension}: {:?}",
                result.err()
            );
        }
    }
    Ok(())
}

fn guest_image() -> DocumentSyntax {
    DocumentSyntax {
        value: DocValue {
            root: DocRoot::Inline(InlineRef(0)),
            nodes: vec![
                DocKind::Anchor {
                    id: "guest".into(),
                    label: InlineRef(1),
                },
                DocKind::InlineImage {
                    asset: AssetRef {
                        id: "logo".into(),
                        digest: None,
                    },
                    alt: SentenceRef(2),
                },
                DocKind::Sentence {
                    inlines: vec![InlineRef(3)],
                },
                DocKind::Text {
                    text: "guest alt".into(),
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
    }
}
fn with_guest<T>(
    set: &PageSet,
    guest: &DocumentSyntax,
    inputs: &[PageMemberAssets<'_>],
    f: impl FnOnce(&routed::PreparedPages<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let registry = registry()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let roots = set
        .pages
        .iter()
        .map(|p| labels::inspect(&p.document, &registry, &mut b(), &mut admission).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let extra = labels::inspect(guest, &registry, &mut b(), &mut admission).map_err(err)?;
    let first = [&roots[0]];
    let second = [&roots[1], &extra];
    let one = labels::resolve(&first, &mut b()).map_err(err)?;
    let two = labels::resolve(&second, &mut b()).map_err(err)?;
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = assets::prepare_embedded(
        set,
        &[&one, &two],
        &options,
        inputs,
        &registry,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    f(&prepared)
}
#[test]
fn embedded_svg_page_adopted_member_keeps_its_own_asset_binding() -> Result<(), String> {
    let set = image_pages();
    let guest = guest_image();
    let a = [SvgInput { id: "logo", svg: A }];
    let bb = [SvgInput { id: "logo", svg: B }];
    let inputs = [input(0, 0, &a), input(1, 0, &a), input(1, 1, &bb)];
    with_guest(&set, &guest, &inputs, |prepared| {
        let page = prepared.page(1).ok_or("page")?;
        let part = nepl3_doc_html::namespace::display::render_part(
            page,
            labels::MemberId(1),
            &mut |_, _, _| Err::<_, String>("unexpected guest".into()),
            &mut b(),
        )
        .map_err(err)?;
        let (member, _, markup, _) = part.part.into_parts();
        assert_eq!(member, labels::MemberId(1));
        let images = markup
            .fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                nepl3_markup::html::HtmlNode::Element { attributes, .. } => Some(attributes),
                _ => None,
            })
            .flatten()
            .filter_map(|attr| match attr {
                nepl3_markup::html::HtmlAttribute::EmbeddedSvg { svg } => Some(svg.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(images, vec![B]);
        Ok(())
    })?;
    let error = with_guest(&set, &guest, &inputs[..2], |_| Ok(()))
        .err()
        .ok_or("missing guest asset accepted")?;
    assert!(
        error.contains("page: 1, member: MemberId(1), error: Missing"),
        "{error}"
    );
    Ok(())
}
#[test]
fn embedded_svg_page_prepared_guest_anchor_is_not_emission_proof() -> Result<(), String> {
    let mut set = image_pages();
    if let DocKind::Link {
        target: LinkTarget::Page { fragment, .. },
        ..
    } = &mut set.pages[0].document.value.nodes[7].kind
    {
        *fragment = Some("guest".into());
    }
    let guest = guest_image();
    let a = [SvgInput { id: "logo", svg: A }];
    let inputs = [input(0, 0, &a), input(1, 0, &a), input(1, 1, &a)];
    with_guest(&set, &guest, &inputs, |prepared| {
        let result = routed::render(
            prepared,
            &mut |_, _, _, _, _| Err::<_, String>("unexpected guest".into()),
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
    })
}

#[allow(clippy::type_complexity)]
fn run_external_mode(
    set: &PageSet,
    inputs: &[PageMemberAssets<'_>],
    budget: &mut Budget,
    render: bool,
) -> Result<
    (
        routed::RenderedPages,
        Vec<(u64, labels::MemberId, String, String)>,
    ),
    String,
> {
    let registry = registry()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let roots = set
        .pages
        .iter()
        .map(|p| labels::inspect(&p.document, &registry, budget, &mut admission).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let members = roots.iter().map(|r| [r]).collect::<Vec<_>>();
    let namespaces = members
        .iter()
        .map(|m| labels::resolve(m, budget).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = assets::prepare_external(
        set,
        &namespaces.iter().collect::<Vec<_>>(),
        &options,
        inputs,
        &registry,
        &mut codec,
        budget,
    )
    .map_err(err)?;
    let admitted = prepared
        .assets()
        .iter()
        .map(|asset| {
            (
                asset.page(),
                asset.member(),
                asset.path().to_owned(),
                asset.svg().to_owned(),
            )
        })
        .collect();
    if !render {
        return Ok((
            routed::RenderedPages {
                fragments: Vec::new(),
            },
            admitted,
        ));
    }
    let rendered = routed::render(
        prepared.pages(),
        &mut |_, _, _, _, _| Err::<_, String>("unexpected guest".into()),
        budget,
    )
    .map_err(err)?;
    Ok((rendered, admitted))
}

#[test]
fn external_svg_page_admission_retains_exact_bindings_and_relative_paths() -> Result<(), String> {
    let set = image_pages();
    let a = [SvgInput { id: "logo", svg: A }];
    let bb = [SvgInput { id: "logo", svg: B }];
    let (rendered, assets) = run_external(&set, &[input(0, 0, &a), input(1, 0, &bb)], &mut b())?;
    assert_eq!(assets.len(), 2);
    for (page, fragment) in rendered.fragments.iter().enumerate() {
        let (owner, member, path, bytes) = &assets[page];
        assert_eq!(*owner, page as u64);
        assert_eq!(*member, labels::MemberId(0));
        assert_eq!(bytes, if page == 0 { A } else { B });
        // This exact content address is the existing external image contract,
        // relative to the containing page rather than the package root.
        let expected = nepl3_core::source::Digest::of(bytes.as_bytes())
            .0
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        assert_eq!(path, &format!("assets/{expected}.svg"));
        let mut count = 0;
        for node in &fragment.fragment.markup.fragment.nodes {
            if let nepl3_markup::html::HtmlNode::Element { attributes, .. } = node {
                for attribute in attributes {
                    match attribute {
                        nepl3_markup::html::HtmlAttribute::Src { path: actual } => {
                            assert_eq!(actual, path);
                            count += 1;
                        }
                        nepl3_markup::html::HtmlAttribute::EmbeddedSvg { .. } => {
                            return Err("embedded output in external mode".into());
                        }
                        _ => {}
                    }
                }
            }
        }
        assert_eq!(count, 2);
    }
    // Equal bytes in separate pages retain separate admission records. This
    // does not grant cross-page binding or imply physical output deduplication.
    let (_, same) = run_external(&set, &[input(0, 0, &a), input(1, 0, &a)], &mut b())?;
    assert_eq!(same.len(), 2);
    assert_eq!(same[0].2, same[1].2);
    assert_ne!(same[0].0, same[1].0);
    Ok(())
}

#[test]
fn external_svg_page_admission_rejects_missing_unsafe_unused_and_digest() -> Result<(), String> {
    let set = image_pages();
    let a = [SvgInput { id: "logo", svg: A }];
    for (id, svg, expected) in [
        ("other", A, "Unused"),
        ("logo", "<svg><script/></svg>", "InvalidSvg"),
    ] {
        let bad = [SvgInput { id, svg }];
        let error = run_external(&set, &[input(0, 0, &bad), input(1, 0, &a)], &mut b())
            .err()
            .ok_or("unexpected admission")?;
        assert!(error.contains(expected), "{error}");
    }
    let error = run_external(&set, &[input(0, 0, &a)], &mut b())
        .err()
        .ok_or("missing accepted")?;
    assert!(
        error.contains("page: 1") && error.contains("Missing"),
        "{error}"
    );
    let mut mismatch = image_pages();
    for node in &mut mismatch.pages[0].document.value.nodes {
        if let DocKind::Image { asset, .. } = &mut node.kind {
            asset.digest = Some(nepl3_core::source::Digest::of(B.as_bytes()));
        }
    }
    let error = run_external(&mismatch, &[input(0, 0, &a), input(1, 0, &a)], &mut b())
        .err()
        .ok_or("digest accepted")?;
    assert!(error.contains("Digest"), "{error}");
    Ok(())
}

#[test]
fn external_svg_page_shared_limits_are_exact() -> Result<(), String> {
    let set = image_pages();
    let a = [SvgInput { id: "logo", svg: A }];
    let inputs = [input(0, 0, &a), input(1, 0, &a)];
    let mut budget = b();
    run_external(&set, &inputs, &mut budget)?;
    let usage = budget.usage();
    for dimension in 0..6 {
        for short in [false, true] {
            let mut limits = b().limits();
            let less = u64::from(short);
            match dimension {
                0 => limits.work = usage.work - less,
                1 => limits.allocation_units = usage.allocation_units - less,
                2 => limits.nodes = usage.nodes - less,
                3 => limits.depth = usage.depth - less,
                4 => limits.output_bytes = usage.output_bytes - less,
                _ => limits.source_bytes = usage.source_bytes - less,
            }
            let result = run_external(&set, &inputs, &mut Budget::new(limits));
            assert_eq!(
                result.is_ok(),
                !short,
                "dimension {dimension}: {:?}",
                result.err()
            );
        }
    }
    Ok(())
}

fn with_external_guest<T>(
    set: &PageSet,
    guest: &DocumentSyntax,
    inputs: &[PageMemberAssets<'_>],
    f: impl FnOnce(&assets::PreparedExternalPages<'_, '_>) -> Result<T, String>,
) -> Result<T, String> {
    let registry = registry()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let roots = set
        .pages
        .iter()
        .map(|p| labels::inspect(&p.document, &registry, &mut b(), &mut admission).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let extra = labels::inspect(guest, &registry, &mut b(), &mut admission).map_err(err)?;
    let first = [&roots[0]];
    let second = [&roots[1], &extra];
    let one = labels::resolve(&first, &mut b()).map_err(err)?;
    let two = labels::resolve(&second, &mut b()).map_err(err)?;
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = assets::prepare_external(
        set,
        &[&one, &two],
        &options,
        inputs,
        &registry,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    f(&prepared)
}

#[test]
fn external_svg_hidden_guest_requires_own_checked_bytes() -> Result<(), String> {
    let set = image_pages();
    let guest = guest_image();
    let a = [SvgInput { id: "logo", svg: A }];
    let bb = [SvgInput { id: "logo", svg: B }];
    let inputs = [input(0, 0, &a), input(1, 0, &a), input(1, 1, &bb)];
    with_external_guest(&set, &guest, &inputs, |prepared| {
        assert_eq!(prepared.assets().len(), 3);
        let asset = &prepared.assets()[2];
        assert_eq!(asset.page(), 1);
        assert_eq!(asset.member(), labels::MemberId(1));
        assert_eq!(asset.svg(), B);
        Ok(())
    })?;
    let error = with_external_guest(&set, &guest, &inputs[..2], |_| Ok(()))
        .err()
        .ok_or("root supplied hidden guest")?;
    assert!(
        error.contains("member: MemberId(1), error: Missing"),
        "{error}"
    );
    let bad = [SvgInput {
        id: "logo",
        svg: "<svg><script/></svg>",
    }];
    let error = with_external_guest(
        &set,
        &guest,
        &[input(0, 0, &a), input(1, 0, &a), input(1, 1, &bad)],
        |_| Ok(()),
    )
    .err()
    .ok_or("unsafe hidden guest accepted")?;
    assert!(
        error.contains("member: MemberId(1), error: InvalidSvg"),
        "{error}"
    );
    Ok(())
}

#[allow(clippy::type_complexity)]
fn run_external(
    set: &PageSet,
    inputs: &[PageMemberAssets<'_>],
    budget: &mut Budget,
) -> Result<
    (
        routed::RenderedPages,
        Vec<(u64, labels::MemberId, String, String)>,
    ),
    String,
> {
    run_external_mode(set, inputs, budget, true)
}
#[test]
fn external_svg_evidence_collection_limits_and_sticky_stop() -> Result<(), String> {
    let set = image_pages();
    let a = [SvgInput { id: "logo", svg: A }];
    let inputs = [input(0, 0, &a), input(1, 0, &a)];
    let mut budget = b();
    run_external_mode(&set, &inputs, &mut budget, false)?;
    let usage = budget.usage();
    for allocation in [false, true] {
        for short in [false, true] {
            let mut limits = b().limits();
            if allocation {
                limits.allocation_units = usage.allocation_units - u64::from(short);
            } else {
                limits.work = usage.work - u64::from(short);
            }
            let mut limited = Budget::new(limits);
            let result = run_external_mode(&set, &inputs, &mut limited, false);
            assert_eq!(
                result.is_ok(),
                !short,
                "{allocation} {short}: {:?}",
                result.err()
            );
            if short {
                let before = limited.usage();
                let again = run_external_mode(&set, &[], &mut limited, false);
                assert!(again.is_err());
                assert_eq!(limited.usage(), before);
            }
        }
    }
    Ok(())
}
