//! The standard host preserves Doc -> Sentence -> Doc -> Sentence ownership.
use nepl3_core::source::SourceStore;
use nepl3_doc_core::{check::Category, lower, model::*};
use nepl3_math_core::print::GuestPrinter;
use nepl3_sentence_core::{lower::ForeignInlineForm, model::Kind};
use nepl3_suite::adapters::{document::sentence, sentence::document_guests};
use nepl3_tools::doc::source::{compiled, err, with_input_route};
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn html_keeps_sentence_label_owners_after_document_composition() -> Result<(), String> {
    use nepl3_markup::html::{HtmlAttribute, HtmlNode};
    use nepl3_tools::doc::annotations::{
        DocumentOutput, ForeignRecord, SentenceAnnotationRenderer,
    };
    let compiled = compiled()?;
    let input = r#"article en sentence "T" body cons paragraph cons sentence sentence
        cons doc anchor target ruby concat cons text "漢字" cons math 7 nil text "かんじ"
        cons doc ref target text "reference" nil nil nil"#;
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            input,
            "Article",
            |tree, profile, b, a| {
                let registry = profile.registry();
                let store = SourceStore::default();
                let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
                let document = lower::document(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let surface = registry
                    .selected("nepl3.syntax.sentence", 1)
                    .ok_or("Sentence schema")?;
                let forms = [ForeignInlineForm {
                    kind: "Form:DocumentInline",
                    guest_schema: &compiled.doc.package.schema,
                    guest_category: "Inline",
                }];
                let content = sentence::lower(
                    &document.value.embeds[1],
                    surface,
                    &forms,
                    registry,
                    &mut codec,
                    b,
                )
                .map_err(err)?;
                let run = |limits| -> Result<_, String> {
                    // Copy the immutable fixture before measurement. Each render then
                    // owns its input and starts with a fresh source admission ledger.
                    let input = content.clone();
                    let mut admission = nepl3_core::source::SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                    let mut host = SentenceAnnotationRenderer {
                        registry,
                        surface,
                        math_surface: registry.selected("standard.math", 1),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    let mut b = nepl3_core::budget::Budget::new(limits);
                    let result = host.render_syntax(input, &mut b);
                    Ok((result, b.usage()))
                };
                let limits = nepl3_tools::doc::source::budget().limits();
                let (rendered, usage) = run(limits)?;
                let rendered = rendered.map_err(err)?;
                {
                    let mut host = SentenceAnnotationRenderer {
                        registry,
                        surface,
                        math_surface: registry.selected("standard.math", 1),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    assert!(matches!(
                        host.render_inline_syntax(
                            content.clone(),
                            &mut nepl3_tools::doc::source::budget()
                        ),
                        Err(nepl3_tools::doc::annotations::Error::Selection)
                    ));
                }
                for reason in [
                    nepl3_core::budget::StopReason::WorkLimit,
                    nepl3_core::budget::StopReason::AllocationLimit,
                    nepl3_core::budget::StopReason::DepthLimit,
                    nepl3_core::budget::StopReason::OutputLimit,
                ] {
                    for shortage in [0, 1] {
                        use nepl3_core::budget::StopReason;
                        let mut limits = limits;
                        match reason {
                            StopReason::WorkLimit => limits.work = usage.work - shortage,
                            StopReason::AllocationLimit => {
                                limits.allocation_units = usage.allocation_units - shortage
                            }
                            StopReason::DepthLimit => limits.depth = usage.depth - shortage,
                            StopReason::OutputLimit => {
                                limits.output_bytes = usage.output_bytes - shortage
                            }
                            _ => unreachable!(),
                        }
                        let (result, _) = run(limits)?;
                        if shortage == 0 {
                            assert_eq!(result.map_err(err)?.markup, rendered.markup);
                        } else {
                            assert!(
                                matches!(result, Err(nepl3_tools::doc::annotations::Error::Stopped(actual)) if actual == reason),
                                "HTML: {reason:?}"
                            );
                        }
                    }
                }
                assert_eq!(rendered.foreign.len(), 2);
                for (foreign, expected) in rendered
                    .foreign
                    .iter()
                    .zip([vec!["漢字", "かんじ"], vec!["reference"]])
                {
                    let ForeignRecord::Document(record) = foreign else {
                        return Err("Doc owner".into());
                    };
                    let [label] = record.foreign.as_slice() else {
                        return Err("one Sentence label".into());
                    };
                    let DocumentOutput::Sentence(label) = &label.output else {
                        return Err("Sentence owner".into());
                    };
                    if expected.len() == 2 {
                        let [ForeignRecord::Math(math)] = label.foreign.as_slice() else {
                            return Err("nested label Math".into());
                        };
                        assert!(!math.output.node_roots.is_empty());
                        for root in &math.output.node_roots {
                            assert!(matches!(
                                rendered.markup.fragment.nodes.get(*root as usize),
                                Some(HtmlNode::MathElement { .. })
                            ));
                        }
                        assert!(
                            rendered
                                .markup
                                .fragment
                                .nodes
                                .iter()
                                .any(|node| matches!(node, HtmlNode::Text { text } if text == "7"))
                        );
                    }
                    for text in expected {
                        assert!(label.origins.iter().any(|origin|
                    matches!(&label.syntax.value.nodes[origin.node as usize], Kind::Text { text: actual } if actual == text)
                    && matches!(&rendered.markup.fragment.nodes[origin.element as usize], HtmlNode::Text { text: actual } if actual == text)
                    && label.syntax.locations[origin.node as usize].cover.as_ref().is_some_and(|span|
                        span.snapshot_ref().source.0 == "doc-input"
                        && input.get(span.start() as usize..span.end() as usize).is_some_and(|source| source.contains(text)))
                ), "label provenance: {text}");
                    }
                }
                assert!(rendered.markup.fragment.nodes.iter().any(|node| matches!(node,
            HtmlNode::Element { attributes, .. } if attributes.iter().any(|a| matches!(a, HtmlAttribute::Id { value } if value == "n-746172676574")))));
                assert!(rendered.markup.fragment.nodes.iter().any(|node| matches!(node,
            HtmlNode::Element { attributes, .. } if attributes.iter().any(|a| matches!(a, HtmlAttribute::Href { value: nepl3_markup::html::HtmlHref::Fragment { id } } if id == "n-746172676574")))));
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn printer_reenters_document_from_a_sentence_label() -> Result<(), String> {
    let compiled = compiled()?;
    let input = r#"article en sentence "T" body cons paragraph cons sentence sentence
        cons doc anchor outer concat cons text "label: "
        cons doc ref inner text "nested" nil nil nil nil"#;
    with_input_route(true, &compiled, input, "Article", |tree, profile, b, a| {
        let store = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &store, a).map_err(err)?;
        let document = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        let mut printer = nepl3_tools::doc::printing::SentenceGuestPrinter {
            registry: profile.registry(),
            sentence_package: profile.language("Sentence", b).map_err(err)?,
            math_surface: None,
            doc_surface: Some(&compiled.doc.package.schema),
            codec: &mut codec,
        };
        // The title owns slot zero; the paragraph owns slot one. The output
        // preserves the nested reference syntax without claiming label resolution.
        let printed = printer
            .print(
                document.value.embeds[1]
                    .syntax()
                    .ok_or("paragraph syntax")?,
                b,
            )
            .map_err(err)?;
        assert_eq!(
            printed,
            "sentence cons doc anchor outer concat cons text \"label: \" cons doc ref inner text \"nested\" nil nil"
        );
        Ok(())
    })
}

#[test]
fn standard_host_preserves_nested_document_labels() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body cons paragraph
        cons sentence sentence
          cons doc anchor target ruby text "漢字" text "かんじ"
          cons doc ref target text "reference"
          nil nil nil"#;
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            source,
            "Article",
            |tree, profile, b, a| {
                let store = SourceStore::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, a).map_err(err)?;
                let document = lower::document(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let surface = profile
                    .registry()
                    .selected("nepl3.syntax.sentence", 1)
                    .ok_or("Sentence surface")?;
                let forms = [ForeignInlineForm {
                    kind: "Form:DocumentInline",
                    guest_schema: &compiled.doc.package.schema,
                    guest_category: "Inline",
                }];
                let mut occurrences = Vec::new();
                let mut printed = Vec::new();
                for embed in &document.value.embeds {
                    let sentence_package = profile.language("Sentence", b).map_err(err)?;
                    let mut printer = nepl3_tools::doc::printing::SentenceGuestPrinter {
                        registry: profile.registry(),
                        sentence_package,
                        math_surface: None,
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    printed.push(
                        printer
                            .print(embed.syntax().ok_or("syntax slot")?, b)
                            .map_err(err)?,
                    );
                    if printed.len() == 2 {
                        use nepl3_core::budget::{Budget, StopReason};
                        use nepl3_tools::doc::{printing::Error, source::budget};
                        let guest = embed.syntax().ok_or("syntax slot")?;
                        let run = |limits| -> Result<_, String> {
                            let mut admission = nepl3_core::source::SourceAdmission::default();
                            let mut codec =
                                FoundationCodec::new(profile.registry(), &store, &mut admission)
                                    .map_err(err)?;
                            let mut printer = nepl3_tools::doc::printing::SentenceGuestPrinter {
                                registry: profile.registry(),
                                sentence_package,
                                math_surface: None,
                                doc_surface: Some(&compiled.doc.package.schema),
                                codec: &mut codec,
                            };
                            let mut b = Budget::new(limits);
                            let result = printer.print(guest, &mut b);
                            Ok((result, b.usage()))
                        };
                        let (result, usage) = run(budget().limits())?;
                        assert_eq!(result.map_err(err)?, printed[1]);
                        for reason in [
                            StopReason::WorkLimit,
                            StopReason::AllocationLimit,
                            StopReason::DepthLimit,
                            StopReason::OutputLimit,
                        ] {
                            // Fresh admission per execution prevents the success run
                            // from hiding charges at the recursive guest boundary.
                            for shortage in [0, 1] {
                                let mut limits = budget().limits();
                                match reason {
                                    StopReason::WorkLimit => limits.work = usage.work - shortage,
                                    StopReason::AllocationLimit => {
                                        limits.allocation_units = usage.allocation_units - shortage
                                    }
                                    StopReason::DepthLimit => limits.depth = usage.depth - shortage,
                                    StopReason::OutputLimit => {
                                        limits.output_bytes = usage.output_bytes - shortage
                                    }
                                    _ => unreachable!(),
                                }
                                let (result, _) = run(limits)?;
                                if shortage == 0 {
                                    assert_eq!(result.map_err(err)?, printed[1]);
                                } else {
                                    assert!(
                                        matches!(result, Err(Error::Stopped(actual)) if actual == reason),
                                        "{reason:?}: {result:?}"
                                    );
                                }
                            }
                        }
                        printer.doc_surface = None;
                        let rejected = printer.print(guest, &mut budget());
                        assert!(
                            matches!(
                                rejected,
                                Err(Error::Lower(
                                    nepl3_sentence_core::lower::presentation::Error::Prefix(
                                        nepl3_sentence_core::lower::Error::Unsupported(_)
                                    )
                                ))
                            ),
                            "{rejected:?}"
                        );
                    }
                    let content =
                        sentence::lower(embed, surface, &forms, profile.registry(), &mut codec, b)
                            .map_err(err)?;
                    let selection = document_guests::collect(
                        &content,
                        &compiled.doc.package.schema,
                        profile.registry(),
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    let (documents, selected) = selection.into_parts();
                    for occurrence in selected {
                        let nested = &documents[occurrence.document.index()];
                        let DocRoot::Inline(root) = nested.value.root else {
                            return Err("Doc Inline root".into());
                        };
                        let (name, label, anchor) = match &nested.value.nodes[root.0 as usize].kind
                        {
                            DocKind::Anchor { id, label } => (id, label, true),
                            DocKind::Reference { target, label } => (target, label, false),
                            other => return Err(format!("unexpected Doc node: {other:?}")),
                        };
                        let label = sentence::lower(
                            &nested.value.embeds[label.0 as usize],
                            surface,
                            &forms,
                            profile.registry(),
                            &mut codec,
                            b,
                        )
                        .map_err(err)?;
                        let nepl3_sentence_core::model::Root::Inline(root) = label.value.root
                        else {
                            return Err("Sentence Inline root".into());
                        };
                        if anchor {
                            let Kind::Ruby { base, reading } = &label.value.nodes[root.0 as usize]
                            else {
                                return Err("Ruby label".into());
                            };
                            assert!(
                                matches!(&label.value.nodes[base.0 as usize], Kind::Text { text } if text == "漢字")
                            );
                            assert!(
                                matches!(&label.value.nodes[reading.0 as usize], Kind::Text { text } if text == "かんじ")
                            );
                        } else {
                            assert!(
                                matches!(&label.value.nodes[root.0 as usize], Kind::Text { text } if text == "reference")
                            );
                        }
                        occurrences.push((name.clone(), anchor));
                    }
                }
                // Independent expectations identify both Doc occurrences, their order,
                // and the Sentence-owned label structure. Roundtrip alone is insufficient.
                assert_eq!(
                    occurrences,
                    [("target".into(), true), ("target".into(), false)]
                );
                assert_eq!(
                    printed,
                    [
                        "sentence cons text \"Title\" nil",
                        "sentence cons doc anchor target ruby text \"漢字\" text \"かんじ\" cons doc ref target text \"reference\" nil",
                    ]
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}
