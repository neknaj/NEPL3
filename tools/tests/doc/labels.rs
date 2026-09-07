use super::*;
use nepl3_core::{schema::SchemaRegistry, value_codec::FoundationValueCodec};
use nepl3_doc_core::{
    check::Category,
    labels::{self, LabelError},
    lower,
    model::*,
};

fn with_document<T>(
    compiled: &Compiled,
    source: &str,
    finish: impl FnOnce(
        &DocumentSyntax,
        &SchemaRegistry,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_input(compiled, source, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
        let doc = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        finish(&doc, profile.registry(), b, codec.source_admission())
    })
}

#[test]
fn article_labels_resolve_forward_names_and_keep_operand_selection() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en "Title" body cons paragraph cons sentence cons ref later text "shown" nil nil cons section later "Heading" body nil nil"#;
    with_document(&compiled, source, |doc, r, b, a| {
        let checked = labels::check(doc, r, b, a).map_err(err)?;
        assert_eq!(checked.definitions().len(), 1);
        let definition = checked.definitions()[0];
        let reference = checked.references()[0];
        assert_eq!(definition.name, "later");
        assert_eq!(reference.target, labels::DocLabelId(0));
        let selected = definition.selection.ok_or("definition selection")?;
        let range = definition.range.ok_or("definition range")?;
        assert!(range.start() < selected.start() && range.end() > selected.end());
        assert!(
            reference
                .reference
                .selection
                .ok_or("reference selection")?
                .start()
                < selected.start()
        );
        assert_eq!(
            doc.sources
                .iter()
                .find(|s| s.identity() == selected.snapshot_ref())
                .ok_or("source")?
                .slice(selected)
                .map_err(err)?,
            "later"
        );
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(r, &empty, a).map_err(err)?;
        let value = nepl3_doc_core::portable::to_value(doc, r, &mut codec, b).map_err(err)?;
        let bytes = nepl3_wire::encode(&value, b).map_err(err)?;
        let mut fresh = SourceAdmission::default();
        let mut codec = FoundationCodec::new(r, &empty, &mut fresh).map_err(err)?;
        let received = nepl3_doc_core::portable::from_value(
            &nepl3_wire::decode(&bytes, b).map_err(err)?,
            r,
            &mut codec,
            b,
        )
        .map_err(err)?;
        assert_doc_retention(doc, &received);
        let after = labels::check(&received, r, b, codec.source_admission()).map_err(err)?;
        assert_eq!(checked.definitions(), after.definitions());
        assert_eq!(checked.references(), after.references());
        Ok(())
    })
}

