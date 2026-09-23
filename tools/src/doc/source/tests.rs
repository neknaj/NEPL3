use super::*;
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_doc_core::{check::Category, lower, model::DocKind};
use nepl3_sentence_core::model::Kind;
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn doc_inline_printing_reenters_math_sentence_and_obeys_limits() -> Result<(), String> {
    use nepl3_core::budget::StopReason;
    use nepl3_grammar_core::compile::package::ForeignForm;
    use nepl3_math_core::print::GuestPrinter;
    let compiled = compiled_with_sentence_forms(&[
        ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "doc",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "document namespace consumer",
        },
        ForeignForm {
            kind: "InlineMath",
            category: "Inline",
            spelling: "math",
            field: "syntax",
            alias: "Math",
            guest_category: "Expr",
            origin_reason: "inline expression consumer",
        },
    ])?;
    let sentence = compiled.others.last().ok_or("Sentence package")?;
    let source = r#"article en "Title" body cons display Math label x Sentence sentence cons doc anchor target math Math label 7 Sentence sentence cons doc ruby text "字" text "じ" nil nil nil"#;
    let expected = "sentence cons doc anchor target math Math label 7 Sentence sentence cons doc ruby text \"字\" text \"じ\" nil nil";
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            source,
            "Article",
            |tree, profile, b, a| {
                let registry = profile.registry();
                let input = tree
                    .tree()
                    .bundle
                    .validate_with_sources(registry, b, a)
                    .map_err(err)?;
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec =
                    FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                let document = lower::document(
                    &input,
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let closure = &document.value.embeds.first().ok_or("Math closure")?.closure;
                let input = closure
                    .syntax
                    .bundle
                    .validate_with_sources(registry, b, codec.source_admission())
                    .map_err(err)?;
                let math = nepl3_math_core::lower::expression(
                    &input,
                    &compiled.others[0].schema,
                    nepl3_math_core::check::Category::Expr,
                    registry,
                    b,
                    codec.source_admission(),
                )
                .map_err(err)?;
                let guest = math.value.embeds.first().ok_or("Sentence closure")?;
                let mut printer = crate::doc::printing::SentenceGuestPrinter {
                    registry,
                    sentence_package: sentence,
                    math_surface: Some(&compiled.others[0].schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                let mut measured = budget();
                assert_eq!(printer.print(guest, &mut measured).map_err(err)?, expected);
                let usage = measured.usage();
                assert!(usage.depth > 1);
                // Exercise shortages near successful totals, including the combined
                // owner depth. No stage may replace the caller's remaining budget.
                for reason in [
                    StopReason::WorkLimit,
                    StopReason::AllocationLimit,
                    StopReason::OutputLimit,
                    StopReason::DepthLimit,
                ] {
                    let mut limits = measured.limits();
                    match reason {
                        StopReason::WorkLimit => limits.work = usage.work - 1,
                        StopReason::AllocationLimit => {
                            limits.allocation_units = usage.allocation_units - 1
                        }
                        StopReason::OutputLimit => limits.output_bytes = usage.output_bytes - 1,
                        StopReason::DepthLimit => limits.depth = usage.depth - 1,
                        _ => unreachable!("fixed resource cases"),
                    }
                    let mut limited = Budget::new(limits);
                    assert!(matches!(printer.print(guest, &mut limited),
                    Err(crate::doc::printing::Error::Stopped(actual)) if actual == reason));
                    assert_eq!(limited.poll(), Err(reason));
                }
                printer.math_surface = None;
                assert!(matches!(
                    printer.print(guest, &mut budget()),
                    Err(crate::doc::printing::Error::Selection)
                ));
                Ok(())
            },
        )?;
        // The expected text is independently specified and accepted by the
        // selected reader profile as a nested Sentence, on both routes.
        let reprinted =
            format!("article en \"Title\" body cons display Math label x Sentence {expected} nil");
        with_input_route(
            native,
            &compiled,
            &reprinted,
            "Article",
            |_, _, _, _| Ok(()),
        )?;
    }
    Ok(())
}

