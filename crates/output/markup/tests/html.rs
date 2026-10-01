use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::html::*;
fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        nodes: 2_000_000,
        depth: 200_000,
        ..Limits::default()
    })
}
fn policy() -> HtmlPolicy {
    HtmlPolicy {
        classes: vec!["doc-paragraph".into()],
    }
}
fn element(tag: HtmlTag, children: &[u64]) -> HtmlNode {
    HtmlNode::Element {
        tag,
        children: children.into(),
        attributes: vec![],
    }
}
fn txt(text: &str) -> HtmlNode {
    HtmlNode::Text { text: text.into() }
}
fn attr(node: &mut HtmlNode, a: HtmlAttribute) {
    if let HtmlNode::Element { attributes, .. } = node {
        attributes.push(a);
    }
}
fn render(f: &HtmlFragment) -> Result<String, HtmlError> {
    let mut b = budget();
    serialize(&validate(f, HtmlSlot::Block, &policy(), &mut b)?, &mut b)
}
#[test]
fn class_membership_finishes_at_match_and_validates_the_whole_policy() -> Result<(), HtmlError> {
    let mut leaf = element(HtmlTag::Span, &[]);
    attr(
        &mut leaf,
        HtmlAttribute::Class {
            values: vec!["allowed".into()],
        },
    );
    let f = HtmlFragment {
        root: 0,
        nodes: vec![element(HtmlTag::Div, &vec![1; 64]), leaf],
    };
    let measure = |count| -> Result<u64, HtmlError> {
        let mut classes = vec!["allowed".into()];
        classes.extend((0..count).map(|i| format!("unused-{i:04}")));
        let mut b = budget();
        validate(&f, HtmlSlot::Block, &HtmlPolicy { classes }, &mut b)?;
        Ok(b.usage().work)
    };
    // Adding unused policy entries should cost one policy validation, not one
    // scan per emitted occurrence of the shared leaf.
    assert!(measure(64)? < measure(16)? * 2);
    let mut p = HtmlPolicy {
        classes: vec!["allowed".into(), "bad class".into()],
    };
    assert!(matches!(
        validate(&f, HtmlSlot::Block, &p, &mut budget()),
        Err(HtmlError::Policy)
    ));
    p.classes = vec!["different".into()];
    assert!(matches!(
        validate(&f, HtmlSlot::Block, &p, &mut budget()),
        Err(HtmlError::Attribute { .. })
    ));
    p.classes = vec!["different".into(), "allowed".into()];
    let mut full = budget();
    validate(&f, HtmlSlot::Block, &p, &mut full)?;
    for work in [0, full.usage().work - 1] {
        let mut limits = budget().limits();
        limits.work = work;
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            validate(&f, HtmlSlot::Block, &p, &mut stopped),
            Err(HtmlError::Stopped(StopReason::WorkLimit))
        ));
    }
    Ok(())
}
#[test]
fn output_has_one_byte_charge_and_no_per_delimiter_owned_strings() -> Result<(), HtmlError> {
    // A roughly 2 KiB document should serialize within a 64 KiB logical
    // allocation allowance, including traversal. The former owned String per
    // delimiter exhausted this allowance even for empty void elements.
    let f = HtmlFragment {
        root: 0,
        nodes: vec![
            element(HtmlTag::Div, &vec![1; 512]),
            element(HtmlTag::Br, &[]),
        ],
    };
    let proof = validate(&f, HtmlSlot::Block, &policy(), &mut budget())?;
    let expected = format!("<div>{}</div>", "<br>".repeat(512));
    let mut limits = budget().limits();
    limits.allocation_units = 64 * 1024;
    limits.output_bytes = expected.len() as u64;
    let mut b = Budget::new(limits);
    assert_eq!(serialize(&proof, &mut b)?, expected);
    assert_eq!(b.usage().output_bytes, expected.len() as u64);
    limits.output_bytes -= 1;
    let mut stopped = Budget::new(limits);
    assert!(matches!(
        serialize(&proof, &mut stopped),
        Err(HtmlError::Stopped(StopReason::OutputLimit))
    ));
    assert!(matches!(
        serialize(&proof, &mut stopped),
        Err(HtmlError::Stopped(StopReason::OutputLimit))
    ));
    Ok(())
}
#[test]
fn xml_embedding_preserves_namespace_void_nodes_and_pre_text() -> Result<(), HtmlError> {
    let f = HtmlFragment {
        root: 0,
        nodes: vec![
            element(HtmlTag::Div, &[1, 6, 7]),
            element(HtmlTag::Ruby, &[2, 3]),
            txt("漢"),
            element(HtmlTag::Rt, &[4]),
            txt("かん"),
            txt("\n<&\r"),
            element(HtmlTag::Br, &[]),
            element(HtmlTag::Pre, &[5]),
        ],
    };
    let proof = validate(&f, HtmlSlot::Block, &policy(), &mut budget())?;
    let mut full = budget();
    let xml = serialize_xhtml(&proof, &mut full)?;
    assert_eq!(
        xml,
        "<div xmlns=\"http://www.w3.org/1999/xhtml\"><ruby>漢<rt>かん</rt></ruby><br /><pre>\n&lt;&amp;&#xD;</pre></div>"
    );
    assert_eq!(full.usage().output_bytes, xml.len() as u64);
    assert_eq!(
        serialize(&proof, &mut budget())?,
        "<div><ruby>漢<rt>かん</rt></ruby><br><pre>\n\n&lt;&amp;&#xD;</pre></div>"
    );
    let mut outer = budget();
    assert_eq!(
        outer.with_depth_at_least(7, |b| serialize_xhtml(&proof, b))?,
        xml
    );
    assert_eq!(outer.usage().depth, full.usage().depth + 7);
    assert_eq!(outer.current_depth(), 0);
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::OutputLimit,
        StopReason::DepthLimit,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = full.usage().work - 1,
            StopReason::AllocationLimit => {
                limits.allocation_units = full.usage().allocation_units - 1
            }
            StopReason::NodeLimit => limits.nodes = full.usage().nodes - 1,
            StopReason::OutputLimit => limits.output_bytes = full.usage().output_bytes - 1,
            _ => limits.depth = full.usage().depth - 1,
        }
        let mut b = Budget::new(limits);
        assert_eq!(
            serialize_xhtml(&proof, &mut b),
            Err(HtmlError::Stopped(reason))
        );
        assert_eq!(b.poll(), Err(reason));
    }
    let mut b = budget();
    b.cancel();
    assert_eq!(
        serialize_xhtml(&proof, &mut b),
        Err(HtmlError::Stopped(StopReason::Cancelled))
    );
    let f = HtmlFragment {
        root: 0,
        nodes: vec![txt("<&")],
    };
    assert_eq!(
        serialize_xhtml(
            &validate(&f, HtmlSlot::Phrasing, &policy(), &mut budget())?,
            &mut budget()
        )?,
        "&lt;&amp;"
    );
    let f = HtmlFragment {
        root: 0,
        nodes: vec![element(HtmlTag::Br, &[])],
    };
    assert_eq!(
        serialize_xhtml(
            &validate(&f, HtmlSlot::Phrasing, &policy(), &mut budget())?,
            &mut budget()
        )?,
        "<br xmlns=\"http://www.w3.org/1999/xhtml\" />"
    );
    Ok(())
}

