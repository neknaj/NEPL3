use super::*;
use nepl3_doc_html::{ParallelMode, RenderOptions};
use nepl3_markup::html::{HtmlFragment, HtmlPolicy, HtmlRequest, HtmlSlot};

fn fixture() -> RenderedFragment {
    RenderedFragment {
        document_digest: Digest::of(b"source identity"),
        options: RenderOptions {
            parallel: ParallelMode::Rows,
        },
        markup: HtmlRequest {
            fragment: HtmlFragment {
                root: 0,
                nodes: vec![
                    HtmlNode::Element {
                        tag: HtmlTag::Article,
                        attributes: vec![],
                        children: vec![1],
                    },
                    HtmlNode::Element {
                        tag: HtmlTag::Section,
                        attributes: vec![HtmlAttribute::Id {
                            value: "n-73".into(),
                        }],
                        children: vec![2],
                    },
                    HtmlNode::Element {
                        tag: HtmlTag::Span,
                        attributes: vec![HtmlAttribute::Id {
                            value: "ordinary-anchor".into(),
                        }],
                        children: vec![],
                    },
                ],
            },
            slot: HtmlSlot::Block,
            policy: HtmlPolicy { classes: vec![] },
        },
        origins: vec![
            ElementOrigin {
                element: 0,
                node: 10,
            },
            ElementOrigin {
                element: 1,
                node: 11,
            },
            ElementOrigin {
                element: 2,
                node: 12,
            },
        ],
    }
}
fn alias(section: Option<&str>, name: &str) -> Alias {
    Alias {
        section: section.map(str::to_owned),
        name: name.into(),
    }
}
#[test]
fn typed_aliases_preserve_nodes_origins_and_destination_order() -> Result<(), String> {
    let mut fragment = fixture();
    let before = fragment.clone();
    apply(
        &mut fragment,
        &[
            alias(None, "題"),
            alias(Some("s"), "節一"),
            alias(Some("s"), "節二"),
        ],
        &mut crate::doc::source::budget(),
    )?;
    assert_eq!(fragment.document_digest, before.document_digest);
    assert_eq!(&fragment.origins[..3], before.origins);
    assert_eq!(fragment.origins.len(), fragment.markup.fragment.nodes.len());
    for (parent, expected) in [(0, vec!["題"]), (1, vec!["節一", "節二"])] {
        let HtmlNode::Element { children, .. } = &fragment.markup.fragment.nodes[parent] else {
            return Err("parent".into());
        };
        for (child, name) in children.iter().zip(expected) {
            let HtmlNode::Element {
                tag,
                attributes,
                children: own,
            } = &fragment.markup.fragment.nodes[*child as usize]
            else {
                return Err("alias".into());
            };
            assert_eq!(*tag, HtmlTag::Span);
            assert_eq!(attributes, &[HtmlAttribute::Id { value: name.into() }]);
            assert!(own.is_empty());
            let origin = &fragment.origins[*child as usize];
            assert_eq!(origin.element, *child);
            assert_eq!(origin.node, before.origins[parent].node);
        }
    }
    let m = &fragment.markup;
    nepl3_markup::html::validate(
        &m.fragment,
        m.slot,
        &m.policy,
        &mut crate::doc::source::budget(),
    )
    .map_err(err)?;
    Ok(())
}
#[test]
fn nested_unicode_section_selects_the_real_destination() -> Result<(), String> {
    let mut fragment = fixture();
    fragment.markup.fragment.nodes.push(HtmlNode::Element {
        tag: HtmlTag::Section,
        attributes: vec![HtmlAttribute::Id {
            value: "n-e7af80".into(),
        }],
        children: vec![],
    });
    fragment.origins.push(ElementOrigin {
        element: 3,
        node: 30,
    });
    if let HtmlNode::Element { children, .. } = &mut fragment.markup.fragment.nodes[1] {
        children.push(3);
    }
    apply(
        &mut fragment,
        &[alias(Some("節"), "旧節")],
        &mut crate::doc::source::budget(),
    )?;
    let HtmlNode::Element { children, .. } = &fragment.markup.fragment.nodes[3] else {
        return Err("target".into());
    };
    assert_eq!(children, &[4]);
    assert_eq!(
        fragment.origins[4],
        ElementOrigin {
            element: 4,
            node: 30
        }
    );
    Ok(())
}

#[test]
fn invalid_aliases_and_nonsection_targets_are_rejected() {
    for values in [
        vec![alias(None, "")],
        vec![alias(None, "bad space")],
        vec![alias(None, "same"), alias(Some("s"), "same")],
        vec![alias(None, "n-73")],
        vec![alias(None, "ordinary-anchor")],
        vec![alias(Some("absent"), "old")],
    ] {
        assert!(apply(&mut fixture(), &values, &mut crate::doc::source::budget()).is_err());
    }
    let mut hidden = fixture();
    if let HtmlNode::Element { tag, .. } = &mut hidden.markup.fragment.nodes[1] {
        *tag = HtmlTag::Span;
    }
    assert!(
        apply(
            &mut hidden,
            &[alias(Some("s"), "old")],
            &mut crate::doc::source::budget()
        )
        .is_err()
    );
}
#[test]
fn missing_and_ambiguous_origins_fail_before_emitting_alias() {
    for duplicate in [false, true] {
        let mut fragment = fixture();
        if duplicate {
            fragment.origins.push(ElementOrigin {
                element: 0,
                node: 20,
            });
        } else {
            fragment.origins.remove(0);
        }
        let before = fragment.markup.clone();
        assert!(
            apply(
                &mut fragment,
                &[alias(None, "old")],
                &mut crate::doc::source::budget()
            )
            .is_err()
        );
        assert_eq!(fragment.markup, before);
    }
}

