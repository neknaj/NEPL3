use super::*;
use alloc::{format, string::String, vec};
use nepl3_core::{budget::Limits, source::SourceId};

fn index_width() -> usize {
    core::mem::size_of::<usize>().min(core::mem::size_of::<u16>())
}
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 100,
        nodes: 10_000,
        allocation_units: 1_000_000,
        output_bytes: 1_000_000,
        diagnostics: 10,
        events: 10,
    })
}
fn sample(name: &str, revision: u64, uri: &str, byte: u8) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId(name.into()),
        revision,
        uri.into(),
        vec![byte],
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))
}
fn cloned_bytes(source: &SourceSnapshot) -> u64 {
    if cfg!(target_has_atomic = "ptr") {
        0
    } else {
        (source.identity().source.0.len() + source.uri().len() + source.text().len()) as u64
    }
}
#[test]
fn request_source_union_preserves_first_identity_order_without_granting_admission()
-> Result<(), String> {
    let z = sample("z", 1, "memory:first", b'x')?;
    let a = sample("a", 1, "memory:a", b'x')?;
    let equal_id_other_uri = sample("z", 1, "memory:other", b'x')?;
    let revision = sample("z", 2, "memory:first", b'x')?;
    let digest = sample("z", 1, "memory:first", b'y')?;
    // Current collectors need not be unique or contain the context prefix.
    // Identity-only selection keeps the first URI; this is NOT a validator.
    let actual = copy_sources(
        [&z, &a, &z, &equal_id_other_uri, &revision, &digest, &a],
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(actual, vec![z, a, revision, digest]);
    assert_eq!(actual[0].uri(), "memory:first");
    Ok(())
}
#[test]
fn request_source_lookup_charges_shared_and_independent_ids_with_exact_sticky_stops()
-> Result<(), String> {
    let name = "界🙂".repeat(8);
    let first = sample(&name, 7, "memory:first", b'x')?;
    let independent = sample(&name, 7, "memory:other", b'x')?;
    let bytes = cloned_bytes(&first);
    let independent_work = name.len() as u64 + 41;
    let repeated_work = if cfg!(target_has_atomic = "ptr") {
        1
    } else {
        independent_work
    };
    let work = bytes + 13 + repeated_work + independent_work;
    let allocation = (core::mem::size_of::<SourceSnapshot>()
        + core::mem::size_of::<index::Index>()
        + 5 * index_width()) as u64
        + bytes;
    let limits = Limits {
        work,
        allocation_units: allocation,
        ..budget().limits()
    };
    let mut exact = Budget::new(limits);
    let actual =
        copy_sources([&first, &first, &independent], &mut exact).map_err(|e| format!("{e:?}"))?;
    assert_eq!(actual, vec![first.clone()]);
    assert_eq!(exact.usage().work, work);
    assert_eq!(exact.usage().allocation_units, allocation);
    for (limits, reason) in [
        (
            Limits {
                work: work - 1,
                ..limits
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: allocation - 1,
                ..limits
            },
            StopReason::AllocationLimit,
        ),
    ] {
        let mut short = Budget::new(limits);
        assert_eq!(
            copy_sources([&first, &first, &independent], &mut short),
            Err(reason)
        );
        assert_eq!(short.poll(), Err(reason));
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        copy_sources([&first], &mut cancelled),
        Err(StopReason::Cancelled)
    );
    assert_eq!(
        copy_sources(core::iter::empty(), &mut cancelled),
        Err(StopReason::Cancelled)
    );
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    Ok(())
}
#[test]
fn request_source_hash_collisions_keep_all_distinct_ids_and_meter_comparisons() -> Result<(), String>
{
    for count in [1usize, 4, 8] {
        let mut sources = Vec::new();
        for index in 0..count {
            sources.push(sample(&format!("s{index}"), 0, "memory:source", b'x')?);
        }
        let mut measured = budget();
        let actual =
            copy_sources_with_hash(&sources, |_| 0, &mut measured).map_err(|e| format!("{e:?}"))?;
        assert_eq!(actual, sources);
        let comparisons = (count * (count - 1) / 2) as u64;
        assert!(measured.usage().work >= comparisons * 43);
        let mut short = Budget::new(Limits {
            work: measured.usage().work - 1,
            ..budget().limits()
        });
        assert_eq!(
            copy_sources_with_hash(&sources, |_| 0, &mut short),
            Err(StopReason::WorkLimit)
        );
        assert_eq!(short.poll(), Err(StopReason::WorkLimit));
    }
    Ok(())
}

struct NoHint<I>(I);
impl<I: Iterator> Iterator for NoHint<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
#[test]
fn request_source_growth_reuses_slots_and_charges_every_forced_collision() -> Result<(), String> {
    let sources: Vec<_> = (0..8)
        .map(|i| sample(&format!("s{i}"), 0, "memory:source", b'x'))
        .collect::<Result<_, _>>()?;
    let bytes: u64 = sources.iter().map(cloned_bytes).sum();
    let allocation = (core::mem::size_of::<index::Index>()
        + 16 * index_width()
        + 8 * core::mem::size_of::<SourceSnapshot>()) as u64
        + bytes;
    let mut hinted = budget();
    assert_eq!(
        copy_sources_with_hash(&sources, |_| 0, &mut hinted).map_err(|e| format!("{e:?}"))?,
        sources
    );
    // Header 1, initialize 12, 8 input hashes, 28 collisions at (1+43)
    // Work, 8 vacant probes, 8 clone slots, plus owned bytes off atomic targets.
    assert_eq!(hinted.usage().work, 1269 + bytes);
    assert_eq!(
        hinted.usage().allocation_units,
        allocation - 4 * index_width() as u64
    );
    let mut grown = budget();
    assert_eq!(
        copy_sources_with_hash(NoHint(sources.iter()), |_| 0, &mut grown)
            .map_err(|e| format!("{e:?}"))?,
        sources
    );
    // Growth at 1,3,6 unique entries: 40 reset/relocation Work, 25 rehash
    // Work, 10 fresh insertion probes; first empty lookup uses no probe.
    assert_eq!(grown.usage().work, 1331 + bytes);
    assert_eq!(grown.usage().allocation_units, allocation);
    // Each stopping point keeps its original cause; no partial Vec is returned.
    for cap in 0..=grown.usage().work {
        let mut b = Budget::new(Limits {
            work: cap,
            ..budget().limits()
        });
        let result = copy_sources_with_hash(NoHint(sources.iter()), |_| 0, &mut b);
        if cap < grown.usage().work {
            assert_eq!(result, Err(StopReason::WorkLimit));
            assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        } else {
            assert_eq!(result.map_err(|e| format!("{e:?}"))?, sources);
        }
    }
    let mut short = Budget::new(Limits {
        allocation_units: allocation - 1,
        ..budget().limits()
    });
    assert_eq!(
        copy_sources_with_hash(NoHint(sources.iter()), |_| 0, &mut short),
        Err(StopReason::AllocationLimit)
    );
    assert_eq!(short.poll(), Err(StopReason::AllocationLimit));
    Ok(())
}
#[test]
fn request_source_permutations_and_duplicate_positions_preserve_native_order() -> Result<(), String>
{
    let sources: Vec<_> = (0..4)
        .map(|i| sample(&format!("s{i}"), 0, "memory:source", b'x'))
        .collect::<Result<_, _>>()?;
    let independent = sample("s0", 0, "memory:different-uri", b'x')?;
    let mut cases = 0;
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if (0..4).any(|i| (i + 1..4).any(|j| order[i] == order[j])) {
                        continue;
                    }
                    let expected: Vec<_> = order.iter().map(|&i| sources[i].clone()).collect();
                    let input = order.iter().map(|&i| &sources[i]).chain([
                        &sources[a],
                        &sources[c],
                        &sources[d],
                        &independent,
                    ]);
                    for collision in [false, true] {
                        let actual = copy_sources_with_hash(
                            NoHint(input.clone()),
                            |s| {
                                if collision {
                                    0
                                } else {
                                    s.identity_lookup_hash()
                                }
                            },
                            &mut budget(),
                        )
                        .map_err(|e| format!("{e:?}"))?;
                        assert_eq!(actual, expected);
                    }
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 24);
    Ok(())
}

