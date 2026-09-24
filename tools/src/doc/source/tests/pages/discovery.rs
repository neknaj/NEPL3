use super::*;
use crate::doc::export::pages::discovery;
use nepl3_core::budget::StopReason;
use nepl3_doc_core::model::EmbedRef;

#[test]
fn recursive_discovery_failure_retains_reentry_source_owner() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body cons paragraph cons sentence sentence cons doc anchor outer math add 1 2 nil nil nil"#;
    with_named_input(
        true,
        &compiled,
        source,
        "failure",
        "Article",
        |tree, profile, b, a| {
            let registry = profile.registry();
            let store = SourceStore::default();
            let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
            let input = tree
                .tree()
                .bundle
                .validate_with_sources(registry, b, &mut SourceAdmission::default())
                .map_err(err)?;
            let document = lower::document(
                &input,
                &compiled.doc.package.schema,
                Category::Article,
                registry,
                b,
                &mut codec,
            )
            .map_err(err)?;
            // Doc is selected, but the host deliberately supplies no InlineMath form.
            // The failure occurs in the independent Inline label of the Doc guest.
            let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
                kind: "Form:DocumentInline",
                guest_schema: &compiled.doc.package.schema,
                guest_category: "Inline",
            }];
            let failure = match discovery::collect(
                &document,
                &compiled.others[3].schema,
                &compiled.doc.package.schema,
                &forms,
                registry,
                &mut codec,
                &mut budget(),
            ) {
                Err(failure) => failure,
                Ok(_) => return Err("unselected InlineMath was accepted".into()),
            };
            let discovery::Error::Sentence {
                document: owner, ..
            } = failure.cause()
            else {
                return Err(format!("expected reentry failure: {failure:?}"));
            };
            assert_eq!(owner.index(), 1);
            let parent = failure.parent(*owner).ok_or("parent")?;
            assert_eq!(parent.document.index(), 0);
            assert_eq!(parent.slot, EmbedRef(1));
            assert_eq!(parent.embed.0, 0);
            assert!(core::ptr::eq(
                failure.document(parent.document).ok_or("root")?,
                &document
            ));
            let retained = failure.document(*owner).ok_or("retained guest")?;
            assert!(
                retained
                    .value
                    .nodes
                    .iter()
                    .any(|node| matches!(&node.kind, DocKind::Anchor { id, .. } if id == "outer"))
            );
            assert!(
                retained
                    .sources
                    .iter()
                    .any(|snapshot| snapshot.text() == source)
            );
            Ok(())
        },
    )
}

