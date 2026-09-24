use super::*;
use nepl3_markup::mathml::{Attribute as M, Display, OperatorForm, Tag};

#[test]
fn budgeted_copy_preserves_raw_fields_sharing_and_stops_atomically() -> Result<(), StopReason> {
    let input = HtmlRequest {
        fragment: HtmlFragment {
            root: 2,
            nodes: vec![
                HtmlNode::Text {
                    text: "世界<&".into(),
                },
                HtmlNode::MathElement {
                    tag: Tag::Text,
                    attributes: vec![
                        M::Display(Display::Inline),
                        M::NormalIdentifier,
                        M::Stretchy(true),
                        M::Symmetric(false),
                        M::LargeOperator(true),
                        M::MovableLimits(false),
                        M::Form(OperatorForm::Prefix),
                        M::Width("1em".into()),
                        M::Height("2ex".into()),
                        M::Depth("3px".into()),
                    ],
                    children: vec![0],
                },
                HtmlNode::Element {
                    tag: HtmlTag::Span,
                    attributes: vec![
                        HtmlAttribute::Id {
                            value: "identity".into(),
                        },
                        HtmlAttribute::Class {
                            values: vec!["first".into(), "second".into()],
                        },
                        HtmlAttribute::Lang { value: "ja".into() },
                        HtmlAttribute::Role {
                            value: HtmlRole::Note,
                        },
                        HtmlAttribute::AriaLabel {
                            value: "説明".into(),
                        },
                        HtmlAttribute::AriaLevel { value: 4 },
                        HtmlAttribute::DataId {
                            value: "data".into(),
                        },
                        HtmlAttribute::DataGroup {
                            value: "group".into(),
                        },
                        HtmlAttribute::Href {
                            value: HtmlHref::BetweenArtifacts {
                                source: "a.html".into(),
                                target: "b.html".into(),
                                fragment: Some("target".into()),
                            },
                        },
                        HtmlAttribute::Href {
                            value: HtmlHref::Fragment {
                                id: "target".into(),
                            },
                        },
                        HtmlAttribute::Href {
                            value: HtmlHref::Artifact {
                                path: "image.png".into(),
                                fragment: None,
                            },
                        },
                        HtmlAttribute::Href {
                            value: HtmlHref::External {
                                uri: "https://example.org/".into(),
                            },
                        },
                        HtmlAttribute::Src {
                            path: "image.png".into(),
                        },
                        HtmlAttribute::Alt {
                            value: "代替".into(),
                        },
                        HtmlAttribute::Width { value: 12 },
                        HtmlAttribute::Height { value: 34 },
                        HtmlAttribute::Start { value: 5 },
                        HtmlAttribute::Scope {
                            value: CellScope::Row,
                        },
                    ],
                    children: vec![0, 1, 0],
                },
            ],
        },
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy {
            classes: vec!["second".into(), "first".into()],
        },
    };
    let original = input.clone();
    let mut measured = budget();
    let copied = input.clone_with_budget(&mut measured)?;
    assert_eq!(copied, original);
    // A copy preserves the raw arena; this intentionally invalid attribute
    // combination never becomes a validated output through copying.
    assert!(validate(&copied.fragment, copied.slot, &copied.policy, &mut budget()).is_err());
    for (reason, amount) in [
        (StopReason::WorkLimit, measured.usage().work),
        (
            StopReason::AllocationLimit,
            measured.usage().allocation_units,
        ),
        (StopReason::NodeLimit, measured.usage().nodes),
    ] {
        assert!(amount > 0);
        for shortage in [0, 1] {
            let mut limits = measured.limits();
            match reason {
                StopReason::WorkLimit => limits.work = amount - shortage,
                StopReason::AllocationLimit => limits.allocation_units = amount - shortage,
                StopReason::NodeLimit => limits.nodes = amount - shortage,
                _ => unreachable!("fixed resource list"),
            }
            let mut limited = Budget::new(limits);
            if shortage == 0 {
                assert_eq!(input.clone_with_budget(&mut limited)?, original);
            } else {
                assert_eq!(input.clone_with_budget(&mut limited), Err(reason));
                assert_eq!(limited.poll(), Err(reason));
            }
            assert_eq!(input, original);
        }
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        input.clone_with_budget(&mut cancelled),
        Err(StopReason::Cancelled)
    );
    Ok(())
}
