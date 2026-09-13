use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::html::*;
use nepl3_markup::mathml::{Attribute as A, Display, Tag as M};
fn b() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        nodes: 100_000,
        output_bytes: 1_000_000,
        depth: 50_000,
        ..Limits::default()
    })
}
fn h(tag: HtmlTag, children: &[u64]) -> HtmlNode {
    HtmlNode::Element {
        tag,
        attributes: vec![],
        children: children.into(),
    }
}
fn m(tag: M, children: &[u64]) -> HtmlNode {
    HtmlNode::MathElement {
        tag,
        attributes: vec![],
        children: children.into(),
    }
}
fn text(s: &str) -> HtmlNode {
    HtmlNode::Text { text: s.into() }
}
fn policy() -> HtmlPolicy {
    HtmlPolicy { classes: vec![] }
}
fn input() -> HtmlFragment {
    // HTML -> MathML -> HTML -> MathML, using only flat index edges.
    HtmlFragment {
        root: 0,
        nodes: vec![
            h(HtmlTag::P, &[1]),
            m(M::Math, &[2]),
            m(M::Text, &[3]),
            h(HtmlTag::Span, &[4, 6]),
            m(M::Math, &[5]),
            m(M::Identifier, &[7]),
            h(HtmlTag::Br, &[]),
            text("x<&"),
        ],
    }
}
#[test]
fn alternating_namespaces_serialize_without_recursive_ownership() -> Result<(), HtmlError> {
    let f = input();
    let p = validate(&f, HtmlSlot::Block, &policy(), &mut b())?;
    let expected = "<p><math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mtext><span xmlns=\"http://www.w3.org/1999/xhtml\"><math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mi>x&lt;&amp;</mi></math><br></span></mtext></math></p>";
    assert_eq!(serialize(&p, &mut b())?, expected);
    assert_eq!(
        serialize_xhtml(&p, &mut b())?,
        expected
            .replacen("<p>", "<p xmlns=\"http://www.w3.org/1999/xhtml\">", 1)
            .replace("<br>", "<br />")
    );
    println!(
        "MIXED_HTML {}",
        expected
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    Ok(())
}
#[test]
fn mixed_content_and_ancestor_rules_cross_namespaces() -> Result<(), HtmlError> {
    for invalid in [
        vec![h(HtmlTag::P, &[1]), m(M::Identifier, &[2]), text("x")],
        vec![m(M::Math, &[1]), h(HtmlTag::Span, &[])],
        vec![m(M::Math, &[1]), m(M::Fraction, &[])],
        vec![h(HtmlTag::Table, &[1]), m(M::Math, &[])],
        vec![m(M::Math, &[1]), m(M::Text, &[2]), h(HtmlTag::P, &[])],
        vec![
            h(HtmlTag::A, &[1]),
            m(M::Math, &[2]),
            m(M::Text, &[3]),
            h(HtmlTag::A, &[]),
        ],
    ] {
        assert!(matches!(
            validate(
                &HtmlFragment {
                    root: 0,
                    nodes: invalid
                },
                HtmlSlot::Block,
                &policy(),
                &mut b()
            ),
            Err(HtmlError::Content(_))
        ));
    }
    let mut f = input();
    if let HtmlNode::MathElement { attributes, .. } = &mut f.nodes[1] {
        attributes.push(A::Display(Display::Block));
    }
    assert!(matches!(
        validate(&f, HtmlSlot::Block, &policy(), &mut b()),
        Err(HtmlError::Content(0))
    ));
    f.nodes[0] = h(HtmlTag::Div, &[1]);
    validate(&f, HtmlSlot::Block, &policy(), &mut b())?;
    if let HtmlNode::MathElement { attributes, .. } = &mut f.nodes[5] {
        attributes.push(A::Display(Display::Inline));
    }
    assert!(matches!(
        validate(&f, HtmlSlot::Block, &policy(), &mut b()),
        Err(HtmlError::Attribute { node: 5, .. })
    ));
    Ok(())
}
#[test]
fn shared_identity_and_deep_cycles_remain_bounded() -> Result<(), HtmlError> {
    let mut f = input();
    if let HtmlNode::Element { attributes, .. } = &mut f.nodes[0] {
        attributes.push(HtmlAttribute::Id {
            value: "same".into(),
        });
    }
    if let HtmlNode::Element { attributes, .. } = &mut f.nodes[3] {
        attributes.push(HtmlAttribute::Id {
            value: "same".into(),
        });
    }
    assert!(matches!(
        validate(&f, HtmlSlot::Block, &policy(), &mut b()),
        Err(HtmlError::DuplicateId(3))
    ));
    let mut nodes = Vec::new();
    for i in 0..3000 {
        nodes.push(match i % 3 {
            0 => h(HtmlTag::Span, &[i + 1]),
            1 => m(M::Math, &[i + 1]),
            _ => m(M::Text, &[i + 1]),
        });
    }
    nodes.push(text("x"));
    let mut f = HtmlFragment { root: 0, nodes };
    let p = validate(&f, HtmlSlot::Phrasing, &policy(), &mut b())?;
    let mut limits = b().limits();
    limits.depth = 3000;
    assert!(matches!(
        serialize(&p, &mut Budget::new(limits)),
        Err(HtmlError::Stopped(StopReason::DepthLimit))
    ));
    assert!(matches!(
        validate(&f, HtmlSlot::Phrasing, &policy(), &mut Budget::new(limits)),
        Err(HtmlError::Stopped(StopReason::DepthLimit))
    ));
    f.nodes[3000] = h(HtmlTag::Span, &[0]);
    assert!(matches!(
        validate(&f, HtmlSlot::Phrasing, &policy(), &mut b()),
        Err(HtmlError::Cycle(0))
    ));
    // Dropping f is a flat Vec drop even for thousands of namespace changes.
    Ok(())
}
#[test]
fn shared_math_does_not_reset_anchor_or_ruby_ancestors() {
    for order in [vec![1, 2], vec![2, 1]] {
        let f = HtmlFragment {
            root: 0,
            nodes: vec![
                h(HtmlTag::Div, &order),
                h(HtmlTag::Span, &[3]),
                h(HtmlTag::A, &[3]),
                m(M::Math, &[4]),
                m(M::Text, &[5]),
                h(HtmlTag::A, &[]),
            ],
        };
        assert!(matches!(
            validate(&f, HtmlSlot::Block, &policy(), &mut b()),
            Err(HtmlError::Content(5))
        ));
    }
    let f = HtmlFragment {
        root: 0,
        nodes: vec![
            h(HtmlTag::Ruby, &[1, 2]),
            m(M::Math, &[3]),
            h(HtmlTag::Rt, &[4]),
            m(M::Text, &[5]),
            text("outer"),
            h(HtmlTag::Ruby, &[6, 7]),
            text("字"),
            h(HtmlTag::Rt, &[8]),
            text("じ"),
        ],
    };
    assert!(matches!(
        validate(&f, HtmlSlot::Phrasing, &policy(), &mut b()),
        Err(HtmlError::Content(5))
    ));
}
#[test]
fn namespace_roots_preserve_their_slot_contract() -> Result<(), HtmlError> {
    let mut f = HtmlFragment {
        root: 0,
        nodes: vec![m(M::Math, &[])],
    };
    validate(&f, HtmlSlot::Phrasing, &policy(), &mut b())?;
    let math = nepl3_markup::mathml::Fragment {
        root: 0,
        html_policy: policy(),
        nodes: vec![
            nepl3_markup::mathml::Node::Element {
                tag: M::Math,
                attributes: vec![],
                children: vec![1],
            },
            nepl3_markup::mathml::Node::Element {
                tag: M::Text,
                attributes: vec![],
                children: vec![2],
            },
            nepl3_markup::mathml::Node::Html {
                fragment: f.clone(),
            },
        ],
    };
    assert!(matches!(
        nepl3_markup::mathml::validate(&math, &mut b()),
        Err(nepl3_markup::mathml::Error::Content(2))
    ));
    if let HtmlNode::MathElement { attributes, .. } = &mut f.nodes[0] {
        attributes.push(A::Display(Display::Block));
    }
    assert!(matches!(
        validate(&f, HtmlSlot::Phrasing, &policy(), &mut b()),
        Err(HtmlError::Content(0))
    ));
    validate(&f, HtmlSlot::Block, &policy(), &mut b())?;
    Ok(())
}
#[test]
fn mixed_operations_keep_each_stop_sticky_and_input_unchanged() -> Result<(), HtmlError> {
    let f = input();
    let saved = f.clone();
    let mut validation = b();
    let proof = validate(&f, HtmlSlot::Block, &policy(), &mut validation)?;
    let mut emission = b();
    serialize(&proof, &mut emission)?;
    for (render, usage) in [(false, validation.usage()), (true, emission.usage())] {
        for reason in [
            StopReason::WorkLimit,
            StopReason::NodeLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
            StopReason::OutputLimit,
            StopReason::Cancelled,
        ] {
            if !render && reason == StopReason::OutputLimit {
                continue;
            }
            let mut limits = b().limits();
            match reason {
                StopReason::WorkLimit => limits.work = usage.work - 1,
                StopReason::NodeLimit => limits.nodes = usage.nodes - 1,
                StopReason::AllocationLimit => limits.allocation_units = usage.allocation_units - 1,
                StopReason::DepthLimit => limits.depth = usage.depth - 1,
                StopReason::OutputLimit => limits.output_bytes = usage.output_bytes - 1,
                _ => (),
            }
            let mut budget = Budget::new(limits);
            if reason == StopReason::Cancelled {
                budget.cancel();
            }
            for _ in 0..2 {
                let result = if render {
                    serialize(&proof, &mut budget).map(|_| ())
                } else {
                    validate(&f, HtmlSlot::Block, &policy(), &mut budget).map(|_| ())
                };
                assert!(matches!(result, Err(HtmlError::Stopped(s)) if s == reason));
            }
            assert_eq!(f, saved);
        }
    }
    Ok(())
}
