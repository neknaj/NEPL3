use super::*;
use alloc::{format, string::String};
use nepl3_core::{
    budget::Limits,
    origin::{MappingKind, OriginId},
    source::{Digest, SourceId, SourceSnapshot},
    syntax::SyntaxNode,
    value::SchemaRef,
};
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
fn source(id: &str, revision: u64, byte: u8) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId(id.into()),
        revision,
        format!("memory:{id}"),
        vec![byte; 100],
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))
}
fn span(s: &SourceSnapshot, a: u64, z: u64) -> Result<Span, String> {
    s.span(a, z).map_err(|e| format!("{e:?}"))
}
fn map(a: Span, z: Span) -> Mapping {
    Mapping {
        source: a,
        target: z,
        kind: MappingKind::Transformed,
    }
}
fn select(maps: &[Mapping], seeds: &[Span]) -> Result<Vec<bool>, String> {
    let mut c = Closure {
        ranges: Vec::new(),
        pending: Vec::new(),
    };
    let mut b = budget();
    for s in seeds {
        c.span(s, &mut b).map_err(|e| format!("{e:?}"))?;
    }
    c.select(maps, &mut b).map_err(|e| format!("{e:?}"))
}
fn arena(s: &SourceSnapshot, maps: Vec<Mapping>) -> Result<ParseArena, String> {
    let h = span(s, 40, 45)?;
    Ok(ParseArena {
        sources: vec![s.clone()],
        nodes: vec![SyntaxNode {
            schema: SchemaRef {
                package: "test".into(),
                revision: 1,
                digest: Digest([0; 32]),
            },
            kind: "Node".into(),
            fields: vec![],
            head: Some(h.clone()),
            cover: Some(h.clone()),
            origin: OriginId(0),
            token: None,
        }],
        origins: vec![Origin::Direct(h)],
        source_maps: maps,
        ..ParseArena::default()
    })
}
#[test]
fn local_range_keeps_both_directions_and_all_alternate_paths() -> Result<(), String> {
    let a = source("a", 0, b'a')?;
    let b = source("b", 0, b'b')?;
    let c = source("c", 0, b'c')?;
    let d = source("d", 0, b'd')?;
    let maps = vec![
        map(span(&a, 0, 10)?, span(&d, 0, 10)?),
        map(span(&a, 40, 50)?, span(&b, 0, 10)?),
        map(span(&c, 20, 30)?, span(&b, 0, 10)?),
        map(span(&b, 0, 10)?, span(&d, 70, 80)?),
    ];
    assert_eq!(
        select(&maps, &[span(&a, 42, 44)?])?,
        vec![false, true, true, true]
    );
    assert_eq!(
        select(&maps, &[span(&d, 75, 75)?])?,
        vec![false, true, true, true]
    );
    Ok(())
}
#[test]
fn full_identity_and_empty_boundaries_are_preserved() -> Result<(), String> {
    let a = source("same", 0, b'a')?;
    let other_revision = source("same", 1, b'a')?;
    let other_digest = source("same", 0, b'b')?;
    let target = source("target", 0, b'x')?;
    let maps = vec![map(span(&a, 10, 10)?, span(&target, 0, 0)?)];
    assert_eq!(
        select(&maps, &[span(&other_revision, 10, 10)?])?,
        vec![false]
    );
    assert_eq!(select(&maps, &[span(&other_digest, 10, 10)?])?, vec![false]);
    assert_eq!(select(&maps, &[span(&a, 0, 10)?])?, vec![true]);
    Ok(())
}
#[test]
fn local_unreferenced_maps_survive_and_filter_preserves_order() -> Result<(), String> {
    let a = source("a", 0, b'a')?;
    let t = source("target", 0, b'x')?;
    let maps = vec![
        map(span(&a, 0, 10)?, span(&t, 0, 10)?),
        map(span(&a, 70, 80)?, span(&t, 70, 80)?),
        map(span(&a, 40, 50)?, span(&t, 40, 50)?),
    ];
    let mut input = arena(&a, maps.clone())?;
    let before = input.clone();
    let mask = prepare(&input, NodeRef(0), 50, &maps[1..2], &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(mask, vec![false, true, true]);
    apply(&mut input, &mask);
    assert_eq!(input.source_maps, maps[1..]);
    assert_eq!(input.nodes, before.nodes);
    assert_eq!(input.sources, before.sources);
    assert_eq!(input.origins, before.origins);
    Ok(())
}
#[test]
fn unreferenced_origin_anchor_keeps_its_mapping() -> Result<(), String> {
    let a = source("a", 0, b'a')?;
    let t = source("target", 0, b'x')?;
    let maps = vec![map(span(&a, 0, 10)?, span(&t, 0, 10)?)];
    let mut input = arena(&a, maps)?;
    input.origins.push(Origin::Synthetic {
        reason: "retained".into(),
        anchor: Some(span(&t, 2, 3)?),
    });
    assert_eq!(
        prepare(&input, NodeRef(0), 50, &[], &[], &mut budget()).map_err(|e| format!("{e:?}"))?,
        vec![true]
    );
    Ok(())
}
#[test]
fn selection_stops_leave_arena_and_masks_unpublished() -> Result<(), String> {
    let a = source("a", 0, b'a')?;
    let t = source("target", 0, b'x')?;
    let input = arena(&a, vec![map(span(&a, 40, 50)?, span(&t, 0, 10)?)])?;
    let before = input.clone();
    for resource in [Resource::Work, Resource::AllocationUnits] {
        let mut limits = budget().limits();
        match resource {
            Resource::Work => limits.work = 0,
            _ => limits.allocation_units = 0,
        };
        let mut b = Budget::new(limits);
        assert!(prepare(&input, NodeRef(0), 50, &[], &[], &mut b).is_err());
        assert!(b.poll().is_err());
        assert_eq!(input, before);
    }
    let mut b = budget();
    b.cancel();
    assert!(prepare(&input, NodeRef(0), 50, &[], &[], &mut b).is_err());
    assert_eq!(input, before);
    Ok(())
}

#[test]
fn fact_only_dependencies_and_generated_callsites_keep_older_maps() -> Result<(), String> {
    use crate::package::{EntryContext, PackageIdentity};
    use nepl3_core::value::OperationRef;
    let a = source("a", 0, b'a')?;
    let t = source("target", 0, b'x')?;
    let maps = vec![map(span(&a, 0, 10)?, span(&t, 0, 10)?)];
    let mut input = arena(&a, maps)?;
    let schema = input.nodes[0].schema.clone();
    let batch = ReaderFactBatch {
        path: vec![],
        node: None,
        entry: EntryContext {
            package: PackageIdentity {
                schema: schema.clone(),
                semantic_digest: Digest([0; 32]),
            },
            alias: "Test".into(),
            category: "Expr".into(),
            mode: "Code".into(),
        },
        facts: vec![ReaderFact::Capture {
            name: "capture".into(),
            span: span(&t, 2, 3)?,
        }],
        trivia: vec![],
    };
    assert_eq!(
        prepare(&input, NodeRef(0), 50, &[], &[batch], &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        vec![true]
    );
    input.origins.push(Origin::Generated {
        operation: OperationRef {
            schema,
            name: "test".into(),
        },
        callsite: Some(span(&t, 3, 3)?),
        inputs: vec![],
    });
    assert_eq!(
        prepare(&input, NodeRef(0), 50, &[], &[], &mut budget()).map_err(|e| format!("{e:?}"))?,
        vec![true]
    );
    Ok(())
}

#[test]
fn stopping_at_filter_precharge_keeps_original_arena() -> Result<(), String> {
    let a = source("a", 0, b'a')?;
    let t = source("target", 0, b'x')?;
    let input = arena(&a, vec![map(span(&a, 40, 50)?, span(&t, 0, 10)?)])?;
    let before = input.clone();
    let mut measured = budget();
    prepare(&input, NodeRef(0), 50, &[], &[], &mut measured).map_err(|e| format!("{e:?}"))?;
    let mut limited = Budget::new(Limits {
        work: measured.usage().work - 1,
        ..budget().limits()
    });
    assert_eq!(
        prepare(&input, NodeRef(0), 50, &[], &[], &mut limited)
            .err()
            .and_then(|e| e.stop_reason()),
        Some(StopReason::WorkLimit)
    );
    assert_eq!(input, before);
    assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
    Ok(())
}

#[test]
fn exact_rows_and_fact_variants_keep_same_mapping_closure() -> Result<(), String> {
    use crate::package::{EntryContext, PackageIdentity};
    use nepl3_core::view::{FallbackRole, PresentationClass, Trivia, TriviaKind};
    let a = source("a", 0, b'a')?;
    let t = source("target", 0, b'a')?;
    let mut m = map(span(&a, 0, 10)?, span(&t, 0, 10)?);
    m.kind = MappingKind::Exact;
    let input = arena(&a, vec![m])?;
    let schema = input.nodes[0].schema.clone();
    let endpoint = span(&t, 2, 3)?;
    let facts = vec![
        ReaderFact::Presentation {
            class: PresentationClass {
                schema: schema.clone(),
                name: "test".into(),
                fallback: FallbackRole::Content,
            },
            span: endpoint.clone(),
        },
        ReaderFact::Relation {
            schema: schema.clone(),
            kind: "test".into(),
            from: span(&a, 40, 41)?,
            to: endpoint.clone(),
        },
    ];
    for fact in facts {
        let batch = ReaderFactBatch {
            path: vec![],
            node: None,
            entry: EntryContext {
                package: PackageIdentity {
                    schema: schema.clone(),
                    semantic_digest: Digest([0; 32]),
                },
                alias: "Test".into(),
                category: "Expr".into(),
                mode: "Code".into(),
            },
            facts: vec![fact],
            trivia: vec![],
        };
        assert_eq!(
            prepare(&input, NodeRef(0), 50, &[], &[batch], &mut budget())
                .map_err(|e| format!("{e:?}"))?,
            vec![true]
        );
    }
    let batch = ReaderFactBatch {
        path: vec![],
        node: None,
        entry: EntryContext {
            package: PackageIdentity {
                schema,
                semantic_digest: Digest([0; 32]),
            },
            alias: "Test".into(),
            category: "Expr".into(),
            mode: "Code".into(),
        },
        facts: vec![],
        trivia: vec![Trivia {
            span: endpoint,
            kind: TriviaKind::Skipped,
        }],
    };
    assert_eq!(
        prepare(&input, NodeRef(0), 50, &[], &[batch], &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        vec![true]
    );
    Ok(())
}

#[test]
fn merged_seed_versions_do_not_repeat_whole_mapping_scans() -> Result<(), String> {
    let a = source("seed", 0, b'a')?;
    let unrelated = source("unrelated", 0, b'b')?;
    let maps: Vec<_> = (0..90)
        .map(|i| {
            Ok(map(
                span(&unrelated, i, i + 1)?,
                span(&unrelated, i, i + 1)?,
            ))
        })
        .collect::<Result<_, String>>()?;
    let mut closure = Closure {
        ranges: Vec::new(),
        pending: Vec::new(),
    };
    let mut b = budget();
    let seeds = (1..=90)
        .map(|end| span(&a, 0, end))
        .collect::<Result<Vec<_>, _>>()?;
    for seed in &seeds {
        closure.span(seed, &mut b).map_err(|e| format!("{e:?}"))?;
    }
    let used = b.usage();
    let mut limits = b.limits();
    limits.work = used.work + 20_000;
    let selected = b
        .with_ceiling(limits, |b| closure.select(&maps, b))
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(selected, vec![false; maps.len()]);
    assert_eq!(
        b.usage().allocation_units - used.allocation_units,
        maps.len() as u64
    );
    Ok(())
}

#[test]
fn compacted_seed_union_matches_unmerged_fixed_point() -> Result<(), String> {
    fn overlaps(a: &Span, b: &Span) -> bool {
        a.snapshot_ref() == b.snapshot_ref() && a.start() <= b.end() && b.start() <= a.end()
    }
    let snapshots = [
        source("same", 0, b'a')?,
        source("same", 1, b'a')?,
        source("same", 0, b'b')?,
    ];
    for variant in 0..64 {
        let mut seeds = (1..=12)
            .map(|end| span(&snapshots[variant % 3], 0, end))
            .collect::<Result<Vec<_>, _>>()?;
        seeds.extend(
            (51..=60)
                .map(|end| span(&snapshots[variant % 3], 50, end))
                .collect::<Result<Vec<_>, _>>()?,
        );
        seeds.extend(
            (21..=25)
                .map(|end| span(&snapshots[(variant + 1) % 3], 20, end))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let mut maps = Vec::new();
        for row in 0..30 {
            let start = ((row * 7 + variant * 3) % 96) as u64;
            let target = ((row * 11 + variant) % 96) as u64;
            maps.push(map(
                span(
                    &snapshots[(row + variant) % 3],
                    start,
                    start + (row % 4) as u64,
                )?,
                span(
                    &snapshots[(row + variant + 1) % 3],
                    target,
                    target + (row % 3) as u64,
                )?,
            ));
        }
        // A separate monotone fixed-point reference keeps every original seed
        // and each selected endpoint. It neither merges nor compacts ranges.
        let mut reachable = seeds.clone();
        let mut expected = vec![false; maps.len()];
        loop {
            let mut changed = false;
            for (index, mapping) in maps.iter().enumerate() {
                if !expected[index]
                    && reachable
                        .iter()
                        .any(|s| overlaps(s, &mapping.source) || overlaps(s, &mapping.target))
                {
                    expected[index] = true;
                    reachable.push(mapping.source.clone());
                    reachable.push(mapping.target.clone());
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        assert_eq!(select(&maps, &seeds)?, expected);
    }
    Ok(())
}

#[test]
fn compacted_seed_selection_keeps_exact_caps_and_sticky_stops() -> Result<(), String> {
    let a = source("a", 0, b'a')?;
    let z = source("z", 0, b'z')?;
    let seeds = (1..=20)
        .map(|end| span(&a, 0, end))
        .collect::<Result<Vec<_>, _>>()?;
    let maps = vec![
        map(span(&a, 20, 20)?, span(&z, 30, 40)?),
        map(span(&z, 40, 40)?, span(&a, 80, 90)?),
    ];
    let prepare = |b: &mut Budget| -> Result<Closure<'_>, StopReason> {
        let mut c = Closure {
            ranges: Vec::new(),
            pending: Vec::new(),
        };
        for seed in &seeds {
            c.span(seed, b)?;
        }
        Ok(c)
    };
    let mut full = budget();
    let expected = prepare(&mut full)
        .map_err(|e| format!("{e:?}"))?
        .select(&maps, &mut full)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(expected, vec![true, true]);
    for allocation in [false, true] {
        for short in [false, true] {
            let mut b = budget();
            let c = prepare(&mut b).map_err(|e| format!("{e:?}"))?;
            let mut cap = b.limits();
            if allocation {
                cap.allocation_units = full.usage().allocation_units - u64::from(short);
            } else {
                cap.work = full.usage().work - u64::from(short);
            }
            let result = b.with_ceiling(cap, |b| c.select(&maps, b));
            if short {
                let reason = if allocation {
                    StopReason::AllocationLimit
                } else {
                    StopReason::WorkLimit
                };
                assert_eq!(result, Err(reason));
                assert_eq!(b.poll(), Err(reason));
            } else {
                assert_eq!(result.map_err(|e| format!("{e:?}"))?, expected);
            }
        }
    }
    let mut b = budget();
    let c = prepare(&mut b).map_err(|e| format!("{e:?}"))?;
    b.cancel();
    assert_eq!(c.select(&maps, &mut b), Err(StopReason::Cancelled));
    Ok(())
}
