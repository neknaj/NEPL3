use super::*;
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_doc_core::{check::Category, model::DocKind};
use nepl3_sentence_core::model::Kind;
use nepl3_wire::foundation::FoundationCodec;

mod namespace;
mod profile;

#[test]
fn composed_doc_namespace_keeps_fragment_source_identity() -> Result<(), String> {
    use nepl3_doc_core::labels::namespace::{self, MemberId};
    let compiled = compiled()?;
    for (native, conflicting) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut documents = Vec::new();
        for (name, source) in [
            ("reference-fragment", "ref target text \"参照\""),
            ("definition-fragment", "anchor target text \"定義\""),
        ] {
            with_named_input(
                native,
                &compiled,
                source,
                if conflicting { "shared-fragment" } else { name },
                "Inline",
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
                    let document = nepl3_suite::adapters::sentence::literal::lower(
                        &input,
                        &compiled.doc.package.schema,
                        Category::Inline,
                        registry,
                        b,
                        &mut codec,
                    )
                    .map_err(err)?;
                    // Inspect while the exact profile registry/source proof is available.
                    namespace::inspect(&document, registry, b, codec.source_admission())
                        .map_err(err)?;
                    documents.push(document);
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
                let inputs = documents
                    .iter()
                    .map(|document| {
                        namespace::inspect(
                            document,
                            profile.registry(),
                            b,
                            &mut SourceAdmission::default(),
                        )
                        .map_err(err)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let members: Vec<_> = inputs.iter().collect();
                let checked = namespace::resolve(&members, b).map_err(err)?;
                let [reference] = checked.references() else {
                    return Err("one reference".into());
                };
                assert_eq!(reference.reference.member, MemberId(0));
                let target = &checked.definitions()[reference.target.0 as usize];
                assert_eq!(target.member, MemberId(1));
                let from = reference
                    .reference
                    .site
                    .selection
                    .ok_or("reference source")?;
                let to = target.site.selection.ok_or("declaration source")?;
                assert_eq!((from.start(), from.end()), (4, 10));
                assert_eq!((to.start(), to.end()), (7, 13));
                assert_eq!(
                    from.snapshot_ref().digest,
                    Digest::of("ref target text \"参照\"".as_bytes())
                );
                assert_eq!(
                    to.snapshot_ref().digest,
                    Digest::of("anchor target text \"定義\"".as_bytes())
                );
                assert_ne!(from.snapshot_ref().digest, to.snapshot_ref().digest);
                assert!(core::ptr::eq(
                    checked.document(MemberId(0)).ok_or("reference owner")?,
                    &documents[0]
                ));
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                let options = nepl3_doc_html::RenderOptions {
                    parallel: nepl3_doc_html::ParallelMode::Rows,
                };
                let prepared = nepl3_doc_html::namespace::prepare(
                    &checked,
                    &options,
                    profile.registry(),
                    &mut codec,
                    b,
                );
                if conflicting {
                    assert!(
                        matches!(
                            prepared,
                            Err(nepl3_doc_html::LocalPreparationError::Input(
                                nepl3_doc_core::prepare::PreparationError::Boundary(
                                    nepl3_doc_core::portable::PortableError::Structure(
                                        nepl3_doc_core::check::StructureError::Source(
                                            nepl3_core::source::SourceError::IdentityConflict
                                        )
                                    )
                                )
                            ))
                        ),
                        "common admission must reject conflicting snapshots"
                    );
                    return Ok(());
                }
                let prepared = prepared.map_err(err)?;
                let part = nepl3_doc_html::namespace::render_part(&prepared, MemberId(0), b)
                    .map_err(err)?;
                let (member, _, pending, origins) = part.into_parts();
                assert_eq!(member, MemberId(0));
                assert!(!origins.is_empty());
                assert!(matches!(
                    nepl3_markup::html::validate(
                        &pending.fragment,
                        pending.slot,
                        &pending.policy,
                        b
                    ),
                    Err(nepl3_markup::html::HtmlError::MissingFragment(_))
                ));
                assert!(matches!(
                    nepl3_doc_html::namespace::render_part(&prepared, MemberId(2), b),
                    Err(nepl3_doc_html::namespace::PartError::Member(MemberId(2)))
                ));
                let mut measured = budget();
                let rendered =
                    nepl3_doc_html::namespace::render(&prepared, &mut measured).map_err(err)?;
                assert_eq!(rendered.documents.len(), 2);
                use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode};
                for (member, is_reference) in [(MemberId(0), true), (MemberId(1), false)] {
                    let nepl3_doc_core::model::DocRoot::Inline(owner) =
                        documents[member.0 as usize].value.root
                    else {
                        return Err("Inline owner".into());
                    };
                    assert!(rendered.origins.iter().any(|origin| {
                        origin.member == member && origin.node == owner.0 && matches!(&rendered.markup.fragment.nodes[origin.element as usize], HtmlNode::Element { attributes, .. } if attributes.iter().any(|attribute| {
                            if is_reference { matches!(attribute, HtmlAttribute::Href { value: HtmlHref::Fragment { id } } if id == "n-746172676574") }
                            else { matches!(attribute, HtmlAttribute::Id { value } if value == "n-746172676574") }
                        }))
                    }));
                }
                let used = measured.usage();
                for reason in [
                    nepl3_core::budget::StopReason::WorkLimit,
                    nepl3_core::budget::StopReason::AllocationLimit,
                    nepl3_core::budget::StopReason::DepthLimit,
                ] {
                    let mut limits = measured.limits();
                    match reason {
                        nepl3_core::budget::StopReason::WorkLimit => limits.work = used.work - 1,
                        nepl3_core::budget::StopReason::AllocationLimit => {
                            limits.allocation_units = used.allocation_units - 1
                        }
                        nepl3_core::budget::StopReason::DepthLimit => limits.depth = used.depth - 1,
                        _ => unreachable!("fixed resource cases"),
                    }
                    let mut limited = Budget::new(limits);
                    assert!(nepl3_doc_html::namespace::render(&prepared, &mut limited).is_err());
                    assert_eq!(limited.poll(), Err(reason));
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn doc_foreign_html_preserves_failures_and_rejects_duplicate_ids() -> Result<(), String> {
    use crate::doc::annotations::{Error, SentenceAnnotationRenderer, document};
    enum Expected {
        EmptyGuest,
        ValidGuest,
        Dependency,
        Label,
        Duplicate,
        Composed,
    }
    let compiled =
        compiled_with_sentence_forms(&[nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "doc",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "document fragment test",
        }])?;
    for (body, expected) in [
        (
            r#"cons ruby text "base" doc anchor target text "" nil"#,
            Expected::EmptyGuest,
        ),
        (
            r#"cons anno doc anchor target text "" cons text "note" nil nil"#,
            Expected::EmptyGuest,
        ),
        (
            r#"cons anno text "base" cons doc anchor target text "" nil nil"#,
            Expected::EmptyGuest,
        ),
        (
            r#"cons ruby doc ref target text "" text "reading" cons doc anchor target text "definition" nil"#,
            Expected::EmptyGuest,
        ),
        (
            r#"cons ruby doc link external "https://example.test/" text "" text "reading" nil"#,
            Expected::EmptyGuest,
        ),
        (
            r#"cons doc anchor target text "" nil"#,
            Expected::ValidGuest,
        ),
        (
            r#"cons ruby concat cons doc anchor target text "" cons text "base" nil text "reading" nil"#,
            Expected::ValidGuest,
        ),
        (
            r#"cons ruby doc anchor target break text "reading" nil"#,
            Expected::ValidGuest,
        ),
        (
            "cons ruby doc anchor target text \"\" text \"reading\" nil",
            Expected::EmptyGuest,
        ),
        (
            "cons doc link external \"https://example.test/\" text \"label\" nil",
            Expected::Dependency,
        ),
        ("cons doc ref missing text \"label\" nil", Expected::Label),
        (
            "cons doc anchor same text \"first\" cons doc anchor same text \"second\" nil",
            Expected::Duplicate,
        ),
        (
            "cons doc ref target math Math 7 cons doc anchor target text \"定義\" nil",
            Expected::Composed,
        ),
    ] {
        let source = format!(
            "article en \"Title\" body cons display Math label x Sentence sentence {body} nil"
        );
        for native in [false, true] {
            with_input_route(
                native,
                &compiled,
                &source,
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
                    let doc = nepl3_suite::adapters::sentence::literal::lower(
                        &input,
                        &compiled.doc.package.schema,
                        Category::Article,
                        registry,
                        b,
                        &mut codec,
                    )
                    .map_err(err)?;
                    let input = doc
                        .value
                        .embeds
                        .get(0)
                        .ok_or("embed")?
                        .closure
                        .syntax()
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
                    let mut host = SentenceAnnotationRenderer {
                        registry,
                        surface: &compiled.others[3].schema,
                        math_surface: Some(&compiled.others[0].schema),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    let result = host.render(&math.value.embeds[0], b);
                    match &expected {
                        Expected::Composed => {
                            let output = result.map_err(err)?;
                            use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode};
                            assert_eq!(output.foreign.len(), 2);
                            assert!(output.markup.fragment.nodes.iter().any(|node| matches!(node,
                                HtmlNode::Element { attributes, .. } if attributes.iter().any(|attribute| matches!(attribute,
                                    HtmlAttribute::Href { value: HtmlHref::Fragment { id } } if id == "n-746172676574")))));
                            assert!(output.markup.fragment.nodes.iter().any(|node| matches!(node,
                                HtmlNode::Element { attributes, .. } if attributes.iter().any(|attribute| matches!(attribute,
                                    HtmlAttribute::Id { value } if value == "n-746172676574")))));
                            let [
                                crate::doc::annotations::ForeignRecord::Document(reference),
                                crate::doc::annotations::ForeignRecord::Document(definition),
                            ] = output.foreign.as_slice()
                            else {
                                return Err("ordered document owners".into());
                            };
                            assert_eq!(reference.foreign.len(), 1);
                            assert!(definition.foreign.is_empty());
                            for root in &reference.foreign[0].output.node_roots {
                                assert!(matches!(
                                    output.markup.fragment.nodes[*root as usize],
                                    HtmlNode::MathElement { .. }
                                ));
                            }
                            assert!(output.markup.fragment.nodes.iter().any(
                                |node| matches!(node, HtmlNode::Text { text } if text == "7")
                            ));
                            for duplicate_definition in [false, true] {
                                let mut shared = output.sentence.clone();
                                let nepl3_sentence_core::model::Root::Sentence(root) =
                                    shared.value.root
                                else {
                                    return Err("sentence root".into());
                                };
                                let Kind::Sentence { inlines } =
                                    &mut shared.value.nodes[root.0 as usize]
                                else {
                                    return Err("sentence node".into());
                                };
                                let repeated = inlines[usize::from(duplicate_definition)];
                                inlines.insert(1, repeated);
                                let selected =
                                    nepl3_suite::adapters::sentence::document_guests::collect(
                                        &shared,
                                        &compiled.doc.package.schema,
                                        registry,
                                        host.codec,
                                        b,
                                    )
                                    .map_err(err)?;
                                // Two unique closures, three display occurrences. Repeated
                                // content shares one document without duplicating its arena.
                                assert_eq!(selected.documents().len(), 2);
                                let occurrences = selected.occurrences();
                                assert_eq!(occurrences.len(), 3);
                                let repeated_index = if duplicate_definition { 2 } else { 0 };
                                assert_eq!(
                                    occurrences[1].document,
                                    occurrences[repeated_index].document
                                );
                                assert_eq!(occurrences[1].embed, occurrences[repeated_index].embed);
                                assert_eq!(
                                    selected.documents()[occurrences[0].document.index()],
                                    *reference.document
                                );
                                assert_eq!(
                                    selected.documents()[occurrences[2].document.index()],
                                    *definition.document
                                );
                                if !duplicate_definition {
                                    use nepl3_suite::adapters::sentence::document_guests;
                                    let store = SourceStore::default();
                                    let run = |limit: &mut Budget| {
                                        let mut admission = SourceAdmission::default();
                                        let mut codec =
                                            FoundationCodec::new(registry, &store, &mut admission)
                                                .map_err(err)?;
                                        Ok::<_, String>(document_guests::collect(
                                            &shared,
                                            &compiled.doc.package.schema,
                                            registry,
                                            &mut codec,
                                            limit,
                                        ))
                                    };
                                    let mut measured = budget();
                                    run(&mut measured)?.map_err(err)?;
                                    let usage = measured.usage();
                                    for (used, reason) in [
                                        (usage.work, StopReason::WorkLimit),
                                        (usage.allocation_units, StopReason::AllocationLimit),
                                        (usage.depth, StopReason::DepthLimit),
                                    ] {
                                        assert!(used > 0);
                                        for exact in [false, true] {
                                            let mut limits = budget().limits();
                                            let value = used - u64::from(!exact);
                                            match reason {
                                                StopReason::WorkLimit => limits.work = value,
                                                StopReason::AllocationLimit => {
                                                    limits.allocation_units = value
                                                }
                                                StopReason::DepthLimit => limits.depth = value,
                                                _ => return Err("selection test resource".into()),
                                            }
                                            let mut limit = Budget::new(limits);
                                            let result = run(&mut limit)?;
                                            if exact {
                                                result.map_err(err)?;
                                            } else {
                                                assert!(
                                                    matches!(result, Err(document_guests::Error::Stopped(actual)) if actual == reason)
                                                );
                                            }
                                        }
                                    }
                                }
                                let result = host.render_syntax(shared, b);
                                if duplicate_definition {
                                    let Err(Error::Document(error)) = result else {
                                        return Err("shared anchor must be duplicate".into());
                                    };
                                    let document::Error::NamespaceDuplicate {
                                        definition,
                                        previous,
                                        diagnostic,
                                    } = *error
                                    else {
                                        return Err("shared anchor owners".into());
                                    };
                                    assert_ne!(definition.member, previous.member);
                                    assert_eq!(definition.node, previous.node);
                                    assert_eq!(diagnostic.code, "DuplicateLabel");
                                    assert_eq!(diagnostic.primary, diagnostic.related[0].span);
                                    assert!(std::sync::Arc::ptr_eq(
                                        &definition.document,
                                        &previous.document
                                    ));
                                } else {
                                    let shared = result.map_err(err)?;
                                    let [
                                        crate::doc::annotations::ForeignRecord::Document(first),
                                        crate::doc::annotations::ForeignRecord::Document(second),
                                        crate::doc::annotations::ForeignRecord::Document(_),
                                    ] = shared.foreign.as_slice()
                                    else {
                                        return Err("three occurrences".into());
                                    };
                                    assert!(std::sync::Arc::ptr_eq(
                                        &first.document,
                                        &second.document
                                    ));
                                    assert_eq!(first.embed, second.embed);
                                    assert_eq!(first.document_digest, second.document_digest);
                                    assert_ne!(first.origins[0].element, second.origins[0].element);
                                    assert_ne!(
                                        first.foreign[0].output.node_roots,
                                        second.foreign[0].output.node_roots
                                    );
                                }
                            }
                            // No ambient source or schema may classify an unselected guest.
                            assert!(matches!(
                                document::namespace::check_content(
                                    &output.sentence,
                                    &[],
                                    None,
                                    registry,
                                    host.codec,
                                    b,
                                ),
                                Err(document::Error::Selection)
                            ));
                            let mut invalid = output.sentence.clone();
                            let nepl3_sentence_core::model::Root::Sentence(root) =
                                invalid.value.root
                            else {
                                return Err("sentence root".into());
                            };
                            invalid.value.nodes[root.0 as usize] = Kind::Sentence {
                                inlines: vec![nepl3_sentence_core::model::InlineRef(u64::MAX)],
                            };
                            assert!(matches!(
                                host.render_syntax(invalid, b),
                                Err(Error::Portable(_))
                            ));
                            let mut cancelled = budget();
                            cancelled.cancel();
                            assert!(matches!(
                                host.render_syntax(output.sentence.clone(), &mut cancelled),
                                Err(Error::Stopped(nepl3_core::budget::StopReason::Cancelled))
                            ));
                        }
                        Expected::EmptyGuest => {
                            assert!(
                                matches!(result, Err(Error::Document(error)) if matches!(*error, document::Error::Content(nepl3_sentence_core::check::Error::EmptyAnnotationPart(_)))),
                                "empty Doc guest must not satisfy annotation content"
                            );
                        }
                        Expected::ValidGuest => {
                            result.map_err(err)?;
                        }
                        Expected::Dependency => {
                            let Err(Error::Document(error)) = result else {
                                return Err("Doc dependency failure required".into());
                            };
                            let document::Error::NeedsResolution(plan) = *error else {
                                return Err("resolution plan required".into());
                            };
                            assert!(
                                matches!(plan.requirements.as_slice(), [nepl3_doc_core::prepare::DocRequirement::Link {
                            target: nepl3_doc_core::model::LinkTarget::External { uri }, ..
                        }] if uri == "https://example.test/")
                            );
                        }
                        Expected::Label => {
                            let Err(Error::Document(error)) = result else {
                                return Err("Doc label failure required".into());
                            };
                            let document::Error::Label(diagnostic) = *error else {
                                return Err("owned label diagnostic required".into());
                            };
                            assert_eq!(diagnostic.code, "UnresolvedLabel");
                            let span = diagnostic
                                .primary
                                .as_ref()
                                .ok_or("label diagnostic source")?;
                            let start = source.find("missing").ok_or("label fixture")? as u64;
                            assert_eq!((span.start(), span.end()), (start, start + 7));
                            assert_eq!(span.snapshot_ref().digest, Digest::of(source.as_bytes()));
                        }
                        Expected::Duplicate => {
                            let Err(Error::Document(error)) = result else {
                                return Err("namespace duplicate required".into());
                            };
                            let document::Error::NamespaceDuplicate {
                                definition,
                                previous,
                                diagnostic,
                            } = *error
                            else {
                                return Err("both definition owners required".into());
                            };
                            assert_ne!(definition.member, previous.member);
                            assert_eq!(diagnostic.code, "DuplicateLabel");
                            assert_eq!(diagnostic.related.len(), 1);
                            assert_ne!(diagnostic.primary, diagnostic.related[0].span);
                            for owner in [definition, previous] {
                                let span = owner.document.value.nodes[owner.node as usize]
                                    .locations[0]
                                    .span
                                    .as_ref()
                                    .ok_or("definition source")?;
                                assert_eq!(
                                    span.snapshot_ref().digest,
                                    Digest::of(source.as_bytes())
                                );
                                assert_eq!(
                                    &source[span.start() as usize..span.end() as usize],
                                    "same"
                                );
                            }
                        }
                    }
                    Ok(())
                },
            )?;
        }
    }
    Ok(())
}

#[test]
fn doc_inline_printing_and_html_reenter_math_sentence_and_obey_limits() -> Result<(), String> {
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
                let document = nepl3_suite::adapters::sentence::literal::lower(
                    &input,
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let closure = &document.value.embeds.get(0).ok_or("Math closure")?.closure;
                let input = closure
                    .syntax()
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
                let mut host = crate::doc::math::MathDisplayHost {
                    registry,
                    math_surface: &compiled.others[0].schema,
                    sentence_surface: Some(&sentence.schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                let mut measured = budget();
                let rendered = host
                    .render_view(closure, nepl3_markup::mathml::Display::Block, &mut measured)
                    .map_err(err)?
                    .into_html(&mut measured)
                    .map_err(err)?;
                let [annotation] = rendered.annotations.as_slice() else {
                    return Err("outer annotation".into());
                };
                let [crate::doc::annotations::ForeignRecord::Document(outer)] =
                    annotation.foreign.as_slice()
                else {
                    return Err("outer Doc record".into());
                };
                let [nested] = outer.foreign.as_slice() else {
                    return Err("Doc Math record".into());
                };
                assert_eq!(
                    outer
                        .document
                        .value
                        .embeds
                        .get(nested.embed.0 as usize)
                        .ok_or("embed")?
                        .kind,
                    nepl3_doc_core::model::EmbedKind::InlineMath
                );
                let [annotation] = nested.output.annotations.as_slice() else {
                    return Err("inner annotation".into());
                };
                let [crate::doc::annotations::ForeignRecord::Document(inner)] =
                    annotation.foreign.as_slice()
                else {
                    return Err("inner Doc record".into());
                };
                assert!(inner.foreign.is_empty());
                // Exercise the public Doc boundary with independently supplied
                // host output: a callback's successful return is not validation.
                use nepl3_doc_html::{
                    ForeignRenderError, ParallelMode, RenderError, RenderOptions,
                };
                use nepl3_markup::html::{
                    HtmlError, HtmlFragment, HtmlNode, HtmlPolicy, HtmlRequest, HtmlSlot, HtmlTag,
                };
                let options = RenderOptions {
                    parallel: ParallelMode::Rows,
                };
                let prepared = nepl3_doc_html::prepare_inline_with_foreign(
                    &outer.document,
                    &options,
                    registry,
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                for (nodes, expected) in [
                    (
                        vec![HtmlNode::Element {
                            tag: HtmlTag::Div,
                            attributes: vec![],
                            children: vec![],
                        }],
                        HtmlError::Content(0),
                    ),
                    (
                        vec![HtmlNode::Element {
                            tag: HtmlTag::Span,
                            attributes: vec![],
                            children: vec![4],
                        }],
                        HtmlError::Reference(4),
                    ),
                ] {
                    let mut calls = 0;
                    let result = nepl3_doc_html::render_inline_with_foreign(
                        &prepared,
                        &mut |_, _, _| {
                            calls += 1;
                            Ok::<_, ()>(HtmlRequest {
                                fragment: HtmlFragment {
                                    root: 0,
                                    nodes: nodes.clone(),
                                },
                                slot: HtmlSlot::Phrasing,
                                policy: HtmlPolicy { classes: vec![] },
                            })
                        },
                        &mut budget(),
                    );
                    assert_eq!(calls, 1);
                    assert!(
                        matches!(result, Err(ForeignRenderError::Render(RenderError::Markup(actual))) if actual == expected)
                    );
                }
                let mut deep = Vec::new();
                for index in 0..256 {
                    deep.push(HtmlNode::Element {
                        tag: HtmlTag::Span,
                        attributes: vec![],
                        children: vec![index + 1],
                    });
                }
                deep.push(HtmlNode::Text {
                    text: "leaf".into(),
                });
                assert!(matches!(
                    nepl3_doc_html::render_inline_with_foreign(
                        &prepared,
                        &mut |_, _, _| {
                            Ok::<_, ()>(HtmlRequest {
                                fragment: HtmlFragment {
                                    root: 0,
                                    nodes: deep.clone(),
                                },
                                slot: HtmlSlot::Phrasing,
                                policy: HtmlPolicy { classes: vec![] },
                            })
                        },
                        &mut budget()
                    ),
                    Err(ForeignRenderError::Render(RenderError::OutputDepth { .. }))
                ));
                let owner = outer
                    .document
                    .value
                    .nodes
                    .iter()
                    .position(|node| matches!(node.kind, DocKind::InlineMath { .. }))
                    .ok_or("Math owner")? as u64;
                let mut shared = outer
                    .document
                    .fragment(
                        nepl3_doc_core::model::DocRoot::Inline(nepl3_doc_core::model::InlineRef(
                            owner,
                        )),
                        registry,
                        &mut budget(),
                        codec.source_admission(),
                    )
                    .map_err(err)?;
                let nepl3_doc_core::model::DocRoot::Inline(owner) = shared.value.root else {
                    return Err("Inline fragment".into());
                };
                let owner = owner.0;
                let root = shared.value.nodes.len() as u64;
                shared.value.nodes.push(nepl3_doc_core::model::DocNode {
                    kind: DocKind::Concat {
                        inlines: vec![nepl3_doc_core::model::InlineRef(owner); 2],
                    },
                    span: None,
                    origin: None,
                    locations: vec![],
                });
                shared.value.root =
                    nepl3_doc_core::model::DocRoot::Inline(nepl3_doc_core::model::InlineRef(root));
                let prepared = nepl3_doc_html::prepare_inline_with_foreign(
                    &shared,
                    &options,
                    registry,
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                let mut calls = 0;
                let shared_result = nepl3_doc_html::render_inline_with_foreign(
                    &prepared,
                    &mut |actual, embed, _| {
                        calls += 1;
                        assert_eq!(Some(*actual), shared.value.embeds.get(embed.0 as usize));
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
                    },
                    &mut budget(),
                )
                .map_err(err)?;
                assert_eq!(calls, 2);
                let mut stopped = budget();
                let mut calls = 0;
                let failed = nepl3_doc_html::render_inline_with_foreign(
                    &prepared,
                    &mut |_, _, b| {
                        calls += 1;
                        b.stop(StopReason::WorkLimit);
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
                    &mut stopped,
                );
                assert_eq!(calls, 1);
                assert!(matches!(
                    failed,
                    Err(ForeignRenderError::Render(RenderError::Stopped(
                        StopReason::WorkLimit
                    )))
                ));
                assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
                assert_eq!(shared_result.foreign.len(), 2);
                assert_eq!(
                    shared_result.foreign[0].embed,
                    shared_result.foreign[1].embed
                );
                assert_ne!(
                    shared_result.foreign[0].first_element,
                    shared_result.foreign[1].first_element
                );
                for placement in &shared_result.foreign {
                    assert_eq!(placement.elements, 1);
                    assert!(
                        shared_result
                            .fragment
                            .origins
                            .iter()
                            .any(|origin| origin.element == placement.first_element
                                && origin.node == owner)
                    );
                }
                for text in ["字", "じ"] {
                    assert!(inner.origins.iter().any(|origin| {
                        matches!(&inner.document.value.nodes[origin.node as usize].kind,
                            DocKind::Text { text: value } if value == text)
                        && matches!(rendered.markup.fragment.nodes.get(origin.element as usize),
                            Some(nepl3_markup::html::HtmlNode::Text { text: value }) if value == text)
                    }), "final HTML owner for {text}");
                }
                assert!(
                    nested
                        .output
                        .node_roots
                        .iter()
                        .all(|element| (*element as usize) < rendered.markup.fragment.nodes.len())
                );
                let usage = measured.usage();
                let mut host = crate::doc::math::MathDisplayHost {
                    registry,
                    math_surface: &compiled.others[0].schema,
                    sentence_surface: Some(&sentence.schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                for reason in [
                    StopReason::WorkLimit,
                    StopReason::AllocationLimit,
                    StopReason::DepthLimit,
                ] {
                    let mut limits = measured.limits();
                    match reason {
                        StopReason::WorkLimit => limits.work = usage.work - 1,
                        StopReason::AllocationLimit => {
                            limits.allocation_units = usage.allocation_units - 1
                        }
                        StopReason::DepthLimit => limits.depth = usage.depth - 1,
                        _ => unreachable!("fixed resource cases"),
                    }
                    let mut limited = Budget::new(limits);
                    let result = host
                        .render_view(closure, nepl3_markup::mathml::Display::Block, &mut limited)
                        .map_err(err)
                        .and_then(|output| output.into_html(&mut limited).map_err(err));
                    assert!(result.is_err(), "no partial HTML after {reason:?}");
                    assert_eq!(limited.poll(), Err(reason));
                }
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
                let document = nepl3_suite::adapters::sentence::literal::lower(
                    &input,
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let math = &document.value.embeds.get(0).ok_or("Math closure")?.closure;
                let input = math
                    .syntax()
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
                let guest = &sentence_value.value.embeds.entry(0).ok_or("embed")?;
                assert_eq!(guest.syntax().schema, compiled.doc.package.schema);
                assert_eq!(guest.syntax().category, "Inline");
                let input = guest
                    .syntax()
                    .bundle
                    .validate_with_sources(registry, b, codec.source_admission())
                    .map_err(err)?;
                let inline = nepl3_suite::adapters::sentence::literal::lower(
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
                let options = nepl3_doc_html::RenderOptions {
                    parallel: nepl3_doc_html::ParallelMode::Rows,
                };
                let prepared = nepl3_doc_html::prepare_local_inline(
                    &inline, &options, registry, &mut codec, b,
                )
                .map_err(err)?;
                let rendered = nepl3_doc_html::render_inline(&prepared, b).map_err(err)?;
                assert_eq!(rendered.markup.slot, nepl3_markup::html::HtmlSlot::Phrasing);
                for (owner, expected) in [(base.0, "字"), (reading.0, "じ")] {
                    assert!(rendered.origins.iter().any(|origin| origin.node == owner
                        && matches!(
                            &rendered.markup.fragment.nodes[origin.element as usize],
                            nepl3_markup::html::HtmlNode::Text { text } if text == expected)));
                }
                assert!(rendered.origins.iter().any(|origin| origin.node == root.0 && matches!(
                    &rendered.markup.fragment.nodes[origin.element as usize],
                    nepl3_markup::html::HtmlNode::Element { attributes, .. }
                    if attributes.iter().any(|attribute| matches!(attribute,
                        nepl3_markup::html::HtmlAttribute::Id { value } if value == "n-746172676574")))));
                let mut host = crate::doc::math::MathDisplayHost {
                    registry,
                    math_surface: &compiled.others[0].schema,
                    sentence_surface: Some(&sentence.schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                let mut measured = budget();
                let composed = host
                    .render_view(
                        &document.value.embeds.get(0).ok_or("embed")?.closure,
                        nepl3_markup::mathml::Display::Block,
                        &mut measured,
                    )
                    .map_err(err)?
                    .into_html(&mut measured)
                    .map_err(err)?;
                let [annotation] = composed.annotations.as_slice() else {
                    return Err("one Sentence annotation expected".into());
                };
                let [crate::doc::annotations::ForeignRecord::Document(record)] =
                    annotation.foreign.as_slice()
                else {
                    return Err("one Doc Inline record expected".into());
                };
                assert_eq!(*record.document, inline);
                assert_eq!(record.document_digest, rendered.document_digest);
                for text in ["字", "じ"] {
                    assert!(record.origins.iter().any(
                        |origin| matches!(&record.document.value.nodes[origin.node as usize].kind,
                            DocKind::Text { text: value } if value == text)
                            && matches!(&composed.markup.fragment.nodes[origin.element as usize],
                            nepl3_markup::html::HtmlNode::Text { text: value } if value == text)
                    ));
                }
                assert!(record.origins.iter().any(|origin| origin.node == root.0 && matches!(
                    &composed.markup.fragment.nodes[origin.element as usize],
                    nepl3_markup::html::HtmlNode::Element { attributes, .. }
                    if attributes.iter().any(|attribute| matches!(attribute,
                        nepl3_markup::html::HtmlAttribute::Id { value } if value == "n-746172676574")))));
                let used = measured.usage();
                for reason in [
                    nepl3_core::budget::StopReason::WorkLimit,
                    nepl3_core::budget::StopReason::AllocationLimit,
                    nepl3_core::budget::StopReason::DepthLimit,
                ] {
                    let mut limits = measured.limits();
                    match reason {
                        nepl3_core::budget::StopReason::WorkLimit => limits.work = used.work - 1,
                        nepl3_core::budget::StopReason::AllocationLimit => {
                            limits.allocation_units = used.allocation_units - 1
                        }
                        nepl3_core::budget::StopReason::DepthLimit => limits.depth = used.depth - 1,
                        _ => unreachable!("fixed shortage cases"),
                    }
                    let mut limited = Budget::new(limits);
                    let result = host
                        .render_view(
                            &document.value.embeds.get(0).ok_or("embed")?.closure,
                            nepl3_markup::mathml::Display::Block,
                            &mut limited,
                        )
                        .map_err(err)
                        .and_then(|rendered| rendered.into_html(&mut limited).map_err(err));
                    assert!(result.is_err());
                    assert_eq!(limited.poll(), Err(reason));
                }
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
                let document = nepl3_suite::adapters::sentence::literal::lower(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let closure = document.value.embeds.get(0).ok_or("Math closure")?;
                let input = closure
                    .closure
                    .syntax()
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
                    doc_surface: None,
                    codec: &mut codec,
                };
                let html = host
                    .render_view(&closure.closure, nepl3_markup::mathml::Display::Block, b)
                    .map_err(err)?
                    .into_html(b)
                    .map_err(err)?;
                assert_eq!(html.annotations.len(), 1);
                let outer = &html.annotations[0];
                assert_eq!(outer.foreign.len(), 1);
                let crate::doc::annotations::ForeignRecord::Math(inner) = &outer.foreign[0] else {
                    return Err("Math record required".into());
                };
                assert_eq!(inner.output.annotations.len(), 1);
                for text in ["字", "じ"] {
                    assert!(inner.output.annotations[0].origins.iter().any(|origin| {
                        matches!(&inner.output.annotations[0].sentence.value.nodes[origin.node as usize],
                            Kind::Text { text: value } if value == text)
                        && matches!(html.markup.fragment.nodes.get(origin.element as usize),
                            Some(nepl3_markup::html::HtmlNode::Text { text: value }) if value == text)
                    }), "nested annotation mapping for {text}");
                }
                assert!(
                    inner
                        .output
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
                let document = nepl3_suite::adapters::sentence::literal::lower(
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
                    doc_surface: None,
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

#[cfg(target_has_atomic = "ptr")]
#[test]
fn literal_lowering_does_not_rescan_owner_for_every_sentence() -> Result<(), String> {
    let compiled = compiled()?;
    let mut usages = Vec::new();
    for count in [16, 32] {
        for owner_bytes in [100_000, 200_000] {
            let mut input = String::from("article en \"Scaling\" body ");
            for _ in 0..count {
                input.push_str("cons paragraph cons parallel cons variant en \"English.\" cons variant ja \"日本語。\" nil nil ");
            }
            input.push_str("nil");
            // Same document bytes at both sizes isolates per-literal work: the old
            // append path charged two full-owner scans per sentence despite sharing
            // the immutable owner. This is a synthetic source, not user material.
            input.extend(core::iter::repeat_n(' ', owner_bytes - input.len()));
            with_input_route(true, &compiled, &input, "Article", |tree, profile, _, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                let mut lower_budget = budget();
                let doc = nepl3_suite::adapters::sentence::literal::lower(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    &mut lower_budget,
                    &mut codec,
                )
                .map_err(err)?;
                assert_eq!(doc.sources.len(), 1);
                assert_eq!(doc.sources[0].text(), input);
                assert_eq!(
                    doc.value
                        .nodes
                        .iter()
                        .filter(|n| matches!(n.kind, DocKind::Parallel { .. }))
                        .count(),
                    count
                );
                assert_eq!(lower_budget.usage().source_bytes, owner_bytes as u64);
                usages.push(lower_budget.usage().work);
                Ok(())
            })?;
        }
    }
    // Compare padding slope at both literal counts. Extra local validation is
    // legitimate per-literal work, but must not rescan N copies of the owner.
    // Each document has 2*count+1 literals; an N-owner scan would add millions
    // of work units for 100k extra padding, and double its slope at 32 blocks.
    let small_padding = usages[1]
        .checked_sub(usages[0])
        .ok_or("padding work decreased")?;
    let large_padding = usages[3]
        .checked_sub(usages[2])
        .ok_or("padding work decreased")?;
    assert!(
        small_padding <= 200_000 && large_padding <= 200_000,
        "owner padding work: {usages:?}"
    );
    assert!(
        large_padding <= small_padding + 10_000,
        "per-literal owner scan: {usages:?}"
    );
    assert!(
        usages[2] < usages[0] * 3 && usages[3] < usages[1] * 3,
        "superlinear literal work: {usages:?}"
    );
    Ok(())
}

#[test]
fn terminal_parse_observations_preserve_failure_usage_and_phase_boundary() -> Result<(), String> {
    let compiled = compiled()?;
    let input = r#"article en "Observation" body nil"#;
    let mut observations = Vec::new();
    let limits = budget().limits();
    let result = with_named_input_limits_observed(
        true,
        &compiled,
        input,
        "observed",
        "Article",
        limits,
        |value| observations.push(value),
        |_, _, b, _| {
            // Later callback work/failure must not be misreported as failed parsing.
            b.charge(Resource::Work, 17).map_err(err)?;
            Err::<(), _>("later phase failed".into())
        },
    );
    assert_eq!(result, Err("later phase failed".into()));
    assert_eq!(observations.len(), 1);
    let complete = observations[0];
    assert_eq!(complete.outcome, ParseObservationOutcome::Complete);
    assert_eq!(complete.limits, limits);
    assert!(complete.usage.work > 0);
    assert!(complete.profile.is_some());
    let mut success = None;
    let usage_at_boundary = with_named_input_limits_observed(
        true,
        &compiled,
        input,
        "observed",
        "Article",
        limits,
        |value| success = Some(value),
        |_, _, b, _| Ok(b.usage()),
    )?;
    assert_eq!(success, Some(complete));
    assert_eq!(usage_at_boundary, complete.usage);
    for work in [0, 7, 100] {
        let mut stopped = Vec::new();
        let low = Limits { work, ..limits };
        let result = with_named_input_limits_observed(
            true,
            &compiled,
            input,
            "observed",
            "Article",
            low,
            |value| stopped.push(value),
            |_, _, _, _| Err::<(), _>("must not reach callback".into()),
        );
        assert!(result.is_err());
        assert_eq!(stopped.len(), 1);
        assert_eq!(
            stopped[0].outcome,
            ParseObservationOutcome::Stopped(StopReason::WorkLimit)
        );
        assert_eq!(stopped[0].limits, low);
        assert!(stopped[0].usage.work <= work);
    }
    let mut source_stop = None;
    let result = with_named_input_limits_observed(
        true,
        &compiled,
        input,
        "observed",
        "Article",
        Limits {
            source_bytes: 0,
            ..limits
        },
        |value| source_stop = Some(value),
        |_, _, _, _| Ok(()),
    );
    assert!(result.is_err());
    assert_eq!(
        source_stop.ok_or("missing source observation")?.outcome,
        ParseObservationOutcome::Stopped(StopReason::SourceLimit)
    );
    let mut invalid = None;
    let result = with_named_input_limits_observed(
        true,
        &compiled,
        "not-an-article",
        "observed",
        "Article",
        limits,
        |value| invalid = Some(value),
        |_, _, _, _| Ok(()),
    );
    assert!(result.is_err());
    let invalid = invalid.ok_or("missing invalid observation")?;
    assert_eq!(invalid.outcome, ParseObservationOutcome::Failed);
    assert!(invalid.profile.is_some());
    let mut broken = super::compiled()?;
    broken.doc.package.schema.digest.0[0] ^= 1;
    let mut setup_failure = None;
    let result = with_named_input_limits_observed(
        true,
        &broken,
        input,
        "observed",
        "Article",
        limits,
        |value| setup_failure = Some(value),
        |_, _, _, _| Ok(()),
    );
    assert!(result.is_err());
    let setup_failure = setup_failure.ok_or("missing setup failure")?;
    assert_eq!(setup_failure.outcome, ParseObservationOutcome::Failed);
    assert!(setup_failure.profile.is_none());
    Ok(())
}

#[test]
fn foreign_math_owns_local_text_maps_without_inheriting_prior_paragraph_maps() -> Result<(), String>
{
    use nepl3_core::syntax::FieldValue;
    let c = compiled()?;
    let mut routes = Vec::new();
    for native in [false, true] {
        let mut results = Vec::new();
        for count in [0, 4] {
            let source = format!(
                "article en \"Math\" body {}cons display Math symbol \"x\" nil",
                "cons paragraph cons sentence cons text \"unrelated prior text\" nil nil "
                    .repeat(count)
            );
            let observed = with_named_input(
                native,
                &c,
                &source,
                "mapping-scope",
                "Article",
                |tree, _, _, _| {
                    let root = &tree.tree().bundle;
                    let mut pending = vec![root];
                    let mut math_maps = Vec::new();
                    while let Some(bundle) = pending.pop() {
                        for node in &bundle.nodes {
                            for field in &node.fields {
                                if let FieldValue::Foreign(guest) = field {
                                    if guest.schema.package == "standard.math" {
                                        let maps = &guest.bundle.source_maps;
                                        assert!(
                                            !maps.is_empty(),
                                            "the symbol's empty-view builtin Text maps must survive"
                                        );
                                        for mapping in maps {
                                            let target = guest
                                                .bundle
                                                .sources
                                                .iter()
                                                .find(|s| {
                                                    s.identity() == mapping.target.snapshot_ref()
                                                })
                                                .ok_or("mapping target")?;
                                            assert_eq!(
                                                target.text(),
                                                "x",
                                                "retain the symbol's actual decoded snapshot, not unrelated title/text maps"
                                            );
                                            assert_eq!(
                                                target.slice(&mapping.target).map_err(err)?,
                                                "x"
                                            );
                                        }
                                        math_maps.push(maps.clone());
                                    }
                                    pending.push(&guest.bundle);
                                }
                            }
                        }
                    }
                    assert_eq!(math_maps.len(), 1);
                    Ok((root.source_maps.len(), math_maps.remove(0)))
                },
            )?;
            results.push(observed);
        }
        assert_eq!(
            results[0].1.len(),
            results[1].1.len(),
            "older unrelated paragraphs must not enlarge Math provenance"
        );
        assert!(
            results[1].0 > results[0].0,
            "the enclosing operation still retains earlier maps: {results:?}"
        );
        routes.push(results);
    }
    assert_eq!(
        routes[0], routes[1],
        "owned and native routes retain the same exact mapping rows"
    );
    Ok(())
}

#[test]
fn sibling_and_nested_foreign_arenas_keep_independent_local_maps() -> Result<(), String> {
    use nepl3_core::syntax::FieldValue;
    let c = compiled()?;
    let source = r#"article en "A" body cons display Math symbol "x" cons display Math label symbol "y" Sentence sentence cons text "z" nil nil"#;
    let mut routes = Vec::new();
    for native in [false, true] {
        routes.push(with_named_input(
            native,
            &c,
            source,
            "nested-map-scope",
            "Article",
            |tree, _, _, _| {
                let mut pending = vec![&tree.tree().bundle];
                let mut local = Vec::new();
                while let Some(bundle) = pending.pop() {
                    for node in &bundle.nodes {
                        for field in &node.fields {
                            if let FieldValue::Foreign(guest) = field {
                                let mut targets = Vec::new();
                                for mapping in &guest.bundle.source_maps {
                                    let target = guest
                                        .bundle
                                        .sources
                                        .iter()
                                        .find(|s| s.identity() == mapping.target.snapshot_ref())
                                        .ok_or("target")?;
                                    targets.push(target.text().to_owned());
                                }
                                targets.sort();
                                targets.dedup();
                                local.push((guest.schema.package.clone(), targets));
                                pending.push(&guest.bundle);
                            }
                        }
                    }
                }
                assert!(
                    local
                        .iter()
                        .any(|(p, t)| p == "standard.math" && t == &vec!["x".to_owned()])
                );
                assert!(local.iter().any(|(p, t)| p == "standard.math"
                    && t.contains(&"y".to_owned())
                    && !t.contains(&"x".to_owned())));
                assert!(
                    local
                        .iter()
                        .any(|(p, t)| p == "nepl3.syntax.sentence" && t == &vec!["z".to_owned()])
                );
                Ok(local)
            },
        )?);
    }
    assert_eq!(routes[0], routes[1]);
    Ok(())
}
