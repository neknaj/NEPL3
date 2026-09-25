use super::*;
use crate::budget::Limits;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        source_bytes: 10_000_000,
        ..Limits::default()
    })
}

fn source(name: &str, revision: u64, text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(name.into()),
        revision,
        alloc::format!("memory:{name}"),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}

#[test]
fn distinct_insert_preserves_keys_order_and_occupied_contents() -> Result<(), SourceError> {
    let originals = [
        source("z", 0, "zero")?,
        source("a", 0, "one")?,
        source("z", 1, "two")?,
    ];
    let mut store = SourceStore::default();
    for value in &originals {
        assert!(store.insert_distinct_ref_with_budget(value, &mut budget())?);
    }
    for repeated in [&originals[0], &source("z", 0, "changed")?] {
        let mut b = budget();
        assert!(!store.insert_distinct_ref_with_budget(repeated, &mut b)?);
        assert_eq!(b.usage().allocation_units, 0);
        assert_eq!(store.snapshots(), &originals);
    }
    for value in &originals {
        assert_eq!(store.get_ref(value.identity()), Some(value));
    }
    Ok(())
}

#[test]
fn distinct_insert_stops_before_mutating_and_avoids_second_search() -> Result<(), SourceError> {
    let first = source("z", 0, "first")?;
    let added = source("a", 0, "second")?;
    let mut initial = SourceStore::default();
    initial.insert(first.clone())?;
    let mut complete = budget();
    assert!(initial.insert_distinct_ref_with_budget(&added, &mut complete)?);
    for resource in [Resource::Work, Resource::AllocationUnits] {
        let used = match resource {
            Resource::Work => complete.usage().work,
            _ => complete.usage().allocation_units,
        };
        for limit in 0..used {
            let mut store = SourceStore::default();
            store.insert(first.clone())?;
            let mut limits = budget().limits();
            let reason = match resource {
                Resource::Work => {
                    limits.work = limit;
                    StopReason::WorkLimit
                }
                _ => {
                    limits.allocation_units = limit;
                    StopReason::AllocationLimit
                }
            };
            let mut b = Budget::new(limits);
            assert_eq!(
                store.insert_distinct_ref_with_budget(&added, &mut b),
                Err(SourceError::Stopped(reason))
            );
            assert_eq!(store.snapshots(), core::slice::from_ref(&first));
            assert_eq!(store.get_ref(first.identity()), Some(&first));
            assert_eq!(store.get_ref(added.identity()), None);
            assert_eq!(
                store.insert_distinct_ref_with_budget(&added, &mut b),
                Err(SourceError::Stopped(reason))
            );
            assert!(store.insert_distinct_ref_with_budget(&added, &mut budget())?);
        }
    }
    let mut direct = SourceStore::default();
    let mut twice = SourceStore::default();
    let mut direct_budget = budget();
    let mut twice_budget = budget();
    for index in (0..128).rev() {
        let value = source(&alloc::format!("source-{index:04}"), 0, "value")?;
        assert!(direct.insert_distinct_ref_with_budget(&value, &mut direct_budget)?);
        assert!(
            twice
                .get_revision_with_budget(&value.identity().source, 0, &mut twice_budget)?
                .is_none()
        );
        twice.insert_ref_with_budget(&value, &mut twice_budget)?;
    }
    assert_eq!(direct.snapshots(), twice.snapshots());
    assert!(direct_budget.usage().work < twice_budget.usage().work);
    Ok(())
}