#[test]
fn nested_paragraph_wrapper_links_lists_and_code_preserve_bytes() -> Result<(), HtmlError> {
    use HtmlTag::*;
    let mut f = HtmlFragment {
        root: 0,
        nodes: vec![
            element(Article, &[1, 3, 6]),
            element(H1, &[2]),
            txt("日本語🙂"),
            element(Div, &[4, 8]),
            element(P, &[5]),
            txt("<&\r\n"),
            element(Pre, &[7]),
            txt("\nlet x = \"<&\";\r\n"),
            element(Ul, &[9]),
            element(Li, &[10]),
            element(A, &[11]),
            txt("先頭へ"),
        ],
    };
    attr(
        &mut f.nodes[1],
        HtmlAttribute::Id {
            value: "title".into(),
        },
    );
    attr(&mut f.nodes[1], HtmlAttribute::Lang { value: "ja".into() });
    attr(
        &mut f.nodes[3],
        HtmlAttribute::Class {
            values: vec!["doc-paragraph".into()],
        },
    );
    attr(
        &mut f.nodes[10],
        HtmlAttribute::Href {
            value: HtmlHref::Fragment { id: "title".into() },
        },
    );
    let expected = "<article><h1 id=\"title\" lang=\"ja\">日本語🙂</h1><div class=\"doc-paragraph\"><p>&lt;&amp;&#xD;\n</p><ul><li><a href=\"#title\">先頭へ</a></li></ul></div><pre>\n\nlet x = \"&lt;&amp;\";&#xD;\n</pre></article>";
    assert_eq!(render(&f)?, expected);
    if let HtmlNode::Element { attributes, .. } = &mut f.nodes[1] {
        attributes.reverse();
    }
    assert_eq!(render(&f)?, expected);
    Ok(())
}

