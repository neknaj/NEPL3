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
