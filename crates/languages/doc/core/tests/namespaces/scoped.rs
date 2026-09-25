use super::*;
use pagespaces::{NamespaceDocument, ScopedError};

fn selected(document: &DocumentSyntax) -> NamespaceDocument<'_> {
    NamespaceDocument {
        document,
        relative_depth: 0,
    }
}

#[test]
fn scoped_resolution_preserves_full_plan_and_reduces_revalidation() -> Result<(), String> {
    let r = registry()?;
    let set = set(&r)?;
    let link = inline(
        DocKind::Link {
            target: LinkTarget::Page {
                page: "second".into(),
                fragment: Some("target".into()),
            },
            label: EmbedRef(0),
        },
        &r,
    )?;
    let anchor = inline(
        DocKind::Anchor {
            id: "target".into(),
            label: EmbedRef(0),
        },
        &r,
    )?;
    let first = [
        selected(&set.pages[0].document),
        selected(&link),
        selected(&link),
    ];
    let second = [selected(&set.pages[1].document), selected(&anchor)];
    let selection = [&first[..], &second[..]];
    let store = SourceStore::default();
    let mut baseline = b();
    let mut admission = SourceAdmission::default();
    let members = selection
        .iter()
        .map(|page| {
            page.iter()
                .map(|item| {
                    names::inspect(item.document, &r, &mut baseline, &mut admission).map_err(err)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let refs = members
        .iter()
        .map(|page| page.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let namespaces = refs
        .iter()
        .map(|page| names::resolve(page, &mut baseline).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let refs = namespaces.iter().collect::<Vec<_>>();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let proof = pagespaces::resolve(&set, &refs, &r, &mut codec, &mut baseline).map_err(err)?;
    let expected =
        portable::pages::namespace::to_value(&proof, &r, &mut codec, &mut baseline).map_err(err)?;
    let run = |budget: &mut Budget| {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        pagespaces::with_resolved(
            &set,
            &selection,
            &r,
            &mut codec,
            budget,
            |proof, codec, budget| {
                assert_eq!(proof.members().len(), 5);
                assert_eq!(proof.members()[1].links(), proof.members()[2].links());
                portable::pages::namespace::to_value(proof, &r, codec, budget)
            },
        )
        .map_err(err)
    };
    let mut measured = b();
    assert_eq!(run(&mut measured)?, expected);
    assert!(measured.usage().work < baseline.usage().work);
    assert!(measured.usage().allocation_units < baseline.usage().allocation_units);
    let exact = Limits {
        work: measured.usage().work,
        allocation_units: measured.usage().allocation_units,
        ..b().limits()
    };
    assert_eq!(run(&mut Budget::new(exact))?, expected);
    let retained = |budget: &mut Budget, source_registry: &SchemaRegistry| {
        let roots = set
            .pages
            .iter()
            .map(|page| {
                nepl3_doc_core::check::RegistryValidatedDocumentSyntax::new(
                    &page.document,
                    source_registry,
                    &mut b(),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        pagespaces::with_validated_roots(
            &set,
            &selection,
            &r,
            &mut codec,
            budget,
            roots,
            |proof, codec, budget| portable::pages::namespace::to_value(proof, &r, codec, budget),
        )
        .map_err(err)
    };
    let mut reused = b();
    assert_eq!(retained(&mut reused, &r)?, expected);
    assert!(reused.usage().work < measured.usage().work);
    assert_eq!(retained(&mut b(), &registry()?)?, expected);
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::SourceLimit,
    ] {
        for below in [false, true] {
            let mut limits = b().limits();
            let used = reused.usage();
            let (limit, used) = match reason {
                StopReason::WorkLimit => (&mut limits.work, used.work),
                StopReason::AllocationLimit => {
                    (&mut limits.allocation_units, used.allocation_units)
                }
                StopReason::DepthLimit => (&mut limits.depth, used.depth),
                StopReason::SourceLimit => (&mut limits.source_bytes, used.source_bytes),
                _ => unreachable!(),
            };
            *limit = used
                .checked_sub(u64::from(below))
                .ok_or("nonzero resource")?;
            let mut receiver = Budget::new(limits);
            let result = retained(&mut receiver, &r);
            if below {
                assert!(result.is_err());
                assert_eq!(receiver.poll(), Err(reason));
            } else {
                assert_eq!(result?, expected);
            }
        }
    }
    Ok(())
}

#[test]
fn retained_roots_reject_wrong_count_order_and_equal_cloned_documents() -> Result<(), String> {
    let r = registry()?;
    let set = set(&r)?;
    let cloned = set.clone();
    let selection = [
        [selected(&set.pages[0].document)],
        [selected(&set.pages[1].document)],
    ];
    let selection = [&selection[0][..], &selection[1][..]];
    for documents in [
        vec![],
        vec![&set.pages[0].document],
        vec![
            &set.pages[0].document,
            &set.pages[1].document,
            &set.pages[1].document,
        ],
        vec![&set.pages[1].document, &set.pages[0].document],
        vec![&cloned.pages[0].document, &cloned.pages[1].document],
        vec![&set.pages[0].document, &set.pages[1].document],
    ] {
        let valid = documents.len() == 2 && core::ptr::eq(documents[0], &set.pages[0].document);
        let roots = documents
            .into_iter()
            .map(|document| {
                nepl3_doc_core::check::RegistryValidatedDocumentSyntax::new(
                    document,
                    &r,
                    &mut b(),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let mut calls = 0;
        let result = pagespaces::with_validated_roots(
            &set,
            &selection,
            &r,
            &mut codec,
            &mut b(),
            roots,
            |_, _, _| {
                calls += 1;
                Ok::<_, ()>(())
            },
        );
        assert_eq!(result.is_ok(), valid);
        assert_eq!(calls, usize::from(valid));
    }
    Ok(())
}

#[test]
fn retained_roots_reject_missing_receiver_schema_before_callback() -> Result<(), String> {
    let r = registry()?;
    let set = set(&r)?;
    let first = [selected(&set.pages[0].document)];
    let second = [selected(&set.pages[1].document)];
    let store = SourceStore::default();
    let mut missing = SchemaRegistry::default();
    missing.finalize(&mut b()).map_err(err)?;
    for receiver in [&r, &missing] {
        let roots = set
            .pages
            .iter()
            .map(|page| {
                nepl3_doc_core::check::RegistryValidatedDocumentSyntax::new(
                    &page.document,
                    &r,
                    &mut b(),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut admission = SourceAdmission::default();
        // Keep the codec valid to isolate the receiving registry's rejection.
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let mut calls = 0;
        let result = pagespaces::with_validated_roots(
            &set,
            &[&first, &second],
            receiver,
            &mut codec,
            &mut b(),
            roots,
            |_, _, _| {
                calls += 1;
                Ok::<_, ()>(())
            },
        );
        let valid = core::ptr::eq(receiver, &r);
        assert_eq!(result.is_ok(), valid);
        assert_eq!(calls, usize::from(valid));
    }
    Ok(())
}

#[test]
fn scoped_resolution_rechecks_each_invocation_and_withholds_callback_on_failure()
-> Result<(), String> {
    let r = registry()?;
    let set = set(&r)?;
    let anchor = inline(
        DocKind::Anchor {
            id: "target".into(),
            label: EmbedRef(0),
        },
        &r,
    )?;
    let store = SourceStore::default();
    let root = selected(&set.pages[0].document);
    let other = selected(&set.pages[1].document);
    // Root ownership is exact; repeated anchors are separate display instances;
    // the final member is checked before any output can be published.
    for (first, second) in [
        (vec![other], vec![other]),
        (
            vec![root],
            vec![other, selected(&anchor), selected(&anchor)],
        ),
        (
            vec![root],
            vec![
                other,
                NamespaceDocument {
                    document: &anchor,
                    relative_depth: u64::MAX,
                },
            ],
        ),
    ] {
        let mut calls = 0;
        let mut budget = b();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        assert!(
            pagespaces::with_resolved(
                &set,
                &[&first, &second],
                &r,
                &mut codec,
                &mut budget,
                |_, _, _| {
                    calls += 1;
                    Ok::<_, ()>(())
                }
            )
            .is_err()
        );
        assert_eq!(calls, 0);
    }
    let first = [root];
    let second = [other];
    let selection = [&first[..], &second[..]];
    let mut measured = b();
    {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        pagespaces::with_resolved(
            &set,
            &selection,
            &r,
            &mut codec,
            &mut measured,
            |_, _, _| Ok::<_, ()>(()),
        )
        .map_err(err)?;
    }
    let usage = measured.usage();
    for cancel in [false, true] {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let mut budget = b();
        let mut calls = 0;
        let result = pagespaces::with_resolved(
            &set,
            &selection,
            &r,
            &mut codec,
            &mut budget,
            |_, _, budget| {
                calls += 1;
                if cancel {
                    budget.cancel();
                    Ok(())
                } else {
                    Err("output failure")
                }
            },
        );
        assert_eq!(calls, 1);
        if cancel {
            assert!(matches!(
                result,
                Err(ScopedError::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(budget.poll(), Err(StopReason::Cancelled));
        } else {
            assert!(matches!(result, Err(ScopedError::Output("output failure"))));
            assert_eq!(budget.poll(), Ok(()));
        }
    }
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::SourceLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::WorkLimit => limits.work = usage.work - 1,
            StopReason::AllocationLimit => limits.allocation_units = usage.allocation_units - 1,
            StopReason::NodeLimit => limits.nodes = usage.nodes - 1,
            StopReason::SourceLimit => limits.source_bytes = usage.source_bytes - 1,
            StopReason::DepthLimit => limits.depth = usage.depth - 1,
            _ => {}
        }
        let mut budget = Budget::new(limits);
        if reason == StopReason::Cancelled {
            budget.cancel();
        }
        let mut calls = 0;
        // A successful earlier invocation provides no admission for this one.
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let failure =
            pagespaces::with_resolved(&set, &selection, &r, &mut codec, &mut budget, |_, _, _| {
                calls += 1;
                Ok::<_, ()>(())
            });
        assert!(
            matches!(failure, Err(ScopedError::Stopped(actual)) if actual == reason),
            "{failure:?}"
        );
        assert_eq!(calls, 0);
        assert_eq!(budget.poll(), Err(reason));
    }
    Ok(())
}
