use super::*;
use nepl3_core::budget::StopReason;
use nepl3_doc_core::labels::namespace::{self, MemberId};
use nepl3_doc_html::namespace::{self as html, ForeignPartError};
use nepl3_doc_html::{ForeignRenderError, LocalPreparationError, RenderError};
use nepl3_markup::html::{
    HtmlError, HtmlFragment, HtmlNode, HtmlPolicy, HtmlRequest, HtmlSlot, HtmlTag,
};

pub(super) fn sentence_label<C: FoundationValueCodec>(
    guest: &nepl3_doc_core::model::DocEmbed,
    compiled: &Compiled,
    registry: &nepl3_core::schema::SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<HtmlRequest, String>
where
    C::Error: core::fmt::Debug,
{
    use nepl3_doc_core::model::EmbedKind;
    assert!(matches!(
        guest.kind,
        EmbedKind::Sentence | EmbedKind::SentenceInline
    ));
    let sentence = nepl3_suite::adapters::document::sentence::lower(
        guest,
        &compiled.others[3].schema,
        &[nepl3_sentence_core::lower::ForeignInlineForm {
            kind: "Form:InlineMath",
            guest_schema: &compiled.others[0].schema,
            guest_category: "Expr",
        }],
        registry,
        codec,
        b,
    )
    .map_err(err)?;
    let mut host = crate::doc::annotations::SentenceAnnotationRenderer {
        registry,
        surface: &compiled.others[3].schema,
        math_surface: Some(&compiled.others[0].schema),
        doc_surface: Some(&compiled.doc.package.schema),
        codec,
    };
    match guest.kind {
        EmbedKind::Sentence => host.render_syntax(sentence, b),
        EmbedKind::SentenceInline => host.render_inline_syntax(sentence, b),
        _ => unreachable!("checked Sentence role"),
    }
    .map(|rendered| rendered.markup)
    .map_err(err)
}

#[test]
fn namespace_diagnostic_keeps_separate_source_owners() -> Result<(), String> {
    let compiled =
        compiled_with_sentence_forms(&[nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "doc",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "namespace diagnostic test",
        }])?;
    for (native, shared_source) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut documents = Vec::new();
        let combined = "sentence sentence cons doc anchor target text \"最初\" cons doc anchor target text \"後続\" nil";
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
                        let nepl3_doc_core::model::DocRoot::Sentence(root) = document.value.root
                        else {
                            return Err("Sentence root".into());
                        };
                        let DocKind::Sentence { syntax } =
                            document.value.nodes[root.0 as usize].kind
                        else {
                            return Err("Sentence slot".into());
                        };
                        let sentence = nepl3_suite::adapters::document::sentence::lower(
                            &document.value.embeds[syntax.0 as usize],
                            &compiled.others[3].schema,
                            &[nepl3_sentence_core::lower::ForeignInlineForm {
                                kind: "Form:DocumentInline",
                                guest_schema: &compiled.doc.package.schema,
                                guest_category: "Inline",
                            }],
                            profile.registry(),
                            &mut codec,
                            b,
                        )
                        .map_err(err)?;
                        let selected = nepl3_suite::adapters::sentence::document_guests::collect(
                            &sentence,
                            &compiled.doc.package.schema,
                            profile.registry(),
                            &mut codec,
                            b,
                        )
                        .map_err(err)?;
                        let (guests, occurrences) = selected.into_parts();
                        assert_eq!(guests.len(), 2);
                        assert_eq!(occurrences.len(), 2);
                        assert_eq!(occurrences[0].document.index(), 0);
                        assert_eq!(occurrences[1].document.index(), 1);
                        documents.extend(guests);
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
            "anchor context text \"context\"",
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
            ("reference-math", Category::Inline, "ref target math 7"),
            (
                "definition",
                Category::Inline,
                "anchor target text \"定義\"",
            ),
            (
                "relative",
                Category::Inline,
                "link relative \"other.nepld\" none text \"link\"",
            ),
            (
                "display",
                Category::Article,
                "article en sentence \"Title\" body cons display Math 7 nil",
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
            "anchor context text \"context\"",
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
                        sentence_label(guest, &compiled, registry, &mut codec, b)
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
                assert_eq!(calls, 0);
                let definition = html::render_part_with_foreign(
                    &prepared,
                    MemberId(1),
                    &mut |guest, embed, b| {
                        calls += 1;
                        assert_eq!(guest, &documents[1].value.embeds[embed.0 as usize]);
                        sentence_label(guest, &compiled, registry, &mut codec, b)
                    },
                    b,
                )
                .map_err(err)?;
                assert_eq!(definition.foreign.len(), 1);
                assert_eq!(calls, 1);
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
                // Article titles remain Sentence-owned. DisplayMath uses its
                // independently selected Block operation in the same member.
                let before = inputs[3].document().clone();
                let sentences = nepl3_suite::adapters::document::sentences::collect(
                    inputs[3].document(),
                    &compiled.others[3].schema,
                    &[],
                    registry,
                    &mut codec,
                    b,
                )
                .map_err(err)?;
                assert_eq!(sentences.occurrences().len(), 1);
                assert_eq!(
                    sentences.occurrences()[0].kind,
                    nepl3_doc_core::model::EmbedKind::Sentence
                );
                assert_eq!(inputs[3].document(), &before);
                let display = [&inputs[3]];
                let checked = namespace::resolve(&display, b).map_err(err)?;
                let display =
                    html::prepare_with_foreign(&checked, &options, registry, &mut codec, b)
                        .map_err(err)?;
                let mut roles = Vec::new();
                let displayed = html::render_part_with_foreign(
                    &display,
                    MemberId(0),
                    &mut |guest, _, b| {
                        use nepl3_doc_core::model::EmbedKind;
                        roles.push(guest.kind);
                        match guest.kind {
                            EmbedKind::Sentence => {
                                sentence_label(guest, &compiled, registry, &mut codec, b)
                            }
                            EmbedKind::DisplayMath => crate::doc::math::MathDisplayHost {
                                registry,
                                math_surface: &compiled.others[0].schema,
                                sentence_surface: Some(&compiled.others[3].schema),
                                doc_surface: Some(&compiled.doc.package.schema),
                                codec: &mut codec,
                            }
                            .render(
                                guest.syntax().ok_or("Math syntax")?,
                                nepl3_markup::mathml::Display::Block,
                                b,
                            )
                            .map_err(err)?
                            .into_html(b)
                            .map(|result| result.markup)
                            .map_err(err),
                            _ => Err("unexpected Article guest".into()),
                        }
                    },
                    b,
                )
                .map_err(err)?;
                assert_eq!(
                    roles,
                    [
                        nepl3_doc_core::model::EmbedKind::Sentence,
                        nepl3_doc_core::model::EmbedKind::DisplayMath
                    ]
                );
                assert_eq!(displayed.foreign.len(), 2);
                let (_, _, markup, _) = displayed.part.into_parts();
                assert_eq!(markup.slot, HtmlSlot::Block);
                assert!(
                    markup
                        .fragment
                        .nodes
                        .iter()
                        .any(|node| matches!(node, HtmlNode::Text { text } if text == "Title"))
                );
                assert!(
                    markup
                        .fragment
                        .nodes
                        .iter()
                        .any(|node| matches!(node, HtmlNode::Text { text } if text == "7"))
                );
                nepl3_markup::html::validate(&markup.fragment, markup.slot, &markup.policy, b)
                    .map_err(err)?;
                Ok(())
            },
        )?;
    }
    Ok(())
}
