use super::*;
use crate::doc::export::pages::discovery;
use nepl3_core::budget::StopReason;
use nepl3_doc_core::model::EmbedRef;

#[test]
fn namespace_discovery_resolves_forward_reference_across_distinct_slots() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence sentence cons doc ref second text "Go" nil body
        cons paragraph cons sentence sentence cons doc anchor first text "First" cons doc anchor second text "Second" nil cons sentence "Tail" nil nil"#;
    with_named_input(
        true,
        &compiled,
        source,
        "forward",
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
            let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
                kind: "Form:DocumentInline",
                guest_schema: &compiled.doc.package.schema,
                guest_category: "Inline",
            }];
            let found = discovery::collect(
                &document,
                &compiled.others[3].schema,
                &compiled.doc.package.schema,
                &forms,
                registry,
                &mut codec,
                b,
            )
            .map_err(err)?;
            let plan =
                discovery::namespace::inspect(&found, registry, b, &mut SourceAdmission::default())
                    .map_err(err)?;
            assert_eq!(
                plan.occurrences()
                    .iter()
                    .map(|o| o.document.index())
                    .collect::<Vec<_>>(),
                [0, 1, 2, 3]
            );
            assert_eq!(plan.occurrences()[1].parent.ok_or("title")?.sentence, 0);
            assert_eq!(
                plan.occurrences()[2].parent.ok_or("first anchor")?.sentence,
                1
            );
            assert_eq!(
                plan.occurrences()[3]
                    .parent
                    .ok_or("second anchor")?
                    .sentence,
                1
            );
            let refs = plan.member_refs(b).map_err(err)?;
            let checked = namespace::resolve(&refs, b).map_err(err)?;
            assert_eq!(checked.definitions().len(), 2);
            assert_eq!(checked.references().len(), 1);
            let reference = &checked.references()[0];
            assert_eq!(reference.reference.member.0, 1);
            assert_eq!(
                checked.definitions()[reference.target.0 as usize].member.0,
                3
            );
            Ok(())
        },
    )
}

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
            let mut namespace_budget = budget();
            let plan = discovery::namespace::inspect(
                &result,
                registry,
                &mut namespace_budget,
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            assert!(core::ptr::eq(plan.input(), &result));
            assert_eq!(
                plan.occurrences()
                    .iter()
                    .map(|o| o.document.index())
                    .collect::<Vec<_>>(),
                [0, 1, 2, 1, 2]
            );
            assert!(plan.occurrences()[0].parent.is_none());
            for (at, parent, sentence) in [(1, 0, 1), (2, 1, 0), (3, 0, 2), (4, 3, 0)] {
                let actual = plan.occurrences()[at].parent.ok_or("namespace parent")?;
                assert_eq!(actual.member.0, parent);
                assert_eq!(actual.sentence, sentence);
                assert_eq!(actual.guest, 0);
            }
            let refs = plan.member_refs(&mut budget()).map_err(err)?;
            assert!(core::ptr::eq(refs[1], refs[3]));
            assert!(core::ptr::eq(refs[2], refs[4]));
            // Repeated display of the same anchor is a real namespace duplicate.
            assert!(matches!(namespace::resolve(&refs, &mut budget()),
            Err(namespace::Error::Duplicate { definition, previous }) if definition.member.0 == 3 && previous.member.0 == 1));
            for (reason, amount) in [
                (StopReason::WorkLimit, namespace_budget.usage().work),
                (
                    StopReason::AllocationLimit,
                    namespace_budget.usage().allocation_units,
                ),
                (StopReason::DepthLimit, namespace_budget.usage().depth),
            ] {
                for shortage in [0, 1] {
                    let mut limits = namespace_budget.limits();
                    match reason {
                        StopReason::WorkLimit => limits.work = amount - shortage,
                        StopReason::AllocationLimit => limits.allocation_units = amount - shortage,
                        StopReason::DepthLimit => limits.depth = amount - shortage,
                        _ => return Err("namespace resource".into()),
                    }
                    let outcome = discovery::namespace::inspect(
                        &result,
                        registry,
                        &mut Budget::new(limits),
                        &mut SourceAdmission::default(),
                    );
                    if shortage == 0 {
                        assert_eq!(outcome.map_err(err)?.occurrences(), plan.occurrences());
                    } else {
                        assert!(
                            matches!(outcome, Err(discovery::namespace::Error::Stopped(actual)) if actual == reason)
                        );
                    }
                }
            }
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
            let mut rebased = budget();
            let rebased_plan = discovery::namespace::inspect(
                &nested_result,
                registry,
                &mut rebased,
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            assert_eq!(rebased_plan.occurrences(), plan.occurrences());
            assert_eq!(rebased.usage().depth, namespace_budget.usage().depth);
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
