use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_core::value::NdfValue;
use nepl3_doc_core::{
    check::{Category, ShapeError},
    lower,
    model::*,
    print::{self, PrintEntry, PrintFailure, PrintMismatch, PrintMode, PrintOutcome},
};
use nepl3_tools::doc::source::{budget, compiled, err, with_input};
use nepl3_wire::foundation::FoundationCodec;
#[path = "doc/print/host.rs"]
mod host;
use host::host_request;
#[path = "doc/retention.rs"]
mod retention;
use retention::assert_doc_retention;

#[test]
fn prefix_and_compact_print_reparse_real_doc_annotation_and_explicit_break() -> Result<(), String> {
    let compiled = compiled()?;
    for input in [
        r#"article en sentence "Title" body cons paragraph cons sentence "[字/じ]{base/note/[n/r]}" nil nil"#,
        r#"article en sentence "Title" body cons paragraph cons sentence sentence cons text "a\n" cons break cons strong text "b" nil nil nil"#,
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
                let request = host_request(&original, profile, mode)?;
                let reply = print::print(&request, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?;
                let PrintOutcome::Complete { artifact } = reply.outcome else {
                    return Err(format!("print: {reply:?}"));
                };
                if mode == PrintMode::Compact && input.contains("strong") {
                    assert!(artifact.text.contains("break"));
                    assert!(artifact.text.contains("strong"));
                }
                assert_eq!(artifact.entry, PrintEntry::Article);
                let (surface, category) = ("Article", Category::Article);
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
                    assert_eq!(original.value.embeds.len(), actual.value.embeds.len());
                    for (before, after) in original.value.embeds.iter().zip(&actual.value.embeds) {
                        let before = nepl3_suite::adapters::document::sentence::lower(
                            before,
                            before.schema(),
                            &[],
                            profile.registry(),
                            &mut codec,
                            &mut budget(),
                        )
                        .map_err(err)?;
                        let after = nepl3_suite::adapters::document::sentence::lower(
                            after,
                            after.schema(),
                            &[],
                            profile.registry(),
                            &mut codec,
                            &mut budget(),
                        )
                        .map_err(err)?;
                        assert_eq!(before.value, after.value);
                    }
                    Ok(())
                })?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn printer_stops_preserve_request_and_report_at_preparation_and_output_boundaries()
-> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        r#"sentence sentence cons ruby text "base" text "reading" cons anno text "base" cons text "note" nil nil"#,
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
            let request = host_request(&doc, profile, PrintMode::Compact)?;
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
            r#"Doc article en sentence sentence cons anno text "" cons text "note" nil nil body nil"#,
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
            assert_doc_retention(&doc, &actual)?;
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

#[path = "doc/print/entry.rs"]
mod entry;
use entry::entry;

#[test]
fn every_form_prints_through_real_parser_and_typed_first_receiver() -> Result<(), String> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../conformance/fixtures/doc/print.json"))
            .map_err(err)?;
    let cases = fixture["cases"].as_array().ok_or("case array")?;
    let forms: serde_json::Value =
        serde_json::from_str(include_str!("../../design/forms.json")).map_err(err)?;
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
    assert_eq!(cases.len(), 55);
    let sentence_cases = fixture["sentence_cases"]
        .as_array()
        .ok_or("sentence cases")?;
    assert_eq!(sentence_cases.len(), 9);
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
    for case in cases.iter().chain(sentence_cases) {
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
                        assert_eq!(doc.value.embeds.len(), result.value.embeds.len());
                        for (expected, actual) in doc.value.embeds.iter().zip(&result.value.embeds)
                        {
                            assert_eq!(expected.kind, actual.kind);
                            if matches!(
                                expected.kind,
                                EmbedKind::Sentence | EmbedKind::SentenceInline
                            ) {
                                let before = nepl3_suite::adapters::document::sentence::lower(
                                    expected,
                                    expected.schema(),
                                    &[],
                                    profile.registry(),
                                    &mut codec,
                                    &mut budget(),
                                )
                                .map_err(err)?;
                                let after = nepl3_suite::adapters::document::sentence::lower(
                                    actual,
                                    actual.schema(),
                                    &[],
                                    profile.registry(),
                                    &mut codec,
                                    &mut budget(),
                                )
                                .map_err(err)?;
                                assert_eq!(before.value, after.value);
                            } else {
                                assert_eq!(
                                    host::guest_signature(
                                        &expected.syntax().ok_or("source syntax")?.syntax.bundle
                                    ),
                                    host::guest_signature(
                                        &actual.syntax().ok_or("source syntax")?.syntax.bundle
                                    )
                                );
                            }
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
        r#"article en sentence sentence nil body cons section sectionName sentence sentence nil body nil nil"#,
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
                let request = host_request(&changed, profile, PrintMode::Prefix)?;
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
                let request = host_request(&changed, profile, PrintMode::Compact)?;
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
fn printer_requires_explicit_current_guest_print_and_binding_at_first_receiver()
-> Result<(), String> {
    let compiled = compiled()?;
    for source in [
        "body cons display Math add 1 2 cons display Math add 3 4 nil",
        r#"body cons paragraph cons sentence "first" cons sentence "second" nil nil"#,
    ] {
        with_input(&compiled, source, "Body", |tree, profile, b, a| {
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
                Category::Body,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let request = host_request(&doc, profile, PrintMode::Prefix)?;
            assert_eq!(request.guests.len(), 2);
            let binding_index = request
                .bindings
                .iter()
                .position(|binding| {
                    &binding.schema == doc.value.embeds[0].schema()
                        && binding.category == doc.value.embeds[0].category()
                })
                .ok_or("selected binding")?;
            let mut cases = Vec::new();
            let mut changed = request.clone();
            changed.bindings.clear();
            cases.push((changed, PrintFailure::MissingBinding { embed: EmbedRef(0) }));
            let mut changed = request.clone();
            changed
                .bindings
                .push(changed.bindings[binding_index].clone());
            cases.push((changed, PrintFailure::ConflictingBinding { binding: 6 }));
            let mut changed = request.clone();
            let mut conflicting = changed.bindings[binding_index].clone();
            conflicting.schema.digest = nepl3_core::source::Digest([0; 32]);
            changed.bindings.push(conflicting);
            // One surface alias cannot select two different schemas in one Profile.
            cases.push((changed, PrintFailure::ConflictingBinding { binding: 6 }));
            let mut changed = request.clone();
            changed.bindings[binding_index].category = "Design".into();
            cases.push((
                changed,
                PrintFailure::InvalidBinding {
                    binding: binding_index as u64,
                },
            ));
            let mut changed = request.clone();
            changed.bindings[binding_index].language = GuestLanguage::Circuit;
            changed.bindings[binding_index].category = "Design".into();
            if let Some(inline) = changed.bindings.iter().position(|binding| {
                binding.language == GuestLanguage::Sentence
                    && binding.category == "Inline"
                    && binding.schema == changed.bindings[binding_index].schema
            }) {
                changed.bindings.remove(inline);
            }
            changed.bindings.remove(1);
            // Binding selection includes category: the selected guest has
            // no binding after its only schema entry is changed to Design.
            cases.push((changed, PrintFailure::MissingBinding { embed: EmbedRef(0) }));
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
            let DocContent::Syntax { closure } = &mut synthetic.document.value.embeds[0].content
            else {
                return Err("source-backed guest".into());
            };
            let bundle = &mut closure.syntax.bundle;
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
        })?;
    }
    Ok(())
}

#[test]
fn fragment_preserves_shared_foreign_syntax_and_rejects_invalid_inputs() -> Result<(), String> {
    use nepl3_doc_core::check::StructureError;
    let compiled = compiled()?;
    with_input(
        &compiled,
        "body cons display Math add 1 2 cons display Math add 3 4 cons display Math add 5 6 nil",
        "Body",
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
                Category::Body,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let DocRoot::Body(body) = doc.value.root else {
                return Err("Body root".into());
            };
            let DocKind::Body { ref blocks } = doc.value.nodes[body.0 as usize].kind else {
                return Err("Body node".into());
            };
            let selected = blocks[1];
            let DocKind::DisplayMath { syntax } = doc.value.nodes[selected.0 as usize].kind else {
                return Err("math".into());
            };
            let retained = doc.value.embeds[syntax.0 as usize].clone();
            let selected_node = doc.value.nodes[selected.0 as usize].clone();
            let shared_embed = FlowRef(doc.value.nodes.len() as u64);
            doc.value.nodes.push(selected_node.clone());
            let paragraph = BlockRef(doc.value.nodes.len() as u64);
            doc.value.nodes.push(DocNode {
                kind: DocKind::Paragraph {
                    items: vec![FlowRef(selected.0), shared_embed, FlowRef(selected.0)],
                },
                locations: vec![],
                origin: None,
                span: None,
            });
            let DocKind::Body { ref mut blocks } = doc.value.nodes[body.0 as usize].kind else {
                return Err("Body node".into());
            };
            blocks[1] = paragraph;
            let before = doc.clone();
            let root = DocRoot::Block(paragraph);
            let mut complete = budget();
            let result = doc
                .fragment(
                    root,
                    profile.registry(),
                    &mut complete,
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
            assert_eq!(doc, before);
            assert_eq!(result.value.root, DocRoot::Block(BlockRef(2)));
            assert_eq!(result.value.nodes.len(), 3);
            let mut expected_node = selected_node;
            expected_node.kind = DocKind::DisplayMath {
                syntax: EmbedRef(0),
            };
            assert_eq!(result.value.nodes[0], expected_node);
            assert_eq!(result.value.nodes[1], expected_node);
            assert_eq!(
                result.value.nodes[2].kind,
                DocKind::Paragraph {
                    items: vec![FlowRef(0), FlowRef(1), FlowRef(0)]
                }
            );
            assert_eq!(result.value.embeds, vec![retained]);
            assert_eq!(result.sources, doc.sources);
            assert_eq!(result.origins, doc.origins);
            assert_eq!(result.views, doc.views);
            assert_eq!(result.source_maps, doc.source_maps);
            for (invalid, expected) in [
                (
                    DocRoot::Block(BlockRef(u64::MAX)),
                    ShapeError::Reference(u64::MAX),
                ),
                (
                    DocRoot::Article(ArticleRef(paragraph.0)),
                    ShapeError::Category {
                        node: paragraph.0,
                        expected: Category::Article,
                    },
                ),
            ] {
                assert_eq!(
                    doc.fragment(
                        invalid,
                        profile.registry(),
                        &mut budget(),
                        &mut SourceAdmission::default()
                    ),
                    Err(StructureError::Shape(expected))
                );
            }
            let mut malformed = doc.clone();
            malformed.value.nodes.push(DocNode {
                kind: DocKind::RawCode {
                    language_hint: None,
                    text: "unreachable".into(),
                },
                locations: vec![],
                origin: None,
                span: None,
            });
            assert_eq!(
                malformed.fragment(
                    root,
                    profile.registry(),
                    &mut budget(),
                    &mut SourceAdmission::default()
                ),
                Err(StructureError::Shape(ShapeError::Unreachable(
                    doc.value.nodes.len() as u64
                )))
            );
            for (limits, reason) in [
                (
                    Limits {
                        work: complete.usage().work - 1,
                        ..budget().limits()
                    },
                    StopReason::WorkLimit,
                ),
                (
                    Limits {
                        allocation_units: complete.usage().allocation_units - 1,
                        ..budget().limits()
                    },
                    StopReason::AllocationLimit,
                ),
                (
                    Limits {
                        work: 0,
                        ..budget().limits()
                    },
                    StopReason::WorkLimit,
                ),
                (
                    Limits {
                        allocation_units: 0,
                        ..budget().limits()
                    },
                    StopReason::AllocationLimit,
                ),
                (
                    Limits {
                        depth: 1,
                        ..budget().limits()
                    },
                    StopReason::DepthLimit,
                ),
            ] {
                let mut limited = Budget::new(limits);
                assert_eq!(
                    doc.fragment(
                        root,
                        profile.registry(),
                        &mut limited,
                        &mut SourceAdmission::default()
                    ),
                    Err(StructureError::Stopped(reason))
                );
                assert_eq!(doc, before);
            }
            Ok(())
        },
    )
}