#[test]
fn request_source_nonpower_table_wraps_and_charges_unused_hint_slots() -> Result<(), String> {
    let sources: Vec<_> = (0..8)
        .map(|i| sample(&format!("s{i}"), 0, "memory:source", b'x'))
        .collect::<Result<_, _>>()?;
    for count in 0..=8 {
        let input = &sources[..count];
        let mut b = budget();
        let actual = copy_sources_with_hash(input, |_| 4, &mut b).map_err(|e| format!("{e:?}"))?;
        assert_eq!(actual, input);
        let capacity = if count == 0 {
            0
        } else {
            (3 * count).div_ceil(2).max(4)
        };
        let copies: u64 = input
            .iter()
            .map(|s| core::mem::size_of::<SourceSnapshot>() as u64 + cloned_bytes(s))
            .sum();
        assert_eq!(
            b.usage().allocation_units,
            core::mem::size_of::<index::Index>() as u64
                + (capacity * index_width()) as u64
                + copies
        );
        if count == 3 {
            // Five slots, forced bucket four: insertion/missing lookup crosses 4→0.
            assert_eq!(
                b.usage().work,
                147 + input.iter().map(cloned_bytes).sum::<u64>()
            );
        }
    }
    let mut duplicate = budget();
    let actual = copy_sources_with_hash(
        [&sources[0], &sources[1], &sources[1]],
        |_| 4,
        &mut duplicate,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(actual, sources[..2]);
    let equality = if cfg!(target_has_atomic = "ptr") {
        1
    } else {
        43
    };
    assert_eq!(
        duplicate.usage().work,
        102 + equality + sources[..2].iter().map(cloned_bytes).sum::<u64>()
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        copy_sources_with_hash(core::iter::empty(), |_| 4, &mut cancelled),
        Err(StopReason::Cancelled)
    );
    assert_eq!(cancelled.usage().allocation_units, 0);
    Ok(())
}

#[test]
fn tiny_source_union_keeps_first_identity_and_charges_all_boundaries() -> Result<(), String> {
    let first = sample("界🙂", 7, "memory:first", b'x')?;
    let same = sample("界🙂", 7, "memory:other", b'x')?;
    let distinct = sample("different", 7, "memory:distinct", b'x')?;
    for inputs in [
        vec![],
        vec![&first],
        vec![&first, &first],
        vec![&first, &same],
        vec![&first, &distinct],
    ] {
        let mut measured = budget();
        let result =
            copy_sources(inputs.iter().copied(), &mut measured).map_err(|e| format!("{e:?}"))?;
        let expected = if inputs.is_empty() {
            vec![]
        } else if inputs.last() == Some(&&distinct) {
            vec![first.clone(), distinct.clone()]
        } else {
            vec![first.clone()]
        };
        assert_eq!(result, expected);
        let bytes: u64 = result.iter().map(cloned_bytes).sum();
        assert_ne!(
            first.identity_lookup_hash(),
            distinct.identity_lookup_hash()
        );
        let compare = if inputs.len() == 2 && result.len() == 1 {
            if cfg!(target_has_atomic = "ptr") && core::ptr::eq(inputs[0], inputs[1]) {
                1
            } else {
                first.identity().source.0.len() as u64 + 41
            }
        } else {
            0
        };
        let expected_work = 1
            + inputs.len() as u64
            + result.len() as u64
            + bytes
            + u64::from(inputs.len() == 2)
            + compare;
        assert_eq!(measured.usage().work, expected_work);
        assert_eq!(
            measured.usage().allocation_units,
            (core::mem::size_of::<Vec<SourceSnapshot>>()
                + result.len() * core::mem::size_of::<SourceSnapshot>()) as u64
                + bytes
        );
        for resource in [Resource::Work, Resource::AllocationUnits] {
            let usage = measured.usage();
            let limits = Limits {
                work: if resource == Resource::Work {
                    usage.work - 1
                } else {
                    usage.work
                },
                allocation_units: if resource == Resource::AllocationUnits {
                    usage.allocation_units - 1
                } else {
                    usage.allocation_units
                },
                ..budget().limits()
            };
            let reason = if resource == Resource::Work {
                StopReason::WorkLimit
            } else {
                StopReason::AllocationLimit
            };
            let mut short = Budget::new(limits);
            assert_eq!(
                copy_sources(inputs.iter().copied(), &mut short),
                Err(reason)
            );
            assert_eq!(short.poll(), Err(reason));
        }
    }
    Ok(())
}
