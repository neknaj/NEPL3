use super::*;
use crate::{budget::Limits, source::SourceId};
use alloc::{string::String, vec};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 100,
        nodes: 1000,
        allocation_units: 1_000_000,
        output_bytes: 1000,
        diagnostics: 10,
        events: 10,
    })
}
fn source(text: &[u8]) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId("x".repeat(100_000)),
        0,
        String::from("memory:x"),
        text.to_vec(),
        &mut budget(),
    )
}
#[test]
fn report_resolver_uses_indexed_key_then_digest_and_geometry() -> Result<(), ReportValidationError>
{
    let original = source(b"a")?;
    let span = original.span(0, 1)?;
    let mut store = SourceStore::default();
    store.insert(original.clone())?;
    let resolver = Sources {
        store: &store,
        added: Vec::new(),
    };
    let mut limits = budget().limits();
    limits.work = 34;
    #[cfg(target_has_atomic = "ptr")]
    {
        let mut exact = Budget::new(limits);
        assert_eq!(resolver.slice(&span, &mut exact)?, "a");
        assert_eq!(exact.usage().work, 34);
    }
    let independent = source(b"a")?.span(0, 1)?;
    assert_eq!(
        resolver.slice(&independent, &mut Budget::new(limits)),
        Err(ReportValidationError::Stopped(StopReason::WorkLimit))
    );
    let conflict = source(b"b")?.span(0, 1)?;
    assert_eq!(
        resolver.slice(&conflict, &mut budget()),
        Err(ReportValidationError::Source(SourceError::MissingSnapshot))
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        resolver.slice(&span, &mut cancelled),
        Err(ReportValidationError::Stopped(StopReason::Cancelled))
    );
    // Added snapshots have no store-index proof and retain their full identity
    // comparison budget. Geometry no longer repeats that identity comparison.
    let empty = SourceStore::default();
    let added = Sources {
        store: &empty,
        added: vec![&original],
    };
    assert_eq!(added.slice(&independent, &mut budget())?, "a");
    assert_eq!(
        added.slice(&conflict, &mut budget()),
        Err(ReportValidationError::Source(SourceError::MissingSnapshot))
    );
    Ok(())
}