#[test]
fn tables_images_and_typed_external_links() -> Result<(), HtmlError> {
    use HtmlTag::*;
    let mut f = HtmlFragment {
        root: 0,
        nodes: vec![
            element(Div, &[1, 8]),
            element(Table, &[2, 5]),
            element(Thead, &[3]),
            element(Tr, &[4]),
            element(Th, &[9]),
            element(Tbody, &[6]),
            element(Tr, &[7]),
            element(Td, &[10]),
            element(Img, &[]),
            txt("見出し"),
            element(A, &[11]),
            txt("例"),
        ],
    };
    attr(
        &mut f.nodes[4],
        HtmlAttribute::Scope {
            value: CellScope::Col,
        },
    );
    attr(
        &mut f.nodes[8],
        HtmlAttribute::Src {
            path: "assets/picture.png".into(),
        },
    );
    attr(
        &mut f.nodes[8],
        HtmlAttribute::Alt {
            value: "代替\n<&>\"".into(),
        },
    );
    attr(
        &mut f.nodes[10],
        HtmlAttribute::Href {
            value: HtmlHref::External {
                uri: "https://example.org/?a=1&b=2".into(),
            },
        },
    );
    assert_eq!(
        render(&f)?,
        "<div><table><thead><tr><th scope=\"col\">見出し</th></tr></thead><tbody><tr><td><a href=\"https://example.org/?a=1&amp;b=2\">例</a></td></tr></tbody></table><img alt=\"代替&#xA;&lt;&amp;&gt;&quot;\" src=\"assets/picture.png\"></div>"
    );
    Ok(())
}

