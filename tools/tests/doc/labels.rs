use super::retention::assert_doc_retention;
use nepl3_core::{
    budget::{Budget, StopReason},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceStore},
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    check::Category,
    labels::{self, LabelError, namespace},
    lower,
    model::*,
};
use nepl3_tools::doc::source::{Compiled, budget, compiled, err, with_input};
use nepl3_wire::foundation::FoundationCodec;

// Explicit one-level fixture composition: Article/Doc Sentence followed by its
// selected Doc Inline occurrences. Production recursive discovery is separate.
fn members(
    compiled: &Compiled,
    document: &DocumentSyntax,
    r: &SchemaRegistry,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<Vec<DocumentSyntax>, String> {
    let empty = SourceStore::default();
    let mut codec = FoundationCodec::new(r, &empty, a).map_err(err)?;
    let surface = r
        .selected("nepl3.syntax.sentence", 1)
        .ok_or("Sentence surface")?;
    let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
        kind: "Form:DocumentInline",
        guest_schema: &compiled.doc.package.schema,
        guest_category: "Inline",
    }];
    let slots = nepl3_suite::adapters::document::sentences::collect(
        document, surface, &forms, r, &mut codec, b,
    )
    .map_err(err)?;
    let mut documents = vec![document.clone()];
    for occurrence in slots.occurrences() {
        let sentence = slots.sentence(occurrence.embed).ok_or("Sentence slot")?;
        let selected = nepl3_suite::adapters::sentence::document_guests::collect(
            sentence,
            &compiled.doc.package.schema,
            r,
            &mut codec,
            b,
        )
        .map_err(err)?;
        for occurrence in selected.occurrences() {
            documents.push(selected.documents()[occurrence.document.index()].clone());
        }
    }
    Ok(documents)
}

