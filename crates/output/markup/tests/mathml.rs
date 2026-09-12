use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::mathml::*;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        work: 1_000_000,
        depth: 1000,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 100_000,
        diagnostics: 10,
        events: 10,
    })
}
fn element(tag: Tag, children: Vec<u64>) -> Node {
    Node::Element {
        tag,
        attributes: vec![],
        children,
    }
}
#[test]
fn serialization_preserves_structure_escaping_and_resource_stops() -> Result<(), Error> {
    let f = Fragment {
        root: 0,
        nodes: vec![
            Node::Element {
                tag: Tag::Math,
                attributes: vec![Attribute::Display(Display::Block)],
                children: vec![1],
            },
            element(Tag::Fraction, vec![2, 2]),
            element(Tag::Text, vec![3]),
            Node::Text("漢<&>\r".into()),
        ],
    };
    let checked = validate(&f, &mut budget())?;
    // Hand-derived XML: a shared numerator/denominator expands twice, and
    // character references preserve literal text rather than injecting markup.
    let expected = concat!(
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\">",
        "<mfrac><mtext>漢&lt;&amp;&gt;&#xD;</mtext>",
        "<mtext>漢&lt;&amp;&gt;&#xD;</mtext></mfrac></math>"
    );
    let mut full = budget();
    assert_eq!(serialize(&checked, &mut full)?, expected);
    assert_eq!(full.usage().output_bytes, expected.len() as u64);
    assert_eq!(full.usage().nodes, 6);
    assert_eq!(full.usage().depth, 4);
    let mut nested = budget();
    assert_eq!(
        nested.with_depth_at_least(7, |b| serialize(&checked, b))?,
        expected
    );
    assert_eq!(nested.usage().depth, 11);
    assert_eq!(nested.current_depth(), 0);
    for reason in [
        StopReason::WorkLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
        StopReason::DepthLimit,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = full.usage().work - 1,
            StopReason::NodeLimit => limits.nodes = full.usage().nodes - 1,
            StopReason::AllocationLimit => {
                limits.allocation_units = full.usage().allocation_units - 1
            }
            StopReason::OutputLimit => limits.output_bytes = full.usage().output_bytes - 1,
            _ => limits.depth = full.usage().depth - 1,
        }
        let mut b = Budget::new(limits);
        assert_eq!(serialize(&checked, &mut b), Err(Error::Stopped(reason)));
        assert_eq!(b.poll(), Err(reason));
        assert_eq!(b.current_depth(), 0);
    }
    let mut b = budget();
    b.cancel();
    assert_eq!(
        serialize(&checked, &mut b),
        Err(Error::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn serialization_emits_typed_attributes_and_mathml_end_tags() -> Result<(), Error> {
    let f = Fragment {
        root: 0,
        nodes: vec![
            element(Tag::Math, vec![1, 3, 5]),
            Node::Element {
                tag: Tag::Identifier,
                attributes: vec![Attribute::NormalIdentifier],
                children: vec![2],
            },
            Node::Text("x".into()),
            Node::Element {
                tag: Tag::Operator,
                attributes: vec![
                    Attribute::Stretchy(false),
                    Attribute::Symmetric(true),
                    Attribute::LargeOperator(true),
                    Attribute::MovableLimits(false),
                    Attribute::Form(OperatorForm::Infix),
                ],
                children: vec![4],
            },
            Node::Text("+".into()),
            Node::Element {
                tag: Tag::Space,
                attributes: vec![
                    Attribute::Width("0em".into()),
                    Attribute::Height("1.25em".into()),
                    Attribute::Depth("0.5em".into()),
                ],
                children: vec![],
            },
        ],
    };
    let checked = validate(&f, &mut budget())?;
    assert_eq!(
        serialize(&checked, &mut budget())?,
        concat!(
            "<math xmlns=\"http://www.w3.org/1998/Math/MathML\">",
            "<mi mathvariant=\"normal\">x</mi>",
            "<mo stretchy=\"false\" symmetric=\"true\" largeop=\"true\" movablelimits=\"false\" form=\"infix\">+</mo>",
            "<mspace width=\"0em\" height=\"1.25em\" depth=\"0.5em\"></mspace></math>"
        )
    );
    Ok(())
}
#[test]
fn mathml_checks_all_profile_tags_and_fixed_arities() -> Result<(), Error> {
    for (tag, arity) in [
        (Tag::Fraction, 2),
        (Tag::Root, 2),
        (Tag::Sub, 2),
        (Tag::Sup, 2),
        (Tag::Under, 2),
        (Tag::Over, 2),
        (Tag::SubSup, 3),
        (Tag::UnderOver, 3),
    ] {
        let mut f = Fragment {
            root: 0,
            nodes: vec![
                element(Tag::Math, vec![1]),
                element(tag, vec![2; arity]),
                element(Tag::Number, vec![3]),
                Node::Text("1".into()),
            ],
        };
        validate(&f, &mut budget())?;
        if let Node::Element { children, .. } = &mut f.nodes[1] {
            children.pop();
        }
        assert_eq!(validate(&f, &mut budget()).err(), Some(Error::Content(1)));
    }
    for tag in [Tag::Identifier, Tag::Number, Tag::Operator, Tag::Text] {
        let f = Fragment {
            root: 0,
            nodes: vec![
                element(Tag::Math, vec![1]),
                element(tag, vec![2]),
                Node::Text("漢字 & x < y".into()),
            ],
        };
        validate(&f, &mut budget())?;
    }
    let table = Fragment {
        root: 0,
        nodes: vec![
            element(Tag::Math, vec![1]),
            element(Tag::Table, vec![2]),
            element(Tag::TableRow, vec![3]),
            element(Tag::Cell, vec![4]),
            element(Tag::Sqrt, vec![5]),
            element(Tag::Row, vec![6]),
            element(Tag::Space, vec![]),
        ],
    };
    validate(&table, &mut budget())?;
    let mut bad = table.clone();
    bad.nodes[2] = element(Tag::Row, vec![3]);
    assert_eq!(validate(&bad, &mut budget()).err(), Some(Error::Content(1)));
    Ok(())
}
#[test]
fn mathml_rejects_attributes_text_and_namespace_confusion() -> Result<(), Error> {
    for length in ["0em", "1em", "12.25em", "0.001em"] {
        let f = Fragment {
            root: 0,
            nodes: vec![
                element(Tag::Math, vec![1]),
                Node::Element {
                    tag: Tag::Space,
                    attributes: vec![Attribute::Width(length.into())],
                    children: vec![],
                },
            ],
        };
        validate(&f, &mut budget())?;
    }
    for length in [
        "-1em",
        "01em",
        "1.0em",
        "0.0em",
        ".5em",
        "1e2em",
        "1px",
        "1em;display:none",
        "nanem",
    ] {
        let f = Fragment {
            root: 0,
            nodes: vec![
                element(Tag::Math, vec![1]),
                Node::Element {
                    tag: Tag::Space,
                    attributes: vec![Attribute::Width(length.into())],
                    children: vec![],
                },
            ],
        };
        assert_eq!(
            validate(&f, &mut budget()).err(),
            Some(Error::Attribute { node: 1, index: 0 })
        );
    }
    for attrs in [
        vec![Attribute::NormalIdentifier],
        vec![
            Attribute::Display(Display::Inline),
            Attribute::Display(Display::Block),
        ],
    ] {
        let f = Fragment {
            root: 0,
            nodes: vec![Node::Element {
                tag: Tag::Math,
                attributes: attrs,
                children: vec![],
            }],
        };
        assert!(matches!(
            validate(&f, &mut budget()),
            Err(Error::Attribute { .. })
        ));
    }
    let mut f = Fragment {
        root: 0,
        nodes: vec![
            element(Tag::Math, vec![1]),
            element(Tag::Text, vec![2]),
            Node::Text("a\0".into()),
        ],
    };
    assert_eq!(
        validate(&f, &mut budget()).err(),
        Some(Error::Text { node: 2, byte: 1 })
    );
    f.nodes[2] = element(Tag::Math, vec![]);
    assert_eq!(validate(&f, &mut budget()).err(), Some(Error::Content(1)));
    f.root = u64::MAX;
    assert_eq!(
        validate(&f, &mut budget()).err(),
        Some(Error::Reference(u64::MAX))
    );
    Ok(())
}
#[test]
fn mathml_shared_depth_cycles_and_stops_are_bounded() -> Result<(), Error> {
    let f = Fragment {
        root: 0,
        nodes: vec![
            element(Tag::Math, vec![1, 3]),
            element(Tag::Row, vec![2]),
            element(Tag::Row, vec![3]),
            element(Tag::Number, vec![4]),
            Node::Text("1".into()),
        ],
    };
    let mut full = budget();
    validate(&f, &mut full)?;
    assert_eq!(full.usage().depth, 5);
    let mut nested = budget();
    nested.with_depth_at_least(7, |b| validate(&f, b))?;
    assert_eq!(nested.usage().depth, 12);
    assert_eq!(nested.current_depth(), 0);
    for reverse in [false, true] {
        let mut f = f.clone();
        if reverse {
            f.nodes[0] = element(Tag::Math, vec![3, 1]);
        }
        let mut limits = budget().limits();
        limits.depth = 4;
        let mut b = Budget::new(limits);
        assert_eq!(
            validate(&f, &mut b).err(),
            Some(Error::Stopped(StopReason::DepthLimit))
        );
        assert_eq!(b.poll(), Err(StopReason::DepthLimit));
    }
    let mut cycle = f.clone();
    cycle.nodes[2] = element(Tag::Row, vec![1]);
    assert_eq!(validate(&cycle, &mut budget()).err(), Some(Error::Cycle(1)));
    let mut unused = f.clone();
    unused.nodes.push(Node::Text("unused".into()));
    assert_eq!(
        validate(&unused, &mut budget()).err(),
        Some(Error::Unreachable(5))
    );
    for reason in [
        StopReason::WorkLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = full.usage().work - 1,
            StopReason::NodeLimit => limits.nodes = full.usage().nodes - 1,
            _ => limits.allocation_units = full.usage().allocation_units - 1,
        }
        let mut b = Budget::new(limits);
        assert_eq!(validate(&f, &mut b).err(), Some(Error::Stopped(reason)));
        assert_eq!(b.poll(), Err(reason));
    }
    let mut b = budget();
    b.cancel();
    assert_eq!(
        validate(&f, &mut b).err(),
        Some(Error::Stopped(StopReason::Cancelled))
    );
    Ok(())
}