#[test]
fn malformed_structure_and_attribute_values_never_become_partial_html() {
    use HtmlTag::*;
    for (inside, descendant) in [(Caption, Table), (Th, H1), (Th, Section)] {
        let nodes = if inside == Caption {
            vec![
                element(Table, &[1]),
                element(Caption, &[2]),
                element(Div, &[3]),
                element(descendant, &[]),
            ]
        } else {
            vec![
                element(Table, &[1]),
                element(Tbody, &[2]),
                element(Tr, &[3]),
                element(Th, &[4]),
                element(Div, &[5]),
                element(descendant, &[]),
            ]
        };
        assert!(render(&HtmlFragment { root: 0, nodes }).is_err());
    }
    let ragged = HtmlFragment {
        root: 0,
        nodes: vec![
            element(Table, &[1]),
            element(Tbody, &[2, 3]),
            element(Tr, &[4]),
            element(Tr, &[5, 6]),
            element(Td, &[]),
            element(Td, &[]),
            element(Td, &[]),
        ],
    };
    assert!(render(&ragged).is_err());
    for (parent, child) in [
        (P, Div),
        (Span, Table),
        (Table, Tr),
        (Ul, Div),
        (Tr, Div),
        (Img, Span),
        (Br, Span),
        (A, A),
    ] {
        let f = HtmlFragment {
            root: 0,
            nodes: vec![element(parent, &[1]), element(child, &[])],
        };
        assert!(render(&f).is_err(), "{parent:?}/{child:?}");
    }
    for uri in [
        "javascript:alert(1)",
        "data:text/html,test",
        "HTTPS://example.org",
        "https://user@example.org/",
        "https://example.org/\\x",
        "https://example.org/\tx",
        "https://example.org/%xx",
        "//example.org",
    ] {
        let mut a = element(A, &[]);
        attr(
            &mut a,
            HtmlAttribute::Href {
                value: HtmlHref::External { uri: uri.into() },
            },
        );
        assert!(
            matches!(
                render(&HtmlFragment {
                    root: 0,
                    nodes: vec![a]
                }),
                Err(HtmlError::Attribute { .. })
            ),
            "{uri}"
        );
    }
    for path in [
        "/root.png",
        "../secret.png",
        "a/../secret.png",
        "//host/x",
        "a%2fb.png",
        "a\\b.png",
    ] {
        let mut n = element(Img, &[]);
        attr(&mut n, HtmlAttribute::Src { path: path.into() });
        attr(&mut n, HtmlAttribute::Alt { value: "".into() });
        assert!(
            matches!(
                render(&HtmlFragment {
                    root: 0,
                    nodes: vec![n]
                }),
                Err(HtmlError::Attribute { .. })
            ),
            "{path}"
        );
    }
    let mut n = element(Div, &[]);
    attr(
        &mut n,
        HtmlAttribute::Class {
            values: vec!["unregistered".into()],
        },
    );
    assert!(
        render(&HtmlFragment {
            root: 0,
            nodes: vec![n]
        })
        .is_err()
    );
    let mut n = element(Div, &[]);
    attr(&mut n, HtmlAttribute::Id { value: "a".into() });
    attr(&mut n, HtmlAttribute::Id { value: "b".into() });
    assert!(
        render(&HtmlFragment {
            root: 0,
            nodes: vec![n]
        })
        .is_err()
    );
}

#[test]
fn dag_appearance_identity_cycles_unreachable_and_deep_drop() -> Result<(), HtmlError> {
    let mut f = HtmlFragment {
        root: 0,
        nodes: vec![
            element(HtmlTag::Div, &[1, 1]),
            element(HtmlTag::Span, &[2]),
            txt("x"),
        ],
    };
    assert_eq!(render(&f)?, "<div><span>x</span><span>x</span></div>");
    attr(
        &mut f.nodes[1],
        HtmlAttribute::Id {
            value: "shared".into(),
        },
    );
    assert_eq!(render(&f), Err(HtmlError::DuplicateId(1)));
    assert!(matches!(
        render(&HtmlFragment {
            root: 0,
            nodes: vec![element(HtmlTag::Div, &[0])]
        }),
        Err(HtmlError::Cycle(0))
    ));
    assert!(matches!(
        render(&HtmlFragment {
            root: 0,
            nodes: vec![txt("x"), txt("y")]
        }),
        Err(HtmlError::Unreachable(1))
    ));
    assert!(matches!(
        render(&HtmlFragment {
            root: u64::MAX,
            nodes: vec![]
        }),
        Err(HtmlError::Reference(u64::MAX))
    ));
    let mut nodes = Vec::new();
    for i in 0..100_000 {
        nodes.push(element(HtmlTag::Div, &[i + 1]));
    }
    nodes.push(txt("終"));
    let deep = HtmlFragment { root: 0, nodes };
    assert!(validate(&deep, HtmlSlot::Block, &policy(), &mut budget()).is_ok());
    drop(deep);
    Ok(())
}

