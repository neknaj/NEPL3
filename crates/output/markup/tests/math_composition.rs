use nepl3_core::budget::{Budget, Limits};
use nepl3_markup::{html::*, katex::fragment as visual, mathml};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        output_bytes: 10_000_000,
        nodes: 10000,
        depth: 1000,
        ..Limits::default()
    })
}
fn source(block: bool, repeated: bool) -> HtmlFragment {
    HtmlFragment {
        root: 2,
        nodes: vec![
            HtmlNode::Text { text: "x".into() },
            HtmlNode::MathElement {
                tag: mathml::Tag::Math,
                attributes: vec![mathml::Attribute::Display(if block {
                    mathml::Display::Block
                } else {
                    mathml::Display::Inline
                })],
                children: vec![3],
            },
            HtmlNode::Element {
                tag: HtmlTag::Div,
                attributes: vec![],
                children: if repeated { vec![1, 1] } else { vec![1] },
            },
            HtmlNode::MathElement {
                tag: mathml::Tag::Identifier,
                attributes: vec![],
                children: vec![0],
            },
        ],
    }
}
fn tree() -> visual::Fragment {
    visual::Fragment {
        nodes: vec![visual::Node::Span {
            classes: "katex".into(),
            style: "height:1em;".into(),
            aria_hidden: None,
            children: vec![],
        }],
    }
}
#[test]
fn inline_and_block_keep_accessible_math_and_generated_styles() -> Result<(), String> {
    for block in [false, true] {
        let f = source(block, false);
        let p = HtmlPolicy { classes: vec![] };
        let checked =
            validate(&f, HtmlSlot::Block, &p, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let tree = tree();
        let visual = visual::validate(
            &tree,
            &visual::Policy {
                classes: &["katex"],
                scope: "nepl-math-case",
            },
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut b = budget();
        let result = serialize_with_math(
            &checked,
            &[MathBinding {
                root: 1,
                visual: &visual,
            }],
            3,
            &mut b,
        )
        .map_err(|e| format!("{e:?}"))?;
        assert!(result.html.contains("aria-hidden=\"true\""));
        assert!(result.html.contains(if block {
            "<div class=\"nepl-math-accessible\"><math"
        } else {
            "<span class=\"nepl-math-accessible\"><math"
        }));
        assert!(result.stylesheet.contains("height:1em!important"));
        assert!(result.stylesheet.contains(".nepl-math-accessible"));
        assert_eq!(
            b.usage().output_bytes,
            (result.html.len() + result.stylesheet.len()) as u64
        );
        assert_eq!(b.usage().depth, 8);
    }
    Ok(())
}
#[test]
fn shared_arena_root_cannot_reuse_a_visual_scope() -> Result<(), String> {
    let f = source(false, true);
    let p = HtmlPolicy { classes: vec![] };
    let checked = validate(&f, HtmlSlot::Block, &p, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let tree = tree();
    let v = visual::validate(
        &tree,
        &visual::Policy {
            classes: &["katex"],
            scope: "nepl-math-case",
        },
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(
        serialize_with_math(
            &checked,
            &[MathBinding {
                root: 1,
                visual: &v
            }],
            1,
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn wrong_root_reserved_classes_and_exhausted_budgets_fail_closed() -> Result<(), String> {
    let mut f = source(false, false);
    let policy = HtmlPolicy {
        classes: vec!["nepl-math-case".into()],
    };
    let tree = tree();
    let visual = visual::validate(
        &tree,
        &visual::Policy {
            classes: &["katex"],
            scope: "nepl-math-case",
        },
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let bindings = [MathBinding {
        root: 1,
        visual: &visual,
    }];
    {
        let checked =
            validate(&f, HtmlSlot::Block, &policy, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert!(
            serialize_with_math(
                &checked,
                &[MathBinding {
                    root: 0,
                    visual: &visual
                }],
                1,
                &mut budget()
            )
            .is_err()
        );
        let mut measured = budget();
        serialize_with_math(&checked, &bindings, 3, &mut measured).map_err(|e| format!("{e:?}"))?;
        for resource in [0, 1, 2] {
            let mut limits = budget().limits();
            match resource {
                0 => limits.output_bytes = measured.usage().output_bytes - 1,
                1 => limits.depth = measured.usage().depth - 1,
                _ => limits.work = 0,
            }
            let mut stopped = Budget::new(limits);
            assert!(matches!(
                serialize_with_math(&checked, &bindings, 3, &mut stopped),
                Err(HtmlError::Stopped(_))
            ));
            assert!(stopped.poll().is_err());
        }
    }
    if let HtmlNode::Element { attributes, .. } = &mut f.nodes[2] {
        attributes.push(HtmlAttribute::Class {
            values: vec!["nepl-math-case".into()],
        });
    }
    let checked =
        validate(&f, HtmlSlot::Block, &policy, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        serialize_with_math(&checked, &bindings, 1, &mut budget()),
        Err(HtmlError::Policy)
    ));
    Ok(())
}

#[test]
fn enclosing_depth_overflow_is_a_sticky_stop() -> Result<(), String> {
    let f = source(false, false);
    let p = HtmlPolicy { classes: vec![] };
    let checked = validate(&f, HtmlSlot::Block, &p, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let tree = tree();
    let visual = visual::validate(
        &tree,
        &visual::Policy {
            classes: &["katex"],
            scope: "nepl-math-case",
        },
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut limits = budget().limits();
    limits.depth = u64::MAX;
    let mut b = Budget::new(limits);
    assert!(matches!(
        serialize_with_math(
            &checked,
            &[MathBinding {
                root: 1,
                visual: &visual
            }],
            u64::MAX - 3,
            &mut b
        ),
        Err(HtmlError::Stopped(
            nepl3_core::budget::StopReason::DepthLimit
        ))
    ));
    assert!(b.poll().is_err());
    Ok(())
}

#[test]
fn two_occurrences_have_separate_css_and_reject_ambiguous_bindings() -> Result<(), String> {
    let mut f = source(false, false);
    f.nodes.push(f.nodes[1].clone());
    if let HtmlNode::Element { children, .. } = &mut f.nodes[2] {
        children.push(4);
    }
    let p = HtmlPolicy { classes: vec![] };
    let checked = validate(&f, HtmlSlot::Block, &p, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let tree = tree();
    for (left, right, okay) in [
        ("nepl-math-a", "nepl-math-b", true),
        ("nepl-math-a", "nepl-math-a", false),
        ("nepl-math-a", "nepl-math-a-n1", false),
        ("nepl-math-a-n1", "nepl-math-a", false),
        ("nepl-math-accessible", "nepl-math-b", false),
        ("nepl-math-presentation", "nepl-math-b", false),
    ] {
        let a = visual::validate(
            &tree,
            &visual::Policy {
                classes: &["katex"],
                scope: left,
            },
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let b = visual::validate(
            &tree,
            &visual::Policy {
                classes: &["katex"],
                scope: right,
            },
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let bindings = [
            MathBinding {
                root: 1,
                visual: &a,
            },
            MathBinding {
                root: 4,
                visual: &b,
            },
        ];
        let result = serialize_with_math(&checked, &bindings, 1, &mut budget());
        assert_eq!(result.is_ok(), okay, "{left}/{right}");
        if let Ok(result) = result {
            assert!(result.stylesheet.contains(".nepl-math-a .nepl-math-a-n0"));
            assert!(result.stylesheet.contains(".nepl-math-b .nepl-math-b-n0"));
        }
        if okay {
            assert!(
                serialize_with_math(
                    &checked,
                    &[
                        MathBinding {
                            root: 1,
                            visual: &a
                        },
                        MathBinding {
                            root: 1,
                            visual: &b
                        }
                    ],
                    1,
                    &mut budget()
                )
                .is_err()
            );
            assert!(
                serialize_with_math(
                    &checked,
                    &[MathBinding {
                        root: 99,
                        visual: &a
                    }],
                    1,
                    &mut budget()
                )
                .is_err()
            );
            assert!(serialize_with_math(&checked, &bindings, 0, &mut budget()).is_err());
        }
    }
    Ok(())
}

#[test]
fn empty_bindings_preserve_html_and_all_measured_resource_edges_stop() -> Result<(), String> {
    let f = source(false, false);
    let p = HtmlPolicy { classes: vec![] };
    let checked = validate(&f, HtmlSlot::Block, &p, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let empty =
        serialize_with_math(&checked, &[], 1, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        empty.html,
        serialize(&checked, &mut budget()).map_err(|e| format!("{e:?}"))?
    );
    assert!(empty.stylesheet.is_empty());
    let tree = tree();
    let visual = visual::validate(
        &tree,
        &visual::Policy {
            classes: &["katex"],
            scope: "nepl-math-case",
        },
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let bindings = [MathBinding {
        root: 1,
        visual: &visual,
    }];
    let mut measured = budget();
    serialize_with_math(&checked, &bindings, 1, &mut measured).map_err(|e| format!("{e:?}"))?;
    for resource in 0..3 {
        for shortage in [0, 1] {
            let mut limits = budget().limits();
            match resource {
                0 => limits.nodes = measured.usage().nodes - shortage,
                1 => limits.allocation_units = measured.usage().allocation_units - shortage,
                _ => limits.work = measured.usage().work - shortage,
            }
            let result = serialize_with_math(&checked, &bindings, 1, &mut Budget::new(limits));
            assert_eq!(
                result.is_ok(),
                shortage == 0,
                "resource {resource} shortage {shortage}"
            );
        }
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(matches!(
        serialize_with_math(&checked, &bindings, 1, &mut cancelled),
        Err(HtmlError::Stopped(
            nepl3_core::budget::StopReason::Cancelled
        ))
    ));
    Ok(())
}