#[test]
fn decorated_output_still_requires_final_depth_and_output_budget() -> Result<(), String> {
    let mut original = fixture();
    original.markup.fragment.nodes.truncate(1);
    original.origins.truncate(1);
    if let HtmlNode::Element { children, .. } = &mut original.markup.fragment.nodes[0] {
        children.clear();
    }
    for depth_failure in [true, false] {
        let mut fragment = original.clone();
        let mut limits = crate::doc::source::budget().limits();
        if depth_failure {
            limits.depth = 1;
        } else {
            limits.output_bytes = 1;
        }
        let mut budget = Budget::new(limits);
        apply(&mut fragment, &[alias(None, "old")], &mut budget)?;
        let checked = nepl3_markup::html::validate(
            &fragment.markup.fragment,
            fragment.markup.slot,
            &fragment.markup.policy,
            &mut budget,
        );
        if depth_failure {
            assert!(checked.is_err());
        } else {
            let checked = checked.map_err(err)?;
            assert!(nepl3_markup::html::serialize(&checked, &mut budget).is_err());
        }
        assert!(budget.poll().is_err());
    }
    Ok(())
}

#[test]
fn augmentation_limits_and_cancellation_remain_sticky() {
    for kind in [Resource::Work, Resource::AllocationUnits, Resource::Nodes] {
        let mut limits = crate::doc::source::budget().limits();
        match kind {
            Resource::Work => limits.work = 1,
            Resource::AllocationUnits => limits.allocation_units = 1,
            Resource::Nodes => limits.nodes = 0,
            _ => {}
        }
        let mut b = Budget::new(limits);
        assert!(apply(&mut fixture(), &[alias(None, "old")], &mut b).is_err());
        assert!(b.poll().is_err());
    }
    let mut b = crate::doc::source::budget();
    b.cancel();
    assert!(apply(&mut fixture(), &[alias(None, "old")], &mut b).is_err());
    assert!(b.poll().is_err());
}

mod binding_tests {
    use super::*;
    fn check_binding(
        inputs: &[(super::super::super::Entry, String)],
        aliases: &[PageAliases],
    ) -> Result<(), String> {
        super::super::check_binding(inputs, aliases, &mut crate::doc::source::budget())
    }
    fn entry(id: &str) -> (super::super::super::Entry, String) {
        (
            super::super::super::Entry {
                id: id.into(),
                source: format!("{id}.nepld"),
                route: format!("{id}.html"),
                input: None,
            },
            String::new(),
        )
    }
    #[test]
    fn alias_binding_is_exact_unique_and_independent_of_page_order() -> Result<(), String> {
        let inputs = [entry("b"), entry("a")];
        let aliases = [
            PageAliases::parse("a".into(), b"[]".to_vec())?,
            PageAliases::parse("b".into(), b"[]".to_vec())?,
        ];
        check_binding(&inputs, &aliases)?;
        assert!(
            check_binding(
                &inputs,
                &[PageAliases::parse("missing".into(), b"[]".to_vec())?]
            )
            .is_err()
        );
        assert!(
            check_binding(
                &inputs,
                &[
                    PageAliases::parse("a".into(), b"[]".to_vec())?,
                    PageAliases::parse("a".into(), b"[]".to_vec())?
                ]
            )
            .is_err()
        );
        assert!(check_binding(&[entry("a"), entry("a")], &aliases[..1]).is_err());
        Ok(())
    }
    #[test]
    fn aggregate_host_input_limits_are_enforced() {
        let too_many: Vec<_> = (0..129)
            .map(|i| PageAliases {
                page: format!("p{i}"),
                raw: b"[]".to_vec(),
                values: vec![],
            })
            .collect();
        let pages: Vec<_> = (0..129).map(|i| entry(&format!("p{i}"))).collect();
        assert!(check_binding(&pages, &too_many).is_err());

        let inputs: Vec<_> = (0..11).map(|i| entry(&format!("p{i}"))).collect();
        let aliases: Vec<_> = (0..11)
            .map(|i| PageAliases {
                page: format!("p{i}"),
                raw: vec![b' '; 1_000_000],
                values: vec![],
            })
            .collect();
        assert!(check_binding(&inputs, &aliases[..10]).is_ok());
        assert!(check_binding(&inputs, &aliases).is_err());
    }

    #[test]
    fn malformed_and_oversized_host_alias_inputs_are_rejected() {
        for raw in [
            b"{bad".as_slice(),
            br#"[{"name":"a","name":"b"}]"#,
            br#"[{"name":"a","unknown":true}]"#,
        ] {
            assert!(PageAliases::parse("a".into(), raw.to_vec()).is_err());
        }
        assert!(PageAliases::parse("a".into(), vec![b' '; 1_048_577]).is_err());
        let entries = (0..4097)
            .map(|_| r#"{"name":"a"}"#)
            .collect::<Vec<_>>()
            .join(",");
        assert!(PageAliases::parse("a".into(), format!("[{entries}]").into_bytes()).is_err());
    }
}
