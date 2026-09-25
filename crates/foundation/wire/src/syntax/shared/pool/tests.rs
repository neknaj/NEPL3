use super::*;
use nepl3_core::{budget::Limits, source::SourceId};

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
