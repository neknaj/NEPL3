use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::katex::fragment::*;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        nodes: 100_000,
        depth: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 10_000_000,
        ..Limits::default()
    })
}
fn span(children: &[u64]) -> Node {
    Node::Span {
        classes: "katex".into(),
        style: "height:1em;top:-.2em;".into(),
        aria_hidden: Some(true),
        children: children.into(),
    }
}
fn policy() -> Policy<'static> {
    Policy {
        classes: &["katex", "mord"],
        scope: "nepl-math-example",
    }
}

#[test]
fn text_svg_and_computed_styles_are_serialized_without_raw_markup() -> Result<(), Error> {
    let f = Fragment {
        nodes: vec![
            Node::Text("<script>\r&日本".into()),
            Node::Path {
                data: "M0 0L10 10Z".into(),
            },
            Node::Line {
                x1: Endpoint::Zero,
                y1: Endpoint::Full,
                x2: Endpoint::Full,
                y2: Endpoint::Zero,
                stroke_width: "0.046em".into(),
            },
            Node::Svg {
                width: "100%".into(),
                height: "1em".into(),
                view_box: Some("0 0 10 10".into()),
                aspect: Some(Aspect::MinSlice),
                children: vec![1, 2],
            },
            span(&[0, 3]),
        ],
    };
    let mut b = budget();
    let checked = validate(&f, &policy(), &mut b)?;
    assert!(std::ptr::eq(checked.fragment(), &f));
    let result = serialize(&checked, &mut b)?;
    assert!(result.html().contains("&lt;script&gt;&#xD;&amp;日本"));
    assert!(
        result
            .html()
            .contains("<svg xmlns=\"http://www.w3.org/2000/svg\"")
    );
    assert!(
        result
            .html()
            .contains("preserveAspectRatio=\"xMinYMin slice\"")
    );
    assert!(result.html().contains("<path d=\"M0 0L10 10Z\"></path>"));
    assert!(
        result
            .html()
            .contains("class=\"katex nepl-math-example-n4\"")
    );
    assert!(!result.html().contains(" style="));
    // Exact values and declaration order, plus explicit scope and precedence.
    assert_eq!(
        result.stylesheet(),
        ".nepl-math-example .nepl-math-example-n4{height:1em!important;top:-.2em!important;}\n"
    );
    assert_eq!(
        b.usage().output_bytes,
        (result.html().len() + result.stylesheet().len()) as u64
    );
    Ok(())
}

#[test]
fn reject_cycles_sharing_unreachable_nodes_and_namespace_crossings() {
    let cases = [
        Fragment {
            nodes: vec![span(&[0])],
        },
        Fragment {
            nodes: vec![Node::Text("x".into()), span(&[0, 0])],
        },
        Fragment {
            nodes: vec![Node::Text("x".into()), span(&[])],
        },
        Fragment {
            nodes: vec![
                Node::Path {
                    data: "M0 0".into(),
                },
                span(&[0]),
            ],
        },
        Fragment {
            nodes: vec![
                Node::Text("x".into()),
                Node::Svg {
                    width: "100%".into(),
                    height: "1em".into(),
                    view_box: None,
                    aspect: None,
                    children: vec![0],
                },
                span(&[1]),
            ],
        },
        Fragment { nodes: vec![] },
        Fragment {
            nodes: vec![Node::Text("x".into())],
        },
    ];
    for fragment in cases {
        assert!(validate(&fragment, &policy(), &mut budget()).is_err());
    }
}

#[test]
fn reject_unregistered_classes_invalid_attributes_and_text() {
    for (classes, style) in [
        ("other", ""),
        ("katex  mord", ""),
        ("katex\"", ""),
        ("katex", "width:url(x)"),
        ("katex", "height:1em;height:2em"),
    ] {
        let f = Fragment {
            nodes: vec![Node::Span {
                classes: classes.into(),
                style: style.into(),
                aria_hidden: None,
                children: vec![],
            }],
        };
        assert!(matches!(
            validate(&f, &policy(), &mut budget()),
            Err(Error::Attribute { node: 0 })
        ));
    }
    for bad in ["x\0y", "\u{ffff}"] {
        let f = Fragment {
            nodes: vec![Node::Text(bad.into()), span(&[0])],
        };
        assert!(matches!(
            validate(&f, &policy(), &mut budget()),
            Err(Error::Text { node: 0, .. })
        ));
    }
    for classes in [
        &["mord", "katex"][..],
        &["katex", "katex"],
        &["nepl-math-example-n0"],
    ] {
        let f = Fragment {
            nodes: vec![span(&[])],
        };
        let p = Policy {
            classes,
            ..policy()
        };
        assert!(matches!(
            validate(&f, &p, &mut budget()),
            Err(Error::Policy)
        ));
    }
}

#[test]
fn all_limits_are_sticky_and_serialization_rechecks_its_own_depth() -> Result<(), Error> {
    let f = Fragment {
        nodes: vec![Node::Text("x".into()), span(&[0])],
    };
    let checked = validate(&f, &policy(), &mut budget())?;
    for (limits, reason) in [
        (
            Limits {
                work: 0,
                ..budget().limits()
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: 0,
                ..budget().limits()
            },
            StopReason::AllocationLimit,
        ),
        (
            Limits {
                nodes: 0,
                ..budget().limits()
            },
            StopReason::NodeLimit,
        ),
        (
            Limits {
                depth: 2,
                ..budget().limits()
            },
            StopReason::DepthLimit,
        ),
        (
            Limits {
                output_bytes: 1,
                ..budget().limits()
            },
            StopReason::OutputLimit,
        ),
    ] {
        let mut b = Budget::new(limits);
        assert!(matches!(serialize(&checked, &mut b), Err(Error::Stopped(r)) if r == reason));
        assert!(matches!(validate(&f, &policy(), &mut b), Err(Error::Stopped(r)) if r == reason));
    }
    Ok(())
}

#[test]
fn deep_tree_uses_explicit_stack_and_linear_work() -> Result<(), Error> {
    let make = |n| Fragment {
        nodes: (0..n)
            .map(|i| {
                if i == 0 {
                    Node::Text("x".into())
                } else {
                    span(&[i - 1])
                }
            })
            .collect(),
    };
    let run = |n| -> Result<u64, Error> {
        let f = make(n);
        let mut b = budget();
        let checked = validate(&f, &policy(), &mut b)?;
        let _result = serialize(&checked, &mut b)?;
        Ok(b.usage().work)
    };
    let small = run(1000)?;
    let large = run(2000)?;
    assert!(large > small && large < 3 * small);
    Ok(())
}
