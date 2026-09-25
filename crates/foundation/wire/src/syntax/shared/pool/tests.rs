use super::*;
use nepl3_core::{budget::Limits, source::SourceId};

#[test]
fn source_position_index_matches_complete_independent_identities() -> Result<(), WireError> {
    let limits = Limits {
        source_bytes: 1024,
        work: 100_000,
        allocation_units: 100_000,
        ..Limits::default()
    };
    let mut sources = Vec::new();
    // Equal content digests still require both source name and revision.
    for (name, revision, text) in [
        ("a", 1, "same"),
        ("a", 2, "same"),
        ("b", 1, "same"),
        ("c", 1, "other"),
    ] {
        sources.push(SourceSnapshot::new(
            SourceId(name.into()),
            revision,
            "memory:test".into(),
            text.as_bytes().to_vec(),
            &mut Budget::new(limits),
        )?);
    }
    let pool = Pool {
        entries: sources.iter().map(|s| (s, Digest::of(b"test"))).collect(),
        values: Vec::new(),
    };
    let mut measured = Budget::new(limits);
    let positions = SourcePositions::new(&pool, &mut measured)?;
    for (expected, source) in sources.iter().enumerate() {
        let independent = source.identity().clone();
        assert_eq!(
            positions.position(&independent, &mut Budget::new(limits))?,
            expected
        );
        for changed in [
            SnapshotId {
                source: SourceId("missing".into()),
                ..independent.clone()
            },
            SnapshotId {
                revision: 99,
                ..independent.clone()
            },
            SnapshotId {
                digest: Digest::of(b"missing"),
                ..independent
            },
        ] {
            assert_eq!(
                positions.position(&changed, &mut Budget::new(limits)),
                Err(WireError::InvalidType)
            );
        }
    }
    let exact = Limits {
        work: measured.usage().work,
        allocation_units: measured.usage().allocation_units,
        ..limits
    };
    let _ = SourcePositions::new(&pool, &mut Budget::new(exact))?;
    for (bounded, reason) in [
        (
            Limits {
                work: exact.work - 1,
                ..exact
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: exact.allocation_units - 1,
                ..exact
            },
            StopReason::AllocationLimit,
        ),
    ] {
        let mut budget = Budget::new(bounded);
        assert!(
            matches!(SourcePositions::new(&pool, &mut budget), Err(WireError::Stopped(actual)) if actual == reason)
        );
        assert_eq!(budget.poll(), Err(reason));
    }
    let wanted = sources[2].identity().clone();
    let mut measured = Budget::new(limits);
    assert_eq!(positions.position(&wanted, &mut measured)?, 2);
    assert_eq!(
        positions.position(
            &wanted,
            &mut Budget::new(Limits {
                work: measured.usage().work,
                ..limits
            })
        )?,
        2
    );
    let mut short = Budget::new(Limits {
        work: measured.usage().work - 1,
        ..limits
    });
    assert_eq!(
        positions.position(&wanted, &mut short),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    let mut cancelled = Budget::new(limits);
    cancelled.cancel();
    assert_eq!(
        positions.position(&wanted, &mut cancelled),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    let empty = Pool {
        entries: Vec::new(),
        values: Vec::new(),
    };
    assert!(matches!(
        SourcePositions::new(&empty, &mut cancelled),
        Err(WireError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}

#[test]
fn source_position_index_bounds_long_name_comparisons() -> Result<(), WireError> {
    let limits = Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        allocation_units: 100_000_000,
        ..Limits::default()
    };
    let mut prior = None;
    for count in [128_usize, 256, 512] {
        let mut sources = Vec::new();
        for number in 0..count {
            sources.push(SourceSnapshot::new(
                SourceId(alloc::format!("{}:{number:04}", "prefix".repeat(256))),
                1,
                "memory:test".into(),
                alloc::format!("text:{number}").into_bytes(),
                &mut Budget::new(limits),
            )?);
        }
        let pool = Pool {
            entries: sources.iter().map(|s| (s, Digest::of(b"test"))).collect(),
            values: Vec::new(),
        };
        let mut setup = Budget::new(limits);
        let positions = SourcePositions::new(&pool, &mut setup)?;
        let mut lookup = Budget::new(limits);
        for (expected, source) in sources.iter().enumerate().rev() {
            // Decoded identities have independent storage and use the same index.
            assert_eq!(
                positions.position(&source.identity().clone(), &mut lookup)?,
                expected
            );
        }
        // At most log2(count)+1 fixed-size comparisons and one full name
        // comparison per hit. Long common prefixes are not revisited per level.
        let per_hit = 33 * (usize::BITS - count.leading_zeros()) as u64
            + sources[0].identity().source.0.len() as u64
            + 1;
        assert!(lookup.usage().work <= count as u64 * per_hit);
        if let Some((setup_work, lookup_work)) = prior {
            assert!(setup.usage().work < setup_work * 3);
            assert!(lookup.usage().work < lookup_work * 3);
        }
        prior = Some((setup.usage().work, lookup.usage().work));
    }
    Ok(())
}

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
    let mut enough = budget(a.source.0.len() as u64 + 41);
    assert_eq!(identity(&a, &independent, &mut enough)?, Ordering::Equal);
    let mut next = independent;
    next.revision += 1;
    assert_eq!(
        identity(&a, &next, &mut budget(a.source.0.len() as u64 + 41))?,
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
