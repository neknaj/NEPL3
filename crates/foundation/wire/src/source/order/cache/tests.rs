use super::*;
use nepl3_core::{budget::Limits, source::SourceId};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: u64::MAX,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
    })
}
fn source(id: usize, text: &[u8]) -> Result<SourceSnapshot, WireError> {
    Ok(SourceSnapshot::new(
        SourceId(alloc::format!("source-{id:04}")),
        1,
        alloc::format!("memory:{id}"),
        text.to_vec(),
        &mut budget(),
    )?)
}
fn compare(cache: &mut OrderCache, input: &[SourceSnapshot]) -> Result<(), WireError> {
    let expected = super::super::super::ordered_sources(input, &mut budget())?;
    let actual = cache.ordered(input, &mut budget())?;
    assert_eq!(expected, actual);
    for (_, current) in actual {
        assert!(input.iter().any(|s| core::ptr::eq(s, current)));
    }
    Ok(())
}
#[test]
fn shuffled_prefix_growth_and_fallback_preserve_current_members() -> Result<(), WireError> {
    let sources: Vec<_> = (0..64).map(|i| source(i, b"x")).collect::<Result<_, _>>()?;
    let mut cache = OrderCache::default();
    let mut input = Vec::new();
    // Descending even IDs followed by odd IDs: growth is never a sorted tail append.
    for i in (0..64)
        .rev()
        .filter(|i| i % 2 == 0)
        .chain((0..64).filter(|i| i % 2 == 1))
    {
        input.push(sources[i].clone());
        compare(&mut cache, &input)?;
    }
    compare(&mut cache, &input)?;
    compare(&mut cache, &input[..1])?;
    compare(&mut cache, &[])?;
    compare(&mut cache, &sources)?;
    let mut independent = Vec::new();
    for i in 0..64 {
        independent.push(source(i, b"x")?);
    }
    compare(&mut cache, &independent)?;
    Ok(())
}
#[test]
fn cross_boundary_duplicates_and_failed_replacement_preserve_cache() -> Result<(), WireError> {
    let a = source(3, b"a")?;
    let z = source(1, b"z")?;
    let mut cache = OrderCache::default();
    cache.ordered(&[a.clone(), z.clone()], &mut budget())?;
    let before = cache.previous.clone();
    let indices = cache.indices.clone();
    for duplicate in [a.clone(), source(3, b"changed digest")?] {
        let input = alloc::vec![a.clone(), z.clone(), duplicate];
        assert_eq!(
            cache.ordered(&input, &mut budget()).err(),
            Some(SourceError::IdentityConflict.into())
        );
        assert_eq!(cache.previous, before);
        assert_eq!(cache.indices, indices);
    }
    // Tail-internal duplicates must also fail, including an otherwise unrelated set.
    assert!(
        cache
            .ordered(&[z.clone(), z.clone()], &mut budget())
            .is_err()
    );
    assert_eq!(cache.previous, before);
    assert_eq!(cache.indices, indices);
    let input = alloc::vec![
        a,
        z,
        source(4, b"tail")?,
        source(0, b"before")?,
        source(2, b"middle")?
    ];
    let mut measured = budget();
    cache.ordered(&input, &mut measured)?;
    let used = measured.usage();
    for resource in 0..3 {
        let mut cache = OrderCache::default();
        cache.ordered(&before, &mut budget())?;
        let mut limits = budget().limits();
        limits.work = used.work;
        limits.allocation_units = used.allocation_units;
        let reason = match resource {
            0 => None,
            1 => {
                limits.work -= 1;
                Some(StopReason::WorkLimit)
            }
            _ => {
                limits.allocation_units -= 1;
                Some(StopReason::AllocationLimit)
            }
        };
        let mut b = Budget::new(limits);
        let result = cache.ordered(&input, &mut b);
        if let Some(reason) = reason {
            assert!(result.is_err());
            assert_eq!(b.poll(), Err(reason));
            assert_eq!(cache.previous, before);
            assert_eq!(cache.indices, indices);
            let saved = b.usage();
            assert!(cache.ordered(&input, &mut b).is_err());
            assert_eq!(saved, b.usage());
            compare(&mut cache, &input)?;
        } else {
            result?;
            assert_eq!(b.usage(), used);
        }
    }
    // Every smaller allowance exercises all interruptible preparation phases,
    // including the unsorted multi-source tail and both merge/remainder paths.
    for (resource, bound) in [
        (Resource::Work, used.work),
        (Resource::AllocationUnits, used.allocation_units),
    ] {
        for limit in 0..bound {
            let mut cache = OrderCache::default();
            cache.ordered(&before, &mut budget())?;
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
            assert!(cache.ordered(&input, &mut b).is_err());
            assert_eq!(b.poll(), Err(reason));
            assert_eq!(cache.previous, before);
            assert_eq!(cache.indices, indices);
            let stopped = b.usage();
            assert!(cache.ordered(&input, &mut b).is_err());
            assert_eq!(b.usage(), stopped);
            compare(&mut cache, &input)?;
        }
    }
    let mut cancelled = budget();
    cancelled.cancel();
    let before = cache.previous.clone();
    let indices = cache.indices.clone();
    assert!(cache.ordered(&input, &mut cancelled).is_err());
    assert_eq!(cache.previous, before);
    assert_eq!(cache.indices, indices);
    Ok(())
}
#[test]
fn repeated_ordering_saves_work_without_independent_text_comparisons() -> Result<(), WireError> {
    let mut input: Vec<_> = (0..64).map(|i| source(i, b"x")).collect::<Result<_, _>>()?;
    input.reverse();
    let mut cache = OrderCache::default();
    cache.ordered(&input, &mut budget())?;
    let mut old = budget();
    let mut new = budget();
    let expected = super::super::super::ordered_sources(&input, &mut old)?;
    assert_eq!(cache.ordered(&input, &mut new)?, expected);
    assert!(new.usage().work < old.usage().work);
    let mut costs = Vec::new();
    for length in [1, 65536] {
        let first = source(1, &alloc::vec![b'x';length])?;
        let independent = source(1, &alloc::vec![b'x';length])?;
        let mut cache = OrderCache::default();
        cache.ordered(&[first], &mut budget())?;
        let mut b = budget();
        cache.ordered(&[independent], &mut b)?;
        costs.push(b.usage().work);
    }
    assert_eq!(costs[0], costs[1]);
    Ok(())
}
