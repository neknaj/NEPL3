use super::*;
use alloc::{format, vec};
use nepl3_core::{budget::Limits, source::SourceId};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn source(name: &str, revision: u64, uri: &str, byte: u8) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(name.into()),
        revision,
        uri.into(),
        vec![byte],
        &mut budget(),
    )
}

#[test]
fn ordered_closure_duplicates_fit_linear_comparison_allowance() -> Result<(), String> {
    let sources = (0..512)
        .map(|i| source(&format!("s{i:07}"), 0, "mem:xx", b'x'))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{e:?}"))?;
    let mut arena = ParseArena {
        sources: sources.clone(),
        ..ParseArena::default()
    };
    // Eight-byte keys cost 17 per comparison. Rebuilding costs <512*17;
    // sequential duplicate lookup needs at most two comparisons per entry
    // plus the initial binary search. Every duplicate also pays 2*6+33 for
    // URI/digest validation. 50,000 allows that complete linear traversal.
    let mut bounded = Budget::new(Limits {
        work: 50_000,
        ..budget().limits()
    });
    arena
        .extend_sources(&sources, &mut bounded)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(arena.sources, sources);
    Ok(())
}

#[test]
fn closure_merge_preserves_source_order_full_identity_and_conflicts() -> Result<(), String> {
    let originals = [
        source("z", 0, "memory:z", b'z'),
        source("日本語", 0, "memory:a", b'a'),
        source("日本語", u64::MAX, "memory:b", b'b'),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| format!("{e:?}"))?;
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut arena = ParseArena::default();
        for i in order {
            arena
                .extend_sources(&originals[i..=i], &mut budget())
                .map_err(|e| format!("{e:?}"))?;
        }
        let expected = order.map(|i| originals[i].clone()).to_vec();
        arena
            .extend_sources(&originals, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(arena.sources, expected);
        for (uri, byte) in [("changed:uri", b'a'), ("memory:a", b'x')] {
            let conflict = source("日本語", 0, uri, byte).map_err(|e| format!("{e:?}"))?;
            assert_eq!(
                arena.extend_sources(&[conflict], &mut budget()),
                Err(SyntaxError::Source(SourceError::IdentityConflict))
            );
            assert_eq!(arena.sources, expected);
        }
        let mut stopped = Budget::new(Limits {
            work: 0,
            ..budget().limits()
        });
        assert!(arena.extend_sources(&originals, &mut stopped).is_err());
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        assert_eq!(arena.sources, expected);
    }
    Ok(())
}

#[test]
fn session_index_appends_each_closure_without_rebuilding_prior_sources() -> Result<(), String> {
    let sources = (0..512)
        .map(|i| source(&format!("s{i:07}"), 0, "mem:x", b'x'))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{e:?}"))?;
    let mut arena = ParseArena::default();
    let mut index =
        SourceIndex::new(&arena.sources, &mut budget()).map_err(|e| format!("{e:?}"))?;
    // Ordered one-at-a-time additions require one key comparison and one
    // snapshot clone each. Rebuilding every prior prefix costs O(n squared).
    let mut b = Budget::new(Limits {
        work: 50_000,
        ..budget().limits()
    });
    for input in &sources {
        arena
            .extend_sources_indexed(core::slice::from_ref(input), &mut index, &mut b)
            .map_err(|e| format!("{e:?}; {:?}", b.usage()))?;
    }
    assert_eq!(arena.sources, sources);
    for (i, source) in sources.iter().enumerate() {
        assert_eq!(
            source_position(&index.entries, &arena.sources, source, None, &mut budget())
                .map_err(|e| format!("{e:?}"))?,
            Ok(i)
        );
    }
    Ok(())
}

#[test]
fn session_index_stops_and_conflicts_keep_source_index_pairs_consistent() -> Result<(), String> {
    let inputs = [
        source("z", 0, "mem:z", b'z'),
        source("a", 0, "mem:a", b'a'),
        source("m", 0, "mem:m", b'm'),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| format!("{e:?}"))?;
    for allowance in 0..180 {
        for resource in [Resource::Work, Resource::AllocationUnits] {
            let mut arena = ParseArena::default();
            let mut index = SourceIndex::new(&[], &mut budget()).map_err(|e| format!("{e:?}"))?;
            let mut limits = budget().limits();
            match resource {
                Resource::Work => limits.work = allowance,
                _ => limits.allocation_units = allowance,
            }
            let mut b = Budget::new(limits);
            let result = arena.extend_sources_indexed(&inputs, &mut index, &mut b);
            assert_eq!(index.entries.len(), arena.sources.len());
            assert_eq!(arena.sources, inputs[..arena.sources.len()]);
            for (i, snapshot) in arena.sources.iter().enumerate() {
                let at = source_position(
                    &index.entries,
                    &arena.sources,
                    snapshot,
                    None,
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?
                .map_err(|_| "missing source")?;
                assert_eq!(index.entries[at], i);
            }
            if result.is_err() {
                let usage = b.usage();
                assert!(
                    arena
                        .extend_sources_indexed(&inputs, &mut index, &mut b)
                        .is_err()
                );
                assert_eq!(usage, b.usage());
            }
        }
    }
    let mut arena = ParseArena::default();
    let mut index = SourceIndex::new(&[], &mut budget()).map_err(|e| format!("{e:?}"))?;
    arena
        .extend_sources_indexed(&inputs, &mut index, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    for (uri, byte) in [("mem:changed", b'a'), ("mem:a", b'x')] {
        let conflict = source("a", 0, uri, byte).map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            arena.extend_sources_indexed(&[conflict], &mut index, &mut budget()),
            Err(SyntaxError::Source(SourceError::IdentityConflict))
        );
        assert_eq!(arena.sources, inputs);
    }
    Ok(())
}
