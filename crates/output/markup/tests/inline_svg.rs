use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::html::*;
use nepl3_markup::mathml::{self, Tag};
fn b() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        nodes: 100_000,
        output_bytes: 1_000_000,
        depth: 1000,
        ..Limits::default()
    })
}
fn policy() -> HtmlPolicy {
    HtmlPolicy {
        classes: vec!["cjk_fallback".into(), "KaTeX-test".into()],
    }
}
fn span(children: &[u64]) -> HtmlNode {
    HtmlNode::Element {
        tag: HtmlTag::Span,
        attributes: vec![],
        children: children.into(),
    }
}
fn svg(children: &[u64]) -> HtmlNode {
    HtmlNode::SvgElement {
        element: HtmlSvgElement::Svg {
            width: "100%".into(),
            height: "1em".into(),
            view_box: Some("0 0 1 1".into()),
            aspect: Some(HtmlSvgAspect::None),
        },
        children: children.into(),
    }
}
fn path() -> HtmlNode {
    HtmlNode::SvgElement {
        element: HtmlSvgElement::Path {
            data: "M0 0L1 1Z".into(),
        },
        children: vec![],
    }
}
fn input() -> HtmlFragment {
    HtmlFragment {
        root: 0,
        nodes: vec![
            span(&[1]),
            svg(&[2, 3]),
            path(),
            HtmlNode::SvgElement {
                element: HtmlSvgElement::Line {
                    x1: HtmlSvgEndpoint::Zero,
                    y1: HtmlSvgEndpoint::Full,
                    x2: HtmlSvgEndpoint::Full,
                    y2: HtmlSvgEndpoint::Zero,
                    stroke_width: ".046em".into(),
                },
                children: vec![],
            },
        ],
    }
}
#[test]
fn closed_svg_and_aria_have_deterministic_html_and_xml_output() -> Result<(), HtmlError> {
    let mut f = input();
    if let HtmlNode::Element { attributes, .. } = &mut f.nodes[0] {
        *attributes = vec![
            HtmlAttribute::Class {
                values: vec!["cjk_fallback".into(), "KaTeX-test".into()],
            },
            HtmlAttribute::AriaHidden { value: true },
        ];
    }
    let checked = validate(&f, HtmlSlot::Phrasing, &policy(), &mut b())?;
    let expected = "<span aria-hidden=\"true\" class=\"cjk_fallback KaTeX-test\"><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100%\" height=\"1em\" viewBox=\"0 0 1 1\" preserveAspectRatio=\"none\"><path d=\"M0 0L1 1Z\"></path><line x1=\"0\" y1=\"100%\" x2=\"100%\" y2=\"0\" stroke-width=\".046em\"></line></svg></span>";
    assert_eq!(serialize(&checked, &mut b())?, expected);
    assert_eq!(
        serialize_xhtml(&checked, &mut b())?,
        expected.replacen("<span ", "<span xmlns=\"http://www.w3.org/1999/xhtml\" ", 1)
    );
    Ok(())
}
#[test]
fn svg_admission_does_not_open_other_namespaces_or_leaf_children() {
    let math = |tag, children: Vec<u64>| HtmlNode::MathElement {
        tag,
        attributes: vec![],
        children,
    };
    for nodes in [
        vec![svg(&[])],
        vec![path()],
        vec![span(&[1]), path()],
        vec![svg(&[1]), span(&[])],
        vec![span(&[1]), svg(&[2]), svg(&[])],
        vec![span(&[1]), svg(&[2]), HtmlNode::Text { text: "x".into() }],
        vec![math(Tag::Math, vec![1]), math(Tag::Text, vec![2]), svg(&[])],
        vec![
            HtmlNode::Element {
                tag: HtmlTag::Table,
                attributes: vec![],
                children: vec![1],
            },
            svg(&[]),
        ],
    ] {
        for slot in [HtmlSlot::Phrasing, HtmlSlot::Block] {
            assert!(
                validate(
                    &HtmlFragment {
                        root: 0,
                        nodes: nodes.clone()
                    },
                    slot,
                    &policy(),
                    &mut b()
                )
                .is_err()
            );
        }
    }
    let mut f = input();
    if let HtmlNode::SvgElement { children, .. } = &mut f.nodes[2] {
        children.push(2);
    }
    assert!(matches!(
        validate(&f, HtmlSlot::Phrasing, &policy(), &mut b()),
        Err(HtmlError::Content(2))
    ));
    let mut f = input();
    if let HtmlNode::SvgElement {
        element: HtmlSvgElement::Path { data },
        ..
    } = &mut f.nodes[2]
    {
        *data = "M0 0<script>".into();
    }
    assert!(matches!(
        validate(&f, HtmlSlot::Phrasing, &policy(), &mut b()),
        Err(HtmlError::Attribute { node: 2, .. })
    ));
}
#[test]
fn class_expansion_does_not_change_data_identifiers_or_duplicate_rules() {
    for attr in [
        HtmlAttribute::DataId {
            value: "cjk_fallback".into(),
        },
        HtmlAttribute::DataGroup {
            value: "KaTeX-test".into(),
        },
        HtmlAttribute::Class { values: vec![] },
        HtmlAttribute::Class {
            values: vec!["cjk_fallback".into(), "cjk_fallback".into()],
        },
        HtmlAttribute::Class {
            values: vec!["unknown".into()],
        },
    ] {
        let f = HtmlFragment {
            root: 0,
            nodes: vec![HtmlNode::Element {
                tag: HtmlTag::Span,
                attributes: vec![attr],
                children: vec![],
            }],
        };
        assert!(validate(&f, HtmlSlot::Phrasing, &policy(), &mut b()).is_err());
    }
    for name in ["", "1class", "a b", "a.b", "a\"", "_class"] {
        assert!(
            validate(
                &input(),
                HtmlSlot::Phrasing,
                &HtmlPolicy {
                    classes: vec![name.into()]
                },
                &mut b()
            )
            .is_err()
        );
    }
    let f = HtmlFragment {
        root: 0,
        nodes: vec![HtmlNode::Element {
            tag: HtmlTag::Span,
            attributes: vec![
                HtmlAttribute::AriaHidden { value: true },
                HtmlAttribute::AriaHidden { value: false },
            ],
            children: vec![],
        }],
    };
    assert!(validate(&f, HtmlSlot::Phrasing, &policy(), &mut b()).is_err());
}
#[test]
fn svg_edges_count_toward_depth_and_expanded_node_budget() -> Result<(), HtmlError> {
    let f = input();
    let mut budget = b();
    let p = validate(&f, HtmlSlot::Phrasing, &policy(), &mut budget)?;
    assert_eq!(budget.usage().nodes, 4);
    let mut short = Budget::new(Limits {
        depth: 2,
        ..b().limits()
    });
    assert!(matches!(
        validate(&f, HtmlSlot::Phrasing, &policy(), &mut short),
        Err(HtmlError::Stopped(StopReason::DepthLimit))
    ));
    let mut output = b();
    serialize(&p, &mut output)?;
    let first = output.usage().output_bytes;
    serialize(&p, &mut output)?;
    assert_eq!(output.usage().output_bytes, first * 2);
    let mut cancelled = b();
    cancelled.cancel();
    assert!(matches!(
        serialize(&p, &mut cancelled),
        Err(HtmlError::Stopped(StopReason::Cancelled))
    ));
    let mut shared = input();
    if let HtmlNode::Element { children, .. } = &mut shared.nodes[0] {
        children.push(1);
    }
    let mut budget = b();
    validate(&shared, HtmlSlot::Phrasing, &policy(), &mut budget)?;
    assert_eq!(budget.usage().nodes, 7);
    Ok(())
}
#[test]
fn mathml_html_projection_rebases_svg_edges_at_nonzero_offset() -> Result<(), String> {
    let f = mathml::Fragment {
        root: 0,
        html_policy: policy(),
        nodes: vec![
            mathml::Node::Element {
                tag: Tag::Math,
                attributes: vec![],
                children: vec![1],
            },
            mathml::Node::Element {
                tag: Tag::Text,
                attributes: vec![],
                children: vec![2],
            },
            mathml::Node::Html { fragment: input() },
        ],
    };
    let projected = mathml::into_html(f, &mut b()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(projected.html_node(2, 2), Some(4));
    let checked = validate(
        &projected.request().fragment,
        projected.request().slot,
        &projected.request().policy,
        &mut b(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let text = serialize(&checked, &mut b()).map_err(|e| format!("{e:?}"))?;
    assert!(text.contains("<mtext><span xmlns=\"http://www.w3.org/1999/xhtml\"><svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(text.ends_with("</svg></span></mtext></math>"));
    assert!(
        matches!(&projected.request().fragment.nodes[3],HtmlNode::SvgElement {children,..} if children==&[4,5])
    );
    Ok(())
}
#[test]
fn svg_and_aria_cross_first_receiver_cbor_and_old_revision_is_rejected() -> Result<(), String> {
    use nepl3_core::{
        schema::SchemaRegistry,
        source::{SourceAdmission, SourceStore},
        value::NdfValue,
    };
    use nepl3_markup::portable;
    use nepl3_wire::foundation::FoundationCodec;
    let err = |e| format!("{e:?}");
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_markup::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        let reference = d.reference(&mut b()).map_err(err)?;
        r.register(reference, d, &mut b()).map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut sender = FoundationCodec::new(&r, &store, &mut a).map_err(|e| format!("{e:?}"))?;
    let mut f = input();
    if let HtmlNode::Element { attributes, .. } = &mut f.nodes[0] {
        *attributes = vec![
            HtmlAttribute::AriaHidden { value: false },
            HtmlAttribute::Class {
                values: vec!["cjk_fallback".into()],
            },
        ];
    }
    let request = HtmlRequest {
        fragment: f,
        slot: HtmlSlot::Phrasing,
        policy: policy(),
    };
    let value =
        portable::to_value(&request, &r, &mut sender, &mut b()).map_err(|e| format!("{e:?}"))?;
    let bytes = nepl3_wire::encode(&value, &mut b()).map_err(|e| format!("{e:?}"))?;
    let value = nepl3_wire::decode(&bytes, &mut b()).map_err(|e| format!("{e:?}"))?;
    let mut fresh = SourceAdmission::default();
    let mut receiver =
        FoundationCodec::new(&r, &store, &mut fresh).map_err(|e| format!("{e:?}"))?;
    let received =
        portable::from_value(&value, &r, &mut receiver, &mut b()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(received, request);
    let mut old = value.clone();
    if let NdfValue::Record(root) = &mut old {
        root.schema.revision = 2;
    }
    let old = nepl3_wire::decode(
        &nepl3_wire::encode(&old, &mut b()).map_err(|e| format!("{e:?}"))?,
        &mut b(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(portable::from_value(&old, &r, &mut receiver, &mut b()).is_err());
    // Mutations retain the neutral shape but violate namespace/tree admission.
    for (index, child) in [(1usize, 0u64), (1, 1), (1, 99), (2, 0)] {
        let mut hostile = value.clone();
        let NdfValue::Record(root) = &mut hostile else {
            return Err("request".into());
        };
        let NdfValue::Record(fragment) = &mut root.fields[0] else {
            return Err("fragment".into());
        };
        let NdfValue::List(nodes) = &mut fragment.fields[1] else {
            return Err("nodes".into());
        };
        let NdfValue::Variant(node) = &mut nodes[index] else {
            return Err("node".into());
        };
        node.fields[1] = NdfValue::List(vec![NdfValue::U64(child)]);
        let hostile = nepl3_wire::decode(
            &nepl3_wire::encode(&hostile, &mut b()).map_err(|e| format!("{e:?}"))?,
            &mut b(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert!(portable::from_value(&hostile, &r, &mut receiver, &mut b()).is_err());
    }
    let mut bad = value;
    let NdfValue::Record(root) = &mut bad else {
        return Err("request".into());
    };
    let NdfValue::Record(fragment) = &mut root.fields[0] else {
        return Err("fragment".into());
    };
    let NdfValue::List(nodes) = &mut fragment.fields[1] else {
        return Err("nodes".into());
    };
    let NdfValue::Variant(path) = &mut nodes[2] else {
        return Err("path node".into());
    };
    let NdfValue::Variant(element) = &mut path.fields[0] else {
        return Err("path element".into());
    };
    element.fields[0] = NdfValue::Text("M0 0<script>".into());
    let bad = nepl3_wire::decode(
        &nepl3_wire::encode(&bad, &mut b()).map_err(|e| format!("{e:?}"))?,
        &mut b(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(portable::from_value(&bad, &r, &mut receiver, &mut b()).is_err());
    Ok(())
}
#[test]
fn every_svg_budget_boundary_is_exact_and_parent_depth_is_preserved() -> Result<(), HtmlError> {
    let f = input();
    let p = policy();
    let mut measured = b();
    validate(&f, HtmlSlot::Phrasing, &p, &mut measured)?;
    let validation = measured.usage();
    let proof = validate(&f, HtmlSlot::Phrasing, &p, &mut b())?;
    let mut measured = b();
    serialize(&proof, &mut measured)?;
    let output = measured.usage();
    for (serial, usage) in [(false, validation), (true, output)] {
        for (reason, amount) in [
            (StopReason::WorkLimit, usage.work),
            (StopReason::AllocationLimit, usage.allocation_units),
            (StopReason::NodeLimit, usage.nodes),
            (StopReason::OutputLimit, usage.output_bytes),
        ] {
            if amount == 0 {
                continue;
            }
            for short in [false, true] {
                let mut limits = b().limits();
                let limit = amount - u64::from(short);
                match reason {
                    StopReason::WorkLimit => limits.work = limit,
                    StopReason::AllocationLimit => limits.allocation_units = limit,
                    StopReason::NodeLimit => limits.nodes = limit,
                    StopReason::OutputLimit => limits.output_bytes = limit,
                    _ => return Err(HtmlError::Policy),
                }
                let mut limited = Budget::new(limits);
                let result = if serial {
                    serialize(&proof, &mut limited).map(|_| ())
                } else {
                    validate(&f, HtmlSlot::Phrasing, &p, &mut limited).map(|_| ())
                };
                if short {
                    assert_eq!(result, Err(HtmlError::Stopped(reason)));
                    let usage = limited.usage();
                    assert_eq!(limited.poll(), Err(reason));
                    assert_eq!(limited.usage(), usage);
                } else {
                    result?;
                }
            }
        }
        for depth in [7, 8] {
            let mut limited = Budget::new(Limits {
                depth,
                ..b().limits()
            });
            let result = limited.with_depth_at_least(5, |b| {
                if serial {
                    serialize(&proof, b).map(|_| ())
                } else {
                    validate(&f, HtmlSlot::Phrasing, &p, b).map(|_| ())
                }
            });
            if depth == 7 {
                assert_eq!(result, Err(HtmlError::Stopped(StopReason::DepthLimit)));
            } else {
                result?;
            }
        }
    }
    Ok(())
}
