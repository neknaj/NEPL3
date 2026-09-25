use super::*;
use nepl3_core::{budget::Limits, source::SourceId};

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
