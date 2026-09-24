use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot},
};
use nepl3_markup::html::*;
use nepl3_sentence_core::{
    literal::{self, SentenceOutcome},
    model::*,
    syntax::{NodeLocation, SentenceSyntax},
};
use nepl3_suite::adapters::sentence::html::{self, Error};

fn b() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        depth: 100_000,
        nodes: 1_000_000,
        output_bytes: 1_000_000,
        source_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn syntax(nodes: Vec<Kind>, root: Root) -> SentenceSyntax {
    SentenceSyntax {
        locations: vec![
            NodeLocation {
                origin: OriginId(0),
                head: None,
                cover: None
            };
            nodes.len()
        ],
        value: SentenceValue {
            root,
            nodes,
            embeds: vec![],
        },
        sources: vec![],
        views: vec![],
        source_maps: vec![],
        origins: vec![Origin::Synthetic {
            reason: "HTML test".into(),
            anchor: None,
        }],
    }
}
fn element(f: &HtmlFragment, id: u64, tag: HtmlTag) -> Result<(&[HtmlAttribute], &[u64]), String> {
    match f.nodes.get(id as usize) {
        Some(HtmlNode::Element {
            tag: actual,
            attributes,
            children,
        }) if *actual == tag => Ok((attributes, children)),
        other => Err(format!("expected {tag:?} at {id}, got {other:?}")),
    }
}
fn child(f: &HtmlFragment, id: u64, tag: HtmlTag, index: usize) -> Result<u64, String> {
    element(f, id, tag)?
        .1
        .get(index)
        .copied()
        .ok_or_else(|| format!("child {index} at {id}"))
}
fn text(f: &HtmlFragment, id: u64) -> Result<&str, String> {
    match f.nodes.get(id as usize) {
        Some(HtmlNode::Text { text }) => Ok(text),
        other => Err(format!("text: {other:?}")),
    }
}

#[test]
fn typed_html_preserves_annotation_structure_order_and_origins() -> Result<(), String> {
    let r = registry()?;
    let input = syntax(
        vec![
            Kind::Text {
                text: "漢<&".into(),
            },
            Kind::Text {
                text: "かん".into(),
            },
            Kind::Ruby {
                base: InlineRef(0),
                reading: InlineRef(1),
            },
            Kind::Code { text: "n".into() },
            Kind::InlineAnno {
                base: InlineRef(2),
                notes: vec![InlineRef(1), InlineRef(3)],
            },
            Kind::Emphasis {
                inline: InlineRef(4),
            },
            Kind::Strong {
                inline: InlineRef(5),
            },
            Kind::ExternalLink {
                uri: "https://example.org/".into(),
                label: InlineRef(6),
            },
            Kind::Break,
            Kind::Concat {
                inlines: vec![InlineRef(7), InlineRef(8), InlineRef(0)],
            },
            Kind::Sentence {
                inlines: vec![InlineRef(9)],
            },
        ],
        Root::Sentence(SentenceRef(10)),
    );
    let before = input.clone();
    let result =
        html::render(&input, &r, &mut b(), &mut SourceAdmission::default()).map_err(err)?;
    assert!(core::ptr::eq(result.input(), &input));
    let m = result.markup();
    assert_eq!(m.slot, HtmlSlot::Phrasing);
    let f = &m.fragment;
    let sentence = child(f, f.root, HtmlTag::Span, 0)?;
    let concat = child(f, sentence, HtmlTag::Span, 0)?;
    let link = child(f, concat, HtmlTag::Span, 0)?;
    assert_eq!(
        element(f, link, HtmlTag::A)?.0,
        &[HtmlAttribute::Href {
            value: HtmlHref::External {
                uri: "https://example.org/".into()
            }
        }]
    );
    let strong = child(f, link, HtmlTag::A, 0)?;
    let emphasis = child(f, strong, HtmlTag::Strong, 0)?;
    let anno = child(f, emphasis, HtmlTag::Em, 0)?;
    let base = child(f, anno, HtmlTag::Span, 0)?;
    let ruby = child(f, base, HtmlTag::Span, 0)?;
    let ruby_base = child(f, ruby, HtmlTag::Span, 0)?;
    let reading = child(f, ruby, HtmlTag::Span, 1)?;
    assert_eq!(text(f, child(f, ruby_base, HtmlTag::Span, 0)?)?, "漢<&");
    assert_eq!(text(f, child(f, reading, HtmlTag::Span, 0)?)?, "かん");
    let notes = child(f, anno, HtmlTag::Span, 1)?;
    assert_eq!(element(f, notes, HtmlTag::Span)?.1.len(), 2);
    let first = child(f, notes, HtmlTag::Span, 0)?;
    let second = child(f, notes, HtmlTag::Span, 1)?;
    assert_eq!(text(f, child(f, first, HtmlTag::Span, 0)?)?, "かん");
    let code = child(f, second, HtmlTag::Span, 0)?;
    assert_eq!(text(f, child(f, code, HtmlTag::Code, 0)?)?, "n");
    element(f, child(f, concat, HtmlTag::Span, 1)?, HtmlTag::Br)?;
    let shared = child(f, concat, HtmlTag::Span, 2)?;
    assert_eq!(text(f, shared)?, "漢<&");
    assert_eq!(result.origins().len(), f.nodes.len());
    for (index, origin) in result.origins().iter().enumerate() {
        assert_eq!(origin.element, index as u64);
        assert!(origin.node < input.value.nodes.len() as u64);
    }
    for id in [shared, child(f, ruby_base, HtmlTag::Span, 0)?] {
        assert_eq!(result.origins()[id as usize].node, 0);
    }
    let checked = validate(f, m.slot, &m.policy, &mut b()).map_err(err)?;
    let serialized = serialize(&checked, &mut b()).map_err(err)?;
    assert!(serialized.contains("漢&lt;&amp;"));
    assert_eq!(input, before);
    Ok(())
}

