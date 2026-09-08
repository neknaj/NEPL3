use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Mapping, MappingKind, OriginError, SourceMap},
    source::{SourceId, SourceSnapshot, SourceStore},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn ordered_graph_hint_survives_insertions_before_and_after_cached_positions() -> TestResult {
    let mut sources = SourceStore::default();
    let mut vertices = Vec::new();
    for name in ["z", "a", "m", "b", "日本語"] {
        let snapshot = source(name, "x")?;
        sources
            .insert(snapshot.clone())
            .map_err(|e| format!("{e:?}"))?;
        vertices.push(snapshot);
    }
    let edge = |i: usize, j: usize| -> Result<Mapping, String> {
        Ok(Mapping {
            source: vertices[i].span(0, 1).map_err(|e| format!("{e:?}"))?,
            target: vertices[j].span(0, 1).map_err(|e| format!("{e:?}"))?,
            kind: MappingKind::Exact,
        })
    };
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if (0..4).any(|i| (0..i).any(|j| order[i] == order[j])) {
                        continue;
                    }
                    let mut maps = order
                        .into_iter()
                        .map(|i| edge(i, i + 1))
                        .collect::<Result<Vec<_>, _>>()?;
                    // A five-vertex chain is acyclic in all 24 edge orders,
                    // independently of source name order and index shifts.
                    for split in 0..=4 {
                        SourceMap::validate_mapping_parts(
                            &maps[..split],
                            &maps[split..],
                            &sources,
                            &mut budget(),
                        )
                        .map_err(|e| format!("{e:?}"))?;
                    }
                    maps.push(edge(4, 0)?);
                    assert!(matches!(
                        SourceMap::validate_mappings(&maps, &sources, &mut budget()),
                        Err(OriginError::Cycle)
                    ));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn indexed_map_graphs_agree_with_transitive_closure_in_both_orders() -> TestResult {
    // The oracle uses Boolean reachability, independently of the production
    // topological/pointwise algorithm. Distinct revisions remain distinct nodes.
    let mut sources = SourceStore::default();
    let mut vertices = Vec::new();
    for (name, revision) in [("z", 0), ("日本語", 7), ("日本語", 1)] {
        let snapshot = SourceSnapshot::new(
            SourceId(name.into()),
            revision,
            format!("memory:{name}:{revision}"),
            vec![b'x'],
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        sources
            .insert(snapshot.clone())
            .map_err(|e| format!("{e:?}"))?;
        vertices.push(snapshot);
    }
    for mask in 0u32..512 {
        let mut reach = [[false; 3]; 3];
        let mut mappings = Vec::new();
        for (i, row) in reach.iter_mut().enumerate() {
            for (j, reachable) in row.iter_mut().enumerate() {
                if mask & (1 << (3 * i + j)) != 0 {
                    *reachable = true;
                    mappings.push(Mapping {
                        source: vertices[i].span(0, 1).map_err(|e| format!("{e:?}"))?,
                        target: vertices[j].span(0, 1).map_err(|e| format!("{e:?}"))?,
                        kind: MappingKind::Exact,
                    });
                }
            }
        }
        for k in 0..3 {
            for i in 0..3 {
                for j in 0..3 {
                    reach[i][j] |= reach[i][k] && reach[k][j];
                }
            }
        }
        let expected = if (0..3).any(|i| reach[i][i]) {
            Err(OriginError::Cycle)
        } else {
            Ok(())
        };
        for _ in 0..2 {
            assert_eq!(
                SourceMap::validate_mappings(&mappings, &sources, &mut budget()).map(|_| ()),
                expected,
                "graph {mask}"
            );
            for split in 0..=mappings.len() {
                assert_eq!(
                    SourceMap::validate_mapping_parts(
                        &mappings[..split],
                        &mappings[split..],
                        &sources,
                        &mut budget()
                    )
                    .map(|_| ()),
                    expected,
                    "graph {mask}, split {split}"
                );
            }
            mappings.reverse();
        }
    }
    Ok(())
}

#[test]
fn borrowed_map_union_checks_containment_and_geometry_in_both_parts() -> TestResult {
    let mut sources = SourceStore::default();
    let vertices = [
        source("a", "x")?,
        source("b", "x")?,
        source("c", "x")?,
        source("other", "x")?,
        source("bad", "y")?,
    ];
    for vertex in &vertices {
        sources
            .insert(vertex.clone())
            .map_err(|e| format!("{e:?}"))?;
    }
    let span = |i: usize| vertices[i].span(0, 1).map_err(|e| format!("{e:?}"));
    let maps = [
        Mapping {
            source: span(0)?,
            target: span(1)?,
            kind: MappingKind::Exact,
        },
        Mapping {
            source: span(1)?,
            target: span(2)?,
            kind: MappingKind::Exact,
        },
        Mapping {
            source: span(3)?,
            target: span(2)?,
            kind: MappingKind::Exact,
        },
    ];
    for split in 0..=maps.len() {
        let checked = SourceMap::validate_mapping_parts(
            &maps[..split],
            &maps[split..],
            &sources,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert!(
            checked
                .contains(&span(0)?, &span(1)?, &mut budget())
                .map_err(|e| format!("{e:?}"))?
        );
        // One path to c reaches a, but another reaches the unrelated source.
        assert!(
            !checked
                .contains(&span(0)?, &span(2)?, &mut budget())
                .map_err(|e| format!("{e:?}"))?
        );
        let mut missing = SourceStore::default();
        for vertex in &vertices[..3] {
            missing
                .insert(vertex.clone())
                .map_err(|e| format!("{e:?}"))?;
        }
        assert!(matches!(
            checked.validate_sources(&missing, &mut budget()),
            Err(OriginError::Source(
                nepl3_core::source::SourceError::MissingSnapshot
            ))
        ));
    }
    let bad = [Mapping {
        source: span(0)?,
        target: span(4)?,
        kind: MappingKind::Exact,
    }];
    for (first, second) in [(&bad[..], &maps[..]), (&maps[..], &bad[..])] {
        assert_eq!(
            SourceMap::validate_mapping_parts(first, second, &sources, &mut budget()).map(|_| ()),
            Err(OriginError::Irreversible)
        );
    }
    Ok(())
}

#[test]
fn borrowed_union_does_not_copy_source_id_payloads() -> TestResult {
    let mut sources = SourceStore::default();
    let mut vertices = Vec::new();
    for suffix in ["a", "b", "c"] {
        let vertex = source(&("long".repeat(2500) + suffix), "x")?;
        sources
            .insert(vertex.clone())
            .map_err(|e| format!("{e:?}"))?;
        vertices.push(vertex);
    }
    let mut maps = Vec::new();
    for i in 0..2 {
        maps.push(Mapping {
            source: vertices[i].span(0, 1).map_err(|e| format!("{e:?}"))?,
            target: vertices[i + 1].span(0, 1).map_err(|e| format!("{e:?}"))?,
            kind: MappingKind::Exact,
        });
    }
    // An owned union would require over 40 KiB for the four SourceIds alone.
    // Only private graph traversal indices are needed for a borrowed proof.
    let mut b = Budget::new(Limits {
        allocation_units: 4096,
        ..budget().limits()
    });
    SourceMap::validate_mapping_parts(&maps[..1], &maps[1..], &sources, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(b.usage().nodes, 3);
    Ok(())
}
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}

#[test]
fn wide_snapshot_dag_work_does_not_scan_all_edges_for_each_vertex() -> TestResult {
    fn measure(width: usize) -> Result<u64, String> {
        let root = source("00000", "x")?;
        let mut sources = SourceStore::default();
        sources.insert(root.clone()).map_err(|e| format!("{e:?}"))?;
        let mut maps = Vec::new();
        for i in 1..=width {
            let leaf = source(&format!("{i:05}"), "x")?;
            maps.push(Mapping {
                source: root.span(0, 1).map_err(|e| format!("{e:?}"))?,
                target: leaf.span(0, 1).map_err(|e| format!("{e:?}"))?,
                kind: MappingKind::Exact,
            });
            sources.insert(leaf).map_err(|e| format!("{e:?}"))?;
        }
        // Repeated root identities must not require a full indexed lookup per
        // edge. The old uncached 2,048-edge star exceeds this operation budget.
        let mut b = Budget::new(Limits {
            work: 1_500_000,
            ..budget().limits()
        });
        SourceMap::validate_mappings(&maps, &sources, &mut b).map_err(|e| format!("{e:?}"))?;
        assert_eq!(b.usage().nodes, width as u64 + 1);
        assert_eq!(b.usage().depth, 2);
        Ok(b.usage().work)
    }
    // A star has V-1 edges and no cycle. Doubling its leaves should cost
    // approximately twice the work plus identity lookup growth, not four times.
    let small = measure(1024)?;
    let large = measure(2048)?;
    eprintln!("wide DAG work: {small} -> {large}");
    assert!(large * 10 < small * 27, "{small} -> {large}");
    Ok(())
}

#[test]
fn duplicate_edges_and_joins_preserve_longest_depth_and_sticky_stops() -> TestResult {
    let mut sources = SourceStore::default();
    let vertices = (0..5)
        .map(|i| source(&format!("v{i}"), "x"))
        .collect::<Result<Vec<_>, _>>()?;
    for vertex in &vertices {
        sources
            .insert(vertex.clone())
            .map_err(|e| format!("{e:?}"))?;
    }
    // 0->1->2->3 has depth four. The duplicate 0->1 and shorter 0->2
    // must neither enqueue a vertex early nor leave an indegree outstanding.
    // The independent 4->3 edge exercises a join across two roots.
    let mut maps = Vec::new();
    for (a, b) in [(0, 1), (0, 1), (1, 2), (0, 2), (2, 3), (4, 3)] {
        maps.push(Mapping {
            source: vertices[a].span(0, 1).map_err(|e| format!("{e:?}"))?,
            target: vertices[b].span(0, 1).map_err(|e| format!("{e:?}"))?,
            kind: MappingKind::Exact,
        });
    }
    for _ in 0..2 {
        let mut complete = budget();
        SourceMap::validate_mappings(&maps, &sources, &mut complete)
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(complete.usage().depth, 4);
        assert_eq!(complete.usage().nodes, 5);
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::NodeLimit,
            StopReason::DepthLimit,
        ] {
            let mut limits = budget().limits();
            let usage = complete.usage();
            match reason {
                StopReason::WorkLimit => limits.work = usage.work - 1,
                StopReason::AllocationLimit => limits.allocation_units = usage.allocation_units - 1,
                StopReason::NodeLimit => limits.nodes = usage.nodes - 1,
                StopReason::DepthLimit => limits.depth = usage.depth - 1,
                _ => unreachable!(),
            }
            let mut stopped = Budget::new(limits);
            assert_eq!(
                SourceMap::validate_mappings(&maps, &sources, &mut stopped).map(|_| ()),
                Err(OriginError::Stopped(reason))
            );
            assert_eq!(stopped.poll(), Err(reason));
            assert_eq!(
                SourceMap::validate_mappings(&[], &sources, &mut stopped).map(|_| ()),
                Err(OriginError::Stopped(reason))
            );
        }
        maps.reverse();
    }
    Ok(())
}
fn source(id: &str, text: &str) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId(id.into()),
        1,
        format!("memory:{id}"),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))
}