#[test]
fn sticky_stops_apply_to_validation_and_serialization_with_caller_depth() -> Result<(), HtmlError> {
    let f = HtmlFragment {
        root: 0,
        nodes: vec![element(HtmlTag::Span, &[1]), txt("<&🙂")],
    };
    let proof = validate(&f, HtmlSlot::Phrasing, &policy(), &mut budget())?;
    let output = serialize(&proof, &mut budget())?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::DepthLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::DepthLimit => limits.depth = 1,
            StopReason::NodeLimit => limits.nodes = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::OutputLimit => limits.output_bytes = output.len() as u64 - 1,
            _ => {}
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        assert_eq!(serialize(&proof, &mut b), Err(HtmlError::Stopped(reason)));
        assert_eq!(serialize(&proof, &mut b), Err(HtmlError::Stopped(reason)));
    }
    let mut b = Budget::new(Limits {
        depth: 8,
        ..budget().limits()
    });
    let result = b.with_depth_at_least(7, |b| serialize(&proof, b));
    assert_eq!(result, Err(HtmlError::Stopped(StopReason::DepthLimit)));
    let mut exact = Budget::new(Limits {
        output_bytes: output.len() as u64,
        ..budget().limits()
    });
    assert_eq!(serialize(&proof, &mut exact)?, output);
    assert_eq!(exact.usage().output_bytes, output.len() as u64);
    Ok(())
}

#[test]
fn static_svg_profile_and_embedded_src() -> Result<(), HtmlError> {
    let svg = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><g id='x'><path d='M0 0L10 0L5 10Z' stroke='#000' fill='none'/></g></svg>";
    assert!(svg::validate(svg, &mut budget())?);
    for bad in [
        svg.replace("<g id='x'>", "<g onclick='x()'>"),
        svg.replace("<g id='x'>", "<g style='fill:url(https://example.org)'>"),
        svg.replace("<g id='x'>", "<foreignObject>"),
        svg.replace("</g>", "</path>"),
        svg.replace("M0 0L10 0L5 10Z", "M1e999 0"),
        svg.replace("M0 0L10 0L5 10Z", "L0 0"),
        svg.replace("<g id='x'>", "<g id='x' id='y'>"),
        svg.replace("http://www.w3.org/2000/svg", "https://evil.example"),
        format!("<!DOCTYPE svg [<!ENTITY x 'x'>]>{svg}"),
        format!("{svg}<svg/>"),
        format!("{svg}trailing"),
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'".into(),
        svg.replace("0 0 10 10", "0,,0,10,10"),
        svg.replace("0 0 10 10", ",0 0 10 10,"),
    ] {
        assert!(!svg::validate(&bad, &mut budget())?, "accepted {bad}");
    }
    let attrs = vec![
        HtmlAttribute::EmbeddedSvg { svg: svg.into() },
        HtmlAttribute::Alt {
            value: "triangle".into(),
        },
    ];
    let fragment = HtmlFragment {
        root: 0,
        nodes: vec![HtmlNode::Element {
            tag: HtmlTag::Img,
            attributes: attrs.clone(),
            children: vec![],
        }],
    };
    let checked = validate(&fragment, HtmlSlot::Phrasing, &policy(), &mut budget())?;
    assert!(serialize(&checked, &mut budget())?.contains("src=\"data:image/svg+xml;base64,"));
    let mut duplicate = fragment.clone();
    attr(
        &mut duplicate.nodes[0],
        HtmlAttribute::Src {
            path: "a.svg".into(),
        },
    );
    assert!(validate(&duplicate, HtmlSlot::Phrasing, &policy(), &mut budget()).is_err());
    let mut no_alt = fragment;
    if let HtmlNode::Element { attributes, .. } = &mut no_alt.nodes[0] {
        attributes.pop();
    }
    assert!(validate(&no_alt, HtmlSlot::Phrasing, &policy(), &mut budget()).is_err());
    let shallow = budget();
    // Full validation is embedded in the caller's current depth envelope.
    let limited = Budget::new(Limits {
        depth: 1,
        ..shallow.limits()
    });
    let mut limited = limited;
    assert!(matches!(
        svg::validate(svg, &mut limited),
        Err(StopReason::DepthLimit)
    ));
    let mut zero = Budget::new(Limits::default());
    assert!(svg::validate(svg, &mut zero).is_err());
    Ok(())
}

