use super::*;
use nepl3_core::budget::StopReason;
use nepl3_doc_core::labels::namespace::{self, MemberId};
use nepl3_doc_html::namespace::{self as html, ForeignPartError};
use nepl3_doc_html::{ForeignRenderError, LocalPreparationError, RenderError};
use nepl3_markup::html::{
    HtmlError, HtmlFragment, HtmlNode, HtmlPolicy, HtmlRequest, HtmlSlot, HtmlTag,
};

#[test]
fn namespace_diagnostic_keeps_separate_source_owners() -> Result<(), String> {
    let compiled = compiled()?;
    for (native, shared_source) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut documents = Vec::new();
        let combined =
            "sentence cons anchor target text \"最初\" cons anchor target text \"後続\" nil";
        let sources = if shared_source {
            [combined, combined]
        } else {
            ["anchor target text \"最初\"", "anchor target text \"後続\""]
        };
        for (name, source) in ["first-definition", "second-definition"]
            .into_iter()
            .zip(sources)
        {
            if shared_source && !documents.is_empty() {
                break;
            }
            with_named_input(
                native,
                &compiled,
                source,
                if shared_source {
                    "shared-definitions"
                } else {
                    name
                },
                if shared_source { "Sentence" } else { "Inline" },
                |tree, profile, b, a| {
                    let input = tree
                        .tree()
                        .bundle
                        .validate_with_sources(profile.registry(), b, a)
                        .map_err(err)?;
                    let store = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &store, &mut admission)
                            .map_err(err)?;
                    let document = lower::document(
                        &input,
                        &compiled.doc.package.schema,
                        if shared_source {
                            Category::Sentence
                        } else {
                            Category::Inline
                        },
                        profile.registry(),
                        b,
                        &mut codec,
                    )
                    .map_err(err)?;
                    if shared_source {
                        let nodes: Vec<_> = document
                            .value
                            .nodes
                            .iter()
                            .enumerate()
                            .filter_map(|(index, node)| {
                                matches!(node.kind, DocKind::Anchor { .. }).then_some(index)
                            })
                            .collect();
                        assert_eq!(nodes.len(), 2);
                        for node in nodes {
                            documents.push(
                                document
                                    .fragment(
                                        nepl3_doc_core::model::DocRoot::Inline(
                                            nepl3_doc_core::model::InlineRef(node as u64),
                                        ),
                                        profile.registry(),
                                        b,
                                        codec.source_admission(),
                                    )
                                    .map_err(err)?,
                            );
                        }
                    } else {
                        documents.push(document);
                    }
                    Ok(())
                },
            )?;
        }
        with_input_route(
            native,
            &compiled,
            "text \"context\"",
            "Inline",
            |_, profile, b, a| {
                let registry = profile.registry();
                let first = namespace::inspect(&documents[0], registry, b, a).map_err(err)?;
                let second = namespace::inspect(&documents[1], registry, b, a).map_err(err)?;
                let members = [&first, &second];
                let failure = match namespace::resolve(&members, b) {
                    Err(error @ namespace::Error::Duplicate { .. }) => error,
                    _ => return Err("duplicate label required".into()),
                };
                let store = SourceStore::default();
                let run = |members: &[&namespace::Member<'_>], b: &mut Budget| {
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                    Ok::<_, String>(failure.diagnostic(members, registry, &mut codec, b))
                };
                let mut measured = budget();
                let diagnostic = run(&members, &mut measured)?.map_err(err)?;
                assert_eq!(diagnostic.code, "DuplicateLabel");
                let primary = diagnostic.primary.as_ref().ok_or("primary")?;
                let [related] = diagnostic.related.as_slice() else {
                    return Err("previous definition".into());
                };
                let previous = related.span.as_ref().ok_or("previous span")?;
                assert_eq!(related.code, "PreviousDefinition");
                let starts = if shared_source {
                    [
                        combined.find("target").ok_or("first target")? as u64,
                        combined.rfind("target").ok_or("second target")? as u64,
                    ]
                } else {
                    [7, 7]
                };
                for (span, source, start) in [
                    (primary, sources[1], starts[1]),
                    (previous, sources[0], starts[0]),
                ] {
                    assert_eq!((span.start(), span.end()), (start, start + 6));
                    assert_eq!(span.snapshot_ref().digest, Digest::of(source.as_bytes()));
                }
                assert_eq!(
                    primary.snapshot_ref().source == previous.snapshot_ref().source,
                    shared_source
                );
                // Both sources still exist in this union. Reversing their member
                // ownership must fail, rather than accepting the union alone.
                assert!(matches!(
                    run(&[&second, &first], b)?,
                    Err(nepl3_doc_core::labels::LabelDiagnosticError::NotSemantic)
                ));
                assert!(matches!(
                    run(&[], b)?,
                    Err(nepl3_doc_core::labels::LabelDiagnosticError::NotSemantic)
                ));
                let used = measured.usage();
                for (reason, amount) in [
                    (StopReason::WorkLimit, used.work),
                    (StopReason::AllocationLimit, used.allocation_units),
                    (StopReason::DiagnosticLimit, used.diagnostics),
                ] {
                    for limit in [amount - 1, amount] {
                        let mut limits = measured.limits();
                        match reason {
                            StopReason::WorkLimit => limits.work = limit,
                            StopReason::AllocationLimit => limits.allocation_units = limit,
                            StopReason::DiagnosticLimit => limits.diagnostics = limit,
                            _ => unreachable!("fixed limits"),
                        }
                        let mut limited = Budget::new(limits);
                        let result = run(&members, &mut limited)?;
                        if limit == amount {
                            assert_eq!(result.map_err(err)?, diagnostic);
                        } else {
                            assert!(
                                matches!(result, Err(nepl3_doc_core::labels::LabelDiagnosticError::Stopped(actual)) if actual == reason)
                            );
                            assert_eq!(limited.poll(), Err(reason));
                        }
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn foreign_namespace_parts_preserve_refs_and_guest_boundaries() -> Result<(), String> {
    let compiled = compiled()?;
    // The reference and its definition are separate display occurrences. Math
    // remains an explicit guest of the reference label, with its own source.
    for native in [false, true] {
        let mut documents = Vec::new();
        for (name, category, source) in [
            ("reference-math", Category::Inline, "ref target math Math 7"),
            (
                "definition",
                Category::Inline,
                "anchor target text \"定義\"",
            ),
            (
                "external",
                Category::Inline,
                "link external \"https://example.test/\" text \"link\"",
            ),
            (
                "display",
                Category::Article,
                "article en \"Title\" body cons display Math 7 nil",
            ),
        ] {
            with_named_input(
                native,
                &compiled,
                source,
                name,
                if category == Category::Article {
                    "Article"
                } else {
                    "Inline"
                },
                |tree, profile, b, a| {
                    let input = tree
                        .tree()
                        .bundle
                        .validate_with_sources(profile.registry(), b, a)
                        .map_err(err)?;
                    let store = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &store, &mut admission)
                            .map_err(err)?;
                    documents.push(
                        lower::document(
                            &input,
                            &compiled.doc.package.schema,
                            category,
                            profile.registry(),
                            b,
                            &mut codec,
                        )
                        .map_err(err)?,
                    );
                    Ok(())
                },
            )?;
        }
        with_input_route(
            native,
            &compiled,
            "text \"context\"",
            "Inline",
            |_, profile, b, _| {
                let registry = profile.registry();
                let mut admission = SourceAdmission::default();
                let inputs = documents
                    .iter()
                    .map(|document| {
                        namespace::inspect(document, registry, b, &mut admission).map_err(err)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let members = [&inputs[0], &inputs[1]];
                let checked = namespace::resolve(&members, b).map_err(err)?;
                let store = SourceStore::default();
                let mut codec =
                    FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                let options = nepl3_doc_html::RenderOptions {
                    parallel: nepl3_doc_html::ParallelMode::Rows,
                };
                assert!(matches!(
                    html::prepare(&checked, &options, registry, &mut codec, b),
                    Err(LocalPreparationError::NeedsResolution(_))
                ));
                let prepared =
                    html::prepare_with_foreign(&checked, &options, registry, &mut codec, b)
                        .map_err(err)?;
                let mut measured = budget();
                let mut calls = 0;
                let rendered = html::render_part_with_foreign(
                    &prepared,
                    MemberId(0),
                    &mut |guest, embed, b| {
                        calls += 1;
                        assert_eq!(guest, &documents[0].value.embeds[embed.0 as usize]);
                        assert_eq!(guest.kind, nepl3_doc_core::model::EmbedKind::InlineMath);
                        let mut host = crate::doc::math::MathDisplayHost {
                            registry,
                            math_surface: &compiled.others[0].schema,
                            sentence_surface: Some(&compiled.others[3].schema),
                            doc_surface: Some(&compiled.doc.package.schema),
                            codec: &mut codec,
                        };
                        Ok::<_, String>(
                            host.render(
                                guest.syntax().ok_or("Math syntax")?,
                                nepl3_markup::mathml::Display::Inline,
                                b,
                            )
                            .map_err(err)?
                            .into_html(b)
                            .map_err(err)?
                            .markup,
                        )
                    },
                    &mut measured,
                )
                .map_err(err)?;
                assert_eq!(calls, 1);
                let [placement] = rendered.foreign.as_slice() else {
                    return Err("one guest placement".into());
                };
                let (member, digest, pending, origins) = rendered.part.into_parts();
                assert_eq!(member, MemberId(0));
                assert!(!origins.is_empty());
                assert_eq!(
                    digest,
                    nepl3_doc_core::prepare::inspect_namespace(&checked, registry, &mut codec, b)
                        .map_err(err)?[0]
                        .document_digest
                );
                let start = usize::try_from(placement.first_element).map_err(err)?;
                let end =
                    usize::try_from(placement.first_element + placement.elements).map_err(err)?;
                assert!(
                    pending.fragment.nodes[start..end]
                        .iter()
                        .any(|node| matches!(node, HtmlNode::Text { text } if text == "7"))
                );
                assert!(matches!(
                    nepl3_markup::html::validate(
                        &pending.fragment,
                        pending.slot,
                        &pending.policy,
                        b
                    ),
                    Err(HtmlError::MissingFragment(_))
                ));
                let mut calls = 0;
                let mut forbidden = |_: &nepl3_doc_core::model::DocEmbed, _, _: &mut Budget| {
                    calls += 1;
                    Err::<HtmlRequest, _>("unexpected callback")
                };
                assert!(matches!(
                    html::render_part_with_foreign(&prepared, MemberId(2), &mut forbidden, b),
                    Err(ForeignPartError::Member(MemberId(2)))
                ));
                let definition =
                    html::render_part_with_foreign(&prepared, MemberId(1), &mut forbidden, b)
                        .map_err(err)?;
                assert!(definition.foreign.is_empty());
                assert_eq!(calls, 0);
                // A composing host places both parts before requesting the
                // complete-output proof. Omitting the definition failed above.
                let (_, _, definition, _) = definition.part.into_parts();
                let mut nodes = vec![HtmlNode::Element {
                    tag: HtmlTag::Span,
                    attributes: vec![],
                    children: vec![],
                }];
                let mut roots = Vec::new();
                let mut classes = Vec::new();
                for part in [pending, definition] {
                    let offset = nodes.len() as u64;
                    roots.push(offset + part.fragment.root);
                    for mut node in part.fragment.nodes {
                        match &mut node {
                            HtmlNode::Element { children, .. }
                            | HtmlNode::MathElement { children, .. } => {
                                for child in children {
                                    *child += offset;
                                }
                            }
                            HtmlNode::Text { .. } => {}
                        }
                        nodes.push(node);
                    }
                    for class in part.policy.classes {
                        if !classes.contains(&class) {
                            classes.push(class);
                        }
                    }
                }
                let HtmlNode::Element { children, .. } = &mut nodes[0] else {
                    return Err("composition wrapper".into());
                };
                *children = roots;
                nepl3_markup::html::validate(
                    &HtmlFragment { root: 0, nodes },
                    HtmlSlot::Phrasing,
                    &HtmlPolicy { classes },
                    b,
                )
                .map_err(err)?;

                let mut measured = budget();
                let mut text_guest = |_: &nepl3_doc_core::model::DocEmbed, _, _: &mut Budget| {
                    Ok::<_, ()>(HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Text {
                                text: "guest".into(),
                            }],
                        },
                        slot: HtmlSlot::Phrasing,
                        policy: HtmlPolicy { classes: vec![] },
                    })
                };
                html::render_part_with_foreign(
                    &prepared,
                    MemberId(0),
                    &mut text_guest,
                    &mut measured,
                )
                .map_err(err)?;
                let used = measured.usage();
                for reason in [
                    StopReason::WorkLimit,
                    StopReason::AllocationLimit,
                    StopReason::DepthLimit,
                ] {
                    let mut limits = measured.limits();
                    match reason {
                        StopReason::WorkLimit => limits.work = used.work - 1,
                        StopReason::AllocationLimit => {
                            limits.allocation_units = used.allocation_units - 1
                        }
                        StopReason::DepthLimit => limits.depth = used.depth - 1,
                        _ => unreachable!("fixed resource cases"),
                    }
                    let mut limited = Budget::new(limits);
                    assert!(
                        matches!(html::render_part_with_foreign(&prepared, MemberId(0), &mut text_guest, &mut limited), Err(ForeignPartError::Render(ForeignRenderError::Render(RenderError::Stopped(actual)))) if actual == reason)
                    );
                    assert_eq!(limited.poll(), Err(reason));
                }

                // Successful callback return cannot bypass Phrasing validation.
                assert!(matches!(
                    html::render_part_with_foreign(
                        &prepared,
                        MemberId(0),
                        &mut |_, _, _| Ok::<_, ()>(HtmlRequest {
                            fragment: HtmlFragment {
                                root: 0,
                                nodes: vec![HtmlNode::Element {
                                    tag: HtmlTag::Div,
                                    attributes: vec![],
                                    children: vec![]
                                }]
                            },
                            slot: HtmlSlot::Phrasing,
                            policy: HtmlPolicy { classes: vec![] },
                        }),
                        b
                    ),
                    Err(ForeignPartError::Render(ForeignRenderError::Render(
                        RenderError::Markup(HtmlError::Content(_))
                    )))
                ));
                assert!(matches!(
                    html::render_part_with_foreign(
                        &prepared,
                        MemberId(0),
                        &mut |_, _, _| Err::<HtmlRequest, _>("guest rejected"),
                        b
                    ),
                    Err(ForeignPartError::Render(ForeignRenderError::Foreign(
                        "guest rejected"
                    )))
                ));
                for reason in [
                    StopReason::WorkLimit,
                    StopReason::AllocationLimit,
                    StopReason::Cancelled,
                ] {
                    let mut limited = budget();
                    let result = html::render_part_with_foreign(
                        &prepared,
                        MemberId(0),
                        &mut |_, _, b| {
                            b.stop(reason);
                            Ok::<_, ()>(HtmlRequest {
                                fragment: HtmlFragment {
                                    root: 0,
                                    nodes: vec![HtmlNode::Text {
                                        text: "discard".into(),
                                    }],
                                },
                                slot: HtmlSlot::Phrasing,
                                policy: HtmlPolicy { classes: vec![] },
                            })
                        },
                        &mut limited,
                    );
                    assert!(
                        matches!(result, Err(ForeignPartError::Render(ForeignRenderError::Render(RenderError::Stopped(actual)))) if actual == reason)
                    );
                    assert_eq!(limited.poll(), Err(reason));
                }
                // The foreign entry point grants no link resolution capability.
                let external = [&inputs[2]];
                let checked = namespace::resolve(&external, b).map_err(err)?;
                assert!(matches!(
                    html::prepare_with_foreign(&checked, &options, registry, &mut codec, b),
                    Err(LocalPreparationError::NeedsResolution(_))
                ));
                // DisplayMath requires a Block guest operation. The InlineMath
                // adapter cannot render it; reject before creating a proof.
                let display = [&inputs[3]];
                let checked = namespace::resolve(&display, b).map_err(err)?;
                let Err(LocalPreparationError::NeedsResolution(plan)) =
                    html::prepare_with_foreign(&checked, &options, registry, &mut codec, b)
                else {
                    return Err("unsupported guest must retain its requirement".into());
                };
                assert!(matches!(
                    plan.requirements.as_slice(),
                    [nepl3_doc_core::prepare::DocRequirement::Foreign {
                        kind: nepl3_doc_core::model::EmbedKind::DisplayMath,
                        ..
                    }]
                ));
                Ok(())
            },
        )?;
    }
    Ok(())
}
