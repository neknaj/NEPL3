use super::*;
use sentence::document_guests::{self as guests, OwnerId, PlacedError, Placement};

#[test]
fn placed_selection_retains_owners_sharing_and_display_order() -> Result<(), String> {
    let (r, surface, input) = retained::fixture()?;
    let inputs = [input.clone(), input];
    let placements = [
        Placement {
            owner: OwnerId(1),
            depth: 2,
        },
        Placement {
            owner: OwnerId(0),
            depth: 7,
        },
        Placement {
            owner: OwnerId(1),
            depth: 9,
        },
    ];
    let store = nepl3_core::source::SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let result = guests::collect_placed(&inputs, &placements, &surface, &r, &mut codec, &mut b())
        .map_err(err)?;
    assert_eq!(result.selections().len(), 2);
    assert_eq!(result.placements(), &placements);
    assert_eq!(result.occurrences().len(), 9);
    assert!(core::ptr::eq(
        result.input(OwnerId(0)).ok_or("owner")?,
        &inputs[0]
    ));
    for (i, group) in result.occurrences().chunks(3).enumerate() {
        assert_eq!(group.len(), 3);
        for occurrence in group {
            assert_eq!(occurrence.placement, i);
            assert_eq!(occurrence.document.owner, placements[i].owner);
            assert_eq!(occurrence.document.document.index(), 0);
            assert_eq!(occurrence.embed, EmbedRef(0));
        }
    }
    let first = result.occurrences()[0].document;
    let second = result.occurrences()[3].document;
    let repeated = result.occurrences()[6].document;
    assert_eq!(first, repeated);
    assert_ne!(
        first, second,
        "equal input bytes do not merge separate owners"
    );
    assert!(core::ptr::eq(
        result.document(first).ok_or("doc")?,
        result.document(repeated).ok_or("repeat")?
    ));
    assert!(!core::ptr::eq(
        result.document(first).ok_or("doc")?,
        result.document(second).ok_or("separate")?
    ));
    // The selected documents are real namespace inputs, not successful stubs.
    let mut members = Vec::new();
    for occurrence in result.occurrences() {
        members.push(
            nepl3_doc_core::labels::namespace::inspect(
                result.document(occurrence.document).ok_or("document")?,
                &r,
                &mut b(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?,
        );
    }
    let borrowed: Vec<_> = members.iter().collect();
    let checked = nepl3_doc_core::labels::namespace::resolve(&borrowed, &mut b()).map_err(err)?;
    assert_eq!(checked.documents().len(), 9);
    Ok(())
}

#[test]
fn placed_selection_rejects_missing_unused_and_invalid_owners() -> Result<(), String> {
    let (r, surface, input) = retained::fixture()?;
    let store = nepl3_core::source::SourceStore::default();
    let run = |inputs: &[SentenceSyntax], placements: &[Placement], budget: &mut Budget| {
        let mut admission = SourceAdmission::default();
        let mut codec = nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission)
            .map_err(err)?;
        Ok::<_, String>(
            guests::collect_placed(inputs, placements, &surface, &r, &mut codec, budget).err(),
        )
    };
    let mut inputs = [input];
    assert!(matches!(
        run(&inputs, &[], &mut b())?,
        Some(PlacedError::UnusedOwner(OwnerId(0)))
    ));
    assert!(matches!(
        run(
            &inputs,
            &[Placement {
                owner: OwnerId(1),
                depth: 0
            }],
            &mut b()
        )?,
        Some(PlacedError::MissingOwner(OwnerId(1)))
    ));
    inputs[0].locations.clear();
    assert!(matches!(
        run(
            &inputs,
            &[Placement {
                owner: OwnerId(0),
                depth: 0
            }],
            &mut b()
        )?,
        Some(PlacedError::Collection {
            owner: OwnerId(0),
            error: guests::Error::Sentence(_)
        })
    ));
    let mut stopped = b();
    stopped.cancel();
    assert!(matches!(
        run(&[], &[], &mut stopped)?,
        Some(PlacedError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}

#[test]
fn placed_selection_uses_deepest_occurrence_and_checks_overflow() -> Result<(), String> {
    let (r, surface, input) = retained::fixture()?;
    let inputs = [input];
    let store = nepl3_core::source::SourceStore::default();
    let run = |placements: &[Placement], budget: &mut Budget| {
        let mut admission = SourceAdmission::default();
        let mut codec = nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission)
            .map_err(err)?;
        Ok::<_, String>(budget.with_depth_at_least(3, |b| {
            guests::collect_placed(&inputs, placements, &surface, &r, &mut codec, b).map(|_| ())
        }))
    };
    let placements = [
        Placement {
            owner: OwnerId(0),
            depth: 0,
        },
        Placement {
            owner: OwnerId(0),
            depth: 9,
        },
    ];
    let mut measured = b();
    run(&placements, &mut measured)?.map_err(err)?;
    let depth = measured.usage().depth;
    assert!(depth > 12);
    for reversed in [false, true] {
        let mut order = placements;
        if reversed {
            order.reverse();
        }
        let mut limited = Budget::new(Limits {
            depth: depth - 1,
            ..b().limits()
        });
        assert!(matches!(
            run(&order, &mut limited)?,
            Err(PlacedError::Stopped(StopReason::DepthLimit))
        ));
        let mut exact = Budget::new(Limits {
            depth,
            ..b().limits()
        });
        run(&order, &mut exact)?.map_err(err)?;
    }
    assert!(matches!(
        run(
            &[Placement {
                owner: OwnerId(0),
                depth: u64::MAX
            }],
            &mut b()
        )?,
        Err(PlacedError::Stopped(StopReason::DepthLimit))
    ));
    Ok(())
}

#[test]
fn placed_selection_keeps_budget_edges_and_reuses_lowered_owners() -> Result<(), String> {
    let (r, surface, input) = retained::fixture()?;
    let inputs = [input];
    let store = nepl3_core::source::SourceStore::default();
    let run = |placements: &[Placement], budget: &mut Budget| {
        let mut admission = SourceAdmission::default();
        let mut codec = nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission)
            .map_err(err)?;
        Ok::<_, String>(
            guests::collect_placed(&inputs, placements, &surface, &r, &mut codec, budget)
                .map(|result| result.occurrences().len()),
        )
    };
    let one = [Placement {
        owner: OwnerId(0),
        depth: 2,
    }];
    let repeated = [one[0]; 64];
    let mut first = b();
    assert_eq!(run(&one, &mut first)?.map_err(err)?, 3);
    let mut all = b();
    assert_eq!(run(&repeated, &mut all)?.map_err(err)?, 192);
    let small = first.usage();
    let used = all.usage();
    // Added placements incur only placement/occurrence bookkeeping. The 12 KiB
    // guest source and its lower/validation are not repeated per placement.
    assert_eq!(used.source_bytes, small.source_bytes);
    assert_eq!(used.nodes, small.nodes);
    assert!(used.work <= small.work + 64 * 16, "{small:?} -> {used:?}");
    assert!(
        used.allocation_units <= small.allocation_units + 64 * 1024,
        "{small:?} -> {used:?}"
    );
    for resource in [
        nepl3_core::budget::Resource::Work,
        nepl3_core::budget::Resource::AllocationUnits,
        nepl3_core::budget::Resource::Nodes,
        nepl3_core::budget::Resource::SourceBytes,
    ] {
        use nepl3_core::budget::Resource;
        let (amount, reason) = match resource {
            Resource::Work => (used.work, StopReason::WorkLimit),
            Resource::AllocationUnits => (used.allocation_units, StopReason::AllocationLimit),
            Resource::Nodes => (used.nodes, StopReason::NodeLimit),
            Resource::SourceBytes => (used.source_bytes, StopReason::SourceLimit),
            _ => unreachable!(),
        };
        assert!(amount > 0);
        for exact in [false, true] {
            let limit = amount - u64::from(!exact);
            let mut limits = b().limits();
            match resource {
                Resource::Work => limits.work = limit,
                Resource::AllocationUnits => limits.allocation_units = limit,
                Resource::Nodes => limits.nodes = limit,
                Resource::SourceBytes => limits.source_bytes = limit,
                _ => unreachable!(),
            }
            let mut budget = Budget::new(limits);
            let result = run(&repeated, &mut budget)?;
            if exact {
                assert_eq!(result.map_err(err)?, 192);
            } else {
                assert!(
                    matches!(result, Err(PlacedError::Stopped(actual)) if actual == reason),
                    "{resource:?}: {result:?}"
                );
                assert_eq!(budget.poll(), Err(reason));
            }
        }
    }
    Ok(())
}

#[test]
fn placed_selection_validates_unselected_foreign_closure() -> Result<(), String> {
    let (r, _, mut input) = retained::fixture()?;
    let surface = r.selected("nepl3.doc", 1).ok_or("different Doc surface")?;
    input.value.embeds.entries[0].syntax.bundle.root = NodeRef(u64::MAX);
    let inputs = [input];
    let store = nepl3_core::source::SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    assert!(matches!(
        guests::collect_placed(
            &inputs,
            &[Placement {
                owner: OwnerId(0),
                depth: 0
            }],
            surface,
            &r,
            &mut codec,
            &mut b()
        ),
        Err(PlacedError::Collection {
            owner: OwnerId(0),
            error: guests::Error::Sentence(_)
        })
    ));
    Ok(())
}