#[test]
fn literal_origin_and_invalid_output_boundaries() -> Result<(), String> {
    let r = registry()?;
    let source = SourceSnapshot::new(
        SourceId("html-sentence".into()),
        1,
        "memory:html".into(),
        "\"世界\"".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let parsed = literal::read(
        &source,
        0,
        source.text().len() as u64,
        true,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let SentenceOutcome::Matched(parsed) = parsed.outcome else {
        return Err("literal".into());
    };
    let output = html::render(
        &parsed.syntax,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    for cause in output.origins() {
        let location = &output.input().locations[cause.node as usize];
        assert!(location.cover.is_some());
    }
    assert_eq!(output.input().sources, vec![source]);
    let mut invalid = syntax(
        vec![
            Kind::Text {
                text: "label".into(),
            },
            Kind::ExternalLink {
                uri: "javascript:alert(1)".into(),
                label: InlineRef(0),
            },
        ],
        Root::Inline(InlineRef(1)),
    );
    assert!(matches!(
        html::render(&invalid, &r, &mut b(), &mut SourceAdmission::default()),
        Err(Error::Markup(_))
    ));
    invalid.value.nodes = vec![Kind::Text { text: "\0".into() }];
    invalid.value.root = Root::Inline(InlineRef(0));
    invalid.locations.truncate(1);
    assert!(matches!(
        html::render(&invalid, &r, &mut b(), &mut SourceAdmission::default()),
        Err(Error::Markup(_))
    ));
    invalid.locations.clear();
    assert!(matches!(
        html::render(&invalid, &r, &mut b(), &mut SourceAdmission::default()),
        Err(Error::Input(_))
    ));
    Ok(())
}

#[test]
fn resource_limits_preserve_input_and_publish_no_partial_fragment() -> Result<(), String> {
    let r = registry()?;
    let mut nodes = vec![Kind::Text { text: "x".into() }];
    for i in 0..64 {
        nodes.push(Kind::Strong {
            inline: InlineRef(i),
        });
    }
    let input = syntax(nodes, Root::Inline(InlineRef(64)));
    let original = input.clone();
    let mut baseline = b();
    html::render(&input, &r, &mut baseline, &mut SourceAdmission::default()).map_err(err)?;
    let usage = baseline.usage();
    for (resource, amount, reason) in [
        (0, usage.work, StopReason::WorkLimit),
        (1, usage.allocation_units, StopReason::AllocationLimit),
        (2, usage.nodes, StopReason::NodeLimit),
        (3, usage.depth, StopReason::DepthLimit),
    ] {
        for ceiling in [0, amount - 1, amount] {
            let mut limits = b().limits();
            match resource {
                0 => limits.work = ceiling,
                1 => limits.allocation_units = ceiling,
                2 => limits.nodes = ceiling,
                _ => limits.depth = ceiling,
            }
            let result = html::render(
                &input,
                &r,
                &mut Budget::new(limits),
                &mut SourceAdmission::default(),
            );
            if ceiling == amount {
                result.map_err(err)?;
            } else {
                assert!(matches!(result,Err(Error::Stopped(actual)) if actual==reason));
            }
        }
    }
    assert_eq!(input, original);
    let mut cancelled = b();
    cancelled.cancel();
    assert!(matches!(
        html::render(&input, &r, &mut cancelled, &mut SourceAdmission::default()),
        Err(Error::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}

#[test]
fn paragraph_composition_preserves_owners_and_resource_boundaries() -> Result<(), String> {
    let registry = registry()?;
    let first = syntax(
        vec![Kind::Text {
            text: "first".into(),
        }],
        Root::Inline(InlineRef(0)),
    );
    let second = syntax(
        vec![Kind::Text {
            text: "後続".into(),
        }],
        Root::Inline(InlineRef(0)),
    );
    let run = |budget: &mut Budget| {
        let mut parts = Vec::new();
        for input in [&first, &second] {
            parts.push(
                html::render_part_with_foreign(
                    input,
                    &registry,
                    &mut |_, _, _| Err::<HtmlRequest, Error>(Error::InternalShape),
                    &mut b(),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?,
            );
        }
        Ok::<_, String>(html::paragraph::compose(parts, budget))
    };
    let mut measured = b();
    let paragraph = run(&mut measured)?.map_err(err)?;
    assert!(core::ptr::eq(paragraph.placements()[0].input(), &first));
    assert!(core::ptr::eq(paragraph.placements()[1].input(), &second));
    let markup = paragraph.markup();
    let (_, children) = element(&markup.fragment, 0, HtmlTag::P)?;
    assert_eq!(children.len(), 2);
    let checked = validate(&markup.fragment, markup.slot, &markup.policy, &mut b()).map_err(err)?;
    let printed = serialize_xhtml(&checked, &mut b()).map_err(err)?;
    assert!(printed.find("first").ok_or("first")? < printed.find("後続").ok_or("second")?);
    let usage = measured.usage();
    for (used, reason) in [
        (usage.work, StopReason::WorkLimit),
        (usage.allocation_units, StopReason::AllocationLimit),
        (usage.nodes, StopReason::NodeLimit),
        (usage.depth, StopReason::DepthLimit),
    ] {
        assert!(used > 0);
        for exact in [false, true] {
            let mut limits = b().limits();
            let value = used - u64::from(!exact);
            match reason {
                StopReason::WorkLimit => limits.work = value,
                StopReason::AllocationLimit => limits.allocation_units = value,
                StopReason::NodeLimit => limits.nodes = value,
                StopReason::DepthLimit => limits.depth = value,
                _ => return Err("paragraph resource".into()),
            }
            let mut bounded = Budget::new(limits);
            let result = run(&mut bounded)?;
            if exact {
                result.map_err(err)?;
            } else {
                assert!(matches!(result, Err(Error::Stopped(actual)) if actual == reason));
            }
            assert_eq!(bounded.current_depth(), 0);
        }
    }
    let mut cancelled = b();
    cancelled.cancel();
    assert!(matches!(
        run(&mut cancelled)?,
        Err(Error::Stopped(StopReason::Cancelled))
    ));
    let empty = html::paragraph::compose([], &mut b()).map_err(err)?;
    assert!(empty.placements().is_empty());
    assert!(
        element(&empty.markup().fragment, 0, HtmlTag::P)?
            .1
            .is_empty()
    );
    Ok(())
}
#[test]
fn foreign_closure_is_checked_before_requiring_a_selected_adapter() -> Result<(), String> {
    use nepl3_core::syntax::*;
    let r = registry()?;
    let foundation = r
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest =
        nepl3_wire::environment::environment_digest(&environment, &foundation, &r, &mut b())
            .map_err(err)?;
    let mut input = syntax(
        vec![Kind::ForeignInline {
            syntax: EmbedRef(0),
        }],
        Root::Inline(InlineRef(0)),
    );
    input.value.embeds.push(
        ForeignClosure {
            syntax: ForeignSyntax {
                schema: foundation.clone(),
                category: "test-inline".into(),
                root: NodeRef(0),
                bundle: SyntaxBundle {
                    sources: vec![],
                    nodes: vec![SyntaxNode {
                        schema: foundation,
                        kind: "NodeRef".into(),
                        fields: vec![],
                        head: None,
                        cover: None,
                        origin: OriginId(0),
                        token: None,
                    }],
                    origins: input.origins.clone(),
                    root: NodeRef(0),
                    environments: vec![],
                    tokens: vec![],
                    source_maps: vec![],
                },
                environment: EnvironmentRef { id: 0, digest },
            },
            owner_environment: EnvironmentEntry {
                id: 0,
                digest,
                value: environment,
            },
            owner_origins: vec![],
            owner_sources: vec![],
            owner_source_maps: vec![],
        }
        .into(),
    );
    assert!(matches!(
        html::render(&input, &r, &mut b(), &mut SourceAdmission::default()),
        Err(Error::ForeignAdapterRequired(EmbedRef(0)))
    ));
    let mut shared = syntax(
        vec![
            Kind::ForeignInline {
                syntax: EmbedRef(0),
            },
            Kind::Sentence {
                inlines: vec![InlineRef(0), InlineRef(0)],
            },
        ],
        Root::Sentence(SentenceRef(1)),
    );
    shared.value.embeds = input.value.embeds.clone();
    let original = shared.clone();
    let mut calls = 0;
    let mut adapter = |closure: &InlineContent, embed, budget: &mut Budget| {
        assert!(core::ptr::eq(closure, &shared.value.embeds[0]));
        assert_eq!(embed, EmbedRef(0));
        calls += 1;
        budget.charge(
            nepl3_core::budget::Resource::AllocationUnits,
            core::mem::size_of::<HtmlNode>() as u64 + 1,
        )?;
        Ok::<_, Error>(HtmlRequest {
            fragment: HtmlFragment {
                root: 0,
                nodes: vec![HtmlNode::Text { text: "7".into() }],
            },
            slot: HtmlSlot::Phrasing,
            policy: HtmlPolicy { classes: vec![] },
        })
    };
    let mut full = b();
    let rendered = html::render_with_foreign(
        &shared,
        &r,
        &mut adapter,
        &mut full,
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let (_, markup, origins, placements) = rendered.into_parts();
    assert_eq!(calls, 2);
    assert_eq!(placements.len(), 2);
    assert_ne!(placements[0].first_element, placements[1].first_element);
    for placement in placements {
        assert_eq!(placement.embed, EmbedRef(0));
        assert_eq!(placement.elements, 1);
        assert_eq!(
            markup.fragment.nodes[placement.first_element as usize],
            HtmlNode::Text { text: "7".into() }
        );
        assert!(
            origins
                .iter()
                .any(|o| o.node == 0 && o.element == placement.first_element)
        );
    }
    assert_eq!(shared, original);
    // Guest occurrences overlap each other and the built-in Ruby class. The
    // paragraph must preserve the first occurrence across both Sentence parts.
    let mut parts = Vec::new();
    for names in [
        ["guest-z", "nepl-ruby", "guest-a"],
        ["guest-a", "nepl-ruby", "guest-b"],
    ] {
        let part = html::render_part_with_foreign(
            &shared,
            &r,
            &mut |_, _, _| {
                Ok::<_, Error>(HtmlRequest {
                    fragment: HtmlFragment {
                        root: 0,
                        nodes: vec![
                            HtmlNode::Element {
                                tag: HtmlTag::Span,
                                attributes: vec![HtmlAttribute::Class {
                                    values: names.iter().map(|s| (*s).into()).collect(),
                                }],
                                children: vec![1],
                            },
                            HtmlNode::Text {
                                text: "class guest".into(),
                            },
                        ],
                    },
                    slot: HtmlSlot::Phrasing,
                    policy: HtmlPolicy {
                        classes: names.iter().map(|s| (*s).into()).collect(),
                    },
                })
            },
            &mut b(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        parts.push(part);
    }
    let paragraph = html::paragraph::compose(parts, &mut b()).map_err(err)?;
    let markup = paragraph.markup();
    assert_eq!(
        markup.policy.classes,
        [
            "guest-z",
            "nepl-ruby",
            "guest-a",
            "nepl-sentence",
            "nepl-base",
            "nepl-reading",
            "nepl-anno",
            "nepl-notes",
            "nepl-note",
            "guest-b",
        ]
    );
    assert_eq!(
        markup
            .fragment
            .nodes
            .iter()
            .filter(|n| matches!(n,
        HtmlNode::Text { text } if text == "class guest"))
            .count(),
        4
    );
    for names in [
        ["guest-z", "nepl-ruby", "guest-a"],
        ["guest-a", "nepl-ruby", "guest-b"],
    ] {
        assert_eq!(
            markup
                .fragment
                .nodes
                .iter()
                .filter(|node| matches!(node,
            HtmlNode::Element { attributes, .. } if attributes.iter().any(|a| matches!(a,
                HtmlAttribute::Class { values } if values.iter().map(String::as_str).eq(names)))))
                .count(),
            2
        );
    }
    validate(&markup.fragment, markup.slot, &markup.policy, &mut b()).map_err(err)?;
    // References across two Sentence occurrences are resolved by the complete
    // document. Pending parts retain their input owner and element mapping.
    for duplicate in [false, true] {
        let mut parts = Vec::new();
        for definition in [false, true] {
            let mut adapter = |_: &InlineContent, _, _: &mut Budget| {
                let anchor = definition || duplicate;
                Ok::<_, Error>(HtmlRequest {
                    fragment: HtmlFragment {
                        root: 0,
                        nodes: vec![HtmlNode::Element {
                            tag: if anchor { HtmlTag::Span } else { HtmlTag::A },
                            attributes: vec![if anchor {
                                HtmlAttribute::Id {
                                    value: "across-sentences".into(),
                                }
                            } else {
                                HtmlAttribute::Href {
                                    value: HtmlHref::Fragment {
                                        id: "across-sentences".into(),
                                    },
                                }
                            }],
                            children: vec![],
                        }],
                    },
                    slot: HtmlSlot::Phrasing,
                    policy: HtmlPolicy { classes: vec![] },
                })
            };
            let mut measured = b();
            let pending = html::render_part_with_foreign(
                &input,
                &r,
                &mut adapter,
                &mut measured,
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            if !definition && !duplicate {
                let usage = measured.usage();
                for (used, reason) in [
                    (usage.work, StopReason::WorkLimit),
                    (usage.allocation_units, StopReason::AllocationLimit),
                    (usage.nodes, StopReason::NodeLimit),
                    (usage.depth, StopReason::DepthLimit),
                ] {
                    assert!(used > 0);
                    for exact in [false, true] {
                        let mut limits = b().limits();
                        let value = used - u64::from(!exact);
                        match reason {
                            StopReason::WorkLimit => limits.work = value,
                            StopReason::AllocationLimit => limits.allocation_units = value,
                            StopReason::NodeLimit => limits.nodes = value,
                            StopReason::DepthLimit => limits.depth = value,
                            _ => return Err("pending resource".into()),
                        }
                        let mut bounded = Budget::new(limits);
                        let result = html::render_part_with_foreign(
                            &input,
                            &r,
                            &mut adapter,
                            &mut bounded,
                            &mut SourceAdmission::default(),
                        );
                        if exact {
                            result.map_err(err)?;
                        } else {
                            assert!(
                                matches!(result, Err(html::RenderFailure::Sentence(Error::Stopped(actual))) if actual == reason)
                            );
                        }
                        assert_eq!(bounded.current_depth(), 0);
                    }
                }
            }
            assert!(core::ptr::eq(pending.input(), &input));
            if !definition && !duplicate {
                let lone = html::render_part_with_foreign(
                    &input,
                    &r,
                    &mut adapter,
                    &mut b(),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
                assert!(matches!(
                    html::paragraph::compose([lone], &mut b()),
                    Err(Error::Markup(HtmlError::MissingFragment(_)))
                ));
            }
            parts.push(pending);
        }
        let result = html::paragraph::compose(parts, &mut b());
        if duplicate {
            assert!(matches!(
                result,
                Err(Error::Markup(HtmlError::DuplicateId(_)))
            ));
        } else {
            let paragraph = result.map_err(err)?;
            assert_eq!(paragraph.placements().len(), 2);
            let mut end = 1;
            for placement in paragraph.placements() {
                assert!(core::ptr::eq(placement.input(), &input));
                assert_eq!(placement.first_element(), end);
                end += placement.elements();
                let [foreign] = placement.foreign() else {
                    return Err("one guest".into());
                };
                assert_eq!(foreign.embed, EmbedRef(0));
                assert!(
                    placement
                        .origins()
                        .iter()
                        .all(|origin| origin.element >= placement.first_element()
                            && origin.element < end)
                );
                assert!(
                    placement
                        .origins()
                        .iter()
                        .any(|origin| origin.element == foreign.first_element && origin.node == 0)
                );
            }
            assert_eq!(end as usize, paragraph.markup().fragment.nodes.len());
            let markup = paragraph.markup();
            let checked =
                validate(&markup.fragment, markup.slot, &markup.policy, &mut b()).map_err(err)?;
            let output = serialize_xhtml(&checked, &mut b()).map_err(err)?;
            assert!(
                output.contains("href=\"#across-sentences\"")
                    && output.contains("id=\"across-sentences\"")
            );
        }
    }
    // A forward reference belongs to the composed Sentence namespace, while
    // each guest must already satisfy the structural phrasing contract.
    for duplicate in [false, true] {
        let mut calls = 0;
        let result = html::render_with_foreign(
            &shared,
            &r,
            &mut |_, _, _| {
                calls += 1;
                let attribute = if calls == 1 && !duplicate {
                    HtmlAttribute::Href {
                        value: HtmlHref::Fragment { id: "later".into() },
                    }
                } else {
                    HtmlAttribute::Id {
                        value: "later".into(),
                    }
                };
                Ok::<_, Error>(HtmlRequest {
                    fragment: HtmlFragment {
                        root: 0,
                        nodes: vec![
                            HtmlNode::Element {
                                tag: if calls == 1 && !duplicate {
                                    HtmlTag::A
                                } else {
                                    HtmlTag::Span
                                },
                                attributes: vec![attribute],
                                children: vec![1],
                            },
                            HtmlNode::Text {
                                text: format!("part{calls}"),
                            },
                        ],
                    },
                    slot: HtmlSlot::Phrasing,
                    policy: HtmlPolicy { classes: vec![] },
                })
            },
            &mut b(),
            &mut SourceAdmission::default(),
        );
        assert_eq!(calls, 2);
        if duplicate {
            assert!(matches!(
                result,
                Err(html::RenderFailure::Sentence(Error::Markup(
                    HtmlError::DuplicateId(_)
                )))
            ));
        } else {
            let output = result.map_err(err)?;
            let validated = validate(
                &output.markup().fragment,
                HtmlSlot::Phrasing,
                &output.markup().policy,
                &mut b(),
            )
            .map_err(err)?;
            let text = serialize_xhtml(&validated, &mut b()).map_err(err)?;
            assert!(text.contains("href=\"#later\"") && text.contains("id=\"later\""));
        }
    }
    let missing = html::render_with_foreign(
        &shared,
        &r,
        &mut |_, _, _| {
            Ok::<_, Error>(HtmlRequest {
                fragment: HtmlFragment {
                    root: 0,
                    nodes: vec![HtmlNode::Element {
                        tag: HtmlTag::A,
                        attributes: vec![HtmlAttribute::Href {
                            value: HtmlHref::Fragment {
                                id: "missing".into(),
                            },
                        }],
                        children: vec![],
                    }],
                },
                slot: HtmlSlot::Phrasing,
                policy: HtmlPolicy { classes: vec![] },
            })
        },
        &mut b(),
        &mut SourceAdmission::default(),
    );
    assert!(matches!(
        missing,
        Err(html::RenderFailure::Sentence(Error::Markup(
            HtmlError::MissingFragment(_)
        )))
    ));
    for (resource, amount, reason) in [
        (0, full.usage().work, StopReason::WorkLimit),
        (
            1,
            full.usage().allocation_units,
            StopReason::AllocationLimit,
        ),
        (2, full.usage().nodes, StopReason::NodeLimit),
        (3, full.usage().depth, StopReason::DepthLimit),
    ] {
        for ceiling in [amount - 1, amount] {
            let mut limits = b().limits();
            match resource {
                0 => limits.work = ceiling,
                1 => limits.allocation_units = ceiling,
                2 => limits.nodes = ceiling,
                _ => limits.depth = ceiling,
            }
            let mut bounded = Budget::new(limits);
            let result = html::render_with_foreign(
                &shared,
                &r,
                &mut |_, _, b| {
                    b.charge(
                        nepl3_core::budget::Resource::AllocationUnits,
                        core::mem::size_of::<HtmlNode>() as u64 + 1,
                    )?;
                    Ok::<_, Error>(HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Text { text: "7".into() }],
                        },
                        slot: HtmlSlot::Phrasing,
                        policy: HtmlPolicy { classes: vec![] },
                    })
                },
                &mut bounded,
                &mut SourceAdmission::default(),
            );
            if ceiling == amount {
                result.map_err(err)?;
            } else {
                assert!(matches!(result,
                    Err(html::RenderFailure::Sentence(Error::Stopped(actual))
                        | html::RenderFailure::Foreign(Error::Stopped(actual))) if actual == reason));
                assert_eq!(bounded.poll(), Err(reason));
            }
            assert_eq!(bounded.current_depth(), 0);
        }
    }
    // Adapter output must satisfy the surrounding phrasing contract.
    let mut limits = b().limits();
    limits.depth = full.usage().depth + 3;
    let mut nested = Budget::new(limits);
    nested
        .with_depth_at_least(3, |b| {
            html::render_with_foreign(
                &shared,
                &r,
                &mut |_, _, _| {
                    Ok::<_, Error>(HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Text { text: "7".into() }],
                        },
                        slot: HtmlSlot::Phrasing,
                        policy: HtmlPolicy { classes: vec![] },
                    })
                },
                b,
                &mut SourceAdmission::default(),
            )
        })
        .map_err(err)?;
    assert_eq!(nested.current_depth(), 0);
    assert_eq!(nested.usage().depth, full.usage().depth + 3);
    let mut cancelled = b();
    let mut calls = 0;
    assert!(matches!(
        html::render_with_foreign(
            &shared,
            &r,
            &mut |_, _, b| {
                calls += 1;
                b.cancel();
                Ok::<_, Error>(HtmlRequest {
                    fragment: HtmlFragment {
                        root: 0,
                        nodes: vec![HtmlNode::Text { text: "7".into() }],
                    },
                    slot: HtmlSlot::Phrasing,
                    policy: HtmlPolicy { classes: vec![] },
                })
            },
            &mut cancelled,
            &mut SourceAdmission::default()
        ),
        Err(html::RenderFailure::Sentence(Error::Stopped(
            StopReason::Cancelled
        )))
    ));
    assert_eq!(calls, 1);
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    let mut invalid = |_: &InlineContent, _, _: &mut Budget| {
        Ok::<_, Error>(HtmlRequest {
            fragment: HtmlFragment {
                root: 0,
                nodes: vec![HtmlNode::Element {
                    tag: HtmlTag::Div,
                    attributes: vec![],
                    children: vec![],
                }],
            },
            slot: HtmlSlot::Block,
            policy: HtmlPolicy { classes: vec![] },
        })
    };
    assert!(matches!(
        html::render_with_foreign(
            &shared,
            &r,
            &mut invalid,
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(html::RenderFailure::Sentence(Error::Markup(_)))
    ));
    let nepl3_sentence_core::model::InlineContent::Syntax { closure } = &mut input.value.embeds[0]
    else {
        return Err("syntax content".into());
    };
    closure.syntax.bundle.origins.clear();
    assert!(matches!(
        html::render(&input, &r, &mut b(), &mut SourceAdmission::default()),
        Err(Error::Input(_))
    ));
    let mut calls = 0;
    assert!(matches!(
        html::render_with_foreign(
            &input,
            &r,
            &mut |_, _, _| {
                calls += 1;
                Err::<HtmlRequest, Error>(Error::InternalShape)
            },
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(html::RenderFailure::Sentence(Error::Input(_)))
    ));
    assert_eq!(calls, 0);
    Ok(())
}