#[test]
fn map_admission_never_copies_long_identity_before_zero_allocation_budget() -> TestResult {
    let a = source(&"a".repeat(100_000), "a")?;
    let b = source("b", "a")?;
    let mapping = Mapping {
        source: a.span(0, 1).map_err(|e| format!("{e:?}"))?,
        target: b.span(0, 1).map_err(|e| format!("{e:?}"))?,
        kind: MappingKind::Exact,
    };
    let mut sources = SourceStore::default();
    sources.insert(a).map_err(|e| format!("{e:?}"))?;
    sources.insert(b).map_err(|e| format!("{e:?}"))?;
    let mut limits = budget().limits();
    limits.allocation_units = 0;
    let mut budget = Budget::new(limits);
    let mut maps = SourceMap::default();
    let result = maps.insert(mapping, &sources, &mut budget);
    assert_eq!(
        result,
        Err(OriginError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(budget.usage().allocation_units, 0);
    Ok(())
}

#[test]
fn mapped_containment_requires_every_byte_and_every_reverse_path() -> TestResult {
    let a = source("a", "abc")?;
    let b = source("b", "xy")?;
    let c = source("c", "XY")?;
    let empty = source("empty", "")?;
    let span = |s: &SourceSnapshot, a, b| s.span(a, b).map_err(|e| format!("{e:?}"));
    let mut store = SourceStore::default();
    for source in [&a, &b, &c, &empty] {
        store.insert(source.clone()).map_err(|e| format!("{e:?}"))?;
    }
    let map = |source, target| Mapping {
        source,
        target,
        kind: MappingKind::Transformed,
    };
    let maps = vec![
        map(span(&a, 0, 1)?, span(&b, 0, 1)?),
        map(span(&a, 1, 2)?, span(&b, 1, 2)?),
        map(span(&b, 0, 2)?, span(&c, 0, 2)?),
        map(span(&a, 1, 1)?, span(&empty, 0, 0)?),
    ];
    let checked =
        SourceMap::validate_mappings(&maps, &store, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut incomplete = SourceStore::default();
    incomplete.insert(a.clone()).map_err(|e| format!("{e:?}"))?;
    incomplete.insert(c.clone()).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        checked.validate_sources(&incomplete, &mut budget()),
        Err(OriginError::Source(
            nepl3_core::source::SourceError::MissingSnapshot
        ))
    ));
    assert!(
        checked
            .contains(&span(&a, 0, 2)?, &span(&c, 0, 2)?, &mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    assert!(
        checked
            .contains(&span(&a, 0, 2)?, &span(&empty, 0, 0)?, &mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    assert!(
        !checked
            .contains(&span(&a, 0, 1)?, &span(&c, 0, 2)?, &mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    let mut hole = maps.clone();
    hole.remove(1);
    let checked =
        SourceMap::validate_mappings(&hole, &store, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(
        !checked
            .contains(&span(&a, 0, 2)?, &span(&c, 0, 2)?, &mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    let mut external = maps.clone();
    external.push(map(span(&a, 2, 3)?, span(&b, 0, 1)?));
    let checked = SourceMap::validate_mappings(&external, &store, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(
        !checked
            .contains(&span(&a, 0, 2)?, &span(&c, 0, 2)?, &mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    // Parent membership is the stopping boundary, even when that parent itself has older provenance.
    let checked =
        SourceMap::validate_mappings(&maps, &store, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(
        checked
            .contains(&span(&b, 0, 2)?, &span(&c, 0, 2)?, &mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    let mut limits = budget().limits();
    limits.work = 0;
    assert!(matches!(
        checked.contains(&span(&a, 0, 2)?, &span(&a, 0, 1)?, &mut Budget::new(limits)),
        Err(OriginError::Stopped(StopReason::WorkLimit))
    ));
    let mut stopped = budget();
    stopped.cancel();
    assert!(matches!(
        SourceMap::validate_mappings(&[], &store, &mut stopped),
        Err(OriginError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}
#[test]
fn snapshot_dag_proves_large_transforms_without_enumerating_the_point_product() -> Result<(), String>
{
    use nepl3_core::{budget::*, origin::*, source::*};
    let limits = Limits {
        source_bytes: 100_000,
        work: 1_000_000,
        depth: 100,
        nodes: 1_000_000,
        allocation_units: 10_000_000,
        output_bytes: 1000,
        diagnostics: 10,
        events: 10,
    };
    let mut setup = Budget::new(limits);
    let mut sources = SourceStore::default();
    let a = SourceSnapshot::new(
        SourceId("large-a".into()),
        0,
        "memory:large-a".into(),
        vec![b'a'; 10000],
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    let b = SourceSnapshot::new(
        SourceId("large-b".into()),
        0,
        "memory:large-b".into(),
        vec![b'b'; 10000],
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mappings = vec![Mapping {
        source: a.span(0, 10000).map_err(|e| format!("{e:?}"))?,
        target: b.span(0, 10000).map_err(|e| format!("{e:?}"))?,
        kind: MappingKind::Transformed,
    }];
    sources.insert(a.clone()).map_err(|e| format!("{e:?}"))?;
    sources.insert(b.clone()).map_err(|e| format!("{e:?}"))?;
    let mut bounded = Budget::new(Limits {
        nodes: 2,
        allocation_units: 4096,
        ..limits
    });
    SourceMap::validate_mappings(&mappings, &sources, &mut bounded)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(bounded.usage().nodes, 2);
    // A coarse cycle is not declared valid. The exact fallback still rejects an
    // actual point cycle, and existing forward-overlap tests cover valid coarse cycles.
    let cycle = vec![
        Mapping {
            source: a.span(0, 1).map_err(|e| format!("{e:?}"))?,
            target: b.span(0, 1).map_err(|e| format!("{e:?}"))?,
            kind: MappingKind::Transformed,
        },
        Mapping {
            source: b.span(0, 1).map_err(|e| format!("{e:?}"))?,
            target: a.span(0, 1).map_err(|e| format!("{e:?}"))?,
            kind: MappingKind::Transformed,
        },
    ];
    assert!(matches!(
        SourceMap::validate_mappings(&cycle, &sources, &mut Budget::new(limits)),
        Err(OriginError::Cycle)
    ));
    Ok(())
}
