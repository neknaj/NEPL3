use super::*;

fn with_body<T>(
    compiled: &Compiled,
    source: &str,
    finish: impl FnOnce(
        &DocumentSyntax,
        &SchemaRegistry,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_input(compiled, source, "Body", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
        let doc = nepl3_suite::adapters::sentence::literal::lower(
            &checked,
            &compiled.doc.package.schema,
            Category::Body,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        finish(&doc, profile.registry(), b, codec.source_admission())
    })
}

#[test]
fn body_labels_resolve_peer_sections_and_roundtrip_without_article_admission() -> Result<(), String>
{
    let compiled = compiled()?;
    let source = "# 日本語🙂\r\nbody cons paragraph cons sentence cons ref 後 text \"later\" cons ref 先 text \"first\" nil nil cons section 先 \"First\" body nil cons section 後 \"Later\" body nil nil";
    with_body(&compiled, source, |doc, r, b, a| {
        let proof = labels::check_body(doc, r, b, a).map_err(err)?;
        assert!(core::ptr::eq(proof.document(), doc));
        assert_eq!(
            proof
                .definitions()
                .iter()
                .map(|s| s.name)
                .collect::<Vec<_>>(),
            ["先", "後"]
        );
        assert_eq!(
            proof
                .references()
                .iter()
                .map(|s| s.target.0)
                .collect::<Vec<_>>(),
            [1, 0]
        );
        let selection = proof.definitions()[0].selection.ok_or("selection")?;
        assert_eq!(
            doc.sources
                .iter()
                .find(|s| s.identity() == selection.snapshot_ref())
                .ok_or("source")?
                .slice(selection)
                .map_err(err)?,
            "先"
        );
        // The new proof must not silently widen existing export/composition gates.
        let reference_selection = proof.references()[0]
            .reference
            .selection
            .ok_or("reference selection")?;
        assert_eq!(
            doc.sources
                .iter()
                .find(|s| s.identity() == reference_selection.snapshot_ref())
                .ok_or("reference source")?
                .slice(reference_selection)
                .map_err(err)?,
            "後"
        );
        assert!(matches!(
            labels::check(doc, r, b, a),
            Err(LabelError::ExpectedArticle)
        ));
        assert!(matches!(
            labels::check_sentence(doc, r, b, a),
            Err(LabelError::ExpectedSentence)
        ));
        assert!(matches!(
            labels::check_inline(doc, r, b, a),
            Err(LabelError::ExpectedInline)
        ));
        assert!(matches!(
            labels::namespace::inspect(doc, r, b, a),
            Err(labels::namespace::Error::Root(Category::Body))
        ));
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
        let restored =
            labels::check_body(&received, r, b, codec.source_admission()).map_err(err)?;
        assert_eq!(proof.definitions(), restored.definitions());
        assert_eq!(proof.references(), restored.references());
        Ok(())
    })
}

#[test]
fn body_labels_reject_wrong_root_unresolved_and_duplicate_names() -> Result<(), String> {
    let compiled = compiled()?;
    with_document(
        &compiled,
        r#"article en "Title" body nil"#,
        |doc, r, b, a| {
            assert!(matches!(
                labels::check_body(doc, r, b, a),
                Err(LabelError::ExpectedBody)
            ));
            Ok(())
        },
    )?;
    for (source, duplicate) in [
        (
            r#"body cons paragraph cons sentence cons ref absent text "missing" nil nil nil"#,
            false,
        ),
        (
            r#"body cons paragraph cons sentence cons ref missing text "Missing" nil nil cons section z "First" body nil cons section z "Duplicate" body nil nil"#,
            true,
        ),
    ] {
        with_body(&compiled, source, |doc, r, b, a| {
            match labels::check_body(doc, r, b, a) {
                Err(LabelError::Duplicate {
                    definition,
                    previous,
                }) if duplicate => {
                    assert_eq!((definition.name, previous.name), ("z", "z"));
                    assert!(
                        previous.selection.ok_or("previous")?.start()
                            < definition.selection.ok_or("duplicate")?.start()
                    );
                }
                Err(LabelError::Unresolved { reference }) if !duplicate => {
                    assert_eq!(reference.name, "absent");
                    assert!(reference.selection.is_some());
                }
                _ => return Err("expected scoped label error".into()),
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn body_labels_reject_repeated_declaration_occurrences() -> Result<(), String> {
    let compiled = compiled()?;
    with_body(
        &compiled,
        r#"body cons section named "Title" body nil cons paragraph cons sentence cons ref missing text "Missing" nil nil nil"#,
        |doc, r, b, a| {
            let mut shared = doc.clone_with_budget(b).map_err(err)?;
            let DocRoot::Body(root) = shared.value.root else {
                return Err("body root".into());
            };
            let DocKind::Body { blocks } = &mut shared.value.nodes[root.0 as usize].kind else {
                return Err("body node".into());
            };
            let section = blocks[0];
            blocks.push(section);
            let Err(LabelError::DuplicateOccurrence { definition, paths }) =
                labels::check_body(&shared, r, b, a)
            else {
                return Err("duplicate occurrence".into());
            };
            assert_eq!(definition.name, "named");
            assert_ne!(paths.first, paths.second);
            assert_eq!(paths.first.last().ok_or("first path")?.target, section.0);
            assert_eq!(paths.second.last().ok_or("second path")?.target, section.0);
            Ok(())
        },
    )
}

#[test]
fn body_labels_preserve_budget_stops_cancellation_and_inputs() -> Result<(), String> {
    let compiled = compiled()?;
    with_body(
        &compiled,
        r#"body cons paragraph cons sentence cons ref later text "later" nil nil cons section later "Later" body nil nil"#,
        |doc, r, b, a| {
            let proof = labels::check_body(doc, r, b, a).map_err(err)?;
            let saved = doc.clone_with_budget(b).map_err(err)?;
            let mut measured = budget();
            labels::check_body(doc, r, &mut measured, &mut SourceAdmission::default())
                .map_err(err)?;
            let work = measured.usage().work;
            for missing in [false, true] {
                let mut limits = budget().limits();
                limits.work = work + 17 - u64::from(missing);
                let mut operation = Budget::new(limits);
                operation
                    .charge(nepl3_core::budget::Resource::Work, 17)
                    .map_err(err)?;
                let result =
                    labels::check_body(doc, r, &mut operation, &mut SourceAdmission::default());
                if missing {
                    assert!(matches!(
                        result,
                        Err(LabelError::Stopped(StopReason::WorkLimit))
                    ));
                    assert_eq!(operation.poll(), Err(StopReason::WorkLimit));
                } else {
                    assert_eq!(result.map_err(err)?.references(), proof.references());
                    assert_eq!(operation.usage().work, work + 17);
                }
            }
            for resource in 0..5 {
                let mut stops = 0;
                let mut successes = 0;
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
                        labels::check_body(doc, r, b, &mut SourceAdmission::default())
                    });
                    match result {
                        Ok(actual) => {
                            successes += 1;
                            assert_eq!(actual.definitions(), proof.definitions());
                            assert_eq!(actual.references(), proof.references());
                        }
                        Err(LabelError::Stopped(reason)) => {
                            stops += 1;
                            assert_eq!(reason, expected);
                            assert_eq!(operation.poll(), Err(expected));
                        }
                        Err(other) => {
                            return Err(format!("resource {resource} cap {cap}: {other:?}"));
                        }
                    }
                    assert_eq!(operation.current_depth(), 0);
                    assert_eq!(doc, &saved);
                }
                assert!(stops > 0 && successes > 0, "resource {resource}");
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                labels::check_body(doc, r, &mut cancelled, &mut SourceAdmission::default()),
                Err(LabelError::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
            assert_eq!(doc, &saved);
            Ok(())
        },
    )
}

#[test]
fn body_labels_validate_empty_and_malformed_roots_before_traversal() -> Result<(), String> {
    let compiled = compiled()?;
    with_body(&compiled, "body nil", |doc, r, b, a| {
        let proof = labels::check_body(doc, r, b, a).map_err(err)?;
        assert!(proof.definitions().is_empty() && proof.references().is_empty());
        let mut invalid = doc.clone_with_budget(b).map_err(err)?;
        invalid.value.root = DocRoot::Body(BodyRef(u64::MAX));
        assert!(matches!(
            labels::check_body(&invalid, r, b, a),
            Err(LabelError::Structure(_))
        ));
        // A malformed wrong-category root is a structure error first.
        invalid.value.root = DocRoot::Article(ArticleRef(u64::MAX));
        assert!(matches!(
            labels::check_body(&invalid, r, b, a),
            Err(LabelError::Structure(_))
        ));
        Ok(())
    })
}

#[test]
fn body_labels_resolve_nested_anchor_in_the_same_body_scope() -> Result<(), String> {
    let compiled = compiled()?;
    with_body(
        &compiled,
        r#"body cons section outer "Outer" body cons paragraph cons sentence cons ref inner text "shown" cons anchor inner text "anchor" nil nil nil nil"#,
        |doc, r, b, a| {
            let proof = labels::check_body(doc, r, b, a).map_err(err)?;
            assert_eq!(
                proof
                    .definitions()
                    .iter()
                    .map(|s| s.name)
                    .collect::<Vec<_>>(),
                ["outer", "inner"]
            );
            assert_eq!(proof.references()[0].target, labels::DocLabelId(1));
            Ok(())
        },
    )
}

#[test]
fn body_labels_keep_foreign_closure_names_outside_the_local_scope() -> Result<(), String> {
    let compiled = compiled()?;
    for (source, missing) in [
        (
            r#"body cons paragraph cons code Doc article en "Guest" body cons section hidden "Heading" body nil nil cons sentence cons ref hidden text "shown" nil nil nil"#,
            true,
        ),
        (
            r#"body cons paragraph cons code Doc article en "Guest" body cons section repeated "One" body nil cons section repeated "Two" body nil nil nil nil"#,
            false,
        ),
    ] {
        with_body(&compiled, source, |doc, r, b, a| {
            assert_eq!(doc.value.embeds.len(), 1);
            let result = labels::check_body(doc, r, b, a);
            if missing {
                assert!(
                    matches!(result, Err(LabelError::Unresolved { reference }) if reference.name == "hidden")
                );
            } else {
                let proof = result.map_err(err)?;
                assert!(proof.definitions().is_empty() && proof.references().is_empty());
            }
            Ok(())
        })?;
    }
    Ok(())
}