#[test]
fn details_requires_one_leading_summary_and_flow_context() -> Result<(), HtmlError> {
    let mut f = HtmlFragment {
        root: 0,
        nodes: vec![
            element(HtmlTag::Details, &[1, 2]),
            element(HtmlTag::Summary, &[3]),
            element(HtmlTag::Div, &[]),
            txt("Full size"),
        ],
    };
    assert_eq!(
        render(&f)?,
        "<details><summary>Full size</summary><div></div></details>"
    );
    assert!(validate(&f, HtmlSlot::Phrasing, &policy(), &mut budget()).is_err());
    for children in [&[2, 1][..], &[1, 1, 2][..]] {
        let mut invalid = f.clone();
        invalid.nodes[0] = element(HtmlTag::Details, children);
        assert!(render(&invalid).is_err());
    }
    let empty = HtmlFragment {
        root: 0,
        nodes: vec![element(HtmlTag::Details, &[])],
    };
    assert!(matches!(render(&empty), Err(HtmlError::Content(0))));
    f.nodes[0] = element(HtmlTag::Div, &[1, 2]);
    assert!(render(&f).is_err());
    f.nodes[0] = element(HtmlTag::Details, &[1]);
    f.nodes[1] = element(HtmlTag::Summary, &[2]);
    f.nodes[2] = element(HtmlTag::Div, &[3]);
    assert!(matches!(render(&f), Err(HtmlError::Content(1))));
    Ok(())
}

#[test]
fn svg_intrinsic_dimensions_are_profile_checked_and_metered()
-> Result<(), nepl3_core::budget::StopReason> {
    let input = "<svg xmlns='http://www.w3.org/2000/svg' width='96pt' height='270pt' viewBox='0 0 10 10'><path d='M0 0L10 10'/></svg>";
    assert_eq!(
        svg::intrinsic_size(input, &mut budget())?,
        Some((128.0, 360.0))
    );
    assert_eq!(
        svg::intrinsic_size(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'/>",
            &mut budget()
        )?,
        None
    );
    assert_eq!(
        svg::intrinsic_size(
            "<svg xmlns='http://www.w3.org/2000/svg' width='-1' height='2' viewBox='0 0 10 10'/>",
            &mut budget()
        )?,
        None
    );
    let mut stopped = budget();
    stopped.cancel();
    assert_eq!(
        svg::intrinsic_size(input, &mut stopped),
        Err(StopReason::Cancelled)
    );
    Ok(())
}

#[test]
fn svg_path_glyphs_admit_only_local_leaf_instances() -> Result<(), StopReason> {
    let input = "<svg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' viewBox='0 0 10 10'><defs><path id='glyph' d='M0 0L2 3Z'/></defs><g transform='matrix(1 0 0 1 -2.5 .3)'><use xlink:href='#glyph' x='1.25' y='-.5'/></g></svg>";
    assert!(svg::validate(input, &mut budget())?);
    assert!(svg::validate(
        &input.replace("xlink:href", "href"),
        &mut budget()
    )?);
    let forward = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><use href='#g'/><defs><path id='g' d='M0 0L1 1'/></defs></svg>";
    assert!(svg::validate(forward, &mut budget())?);
    for bad in [
        input.replace("#glyph", "other.svg#glyph"),
        input.replace("#glyph", "https://example.org/a.svg#glyph"),
        input.replace("#glyph", "data:image/svg+xml,abc"),
        input.replace("#glyph", "#missing"),
        input.replace("#glyph", "#%67lyph"),
        input.replace("#glyph", "#glyph&#32;"),
        input.replace("xmlns:xlink='http://www.w3.org/1999/xlink'", ""),
        input.replace("xlink:href='#glyph'", "href='#glyph' xlink:href='#glyph'"),
        input.replace("x='1.25'", "x='1e2'"),
        input.replace("x='1.25'", "x='1px'"),
        input.replace("matrix(1 0 0 1 -2.5 .3)", "matrix(2 0 0 1 0 0)"),
        input.replace("matrix(1 0 0 1 -2.5 .3)", "matrix(1 0 0 1 1e2 0)"),
        input.replace("matrix(1 0 0 1 -2.5 .3)", "translate(2 3)"),
        input.replace(
            "matrix(1 0 0 1 -2.5 .3)",
            "matrix(1 0 0 1 0 0) matrix(1 0 0 1 0 0)",
        ),
        input.replace("y='-.5'/>", "y='-.5'><path d='M0 0'/></use>"),
        input.replace(
            "<path id='glyph' d='M0 0L2 3Z'/>",
            "<path id='glyph' d='M0 0L2 3Z'></path>",
        ),
        input.replace(
            "<path id='glyph' d='M0 0L2 3Z'/>",
            "<use id='glyph' href='#glyph'/>",
        ),
        input.replace(
            "<path id='glyph' d='M0 0L2 3Z'/>",
            "<g id='glyph'><path d='M0 0'/></g>",
        ),
        input.replace("<defs>", "<g>").replace("</defs>", "</g>"),
        input
            .replace("<defs>", "<g><defs>")
            .replace("</defs>", "</defs></g>"),
        input.replace("<g transform", "<g id='glyph' transform"),
    ] {
        assert!(!svg::validate(&bad, &mut budget())?, "accepted {bad}");
    }
    Ok(())
}