#[test]
fn recursive_discovery_keeps_unique_owners_occurrences_and_total_depth() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body
        cons paragraph cons sentence sentence cons doc anchor outer concat cons doc anchor inner text "Leaf" nil nil nil
        cons code Math add 1 2 nil"#;
    with_named_input(
        true,
        &compiled,
        source,
        "recursive",
        "Article",
        |tree, profile, b, a| {
            let registry = profile.registry();
            let store = SourceStore::default();
            let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
            let input = tree
                .tree()
                .bundle
                .validate_with_sources(registry, b, &mut SourceAdmission::default())
                .map_err(err)?;
            let mut document = lower::document(
                &input,
                &compiled.doc.package.schema,
                Category::Article,
                registry,
                b,
                &mut codec,
            )
            .map_err(err)?;
            let blocks = document
                .value
                .nodes
                .iter_mut()
                .find_map(|node| match &mut node.kind {
                    DocKind::Body { blocks } => Some(blocks),
                    _ => None,
                })
                .ok_or("body")?;
            blocks.push(blocks[0]);
            let original = document.clone();
            let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
                kind: "Form:DocumentInline",
                guest_schema: &compiled.doc.package.schema,
                guest_category: "Inline",
            }];
            let mut measured = budget();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
            let result = discovery::collect(
                &document,
                &compiled.others[3].schema,
                &compiled.doc.package.schema,
                &forms,
                registry,
                &mut codec,
                &mut measured,
            )
            .map_err(err)?;
            let members = result.members();
            assert_eq!(members.len(), 3);
            assert!(core::ptr::eq(members[0].document(), &document));
            assert_eq!(
                members
                    .iter()
                    .map(|member| member.depth())
                    .collect::<Vec<_>>(),
                [0, 6, 9]
            );
            assert_eq!(
                members[0]
                    .occurrences()
                    .iter()
                    .map(|o| o.embed)
                    .collect::<Vec<_>>(),
                [EmbedRef(0), EmbedRef(1), EmbedRef(1)]
            );
            assert_eq!(members[0].sentences().len(), 3);
            assert!(members[0].sentences()[2].is_none());
            for (owner, target, slot, id) in [(0, 1, 1, "outer"), (1, 2, 0, "inner")] {
                let [guest] = members[owner].guests() else {
                    return Err("one nested guest".into());
                };
                assert_eq!(guest.slot, EmbedRef(slot));
                assert_eq!(guest.document.index(), target);
                assert_eq!(guest.occurrence.depth, 2);
                assert!(members[target].document().value.nodes.iter().any(
                    |node| matches!(&node.kind, DocKind::Anchor { id: actual, .. } if actual == id)
                ));
            }
            assert!(members[2].guests().is_empty());
            let mut nested = budget();
            let mut nested_admission = SourceAdmission::default();
            let mut nested_codec =
                FoundationCodec::new(registry, &store, &mut nested_admission).map_err(err)?;
            let nested_result = nested
                .with_depth_at_least(7, |b| {
                    discovery::collect(
                        &document,
                        &compiled.others[3].schema,
                        &compiled.doc.package.schema,
                        &forms,
                        registry,
                        &mut nested_codec,
                        b,
                    )
                })
                .map_err(err)?;
            assert_eq!(
                nested_result
                    .members()
                    .iter()
                    .map(|m| m.depth())
                    .collect::<Vec<_>>(),
                [7, 13, 16]
            );
            assert_eq!(nested.current_depth(), 0);
            assert_eq!(nested.usage().depth, measured.usage().depth + 7);
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                discovery::collect(
                    &document,
                    &compiled.others[3].schema,
                    &compiled.doc.package.schema,
                    &forms,
                    registry,
                    &mut codec,
                    &mut cancelled
                ),
                Err(ref failure) if matches!(failure.cause(), discovery::Error::Stopped(StopReason::Cancelled))
            ));
            for (reason, usage) in [
                (StopReason::WorkLimit, measured.usage().work),
                (
                    StopReason::AllocationLimit,
                    measured.usage().allocation_units,
                ),
                (StopReason::DepthLimit, measured.usage().depth),
                (StopReason::SourceLimit, measured.usage().source_bytes),
            ] {
                assert!(usage > 0);
                for shortage in [0, 1] {
                    let mut limits = measured.limits();
                    match reason {
                        StopReason::WorkLimit => limits.work = usage - shortage,
                        StopReason::AllocationLimit => limits.allocation_units = usage - shortage,
                        StopReason::DepthLimit => limits.depth = usage - shortage,
                        StopReason::SourceLimit => limits.source_bytes = usage - shortage,
                        _ => return Err("fixture resource".into()),
                    }
                    let mut limited = Budget::new(limits);
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                    let actual = discovery::collect(
                        &document,
                        &compiled.others[3].schema,
                        &compiled.doc.package.schema,
                        &forms,
                        registry,
                        &mut codec,
                        &mut limited,
                    );
                    if shortage == 0 {
                        assert_eq!(actual.map_err(err)?.members().len(), 3);
                    } else {
                        assert!(
                            matches!(actual, Err(ref failure) if matches!(failure.cause(), discovery::Error::Stopped(actual) if *actual == reason)),
                            "{reason:?}"
                        );
                        if let Err(failure) = &actual {
                            for (id, owner, parent) in failure.owners() {
                                if id.index() == 0 {
                                    assert!(core::ptr::eq(owner, &document));
                                    assert!(parent.is_none());
                                } else {
                                    let parent = parent.ok_or("stopped guest parent")?;
                                    assert!(parent.document.index() < id.index());
                                    assert!(failure.document(parent.document).is_some());
                                }
                            }
                        }
                    }
                    assert_eq!(document, original);
                }
            }
            Ok(())
        },
    )
}
