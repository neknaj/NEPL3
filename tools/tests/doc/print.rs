use super::*;
use nepl3_doc_core::print::{
    self, PrintEntry, PrintFailure, PrintMismatch, PrintMode, PrintOutcome, PrintRequest,
};

fn guest_signature(bundle: &nepl3_core::syntax::SyntaxBundle) -> Vec<(String, Option<NdfValue>)> {
    bundle
        .nodes
        .iter()
        .map(|node| {
            (
                node.kind.clone(),
                node.token
                    .and_then(|id| bundle.tokens.get(id.0 as usize))
                    .map(|token| token.payload.clone()),
            )
        })
        .collect()
}
fn host_request(
    doc: &DocumentSyntax,
    profile: &ResolvedParseProfile<'_>,
    mode: PrintMode,
) -> Result<PrintRequest, String> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
    let identity =
        print::identity(doc, profile.registry(), &mut codec, &mut budget()).map_err(err)?;
    let selected = [
        (GuestLanguage::Math, "Math", "Expr"),
        (GuestLanguage::Circuit, "Circuit", "Design"),
        (GuestLanguage::Grammar, "Grammar", "Root"),
        (GuestLanguage::Doc, "Doc", "Article"),
    ];
    let bindings = selected
        .iter()
        .map(|(language, alias, category)| {
            Ok(print::GuestBinding {
                schema: profile
                    .language(alias, &mut budget())
                    .map_err(err)?
                    .schema
                    .clone(),
                category: (*category).into(),
                language: *language,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut guests = Vec::new();
    for target in &identity.guests {
        let embed = &doc.value.embeds[target.embed.0 as usize];
        let binding = bindings
            .iter()
            .find(|v| v.schema == embed.closure.syntax.schema)
            .ok_or("explicit guest binding")?;
        let (_, alias, category) = selected
            .iter()
            .find(|v| v.0 == binding.language)
            .ok_or("guest language")?;
        // Source retrieval alone is not accepted as printed source. The host
        // reparses it, checks the selected tree, and runs the real engine printer.
        let raw = print::original_guest_source(
            doc,
            target.embed,
            profile.registry(),
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?
        .ok_or("source-less guest in source-backed fixture")?;
        let source = SourceSnapshot::new(
            SourceId(format!("host-guest-{}", target.embed.0)),
            0,
            format!("memory:host-guest/{}", target.embed.0),
            raw.as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(err)?;
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let tree = parse_source_as(&source, profile, alias, category, &mut b, &mut a)?;
        let proof = tree.validate(profile, &mut b, &mut a).map_err(err)?;
        assert_eq!(
            guest_signature(&embed.closure.syntax.bundle),
            guest_signature(&tree.bundle)
        );
        let reply = nepl3_engine::parse::print::source_tree(&proof, &mut b, &mut a).map_err(err)?;
        let nepl3_engine::parse::print::PrintOutcome::Complete(text) = reply.outcome else {
            return Err("host guest print stopped".into());
        };
        guests.push(print::PrintedGuest {
            document_digest: identity.document_digest,
            embed: target.embed,
            guest_digest: target.guest_digest,
            text,
        });
    }
    Ok(PrintRequest {
        document: doc.clone(),
        mode,
        bindings,
        guests,
    })
}
use nepl3_doc_core::{
    check::{Category, ShapeError},
    lower,
    model::*,
};

fn entry(value: PrintEntry) -> (&'static str, Category) {
    match value {
        PrintEntry::Article => ("Article", Category::Article),
        PrintEntry::Body => ("Body", Category::Body),
        PrintEntry::Block => ("Block", Category::Block),
        PrintEntry::Flow => ("Flow", Category::Flow),
        PrintEntry::Sentence => ("Sentence", Category::Sentence),
        PrintEntry::Inline => ("Inline", Category::Inline),
        PrintEntry::Variant => ("Variant", Category::Variant),
        PrintEntry::Row => ("Row", Category::Row),
        PrintEntry::ListItem => ("ListItem", Category::ListItem),
        PrintEntry::Alignment => ("Alignment", Category::Alignment),
        PrintEntry::ListStyle => ("ListStyle", Category::ListStyle),
        PrintEntry::Check => ("Check", Category::Check),
        PrintEntry::LinkTarget => ("LinkTarget", Category::Target),
        PrintEntry::Asset => ("Asset", Category::Asset),
        PrintEntry::OptionalRow => ("OptionalRow", Category::OptionalRow),
        PrintEntry::OptionalSentence => ("OptionalSentence", Category::OptionalSentence),
        PrintEntry::OptionalText => ("OptionalText", Category::OptionalText),
        PrintEntry::MathGuest => ("MathGuest", Category::MathGuest),
        PrintEntry::CircuitGuest => ("CircuitGuest", Category::CircuitGuest),
        PrintEntry::Guest => ("Guest", Category::Guest),
    }
}

#[test]
fn prefix_and_compact_print_reparse_real_doc_annotation_and_explicit_break() -> Result<(), String> {
    let compiled = compiled()?;
    for input in [
        r#"article en "Title" body cons paragraph cons "[字/じ]{base/note/[n/r]}" nil nil"#,
        r#"article en "Title" body cons paragraph cons sentence cons text "a\n" cons break cons strong text "b" nil nil nil"#,
    ] {
        with_input(&compiled, input, "Article", |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let original = lower::document(
                &checked,
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            for mode in [PrintMode::Prefix, PrintMode::Compact] {
                let request = PrintRequest {
                    document: original.clone(),
                    mode,
                    bindings: vec![],
                    guests: vec![],
                };
                let reply = print::print(&request, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?;
                let PrintOutcome::Complete { artifact } = reply.outcome else {
                    return Err(format!("print: {reply:?}"));
                };
                if mode == PrintMode::Compact && input.contains("strong") {
                    assert!(artifact.text.contains("break"));
                    assert!(artifact.text.contains("strong"));
                }
                let (surface, category) = entry(artifact.entry);
                with_input(&compiled, &artifact.text, surface, |tree, profile, b, a| {
                    let checked = tree
                        .tree()
                        .bundle
                        .validate_with_sources(profile.registry(), b, a)
                        .map_err(err)?;
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &empty, &mut admission)
                            .map_err(err)?;
                    let actual = lower::document(
                        &checked,
                        &compiled.doc.package.schema,
                        category,
                        profile.registry(),
                        &mut budget(),
                        &mut codec,
                    )
                    .map_err(err)?;
                    assert_eq!(original.value.root, actual.value.root);
                    assert_eq!(
                        original
                            .value
                            .nodes
                            .iter()
                            .map(|n| &n.kind)
                            .collect::<Vec<_>>(),
                        actual
                            .value
                            .nodes
                            .iter()
                            .map(|n| &n.kind)
                            .collect::<Vec<_>>()
                    );
                    Ok(())
                })?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn every_form_prints_through_real_parser_and_typed_first_receiver() -> Result<(), String> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../conformance/fixtures/doc/print.json"))
            .map_err(err)?;
    let cases = fixture["cases"].as_array().ok_or("case array")?;
    let forms: serde_json::Value =
        serde_json::from_str(include_str!("../../../design/forms.json")).map_err(err)?;
    let mut expected = std::collections::BTreeSet::new();
    for (category, value) in forms["categories"].as_object().ok_or("categories")? {
        if let Some(category) = category.strip_prefix("Doc/") {
            for form in value["forms"].as_object().ok_or("forms")?.values() {
                expected.insert((
                    category.to_owned(),
                    form["kind"].as_str().ok_or("kind")?.to_owned(),
                ));
            }
        }
    }
    let actual = cases
        .iter()
        .map(|v| {
            Ok((
                v["entry"].as_str().ok_or("entry")?.to_owned(),
                v["kind"].as_str().ok_or("kind")?.to_owned(),
            ))
        })
        .collect::<Result<std::collections::BTreeSet<_>, String>>()?;
    assert_eq!(actual, expected);
    assert_eq!(cases.len(), 64);
    let compiled = compiled()?;
    let entries = [
        PrintEntry::Article,
        PrintEntry::Body,
        PrintEntry::Block,
        PrintEntry::Flow,
        PrintEntry::Sentence,
        PrintEntry::Inline,
        PrintEntry::Variant,
        PrintEntry::Row,
        PrintEntry::ListItem,
        PrintEntry::Alignment,
        PrintEntry::ListStyle,
        PrintEntry::Check,
        PrintEntry::LinkTarget,
        PrintEntry::Asset,
        PrintEntry::OptionalRow,
        PrintEntry::OptionalSentence,
        PrintEntry::OptionalText,
        PrintEntry::MathGuest,
        PrintEntry::CircuitGuest,
        PrintEntry::Guest,
    ];
    for case in cases {
        let source = case["source"].as_str().ok_or("case source")?;
        let category = case["entry"].as_str().ok_or("case entry")?;
        let chosen = *entries
            .iter()
            .find(|&&v| entry(v).0 == category)
            .ok_or("surface category")?;
        with_input(&compiled, source, category, |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let doc = lower::document(
                &checked,
                &compiled.doc.package.schema,
                entry(chosen).1,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            for mode in [PrintMode::Prefix, PrintMode::Compact] {
                let request = host_request(&doc, profile, mode)?;
                let native = print::print(&request, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?;
                let wire = nepl3_doc_core::portable::print::request_to_value(
                    &request,
                    profile.registry(),
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                let bytes = nepl3_wire::encode(&wire, &mut budget()).map_err(err)?;
                let mut admission = SourceAdmission::default();
                let mut receiver = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let request = nepl3_doc_core::portable::print::request_from_value(
                    &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                    profile.registry(),
                    &mut receiver,
                    &mut budget(),
                )
                .map_err(err)?;
                let received =
                    print::print(&request, profile.registry(), &mut receiver, &mut budget())
                        .map_err(err)?;
                assert_eq!(native.outcome, received.outcome);
                let wire = nepl3_doc_core::portable::print::reply_to_value(
                    &received,
                    &request.document,
                    profile.registry(),
                    &mut receiver,
                    &mut budget(),
                )
                .map_err(err)?;
                assert_eq!(
                    nepl3_doc_core::portable::print::reply_from_value(
                        &wire,
                        &request.document,
                        profile.registry(),
                        &mut receiver,
                        &mut budget()
                    )
                    .map_err(err)?,
                    received
                );
                let PrintOutcome::Complete { artifact } = received.outcome else {
                    return Err(format!(
                        "print failed for {category}: {source}: {received:?}"
                    ));
                };
                assert_eq!(artifact.entry, chosen);
                with_input(
                    &compiled,
                    &artifact.text,
                    category,
                    |tree, profile, b, a| {
                        let checked = tree
                            .tree()
                            .bundle
                            .validate_with_sources(profile.registry(), b, a)
                            .map_err(err)?;
                        let mut admission = SourceAdmission::default();
                        let mut codec =
                            FoundationCodec::new(profile.registry(), &empty, &mut admission)
                                .map_err(err)?;
                        let result = lower::document(
                            &checked,
                            &compiled.doc.package.schema,
                            entry(chosen).1,
                            profile.registry(),
                            &mut budget(),
                            &mut codec,
                        )
                        .map_err(err)?;
                        assert_eq!(doc.value.root, result.value.root, "{category}: {source}");
                        assert_eq!(
                            doc.value.nodes.iter().map(|n| &n.kind).collect::<Vec<_>>(),
                            result
                                .value
                                .nodes
                                .iter()
                                .map(|n| &n.kind)
                                .collect::<Vec<_>>(),
                            "{category}: {source}"
                        );
                        for (expected, actual) in doc.value.embeds.iter().zip(&result.value.embeds)
                        {
                            assert_eq!(expected.kind, actual.kind);
                            assert_eq!(
                                guest_signature(&expected.closure.syntax.bundle),
                                guest_signature(&actual.closure.syntax.bundle)
                            );
                        }
                        Ok(())
                    },
                )?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn printer_rejects_unprintable_source_less_names_and_languages_without_changing_values()
-> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        r#"article en sentence nil body cons section sectionName sentence nil body nil nil"#,
        "Article",
        |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let mut doc = lower::document(
                &checked,
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            for node in &mut doc.value.nodes {
                node.span = None;
                node.origin = None;
                node.locations.clear();
            }
            doc.sources.clear();
            doc.origins.clear();
            doc.views.clear();
            doc.source_maps.clear();
            for (value, valid) in [
                ("", false),
                ("a b", false),
                ("0start", false),
                ("x-y", false),
                ("日本語", true),
                ("_name", true),
                ("section", true),
            ] {
                let mut changed = doc.clone();
                let node = changed
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::Section { .. }))
                    .ok_or("section")?;
                if let DocKind::Section { id, .. } = &mut changed.value.nodes[node].kind {
                    *id = value.into();
                }
                let saved = changed.clone();
                let request = PrintRequest {
                    document: changed,
                    mode: PrintMode::Prefix,
                    bindings: vec![],
                    guests: vec![],
                };
                let reply = print::print(&request, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?;
                if valid {
                    assert!(matches!(reply.outcome, PrintOutcome::Complete { .. }));
                } else {
                    assert_eq!(
                        reply.outcome,
                        PrintOutcome::Invalid {
                            error: print::PrintFailure::UnprintableName {
                                node: node as u64,
                                field: DocField::SectionId
                            }
                        }
                    );
                }
                assert_eq!(request.document, saved);
            }
            for (language, valid) in [
                ("", false),
                ("en_US", false),
                ("ja 日本語", false),
                ("en-a-foo-a-bar", true),
                ("i-klingon", true),
                ("x-private", true),
            ] {
                let mut changed = doc.clone();
                let node = changed
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::Article { .. }))
                    .ok_or("article")?;
                if let DocKind::Article {
                    language: value, ..
                } = &mut changed.value.nodes[node].kind
                {
                    *value = language.into();
                }
                let request = PrintRequest {
                    document: changed,
                    mode: PrintMode::Compact,
                    bindings: vec![],
                    guests: vec![],
                };
                let reply = print::print(&request, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?;
                if valid {
                    assert!(matches!(reply.outcome, PrintOutcome::Complete { .. }));
                } else {
                    assert_eq!(
                        reply.outcome,
                        PrintOutcome::Invalid {
                            error: print::PrintFailure::UnprintableLanguage { node: node as u64 }
                        }
                    );
                }
            }
            Ok(())
        },
    )
}

#[test]
fn printer_stops_preserve_request_and_report_at_preparation_and_output_boundaries()
-> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        r#"sentence cons ruby text "base" text "reading" cons anno text "base" cons text "note" nil nil"#,
        "Sentence",
        |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let doc = lower::document(
                &checked,
                &compiled.doc.package.schema,
                Category::Sentence,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let request = PrintRequest {
                document: doc.clone(),
                mode: PrintMode::Compact,
                bindings: vec![],
                guests: vec![],
            };
            let mut full = budget();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let baseline =
                print::print(&request, profile.registry(), &mut c, &mut full).map_err(err)?;
            assert!(matches!(baseline.outcome, PrintOutcome::Complete { .. }));
            let u = full.usage();
            let mut stopped = 0;
            for (reason, used) in [
                (StopReason::SourceLimit, u.source_bytes),
                (StopReason::WorkLimit, u.work),
                (StopReason::NodeLimit, u.nodes),
                (StopReason::AllocationLimit, u.allocation_units),
                (StopReason::DepthLimit, u.depth),
                (StopReason::OutputLimit, u.output_bytes),
            ] {
                for cap in [0, used / 2, used.saturating_sub(1), used] {
                    let mut limits = budget().limits();
                    match reason {
                        StopReason::SourceLimit => limits.source_bytes = cap,
                        StopReason::WorkLimit => limits.work = cap,
                        StopReason::NodeLimit => limits.nodes = cap,
                        StopReason::AllocationLimit => limits.allocation_units = cap,
                        StopReason::DepthLimit => limits.depth = cap,
                        StopReason::OutputLimit => limits.output_bytes = cap,
                        _ => {}
                    }
                    let mut b = Budget::new(limits);
                    let mut a = SourceAdmission::default();
                    let mut c =
                        FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
                    let reply =
                        print::print(&request, profile.registry(), &mut c, &mut b).map_err(err)?;
                    match reply.outcome {
                        PrintOutcome::Stopped { reason: actual } => {
                            assert_eq!(actual, reason);
                            assert_eq!(b.poll(), Err(reason));
                            stopped += 1;
                        }
                        _ => assert_eq!(reply.outcome, baseline.outcome),
                    }
                    assert_eq!(reply.report.usage, b.usage());
                    assert_eq!(request.document, doc);
                    let wire = nepl3_doc_core::portable::print::reply_to_value(
                        &reply,
                        &doc,
                        profile.registry(),
                        &mut codec,
                        &mut budget(),
                    )
                    .map_err(err)?;
                    assert_eq!(
                        nepl3_doc_core::portable::print::reply_from_value(
                            &wire,
                            &doc,
                            profile.registry(),
                            &mut codec,
                            &mut budget()
                        )
                        .map_err(err)?,
                        reply
                    );
                }
            }
            assert!(stopped >= 12);
            let mut b = budget();
            b.cancel();
            assert_eq!(
                print::print(&request, profile.registry(), &mut codec, &mut b)
                    .map_err(err)?
                    .outcome,
                PrintOutcome::Stopped {
                    reason: StopReason::Cancelled
                }
            );
            let mut b = budget();
            let result = b
                .with_depth_at_least(7, |b| {
                    print::print(&request, profile.registry(), &mut codec, b)
                })
                .map_err(err)?;
            assert_eq!(result.outcome, baseline.outcome);
            assert_eq!(b.current_depth(), 0);
            assert!(b.usage().depth >= u.depth + 7);
            Ok(())
        },
    )
}

#[test]
fn standalone_guest_fragments_keep_concrete_entry_and_foreign_syntax() -> Result<(), String> {
    let compiled = compiled()?;
    let cases = [
        (
            "Math add 1 2",
            "MathGuest",
            Category::MathGuest,
            GuestLanguage::Math,
        ),
        (
            "Circuit design nil Main nil",
            "CircuitGuest",
            Category::CircuitGuest,
            GuestLanguage::Circuit,
        ),
        (
            "Math add 1 2",
            "Guest",
            Category::Guest,
            GuestLanguage::Math,
        ),
        (
            "Circuit design nil Main nil",
            "Guest",
            Category::Guest,
            GuestLanguage::Circuit,
        ),
        (
            "Grammar language Tiny 1 Expr nil",
            "Guest",
            Category::Guest,
            GuestLanguage::Grammar,
        ),
        (
            r#"Doc article en sentence cons anno text "" cons text "note" nil nil body nil"#,
            "Guest",
            Category::Guest,
            GuestLanguage::Doc,
        ),
    ];
    for (source, entry, category, language) in cases {
        with_input(&compiled, source, entry, |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let doc = lower::document(
                &checked,
                &compiled.doc.package.schema,
                category,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            assert_eq!(doc.value.nodes.len(), 1);
            assert_eq!(doc.value.embeds.len(), 1);
            assert_eq!(
                doc.value.nodes[0].kind,
                DocKind::Guest {
                    language,
                    syntax: EmbedRef(0)
                }
            );
            assert_eq!(doc.value.embeds[0].kind, EmbedKind::Guest);
            assert_eq!(
                doc.value.root,
                match category {
                    Category::MathGuest => DocRoot::MathGuest(GuestRef(0)),
                    Category::CircuitGuest => DocRoot::CircuitGuest(GuestRef(0)),
                    _ => DocRoot::Guest(GuestRef(0)),
                }
            );
            // The Grammar has no declared Expr; the Doc annotation is invalid.
            // Both are source syntax fragments, not guest semantic lower results.
            let value = nepl3_doc_core::portable::to_value(
                &doc,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            let mut admission = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let actual = nepl3_doc_core::portable::from_value(
                &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                profile.registry(),
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            assert_doc_retention(&doc, &actual);
            let mut wrong = doc.value.clone();
            wrong.root = if language == GuestLanguage::Math {
                DocRoot::CircuitGuest(GuestRef(0))
            } else {
                DocRoot::MathGuest(GuestRef(0))
            };
            assert!(matches!(
                wrong.validate_shape(&mut budget()),
                Err(ShapeError::Category { .. })
            ));
            wrong.root = DocRoot::Guest(GuestRef(u64::MAX));
            assert!(matches!(
                wrong.validate_shape(&mut budget()),
                Err(ShapeError::Reference(_))
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn printer_requires_explicit_current_guest_print_and_binding_at_first_receiver()
-> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "sentence cons math Math add 1 2 cons math Math add 3 4 nil",
        "Sentence",
        |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let doc = lower::document(
                &checked,
                &compiled.doc.package.schema,
                Category::Sentence,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let request = host_request(&doc, profile, PrintMode::Prefix)?;
            assert_eq!(request.guests.len(), 2);
            let mut cases = Vec::new();
            let mut changed = request.clone();
            changed.bindings.clear();
            cases.push((changed, PrintFailure::MissingBinding { embed: EmbedRef(0) }));
            let mut changed = request.clone();
            changed.bindings.push(changed.bindings[0].clone());
            cases.push((changed, PrintFailure::ConflictingBinding { binding: 4 }));
            let mut changed = request.clone();
            let mut conflicting = changed.bindings[0].clone();
            conflicting.schema.digest = nepl3_core::source::Digest([0; 32]);
            changed.bindings.push(conflicting);
            // One surface alias cannot select two different schemas in one Profile.
            cases.push((changed, PrintFailure::ConflictingBinding { binding: 4 }));
            let mut changed = request.clone();
            changed.bindings[0].category = "Design".into();
            cases.push((changed, PrintFailure::InvalidBinding { binding: 0 }));
            let mut changed = request.clone();
            changed.bindings[0].language = GuestLanguage::Circuit;
            changed.bindings[0].category = "Design".into();
            changed.bindings.remove(1);
            cases.push((changed, PrintFailure::GuestCategory { embed: EmbedRef(0) }));
            let mut changed = request.clone();
            changed.guests.clear();
            // A real retained source cover does not authorize automatic guest printing.
            cases.push((
                changed,
                PrintFailure::UnresolvedGuest { embed: EmbedRef(0) },
            ));
            for (reason, mode) in [
                (PrintMismatch::Document, 0),
                (PrintMismatch::Guest, 1),
                (PrintMismatch::Embed, 2),
                (PrintMismatch::Embed, 3),
            ] {
                let mut changed = request.clone();
                match mode {
                    0 => changed.guests[0].document_digest = nepl3_core::source::Digest([0; 32]),
                    1 => changed.guests[0].guest_digest = changed.guests[1].guest_digest,
                    2 => changed.guests[0].embed = EmbedRef(u64::MAX),
                    _ => changed.guests[0].embed = EmbedRef(1_u64 << 32),
                }
                cases.push((changed, PrintFailure::InvalidGuest { entry: 0, reason }));
            }
            let mut changed = request.clone();
            changed.guests.push(changed.guests[0].clone());
            cases.push((
                changed,
                PrintFailure::InvalidGuest {
                    entry: 2,
                    reason: PrintMismatch::Duplicate,
                },
            ));
            for (request, error) in cases {
                let wire = nepl3_doc_core::portable::print::request_to_value(
                    &request,
                    profile.registry(),
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                let bytes = nepl3_wire::encode(&wire, &mut budget()).map_err(err)?;
                let mut a = SourceAdmission::default();
                let mut receiver =
                    FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
                let actual = nepl3_doc_core::portable::print::request_from_value(
                    &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                    profile.registry(),
                    &mut receiver,
                    &mut budget(),
                )
                .map_err(err)?;
                let reply = print::print(&actual, profile.registry(), &mut receiver, &mut budget())
                    .map_err(err)?;
                assert_eq!(reply.outcome, PrintOutcome::Invalid { error });
                assert_eq!(request, actual);
            }
            let mut synthetic = request.clone();
            let identity = print::identity(&doc, profile.registry(), &mut codec, &mut budget())
                .map_err(err)?;
            let identity_value = nepl3_doc_core::portable::print::identity_to_value(
                &identity,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            for length in [31, 33] {
                let mut malformed = identity_value.clone();
                let NdfValue::Record(record) = &mut malformed else {
                    return Err("identity record".into());
                };
                record.fields[0] = NdfValue::Bytes(vec![0; length]);
                assert!(
                    nepl3_doc_core::portable::print::identity_from_value(
                        &malformed,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    )
                    .is_err()
                );
            }
            let mut reply = print::print(&request, profile.registry(), &mut codec, &mut budget())
                .map_err(err)?;
            let complete_value = nepl3_doc_core::portable::print::reply_to_value(
                &reply,
                &doc,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            reply.report.trace_overflow =
                Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 });
            assert!(matches!(
                nepl3_doc_core::portable::print::reply_to_value(
                    &reply,
                    &doc,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(nepl3_doc_core::portable::PortableError::Shape)
            ));
            for reason in [StopReason::Cancelled, StopReason::WorkLimit] {
                reply.outcome = PrintOutcome::Stopped { reason };
                let value = nepl3_doc_core::portable::print::reply_to_value(
                    &reply,
                    &doc,
                    profile.registry(),
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                assert_eq!(
                    nepl3_doc_core::portable::print::reply_from_value(
                        &value,
                        &doc,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    )
                    .map_err(err)?,
                    reply
                );
                let mut malformed = complete_value.clone();
                let (NdfValue::Record(record), NdfValue::Record(stopped)) =
                    (&mut malformed, &value)
                else {
                    return Err("reply record".into());
                };
                record.fields[1] = stopped.fields[1].clone();
                assert!(matches!(
                    nepl3_doc_core::portable::print::reply_from_value(
                        &malformed,
                        &doc,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    ),
                    Err(nepl3_doc_core::portable::PortableError::Shape)
                ));
            }
            let bundle = &mut synthetic.document.value.embeds[0].closure.syntax.bundle;
            let root = bundle.root.0 as usize;
            bundle.nodes[root].cover = None;
            bundle.nodes[root].head = None;
            assert!(
                print::original_guest_source(
                    &synthetic.document,
                    EmbedRef(0),
                    profile.registry(),
                    &mut budget(),
                    &mut SourceAdmission::default()
                )
                .map_err(err)?
                .is_none()
            );
            // A host can print a synthetic root explicitly, but the removed
            // cover invalidates the previously issued document identity.
            assert_eq!(
                print::print(&synthetic, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?
                    .outcome,
                PrintOutcome::Invalid {
                    error: PrintFailure::InvalidGuest {
                        entry: 0,
                        reason: PrintMismatch::Document
                    }
                }
            );
            let current = print::identity(
                &synthetic.document,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            for (guest, target) in synthetic.guests.iter_mut().zip(&current.guests) {
                guest.document_digest = current.document_digest;
                guest.guest_digest = target.guest_digest;
            }
            assert!(matches!(
                print::print(&synthetic, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?
                    .outcome,
                PrintOutcome::Complete { .. }
            ));
            Ok(())
        },
    )
}

#[test]
fn printer_expands_shared_paths_with_bounded_output_and_deep_cleanup() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(&compiled, "sentence nil", "Sentence", |_, profile, _, _| {
        let node = |kind| DocNode {
            locations: vec![],
            kind,
            origin: None,
            span: None,
        };
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        for (depth, shared) in [(512, false), (20, true)] {
            let mut nodes = vec![node(DocKind::Text { text: "x".into() })];
            for id in 0..depth {
                nodes.push(node(if shared {
                    DocKind::Concat {
                        inlines: vec![InlineRef(id), InlineRef(id)],
                    }
                } else {
                    DocKind::Strong {
                        inline: InlineRef(id),
                    }
                }));
            }
            let document = DocumentSyntax {
                value: DocValue {
                    root: DocRoot::Inline(InlineRef(depth)),
                    nodes,
                    embeds: vec![],
                },
                sources: vec![],
                origins: vec![],
                views: vec![],
                source_maps: vec![],
            };
            for mode in [PrintMode::Prefix, PrintMode::Compact] {
                let request = PrintRequest {
                    document: document.clone(),
                    mode,
                    bindings: vec![],
                    guests: vec![],
                };
                let mut preparation = budget();
                preparation = Budget::new(Limits {
                    depth: 2000,
                    ..preparation.limits()
                });
                print::identity(&document, profile.registry(), &mut codec, &mut preparation)
                    .map_err(err)?;
                let preparation_output = preparation.usage().output_bytes;
                let mut limits = budget().limits();
                limits.depth = 2000;
                limits.output_bytes = if shared {
                    preparation_output + 1024
                } else {
                    1_000_000
                };
                let mut b = Budget::new(limits);
                let reply =
                    print::print(&request, profile.registry(), &mut codec, &mut b).map_err(err)?;
                if shared {
                    assert_eq!(
                        reply.outcome,
                        PrintOutcome::Stopped {
                            reason: StopReason::OutputLimit
                        }
                    );
                    assert_eq!(b.poll(), Err(StopReason::OutputLimit));
                    assert!(b.usage().output_bytes > preparation_output);
                    assert!(b.usage().output_bytes <= preparation_output + 1024);
                } else {
                    let PrintOutcome::Complete { artifact } = reply.outcome else {
                        return Err(format!("deep prefix unexpectedly stopped: {reply:?}"));
                    };
                    assert_eq!(artifact.text.matches("strong ").count(), depth as usize);
                    assert!(artifact.text.ends_with("text \"x\""));
                }
                assert_eq!(request.document, document);
                let mut limits = budget().limits();
                limits.depth = 8;
                let mut b = Budget::new(limits);
                assert_eq!(
                    print::print(&request, profile.registry(), &mut codec, &mut b)
                        .map_err(err)?
                        .outcome,
                    PrintOutcome::Stopped {
                        reason: StopReason::DepthLimit
                    }
                );
            }
        }
        Ok(())
    })
}
