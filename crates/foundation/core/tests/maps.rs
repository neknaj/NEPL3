use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Mapping, MappingKind, OriginError, SourceMap},
    source::{SourceId, SourceSnapshot, SourceStore},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
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