#[test]
fn svg_glyph_expansion_caps_whole_elements_and_meters_instances() -> Result<(), StopReason> {
    let wrap = |body: &str| {
        format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'>{body}</svg>")
    };
    let definition = "<defs><path id='g' d='M0 0'/></defs>";
    let source = wrap(&format!(
        "{definition}{}",
        "<use href='#g'/>".repeat(svg::MAX_USES)
    ));
    assert!(svg::validate(&source, &mut budget())?);
    let too_many = source.replace("</svg>", "<use href='#g'/></svg>");
    assert!(!svg::validate(&too_many, &mut budget())?);
    // Large whitespace is part of the instantiated element cost, even with tiny d.
    let huge = wrap(&format!(
        "<defs><path {}id='g' d='M0 0'/></defs>{}",
        " ".repeat(65_536),
        "<use href='#g'/>".repeat(128)
    ));
    assert!(huge.len() < svg::MAX_BYTES);
    assert!(!svg::validate(&huge, &mut budget())?);
    let simple = wrap(&format!("{definition}<use href='#g'/>"));
    let mut usage = budget();
    assert!(svg::validate(&simple, &mut usage)?);
    // svg, defs, path and use plus the instantiated path.
    assert_eq!(usage.usage().nodes, 5);
    for limits in [
        Limits {
            nodes: 4,
            ..budget().limits()
        },
        Limits {
            allocation_units: 0,
            ..budget().limits()
        },
        Limits {
            work: usage.usage().work - 1,
            ..budget().limits()
        },
    ] {
        assert!(svg::validate(&simple, &mut Budget::new(limits)).is_err());
    }
    Ok(())
}

#[test]
fn svg_glyph_exact_expansion_and_nested_budget_boundaries() -> Result<(), StopReason> {
    let path = |id: &str, bytes: usize| {
        let empty = format!("<path id='{id}' d='M0 0'/>");
        format!(
            "<path {}id='{id}' d='M0 0'/>",
            " ".repeat(bytes - empty.len())
        )
    };
    for (extra, expected) in [(0, true), (1, false)] {
        let input = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><defs>{}{}</defs>{}<use href='#g1'/></svg>",
            path("g0", 65_536),
            path("g1", 65_536 + extra),
            "<use href='#g0'/>".repeat(127)
        );
        assert_eq!(svg::validate(&input, &mut budget())?, expected);
    }
    let input = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><defs><path id='g' d='M0 0'/></defs><g><use href='#g'/></g></svg>";
    for base in [0, 5] {
        let mut exact = Budget::new(Limits {
            depth: base + 4,
            ..budget().limits()
        });
        assert!(exact.with_depth_at_least(base, |b| svg::validate(input, b))?);
        assert_eq!(exact.usage().depth, base + 4);
        let mut limited = Budget::new(Limits {
            depth: base + 3,
            ..budget().limits()
        });
        assert_eq!(
            limited.with_depth_at_least(base, |b| svg::validate(input, b)),
            Err(StopReason::DepthLimit)
        );
        assert_eq!(limited.poll(), Err(StopReason::DepthLimit));
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        svg::validate(input, &mut cancelled),
        Err(StopReason::Cancelled)
    );
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    Ok(())
}
