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