#[test]
fn label_duplicates_use_name_operands_and_source_less_positions_stay_absent() -> Result<(), String>
{
    let compiled = compiled()?;
    let source = r#"article en "same" body cons section same "same" body nil cons paragraph cons sentence cons anchor same text "same" nil nil nil"#;
    with_document(&compiled, source, |doc, r, b, a| {
        let Err(LabelError::Duplicate {
            definition,
            previous,
        }) = labels::check(doc, r, b, a)
        else {
            return Err("expected duplicate".into());
        };
        assert_eq!(
            (
                previous.selection.ok_or("previous")?.start(),
                previous.selection.ok_or("previous")?.end()
            ),
            (
                r#"article en "same" body cons section "#.len() as u64,
                r#"article en "same" body cons section same"#.len() as u64
            )
        );
        assert_eq!(
            (
                definition.selection.ok_or("definition")?.start(),
                definition.selection.ok_or("definition")?.end()
            ),
            (r#"article en "same" body cons section same "same" body nil cons paragraph cons sentence cons anchor "#.len() as u64,
             r#"article en "same" body cons section same "same" body nil cons paragraph cons sentence cons anchor same"#.len() as u64)
        );
        let mut raw = doc.clone_with_budget(b).map_err(err)?;
        for node in &mut raw.value.nodes {
            node.span = None;
            node.origin = None;
            for location in &mut node.locations {
                location.span = None;
                location.origin = None;
            }
        }
        let Err(LabelError::Duplicate {
            definition,
            previous,
        }) = labels::check(&raw, r, b, a)
        else {
            return Err("expected unlocated duplicate".into());
        };
        assert!(
            definition.selection.is_none()
                && previous.selection.is_none()
                && definition.range.is_none()
        );
        Ok(())
    })
}

#[test]
fn article_labels_do_not_import_or_validate_guest_labels() -> Result<(), String> {
    let compiled = compiled()?;
    for (source, missing) in [
        (
            r#"article en "Host" body cons paragraph cons code Doc article en "Guest" body cons section hidden "Heading" body nil nil cons sentence cons ref hidden text "shown" nil nil nil"#,
            true,
        ),
        (
            r#"article en "Host" body cons paragraph cons code Doc article en "Guest" body cons section repeated "One" body nil cons section repeated "Two" body nil nil nil nil"#,
            false,
        ),
    ] {
        with_document(&compiled, source, |doc, r, b, a| {
            assert_eq!(doc.value.embeds.len(), 1);
            let result = labels::check(doc, r, b, a);
            if missing {
                assert!(
                    matches!(result,Err(LabelError::Unresolved{reference}) if reference.name=="hidden")
                );
            } else {
                assert!(result.map_err(err)?.definitions().is_empty());
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn record_fields(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    let NdfValue::Record(record) = value else {
        return Err("record".into());
    };
    Ok(&mut record.fields)
}
#[test]
fn field_location_first_receiver_rejects_kind_duplicates_and_position_claims() -> Result<(), String>
{
    let compiled = compiled()?;
    with_document(
        &compiled,
        r#"article en "Title" body cons section named "Heading" body nil nil"#,
        |doc, r, b, a| {
            let named = doc
                .value
                .nodes
                .iter()
                .position(|n| matches!(n.kind, DocKind::Section { .. }))
                .ok_or("section")?;
            let other = doc
                .value
                .nodes
                .iter()
                .position(|n| matches!(&n.kind,DocKind::Text{text} if text=="Title"))
                .ok_or("title")?;
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(r, &empty, a).map_err(err)?;
            let wire = nepl3_doc_core::portable::to_value(doc, r, &mut codec, b).map_err(err)?;
            for case in 0..5 {
                let mut value = wire.clone();
                let document = record_fields(&mut value)?;
                let arena = record_fields(&mut document[0])?;
                let NdfValue::List(nodes) = &mut arena[1] else {
                    return Err("nodes".into());
                };
                let other_span = record_fields(&mut nodes[other])?[2].clone();
                let other_origin = record_fields(&mut nodes[other])?[1].clone();
                let fields = record_fields(&mut nodes[named])?;
                let NdfValue::List(locations) = &mut fields[3] else {
                    return Err("locations".into());
                };
                if case == 1 {
                    locations.push(locations[0].clone());
                } else {
                    let location = record_fields(&mut locations[0])?;
                    match case {
                        0 => {
                            let NdfValue::Variant(tag) = &mut location[0] else {
                                return Err("field tag".into());
                            };
                            tag.variant = "AnchorId".into();
                        }
                        2 => location[2] = other_span,
                        3 => {
                            let NdfValue::Some(origin) = &mut location[1] else {
                                return Err("origin option".into());
                            };
                            record_fields(origin)?[0] = NdfValue::U64(u64::MAX);
                        }
                        _ => location[1] = other_origin,
                    }
                }
                // The claims retain valid NDF/schema types. The Doc receiver must
                // additionally enforce the constructor and position relationships.
                let mut fresh = SourceAdmission::default();
                let mut receiver = FoundationCodec::new(r, &empty, &mut fresh).map_err(err)?;
                let result =
                    nepl3_doc_core::portable::from_value(&value, r, &mut receiver, &mut budget());
                assert!(
                    matches!(
                        result,
                        Err(nepl3_doc_core::portable::PortableError::Structure(_))
                    ),
                    "case {case}: {result:?}"
                );
            }
            Ok(())
        },
    )
}

#[test]
fn unicode_crlf_labels_and_budget_stops_keep_their_input_positions() -> Result<(), String> {
    let compiled = compiled()?;
    let source = "# 日本語🙂\r\narticle en \"前\" body cons paragraph cons sentence cons ref 節 text \"表示\" nil nil cons section 節 \"題\" body nil nil";
    with_document(&compiled, source, |doc, r, b, a| {
        let baseline = labels::check(doc, r, b, a).map_err(err)?;
        let selection = baseline.definitions()[0].selection.ok_or("selection")?;
        assert_eq!(selection.end() - selection.start(), 3);
        assert_eq!(
            doc.sources
                .iter()
                .find(|s| s.identity() == selection.snapshot_ref())
                .ok_or("source")?
                .slice(selection)
                .map_err(err)?,
            "節"
        );
        let saved = doc.clone_with_budget(b).map_err(err)?;
        let mut stops = 0;
        let mut successes = 0;
        for resource in 0..5 {
            for cap in [0, 1, 8, 64, 512, 4096, 32768, 262144, 2097152] {
                let mut limits = budget().limits();
                let expected = match resource {
                    0 => {
                        limits.work = cap;
                        StopReason::WorkLimit
                    }
                    1 => {
                        limits.allocation_units = cap;
                        StopReason::AllocationLimit
                    }
                    2 => {
                        limits.source_bytes = cap;
                        StopReason::SourceLimit
                    }
                    3 => {
                        limits.nodes = cap;
                        StopReason::NodeLimit
                    }
                    _ => {
                        limits.depth = cap;
                        StopReason::DepthLimit
                    }
                };
                let mut operation = Budget::new(limits);
                let result = operation.with_depth_at_least(7, |b| {
                    labels::check(doc, r, b, &mut SourceAdmission::default())
                });
                match result {
                    Ok(result) => {
                        successes += 1;
                        assert_eq!(result.definitions(), baseline.definitions());
                        assert_eq!(result.references(), baseline.references());
                    }
                    Err(LabelError::Stopped(reason)) => {
                        stops += 1;
                        assert_eq!(reason, expected);
                        assert_eq!(operation.poll(), Err(expected));
                    }
                    Err(other) => return Err(format!("resource {resource} cap {cap}: {other:?}")),
                }
                assert_eq!(operation.current_depth(), 0);
                assert_eq!(doc, &saved);
            }
        }
        assert!(stops > 0 && successes > 0);
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            labels::check(doc, r, &mut cancelled, &mut SourceAdmission::default()),
            Err(LabelError::Stopped(StopReason::Cancelled))
        ));
        Ok(())
    })
}

#[test]
fn shared_label_display_occurrences_have_two_structural_paths() -> Result<(), String> {
    let compiled = compiled()?;
    with_document(
        &compiled,
        r#"article en "Title" body cons paragraph cons sentence cons anchor named text "shown" nil nil nil"#,
        |doc, r, b, a| {
            for parent in ["sentence", "paragraph"] {
                let mut shared = doc.clone_with_budget(b).map_err(err)?;
                let anchor = shared
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::Anchor { .. }))
                    .ok_or("anchor")? as u64;
                if parent == "sentence" {
                    let owner=shared.value.nodes.iter_mut().find(|n|matches!(&n.kind,DocKind::Sentence{inlines} if inlines.iter().any(|r|r.0==anchor))).ok_or("sentence")?;
                    let DocKind::Sentence { inlines } = &mut owner.kind else {
                        return Err("sentence".into());
                    };
                    inlines.push(InlineRef(anchor));
                } else {
                    let owner = shared
                        .value
                        .nodes
                        .iter_mut()
                        .find(|n| matches!(n.kind, DocKind::Paragraph { .. }))
                        .ok_or("paragraph")?;
                    let DocKind::Paragraph { items } = &mut owner.kind else {
                        return Err("paragraph".into());
                    };
                    items.push(items[0]);
                }
                let Err(LabelError::DuplicateOccurrence { definition, paths }) =
                    labels::check(&shared, r, b, a)
                else {
                    return Err(format!("shared {parent}"));
                };
                assert_eq!(definition.node, anchor);
                assert_ne!(paths.first, paths.second);
                assert_eq!(paths.first.last().ok_or("first path")?.target, anchor);
                assert_eq!(paths.second.last().ok_or("second path")?.target, anchor);
                assert_eq!(
                    definition.selection,
                    doc.value.nodes[anchor as usize].locations[0].span.as_ref()
                );
            }
            // Sharing a non-label Text remains legal. It does not require changing
            // any HTML id or silently cloning a semantic declaration.
            let mut shared = doc.clone_with_budget(b).map_err(err)?;
            let title = shared
                .value
                .nodes
                .iter()
                .position(|n| matches!(&n.kind,DocKind::Text{text} if text=="Title"))
                .ok_or("title")? as u64;
            let owner=shared.value.nodes.iter_mut().find(|n|matches!(&n.kind,DocKind::Sentence{inlines} if inlines.iter().any(|r|r.0==title))).ok_or("title sentence")?;
            let DocKind::Sentence { inlines } = &mut owner.kind else {
                return Err("sentence".into());
            };
            inlines.push(InlineRef(title));
            assert_eq!(
                labels::check(&shared, r, b, a)
                    .map_err(err)?
                    .definitions()
                    .len(),
                1
            );
            Ok(())
        },
    )
}

#[test]
fn label_diagnostics_keep_typed_names_paths_and_original_failure_on_stop() -> Result<(), String> {
    let compiled = compiled()?;
    with_document(
        &compiled,
        r#"article en "Title" body cons paragraph cons sentence cons anchor named text "shown" nil nil nil"#,
        |doc, r, b, a| {
            let mut shared = doc.clone_with_budget(b).map_err(err)?;
            let anchor = shared
                .value
                .nodes
                .iter()
                .position(|n| matches!(n.kind, DocKind::Anchor { .. }))
                .ok_or("anchor")? as u64;
            let owner=shared.value.nodes.iter_mut().find(|n|matches!(&n.kind,DocKind::Sentence{inlines} if inlines.iter().any(|r|r.0==anchor))).ok_or("sentence")?;
            let DocKind::Sentence { inlines } = &mut owner.kind else {
                return Err("sentence".into());
            };
            inlines.push(InlineRef(anchor));
            let failure = labels::check(&shared, r, b, a)
                .err()
                .ok_or("duplicate occurrence")?;
            let mut declared = SourceStore::default();
            for source in &doc.sources {
                declared.insert(source.clone()).map_err(err)?;
            }
            let mut codec = FoundationCodec::new(r, &declared, a).map_err(err)?;
            let diagnostic = failure.diagnostic(&shared, r, &mut codec, b).map_err(err)?;
            assert_eq!(diagnostic.code, "DuplicateOccurrence");
            let nepl3_core::value::TypedValue::Record(args) = &diagnostic.arguments else {
                return Err("arguments".into());
            };
            assert_eq!(args.kind, "LabelDiagnosticArguments");
            assert_eq!(args.fields[0], NdfValue::Text("named".into()));
            let NdfValue::Some(paths) = &args.fields[1] else {
                return Err("paths".into());
            };
            let NdfValue::Record(paths) = paths.as_ref() else {
                return Err("path record".into());
            };
            assert_ne!(paths.fields[0], paths.fields[1]);
            let report = nepl3_core::diagnostic::Report {
                diagnostics: vec![diagnostic],
                usage: b.usage(),
                ..Default::default()
            };
            let encoded = codec.encode_report(&report, b).map_err(err)?;
            let bytes = nepl3_wire::encode(&encoded, b).map_err(err)?;
            let mut fresh = SourceAdmission::default();
            let mut receiver = FoundationCodec::new(r, &declared, &mut fresh).map_err(err)?;
            let received = receiver
                .decode_report(&nepl3_wire::decode(&bytes, b).map_err(err)?, b)
                .map_err(err)?;
            assert_eq!(report, received);
            for resource in 0..4 {
                let mut limits = budget().limits();
                let expected = match resource {
                    0 => {
                        limits.work = 0;
                        StopReason::WorkLimit
                    }
                    1 => {
                        limits.allocation_units = 0;
                        StopReason::AllocationLimit
                    }
                    2 => {
                        limits.source_bytes = 0;
                        StopReason::SourceLimit
                    }
                    _ => {
                        limits.diagnostics = 0;
                        StopReason::DiagnosticLimit
                    }
                };
                let mut operation = Budget::new(limits);
                let mut fresh = SourceAdmission::default();
                let mut codec = FoundationCodec::new(r, &declared, &mut fresh).map_err(err)?;
                assert!(
                    matches!(failure.diagnostic(&shared,r,&mut codec,&mut operation),Err(labels::LabelDiagnosticError::Stopped(s)) if s==expected)
                );
                assert_eq!(operation.poll(), Err(expected));
                assert!(
                    matches!(&failure,LabelError::DuplicateOccurrence{definition,..} if definition.name=="named")
                );
            }
            Ok(())
        },
    )
}
