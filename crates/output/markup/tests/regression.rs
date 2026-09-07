use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::html::*;
fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        depth: 1000,
        output_bytes: 1000000,
        ..Limits::default()
    })
}
fn element(tag: HtmlTag, children: &[u64]) -> HtmlNode {
    HtmlNode::Element {
        tag,
        children: children.into(),
        attributes: vec![],
    }
}
fn text(text: &str) -> HtmlNode {
    HtmlNode::Text { text: text.into() }
}
fn policy() -> HtmlPolicy {
    HtmlPolicy { classes: vec![] }
}
#[test]
fn external_port_is_unsigned_ascii_decimal() {
    for (port, accepted) in [
        ("0", true),
        ("80", true),
        ("00080", true),
        ("65535", true),
        ("+80", false),
        ("+0", false),
        ("-0", false),
        ("65536", false),
        ("", false),
        ("80x", false),
        ("８０", false),
    ] {
        let f = HtmlFragment {
            root: 0,
            nodes: vec![HtmlNode::Element {
                tag: HtmlTag::A,
                attributes: vec![HtmlAttribute::Href {
                    value: HtmlHref::External {
                        uri: format!("https://example.org:{port}/"),
                    },
                }],
                children: vec![],
            }],
        };
        assert_eq!(
            validate(&f, HtmlSlot::Phrasing, &policy(), &mut budget()).is_ok(),
            accepted,
            "port {port}"
        );
    }
}
#[test]
fn serializer_work_limit_bounds_frontier_before_enqueue() -> Result<(), HtmlError> {
    for width in [1, 1000, 100000] {
        let f = HtmlFragment {
            root: 0,
            nodes: vec![element(HtmlTag::Div, &vec![1; width]), text("x")],
        };
        let proof = validate(&f, HtmlSlot::Block, &policy(), &mut budget())?;
        let mut b = Budget::new(Limits {
            work: 6,
            ..budget().limits()
        });
        assert_eq!(
            serialize(&proof, &mut b),
            Err(HtmlError::Stopped(StopReason::WorkLimit))
        );
        assert!(
            b.usage().allocation_units <= 1024,
            "width {width}: {:?}",
            b.usage()
        );
    }
    Ok(())
}
#[test]
fn zero_work_does_not_allocate_a_validation_arena() {
    for size in [1, 1000, 100000] {
        let f = HtmlFragment {
            root: 0,
            nodes: vec![text("x"); size],
        };
        let mut b = Budget::new(Limits {
            work: 0,
            ..budget().limits()
        });
        assert!(matches!(
            validate(&f, HtmlSlot::Block, &policy(), &mut b),
            Err(HtmlError::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(b.usage().allocation_units, 0);
    }
}
#[test]
fn ruby_grammar_rejects_invalid_fallback_and_base_sequences() {
    use HtmlTag::*;
    for nodes in [
        vec![element(Ruby, &[])],
        vec![element(Ruby, &[1]), text("字")],
        vec![
            element(Ruby, &[1, 2]),
            text("字"),
            element(Rp, &[3]),
            text("("),
        ],
        vec![
            element(Ruby, &[1, 2, 3, 4]),
            text("字"),
            element(Rp, &[5]),
            element(Rt, &[6]),
            element(Rp, &[7]),
            element(Em, &[8]),
            text("じ"),
            text(")"),
            text("("),
        ],
        vec![
            element(Ruby, &[1, 4]),
            element(Span, &[2]),
            element(Ruby, &[3, 5]),
            text("字"),
            element(Rt, &[6]),
            element(Rt, &[6]),
            text("じ"),
        ],
    ] {
        let f = HtmlFragment { root: 0, nodes };
        assert!(matches!(
            validate(&f, HtmlSlot::Phrasing, &policy(), &mut budget()),
            Err(HtmlError::Content(_))
        ));
    }
}
#[test]
fn ruby_group_annotations_and_single_nested_base_are_retained() -> Result<(), HtmlError> {
    use HtmlTag::*;
    for (nodes, expected) in [
        (
            vec![
                element(Ruby, &[1, 2, 3, 4, 5, 6]),
                text("字"),
                element(Rp, &[7]),
                element(Rt, &[8]),
                element(Rp, &[9]),
                element(Rt, &[10]),
                element(Rp, &[11]),
                text("("),
                text("じ"),
                text(","),
                text("character"),
                text(")"),
            ],
            "<ruby>字<rp>(</rp><rt>じ</rt><rp>,</rp><rt>character</rt><rp>)</rp></ruby>",
        ),
        (
            vec![
                element(Ruby, &[1, 2]),
                element(Ruby, &[3, 4]),
                element(Rt, &[5]),
                text("字"),
                element(Rt, &[6]),
                text("character"),
                text("じ"),
            ],
            "<ruby><ruby>字<rt>じ</rt></ruby><rt>character</rt></ruby>",
        ),
    ] {
        let f = HtmlFragment { root: 0, nodes };
        let proof = validate(&f, HtmlSlot::Phrasing, &policy(), &mut budget())?;
        assert_eq!(serialize(&proof, &mut budget())?, expected);
    }
    Ok(())
}