// Copy only the observed sites for comparison after the temporary member list
// expires. These test observations grant no namespace or rendering proof.
type ObservedLabels<'a> = (
    Vec<namespace::NamespaceSite<'a>>,
    Vec<namespace::NamespaceReference<'a>>,
);
fn resolve<'a>(
    documents: &'a [DocumentSyntax],
    r: &SchemaRegistry,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<ObservedLabels<'a>, namespace::Error<'a>> {
    let scopes = documents
        .iter()
        .map(|doc| namespace::inspect(doc, r, b, a))
        .collect::<Result<Vec<_>, _>>()?;
    let refs = scopes.iter().collect::<Vec<_>>();
    let checked = namespace::resolve(&refs, b)?;
    Ok((
        checked.definitions().to_vec(),
        checked.references().to_vec(),
    ))
}

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
fn sentence_labels_keep_preorder_ids_and_first_duplicate_diagnostic() -> Result<(), String> {
    let compiled = compiled()?;
    for (source, duplicate) in [
        (
            r#"sentence sentence cons doc ref a text "A" cons doc ref z text "Z" cons doc anchor z text "first" cons doc anchor a text "second" nil"#,
            false,
        ),
        (
            r#"sentence sentence cons doc anchor z text "first" cons doc anchor z text "duplicate" cons doc anchor a text "later" cons doc anchor a text "later duplicate" nil"#,
            true,
        ),
    ] {
        with_input(&compiled, source, "Sentence", |tree, profile, b, a| {
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
                Category::Sentence,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let documents = members(
                &compiled,
                &doc,
                profile.registry(),
                b,
                codec.source_admission(),
            )?;
            let result = resolve(&documents, profile.registry(), b, codec.source_admission());
            if duplicate {
                // The first repeated declaration is z, although a sorts first.
                match result {
                    Err(namespace::Error::Duplicate {
                        definition,
                        previous,
                    }) => {
                        assert_eq!(definition.site.name, "z");
                        assert_eq!(previous.site.name, "z");
                        assert!(
                            previous.site.selection.ok_or("previous span")?.start()
                                < definition.site.selection.ok_or("duplicate span")?.start()
                        );
                    }
                    _ => return Err("expected first duplicate in sentence order".into()),
                }
            } else {
                let (definitions, references) = result.map_err(err)?;
                // Forward references resolve to declaration-preorder IDs,
                // never to positions in the sorted name index.
                assert_eq!(
                    definitions.iter().map(|s| s.site.name).collect::<Vec<_>>(),
                    vec!["z", "a"]
                );
                assert_eq!(
                    references.iter().map(|s| s.target.0).collect::<Vec<_>>(),
                    vec![1, 0]
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn nested_labels_keep_declaration_order_and_pending_siblings() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body cons section z sentence "First" body cons section a sentence "Nested" body nil nil cons section m sentence "Last" body cons paragraph cons sentence sentence cons doc ref a text "nested" cons doc ref z text "first" cons doc ref m text "last" nil nil nil nil"#;
    with_document(&compiled, source, |doc, r, b, a| {
        let documents = members(&compiled, doc, r, b, a)?;
        let (definitions, references) = resolve(&documents, r, b, a).map_err(err)?;
        // Constructor preorder, neither name sorting nor arena index order:
        // nested a is visited before the already queued sibling m.
        assert_eq!(
            definitions.iter().map(|s| s.site.name).collect::<Vec<_>>(),
            vec!["z", "a", "m"]
        );
        assert_eq!(
            references.iter().map(|s| s.target.0).collect::<Vec<_>>(),
            vec![1, 0, 2]
        );
        Ok(())
    })
}

#[test]
fn article_labels_resolve_forward_names_and_keep_operand_selection() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body cons paragraph cons sentence sentence cons doc ref later text "shown" nil nil cons section later sentence "Heading" body nil nil"#;
    with_document(&compiled, source, |doc, r, b, a| {
        let documents = members(&compiled, doc, r, b, a)?;
        let (definitions, references) = resolve(&documents, r, b, a).map_err(err)?;
        assert_eq!(definitions.len(), 1);
        let definition = definitions[0].site;
        let reference = references[0];
        assert_eq!(definition.name, "later");
        assert_eq!(reference.target, labels::DocLabelId(0));
        let selected = definition.selection.ok_or("definition selection")?;
        let range = definition.range.ok_or("definition range")?;
        assert!(range.start() < selected.start() && range.end() > selected.end());
        assert!(
            reference
                .reference
                .site
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
        assert_doc_retention(doc, &received)?;
        let received_members = members(&compiled, &received, r, b, codec.source_admission())?;
        let after = resolve(&received_members, r, b, codec.source_admission()).map_err(err)?;
        assert_eq!((definitions, references), after);
        Ok(())
    })
}

#[test]
fn label_duplicates_use_name_operands_and_source_less_positions_stay_absent() -> Result<(), String>
{
    let compiled = compiled()?;
    let source = r#"article en sentence "same" body cons section same sentence "same" body nil cons paragraph cons sentence sentence cons doc anchor same text "same" nil nil nil"#;
    with_document(&compiled, source, |doc, r, b, a| {
        let documents = members(&compiled, doc, r, b, a)?;
        let Err(namespace::Error::Duplicate {
            definition,
            previous,
        }) = resolve(&documents, r, b, a)
        else {
            return Err("expected duplicate".into());
        };
        let previous = previous.site;
        let definition = definition.site;
        assert_eq!(
            (
                previous.selection.ok_or("previous")?.start(),
                previous.selection.ok_or("previous")?.end()
            ),
            (
                r#"article en sentence "same" body cons section "#.len() as u64,
                r#"article en sentence "same" body cons section same"#.len() as u64
            )
        );
        assert_eq!(
            (
                definition.selection.ok_or("definition")?.start(),
                definition.selection.ok_or("definition")?.end()
            ),
            (r#"article en sentence "same" body cons section same sentence "same" body nil cons paragraph cons sentence sentence cons doc anchor "#.len() as u64,
             r#"article en sentence "same" body cons section same sentence "same" body nil cons paragraph cons sentence sentence cons doc anchor same"#.len() as u64)
        );
        let mut raw = documents.clone();
        for node in raw
            .iter_mut()
            .flat_map(|document| &mut document.value.nodes)
        {
            node.span = None;
            node.origin = None;
            for location in &mut node.locations {
                location.span = None;
                location.origin = None;
            }
        }
        let Err(namespace::Error::Duplicate {
            definition,
            previous,
        }) = resolve(&raw, r, b, a)
        else {
            return Err("expected unlocated duplicate".into());
        };
        assert!(
            definition.site.selection.is_none()
                && previous.site.selection.is_none()
                && definition.site.range.is_none()
        );
        Ok(())
    })
}

#[test]
fn article_labels_do_not_import_or_validate_guest_labels() -> Result<(), String> {
    let compiled = compiled()?;
    for (source, missing) in [
        (
            r#"article en sentence "Host" body cons paragraph cons code Doc article en sentence "Guest" body cons section hidden sentence "Heading" body nil nil cons sentence sentence cons doc ref hidden text "shown" nil nil nil"#,
            true,
        ),
        (
            r#"article en sentence "Host" body cons paragraph cons code Doc article en sentence "Guest" body cons section repeated sentence "One" body nil cons section repeated sentence "Two" body nil nil nil nil"#,
            false,
        ),
    ] {
        with_document(&compiled, source, |doc, r, b, a| {
            assert_eq!(
                doc.value
                    .embeds
                    .iter()
                    .filter(|e| e.kind == EmbedKind::Code)
                    .count(),
                1
            );
            let documents = members(&compiled, doc, r, b, a)?;
            let result = resolve(&documents, r, b, a);
            if missing {
                assert!(
                    matches!(result,Err(namespace::Error::Unresolved{reference}) if reference.site.name=="hidden")
                );
            } else {
                assert!(result.map_err(err)?.0.is_empty());
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
        r#"article en sentence "Title" body cons section named sentence "Heading" body nil nil"#,
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
                .position(|n| matches!(&n.kind, DocKind::Sentence { .. }))
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
    let source = "# 日本語🙂\r\narticle en sentence \"前\" body cons paragraph cons sentence sentence cons doc ref 節 text \"表示\" nil nil cons section 節 sentence \"題\" body nil nil";
    with_document(&compiled, source, |doc, r, b, a| {
        let documents = members(&compiled, doc, r, b, a)?;
        let baseline = resolve(&documents, r, b, a).map_err(err)?;
        let selection = baseline.0[0].site.selection.ok_or("selection")?;
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
        let saved = documents.clone();
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
                    resolve(&documents, r, b, &mut SourceAdmission::default())
                });
                match result {
                    Ok(result) => {
                        successes += 1;
                        assert_eq!(result, baseline);
                    }
                    Err(namespace::Error::Stopped(reason)) => {
                        stops += 1;
                        assert_eq!(reason, expected);
                        assert_eq!(operation.poll(), Err(expected));
                    }
                    Err(other) => return Err(format!("resource {resource} cap {cap}: {other:?}")),
                }
                assert_eq!(operation.current_depth(), 0);
                assert_eq!(documents, saved);
            }
        }
        assert!(stops > 0 && successes > 0);
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            resolve(
                &documents,
                r,
                &mut cancelled,
                &mut SourceAdmission::default()
            ),
            Err(namespace::Error::Stopped(StopReason::Cancelled))
        ));
        Ok(())
    })
}

#[test]
fn shared_label_display_occurrences_have_two_structural_paths() -> Result<(), String> {
    let compiled = compiled()?;
    with_document(
        &compiled,
        r#"article en sentence "Title" body cons paragraph cons section named sentence "shown" body nil nil nil"#,
        |doc, r, b, a| {
            // Local structural paths are owned by Doc. Inline declarations
            // inside independent Sentence use namespace member occurrences.
            for parent in ["paragraph", "body"] {
                let mut shared = doc.clone_with_budget(b).map_err(err)?;
                let section = shared
                    .value
                    .nodes
                    .iter()
                    .position(|n| matches!(n.kind, DocKind::Section { .. }))
                    .ok_or("section")? as u64;
                if parent == "paragraph" {
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
                } else {
                    let owner = shared
                        .value
                        .nodes
                        .iter_mut()
                        .find(|n| matches!(&n.kind, DocKind::Body { blocks } if !blocks.is_empty()))
                        .ok_or("body")?;
                    let DocKind::Body { blocks } = &mut owner.kind else {
                        return Err("body".into());
                    };
                    blocks.push(blocks[0]);
                }
                let Err(LabelError::DuplicateOccurrence { definition, paths }) =
                    labels::check(&shared, r, b, a)
                else {
                    return Err(format!("shared {parent}"));
                };
                assert_eq!(definition.node, section);
                assert_ne!(paths.first, paths.second);
                assert_eq!(paths.first.last().ok_or("first path")?.target, section);
                assert_eq!(paths.second.last().ok_or("second path")?.target, section);
                assert_eq!(
                    definition.selection,
                    doc.value.nodes[section as usize].locations[0].span.as_ref()
                );
            }
            // Sharing a non-label Sentence remains legal. It does not require changing
            // any HTML id or silently cloning a semantic declaration.
            let mut shared = doc.clone_with_budget(b).map_err(err)?;
            let title = shared
                .value
                .nodes
                .iter()
                .position(|n| matches!(&n.kind, DocKind::Sentence { .. }))
                .ok_or("title")? as u64;
            let owner = shared
                .value
                .nodes
                .iter_mut()
                .find(|n| matches!(n.kind, DocKind::Paragraph { .. }))
                .ok_or("paragraph")?;
            let DocKind::Paragraph { items } = &mut owner.kind else {
                return Err("paragraph".into());
            };
            items.push(FlowRef(title));
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
fn shared_sentence_guests_keep_distinct_namespace_occurrences() -> Result<(), String> {
    let compiled = compiled()?;
    with_document(
        &compiled,
        r#"article en sentence "Title" body cons paragraph cons sentence sentence cons doc anchor named text "shown" nil nil nil"#,
        |doc, r, b, a| {
            for parent in ["paragraph", "body"] {
                let mut shared = doc.clone_with_budget(b).map_err(err)?;
                match parent {
                    "paragraph" => {
                        let items = shared
                            .value
                            .nodes
                            .iter_mut()
                            .find_map(|node| {
                                if let DocKind::Paragraph { items } = &mut node.kind {
                                    Some(items)
                                } else {
                                    None
                                }
                            })
                            .ok_or("paragraph")?;
                        items.push(items[0]);
                    }
                    _ => {
                        let blocks = shared
                            .value
                            .nodes
                            .iter_mut()
                            .find_map(|node| {
                                if let DocKind::Body { blocks } = &mut node.kind {
                                    Some(blocks)
                                } else {
                                    None
                                }
                            })
                            .ok_or("body")?;
                        blocks.push(blocks[0]);
                    }
                }
                let documents = members(&compiled, &shared, r, b, a)?;
                assert_eq!(documents.len(), 3);
                let Err(namespace::Error::Duplicate {
                    definition,
                    previous,
                }) = resolve(&documents, r, b, a)
                else {
                    return Err(format!("shared guest in {parent}"));
                };
                // Equal semantic sites still denote two display occurrences.
                assert_eq!(previous.member, namespace::MemberId(1));
                assert_eq!(definition.member, namespace::MemberId(2));
                assert_eq!(definition.site, previous.site);
                assert_eq!(definition.site.name, "named");
                let span = definition.site.selection.ok_or("name selection")?;
                let source = documents[1]
                    .sources
                    .iter()
                    .find(|source| source.identity() == span.snapshot_ref())
                    .ok_or("name source")?;
                assert_eq!(source.slice(span).map_err(err)?, "named");
            }
            Ok(())
        },
    )
}

#[test]
fn label_diagnostics_keep_typed_names_paths_and_original_failure_on_stop() -> Result<(), String> {
    let compiled = compiled()?;
    with_document(
        &compiled,
        r#"article en sentence "Title" body cons paragraph cons section named sentence "shown" body nil nil nil"#,
        |doc, r, b, a| {
            let mut shared = doc.clone_with_budget(b).map_err(err)?;
            let section = shared
                .value
                .nodes
                .iter()
                .position(|n| matches!(n.kind, DocKind::Section { .. }))
                .ok_or("section")? as u64;
            let owner = shared
                .value
                .nodes
                .iter_mut()
                .find(|n| matches!(n.kind, DocKind::Paragraph { .. }))
                .ok_or("paragraph")?;
            let DocKind::Paragraph { items } = &mut owner.kind else {
                return Err("paragraph".into());
            };
            items.push(FlowRef(section));
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
