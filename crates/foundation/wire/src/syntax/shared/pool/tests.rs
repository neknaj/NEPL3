use super::*;
use nepl3_core::{budget::Limits, source::SourceId};

#[test]
fn storage_deduplication_preserves_independent_values_and_exact_limits() -> Result<(), WireError> {
    let make = || {
        SourceSnapshot::new(
            SourceId("same".into()),
            1,
            "memory:same".into(),
            b"same".to_vec(),
            &mut Budget::new(Limits {
                source_bytes: 64,
                work: 1024,
                allocation_units: 1024,
                ..Limits::default()
            }),
        )
    };
    let first = make()?;
    let second = make()?;
    assert!(!core::ptr::eq(first.identity(), second.identity()));
    let allocation =
        (4 * (2 * core::mem::size_of::<usize>() + core::mem::size_of::<bool>())) as u64;
    for inputs in [
        alloc::vec![&first, &second, &first, &second],
        alloc::vec![&second, &second, &first, &first],
    ] {
        let mut budget = Budget::new(Limits {
            work: 36,
            allocation_units: allocation,
            ..Limits::default()
        });
        let original_first = inputs[0];
        let result = unique_source_storage(inputs, &mut budget)?;
        assert!(core::ptr::eq(
            result[0].identity(),
            original_first.identity()
        ));
        assert_eq!(result.len(), 2);
        assert!(
            result
                .iter()
                .any(|source| core::ptr::eq(source.identity(), first.identity()))
        );
        assert!(
            result
                .iter()
                .any(|source| core::ptr::eq(source.identity(), second.identity()))
        );
        assert_eq!(budget.usage().work, 36);
        assert_eq!(budget.usage().allocation_units, allocation);
    }
    for (work, allocation_units, reason) in [
        (35, allocation, StopReason::WorkLimit),
        (36, allocation - 1, StopReason::AllocationLimit),
    ] {
        let mut budget = Budget::new(Limits {
            work,
            allocation_units,
            ..Limits::default()
        });
        assert!(
            matches!(unique_source_storage(alloc::vec![&first; 4], &mut budget), Err(WireError::Stopped(actual)) if actual == reason)
        );
        assert_eq!(budget.poll(), Err(reason));
    }
    let mut budget = Budget::new(Limits::default());
    budget.cancel();
    assert!(matches!(
        unique_source_storage(Vec::new(), &mut budget),
        Err(WireError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}

#[test]
fn pool_search_commits_the_matched_position_without_recomparison() -> Result<(), WireError> {
    let values = [2_u64, 4, 6, 8, 10, 12, 14];
    let pool = Pool {
        entries: values.iter().map(|v| (v, Digest::of(b"test"))).collect(),
        values: Vec::new(),
    };
    for wanted in 0..=16 {
        let mut budget = Budget::new(Limits {
            work: 3,
            ..Limits::default()
        });
        let actual = pool.find(
            |candidate, b| {
                b.charge(Resource::Work, 1)?;
                Ok(candidate.cmp(&wanted))
            },
            &mut budget,
        );
        let expected = values
            .iter()
            .position(|v| *v == wanted)
            .ok_or(WireError::InvalidType);
        assert_eq!(actual, expected);
    }
    // The middle entry succeeds on the only permitted comparison. No search
    // or equality check may follow the successful match.
    let mut budget = Budget::new(Limits {
        work: 1,
        ..Limits::default()
    });
    assert_eq!(
        pool.find(
            |candidate, b| {
                b.charge(Resource::Work, 1)?;
                Ok(candidate.cmp(&8))
            },
            &mut budget
        )?,
        3
    );
    let mut budget = Budget::new(Limits {
        work: 0,
        ..Limits::default()
    });
    assert_eq!(
        pool.find(
            |candidate, b| {
                b.charge(Resource::Work, 1)?;
                Ok(candidate.cmp(&8))
            },
            &mut budget
        ),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    Ok(())
}

#[test]
fn borrowed_identity_equality_is_metered_and_storage_independent() -> Result<(), WireError> {
    let a = SnapshotId {
        source: SourceId("long-source".repeat(1024)),
        revision: 7,
        digest: Digest::of(b"source"),
    };
    let independent = a.clone();
    let budget = |work| {
        Budget::new(Limits {
            work,
            ..Limits::default()
        })
    };
    let mut one = budget(1);
    assert_eq!(identity(&a, &a, &mut one)?, Ordering::Equal);
    assert_eq!(one.usage().work, 1);
    let mut zero = budget(0);
    assert_eq!(
        identity(&a, &a, &mut zero),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    let mut one = budget(1);
    assert_eq!(
        identity(&a, &independent, &mut one),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    let mut enough = budget(a.source.0.len() as u64 + 35);
    assert_eq!(identity(&a, &independent, &mut enough)?, Ordering::Equal);
    let mut next = independent;
    next.revision += 1;
    assert_eq!(
        identity(&a, &next, &mut budget(a.source.0.len() as u64 + 35))?,
        Ordering::Less
    );
    let mut cancelled = budget(1);
    cancelled.cancel();
    assert_eq!(
        identity(&a, &a, &mut cancelled),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}