#[test]
fn selected_sentence_doc_inline_keeps_owner_and_source_on_both_routes() -> Result<(), String> {
    let default = compiled()?;
    let compiled =
        compiled_with_sentence_forms(&[nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "document",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "explicit document namespace consumer",
        }])?;
    let sentence = compiled.others.last().ok_or("Sentence package")?;
    assert_ne!(
        sentence.schema,
        default.others.last().ok_or("default Sentence")?.schema
    );
    let source = r#"article en "Title" body cons display Math label x Sentence sentence cons document anchor target ruby text "字" text "じ" nil nil"#;
    for native in [false, true] {
        assert!(
            with_input_route(native, &default, source, "Article", |_, _, _, _| Ok(())).is_err()
        );
        with_input_route(
            native,
            &compiled,
            source,
            "Article",
            |tree, profile, b, a| {
                let registry = profile.registry();
                let input = tree
                    .tree()
                    .bundle
                    .validate_with_sources(registry, b, a)
                    .map_err(err)?;
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec =
                    FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                let document = lower::document(
                    &input,
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let math = &document.value.embeds.first().ok_or("Math closure")?.closure;
                let input = math
                    .syntax
                    .bundle
                    .validate_with_sources(registry, b, codec.source_admission())
                    .map_err(err)?;
                let math = nepl3_math_core::lower::expression(
                    &input,
                    &compiled.others[0].schema,
                    nepl3_math_core::check::Category::Expr,
                    registry,
                    b,
                    codec.source_admission(),
                )
                .map_err(err)?;
                let guest = math.value.embeds.first().ok_or("Sentence closure")?;
                {
                    use nepl3_math_core::print::GuestPrinter;
                    let mut printer = crate::doc::printing::SentenceGuestPrinter {
                        registry,
                        sentence_package: sentence,
                        math_surface: None,
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    // Doc owns the anchor and Ruby label; Sentence owns only
                    // the explicitly selected one-field foreign form.
                    assert_eq!(
                        printer.print(guest, b).map_err(err)?,
                        "sentence cons document anchor target ruby text \"字\" text \"じ\" nil"
                    );
                    printer.doc_surface = None;
                    assert!(matches!(
                        printer.print(guest, b),
                        Err(crate::doc::printing::Error::Lower(
                            nepl3_sentence_core::lower::presentation::Error::Prefix(
                                nepl3_sentence_core::lower::Error::Unsupported(_)
                            )
                        ))
                    ));
                    printer.doc_surface = Some(&compiled.others[0].schema);
                    assert!(matches!(
                        printer.print(guest, b),
                        Err(crate::doc::printing::Error::Lower(
                            nepl3_sentence_core::lower::presentation::Error::Prefix(
                                nepl3_sentence_core::lower::Error::Operand { field: 0, .. }
                            )
                        ))
                    ));
                }
                let input = guest
                    .syntax
                    .bundle
                    .validate_with_sources(registry, b, codec.source_admission())
                    .map_err(err)?;
                let sentence_value =
                    nepl3_sentence_core::lower::presentation::sentence_with_foreign(
                        &input,
                        &sentence.schema,
                        &[nepl3_sentence_core::lower::ForeignInlineForm {
                            kind: "Form:DocumentInline",
                            guest_schema: &compiled.doc.package.schema,
                            guest_category: "Inline",
                        }],
                        registry,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                assert_eq!(sentence_value.value.embeds.len(), 1);
                assert!(
                    sentence_value
                        .value
                        .nodes
                        .iter()
                        .any(|kind| matches!(kind, Kind::ForeignInline { .. }))
                );
                let guest = &sentence_value.value.embeds[0];
                assert_eq!(guest.syntax.schema, compiled.doc.package.schema);
                assert_eq!(guest.syntax.category, "Inline");
                let input = guest
                    .syntax
                    .bundle
                    .validate_with_sources(registry, b, codec.source_admission())
                    .map_err(err)?;
                let inline = lower::document(
                    &input,
                    &compiled.doc.package.schema,
                    Category::Inline,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let nepl3_doc_core::model::DocRoot::Inline(root) = inline.value.root else {
                    return Err("Doc Inline root required".into());
                };
                let anchor = inline
                    .value
                    .nodes
                    .get(root.0 as usize)
                    .ok_or("anchor root")?;
                let DocKind::Anchor { id, label } = &anchor.kind else {
                    return Err("anchor kind required".into());
                };
                assert_eq!(id, "target");
                let DocKind::Ruby { base, reading } = &inline.value.nodes[label.0 as usize].kind
                else {
                    return Err("anchor Ruby label required".into());
                };
                assert!(
                    matches!(&inline.value.nodes[base.0 as usize].kind, DocKind::Text { text } if text == "字")
                );
                assert!(
                    matches!(&inline.value.nodes[reading.0 as usize].kind, DocKind::Text { text } if text == "じ")
                );
                // The namespace operand is owned by Doc and retains the original
                // byte selection through Doc -> Math -> Sentence -> Doc re-entry.
                let location = anchor
                    .locations
                    .iter()
                    .find(|location| location.field == nepl3_doc_core::model::DocField::AnchorId)
                    .ok_or("anchor ID location")?;
                let span = location.span.as_ref().ok_or("anchor ID span")?;
                let start = source.find("target").ok_or("fixture target")? as u64;
                assert_eq!(span.start(), start);
                assert_eq!(span.end(), start + 6);
                assert_eq!(span.snapshot_ref().source, SourceId("doc-input".into()));
                assert_eq!(span.snapshot_ref().digest, Digest::of(source.as_bytes()));
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn math_sentence_math_printing_preserves_recursive_source() -> Result<(), String> {
    let compiled = compiled()?;
    let sentence = compiled.others.last().ok_or("Sentence package")?;
    let input = r#"article en "Math" body cons display Math label x Sentence sentence cons math label 7 Sentence "[字/じ]" nil nil"#;
    let mut outputs = Vec::new();
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            input,
            "Article",
            |tree, profile, b, a| {
                let checked = tree
                    .tree()
                    .bundle
                    .validate_with_sources(profile.registry(), b, a)
                    .map_err(err)?;
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let document = lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let closure = document.value.embeds.first().ok_or("Math closure")?;
                let input = closure
                    .closure
                    .syntax
                    .bundle
                    .validate_with_sources(profile.registry(), b, codec.source_admission())
                    .map_err(err)?;
                let math = nepl3_math_core::lower::expression(
                    &input,
                    &compiled.others[0].schema,
                    nepl3_math_core::check::Category::Expr,
                    profile.registry(),
                    b,
                    codec.source_admission(),
                )
                .map_err(err)?;
                let shape = math.value.validate_shape(b).map_err(err)?;
                let mut printer = crate::doc::printing::SentenceGuestPrinter {
                    registry: profile.registry(),
                    sentence_package: sentence,
                    math_surface: Some(&compiled.others[0].schema),
                    doc_surface: None,
                    codec: &mut codec,
                };
                let output =
                    nepl3_math_core::print::prefix(&shape, &mut printer, b).map_err(err)?;
                assert_eq!(
                    output.text,
                    "label symbol \"x\" Sentence sentence cons math label 7 Sentence sentence cons ruby text \"字\" text \"じ\" nil nil"
                );
                let mut host = crate::doc::math::MathDisplayHost {
                    registry: profile.registry(),
                    math_surface: &compiled.others[0].schema,
                    sentence_surface: Some(&sentence.schema),
                    codec: &mut codec,
                };
                let html = host
                    .render(&closure.closure, nepl3_markup::mathml::Display::Block, b)
                    .map_err(err)?
                    .into_html(b)
                    .map_err(err)?;
                assert_eq!(html.annotations.len(), 1);
                let outer = &html.annotations[0];
                assert_eq!(outer.foreign.len(), 1);
                let inner = &outer.foreign[0];
                assert_eq!(inner.annotations.len(), 1);
                for text in ["字", "じ"] {
                    assert!(inner.annotations[0].origins.iter().any(|origin| {
                        matches!(&inner.annotations[0].sentence.value.nodes[origin.node as usize],
                            Kind::Text { text: value } if value == text)
                        && matches!(html.markup.fragment.nodes.get(origin.element as usize),
                            Some(nepl3_markup::html::HtmlNode::Text { text: value }) if value == text)
                    }), "nested annotation mapping for {text}");
                }
                assert!(
                    inner
                        .node_roots
                        .iter()
                        .all(|id| (*id as usize) < html.markup.fragment.nodes.len())
                );
                let proof = nepl3_markup::html::validate(
                    &html.markup.fragment,
                    html.markup.slot,
                    &html.markup.policy,
                    b,
                )
                .map_err(err)?;
                outputs.push(nepl3_markup::html::serialize_xhtml(&proof, b).map_err(err)?);
                Ok(())
            },
        )?;
    }
    assert_eq!(outputs[0], outputs[1]);
    println!(
        "MATH_RECURSIVE_HTML {}",
        outputs[0]
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    Ok(())
}

#[test]
fn math_annotation_uses_registered_sentence_on_both_reader_routes() -> Result<(), String> {
    let compiled = compiled()?;
    let sentence = compiled.others.last().ok_or("Sentence package")?;
    assert_eq!(sentence.root, "Sentence");
    // Ruby belongs to the independent Sentence value inside the Math closure.
    // Both routes must preserve that value and its original Unicode source.
    for (native, input) in [false, true].into_iter().flat_map(|native| {
        [
            r#"article en "Math" body cons display Math label x Sentence "[字/じ]" nil"#,
            r#"article en "Math" body cons display Math label x Sentence sentence cons ruby text "字" text "じ" nil nil"#,
        ].into_iter().map(move |input| (native, input))
    }) {
        with_input_route(
            native,
            &compiled,
            input,
            "Article",
            |tree, profile, b, a| {
                let checked = tree
                    .tree()
                    .bundle
                    .validate_with_sources(profile.registry(), b, a)
                    .map_err(err)?;
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let document = lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let display = document
                    .value
                    .nodes
                    .iter()
                    .position(|node| matches!(node.kind, DocKind::DisplayMath { .. }))
                    .ok_or("display Math")?;
                let mut host = crate::doc::math::MathDisplayHost {
                    registry: profile.registry(),
                    math_surface: &compiled.others[0].schema,
                    sentence_surface: Some(&sentence.schema),
                    codec: &mut codec,
                };
                let rendered = host
                    .render_node(&document, display as u64, b)
                    .map_err(err)?;
                assert_eq!(rendered.annotations.len(), 1);
                let annotation = &rendered.annotations[0];
                let ruby = annotation
                    .sentence
                    .value
                    .nodes
                    .iter()
                    .find_map(|node| match node {
                        Kind::Ruby { base, reading } => Some((*base, *reading)),
                        _ => None,
                    })
                    .ok_or("Ruby")?;
                assert_eq!(
                    annotation.sentence.value.nodes[ruby.0.0 as usize],
                    Kind::Text { text: "字".into() }
                );
                assert_eq!(
                    annotation.sentence.value.nodes[ruby.1.0 as usize],
                    Kind::Text { text: "じ".into() }
                );
                assert!(
                    annotation
                        .sentence
                        .sources
                        .iter()
                        .any(|source| source.text() == input)
                );
                assert!(!annotation.origins.is_empty());
                let shape = rendered.syntax.value.validate_shape(b).map_err(err)?;
                let mut printer = crate::doc::printing::SentenceGuestPrinter {
                    registry: profile.registry(), sentence_package: sentence,
                    math_surface: None, doc_surface: None, codec: &mut codec,
                };
                let printed = nepl3_math_core::print::prefix(&shape, &mut printer, b)
                    .map_err(err)?;
                assert_eq!(printed.text,
                    "label symbol \"x\" Sentence sentence cons ruby text \"字\" text \"じ\" nil");
                let html = rendered.into_html(b).map_err(err)?;
                assert_eq!(html.annotations.len(), 1);
                assert!(!html.annotations[0].origins.is_empty());
                Ok(())
            },
        )?;
    }
    Ok(())
}
