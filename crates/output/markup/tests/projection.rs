use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::{
    html::*,
    mathml::{self, Error, Fragment, Node, Tag},
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        depth: 1000,
        ..Limits::default()
    })
}
fn input(s: &str) -> Fragment {
    Fragment {
        root: 2,
        html_policy: HtmlPolicy { classes: vec![] },
        nodes: vec![
            Node::Element {
                tag: Tag::Text,
                attributes: vec![],
                children: vec![1],
            },
            Node::Html {
                fragment: HtmlFragment {
                    root: 1,
                    nodes: vec![
                        HtmlNode::Text { text: s.into() },
                        HtmlNode::Element {
                            tag: HtmlTag::Span,
                            attributes: vec![],
                            children: vec![0],
                        },
                    ],
                },
            },
            Node::Element {
                tag: Tag::Math,
                attributes: vec![],
                children: vec![0],
            },
        ],
    }
}
#[test]
fn flatten_keeps_nonzero_roots_and_local_owner_maps() -> Result<(), Error> {
    let output = mathml::into_html(input("x<&"), &mut budget())?;
    assert_eq!(output.math_node(0), Some(0));
    assert_eq!(output.math_node(1), Some(2));
    assert_eq!(output.math_node(2), Some(3));
    assert_eq!(output.math_node(3), None);
    assert_eq!(output.html_node(1, 0), Some(1));
    assert_eq!(output.html_node(1, 1), Some(2));
    assert_eq!(output.html_node(1, 2), None);
    assert_eq!(output.html_node(0, 0), None);
    assert_eq!(output.html_node(u64::MAX, 0), None);
    let request = output.into_request();
    assert_eq!(request.fragment.root, 3);
    let checked = validate(
        &request.fragment,
        request.slot,
        &request.policy,
        &mut budget(),
    )?;
    assert_eq!(
        serialize_xhtml(&checked, &mut budget())?,
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mtext><span xmlns=\"http://www.w3.org/1999/xhtml\">x&lt;&amp;</span></mtext></math>"
    );
    Ok(())
}
#[test]
fn flatten_moves_large_text_and_refuses_partial_results() -> Result<(), Error> {
    // A 100 KiB owned string already exists at admission. Conversion must not
    // charge another copy of it against a 16 KiB logical allocation allowance.
    let mut limits = budget().limits();
    limits.allocation_units = 16 * 1024;
    mathml::into_html(input(&"x".repeat(100 * 1024)), &mut Budget::new(limits))?;
    let mut full = budget();
    mathml::into_html(input("x"), &mut full)?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = full.usage().work - 1,
            StopReason::NodeLimit => limits.nodes = full.usage().nodes - 1,
            StopReason::AllocationLimit => {
                limits.allocation_units = full.usage().allocation_units - 1
            }
            StopReason::DepthLimit => limits.depth = full.usage().depth - 1,
            _ => (),
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        for _ in 0..2 {
            assert!(
                matches!(mathml::into_html(input("x"), &mut b), Err(Error::Stopped(s)) if s == reason)
            );
        }
    }
    let mut broken = input("x");
    broken.root = u64::MAX;
    assert!(matches!(
        mathml::into_html(broken, &mut budget()),
        Err(Error::Reference(u64::MAX))
    ));
    Ok(())
}
#[test]
fn separate_html_owners_do_not_alias_local_node_numbers() -> Result<(), Error> {
    let mut f = input("left");
    f.nodes.push(Node::Html {
        fragment: HtmlFragment {
            root: 0,
            nodes: vec![HtmlNode::Text {
                text: "right".into(),
            }],
        },
    });
    if let Node::Element { children, .. } = &mut f.nodes[0] {
        children.push(3);
    }
    let p = mathml::into_html(f, &mut budget())?;
    assert_eq!(p.html_node(1, 0), Some(1));
    assert_eq!(p.html_node(3, 0), Some(4));
    assert_eq!(p.math_node(3), Some(4));
    let request = p.request();
    assert!(matches!(&request.fragment.nodes[1], HtmlNode::Text { text } if text == "left"));
    assert!(matches!(&request.fragment.nodes[4], HtmlNode::Text { text } if text == "right"));
    Ok(())
}
#[test]
fn shared_leaf_and_nested_math_edges_keep_their_exact_targets() -> Result<(), Error> {
    let mut f = input("x");
    if let Node::Element { children, .. } = &mut f.nodes[0] {
        children.push(1);
    }
    if let Node::Html { fragment } = &mut f.nodes[1] {
        fragment.nodes[1] = HtmlNode::Element {
            tag: HtmlTag::Span,
            attributes: vec![],
            children: vec![2],
        };
        fragment.nodes.push(HtmlNode::MathElement {
            tag: Tag::Math,
            attributes: vec![],
            children: vec![3],
        });
        fragment.nodes.push(HtmlNode::MathElement {
            tag: Tag::Identifier,
            attributes: vec![],
            children: vec![0],
        });
    }
    let p = mathml::into_html(f, &mut budget())?;
    assert_eq!(p.math_node(2), Some(5));
    assert_eq!(p.html_node(1, 3), Some(4));
    let request = p.request();
    assert!(
        matches!(&request.fragment.nodes[0], HtmlNode::MathElement { children, .. } if children == &[2, 2])
    );
    assert!(
        matches!(&request.fragment.nodes[4], HtmlNode::MathElement { children, .. } if children == &[1])
    );
    let proof = validate(
        &request.fragment,
        request.slot,
        &request.policy,
        &mut budget(),
    )?;
    let leaf = "<span xmlns=\"http://www.w3.org/1999/xhtml\"><math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mi>x</mi></math></span>";
    assert_eq!(
        serialize_xhtml(&proof, &mut budget())?,
        format!(
            "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mtext>{leaf}{leaf}</mtext></math>"
        )
    );
    Ok(())
}
