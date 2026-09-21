use super::*;
use alloc::{format, vec};
use nepl3_core::{
    budget::{Limits, StopReason},
    source::SourceId,
};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 100,
        nodes: 100_000,
        allocation_units: 1_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn source(id: &str, uri: &str, text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(id.into()),
        0,
        uri.into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}
#[test]
fn reuse_is_bound_to_complete_environment_and_prefix() -> Result<(), SourceError> {
    let mut store = SourceStore::default();
    let mut incoming = Vec::new();
    for i in 0..128 {
        let snapshot = source(&format!("source-{i:04}"), "memory:s", "x")?;
        store.insert(snapshot.clone())?;
        incoming.push(snapshot);
    }
    let mut checks = SourceChecks::default();
    checks.check(&incoming, &store, &mut budget())?;
    // Shared immutable snapshots still cost comparisons; source names need not
    // be searched repeatedly. The old per-source binary search exceeds 300 Work.
    let mut limited = Budget::new(Limits {
        work: if cfg!(target_has_atomic = "ptr") {
            300
        } else {
            1_000_000
        },
        ..budget().limits()
    });
    checks.check(&incoming, &store, &mut limited)?;
    let valid = incoming[40].clone();
    incoming[40] = source("source-0040", "memory:s", "changed")?;
    assert_eq!(
        checks.check(&incoming, &store, &mut budget()),
        Err(SourceError::IdentityConflict)
    );
    incoming[40] = valid;
    checks.check(&incoming[..20], &store, &mut budget())?;
    checks.check(&incoming, &store, &mut budget())?;
    // Same source/revision and text but a different locator is also a conflict.
    let mut different = SourceStore::default();
    different.insert(source("source-0040", "memory:other", "x")?)?;
    assert_eq!(
        checks.check(&incoming, &different, &mut budget()),
        Err(SourceError::IdentityConflict)
    );
    // A source absent in one environment must be checked when later declared.
    let absent = vec![source("later", "memory:s", "x")?];
    checks.check(&absent, &store, &mut budget())?;
    store.insert(source("later", "memory:s", "y")?)?;
    assert_eq!(
        checks.check(&absent, &store, &mut budget()),
        Err(SourceError::IdentityConflict)
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        checks.check(&incoming, &store, &mut cancelled),
        Err(SourceError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}
#[test]
fn decoded_storage_and_failed_cache_growth_do_not_bypass_checks() -> Result<(), SourceError> {
    let text = "x".repeat(4096);
    let original = source("id", "memory:s", &text)?;
    let mut store = SourceStore::default();
    store.insert(original.clone())?;
    let mut checks = SourceChecks::default();
    let incoming = vec![original];
    let mut no_allocation = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    assert_eq!(
        checks.check(&incoming, &store, &mut no_allocation),
        Err(SourceError::Stopped(StopReason::AllocationLimit))
    );
    checks.check(&incoming, &store, &mut budget())?;
    let decoded = vec![source("id", "memory:s", &text)?];
    let mut limited = Budget::new(Limits {
        work: 1000,
        ..budget().limits()
    });
    assert_eq!(
        checks.check(&decoded, &store, &mut limited),
        Err(SourceError::Stopped(StopReason::WorkLimit))
    );
    checks.check(&decoded, &store, &mut budget())?;
    let invalid = vec![source("id", "memory:s", "different")?];
    assert_eq!(
        checks.check(&invalid, &store, &mut budget()),
        Err(SourceError::IdentityConflict)
    );
    Ok(())
}

#[test]
fn partial_cache_growth_only_retains_checked_prefix() -> Result<(), SourceError> {
    let incoming = vec![source("a", "memory:s", "x")?, source("b", "memory:s", "y")?];
    let mut store = SourceStore::default();
    for source in &incoming {
        store.insert(source.clone())?;
    }
    // Measure actual clone cost, including owned copies on non-atomic targets.
    let mut clones = budget();
    for source in store.snapshots().iter().chain(incoming.iter().take(1)) {
        source.clone_with_budget(&mut clones)?;
    }
    let mut partial = Budget::new(Limits {
        allocation_units: clones.usage().allocation_units,
        ..budget().limits()
    });
    let mut checks = SourceChecks::default();
    assert_eq!(
        checks.check(&incoming, &store, &mut partial),
        Err(SourceError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(checks.checked.len(), 1);
    assert_eq!(checks.environment.len(), 2);
    let changed = vec![incoming[0].clone(), source("b", "memory:s", "changed")?];
    assert_eq!(
        checks.check(&changed, &store, &mut budget()),
        Err(SourceError::IdentityConflict)
    );
    // Equal-size environment replacement must compare identities, not lengths.
    let mut replaced = SourceStore::default();
    replaced.insert(incoming[0].clone())?;
    replaced.insert(source("b", "memory:other", "y")?)?;
    assert_eq!(
        checks.check(&incoming, &replaced, &mut budget()),
        Err(SourceError::IdentityConflict)
    );
    checks.check(&incoming, &store, &mut budget())?;
    Ok(())
}
