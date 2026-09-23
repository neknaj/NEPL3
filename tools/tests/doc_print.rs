use nepl3_core::budget::{Budget, StopReason};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::{
    check::{Category, ShapeError},
    lower,
    model::*,
    print::{self, PrintEntry, PrintMode, PrintOutcome},
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
