use nepl3_core::{budget::*, source::*};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 20_000_000,
        depth: 100,
        nodes: 10000,
        allocation_units: 20_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn source(id: &str, revision: u64, uri: &str, text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(id.into()),
        revision,
        uri.into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}
fn edit(s: &SourceSnapshot) -> Result<TextEdit, SourceError> {
    Ok(TextEdit {
        span: s.span(0, 1)?,
        expected_digest: Digest::of(b"a"),
        replacement: "A".into(),
    })
}
#[test]
fn guarded_transaction_applies_only_exact_granted_revisions() -> Result<(), GuardedEditError> {
    let a = source("a", 0, "memory:a", "abc")?;
    let dependency = source("dependency", 2, "memory:dep", "x")?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    store.insert(dependency.clone())?;
    let ids = store.apply_guarded(
        &[edit(&a)?],
        &[&dependency, &dependency],
        &[a.identity().clone()],
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(ids.len(), 1);
    assert_eq!(
        store.get_ref(&ids[0]).map(SourceSnapshot::text),
        Some("Abc")
    );
    assert_eq!(store.get_ref(dependency.identity()), Some(&dependency));
    // A permission for revision zero is not a permission for the new revision.
    let current = store
        .get_ref(&ids[0])
        .cloned()
        .ok_or(SourceError::MissingSnapshot)?;
    let next = TextEdit {
        span: current.span(1, 2)?,
        expected_digest: Digest::of(b"b"),
        replacement: "B".into(),
    };
    let before = store.snapshots().to_vec();
    assert_eq!(
        store.apply_guarded(
            &[next],
            &[],
            &[a.identity().clone()],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::NotWritable { edit: 0 })
    );
    assert_eq!(store.snapshots(), before);
    Ok(())
}
#[test]
fn missing_changed_and_historical_dependencies_reject_without_mutation()
-> Result<(), GuardedEditError> {
    let a = source("a", 0, "memory:a", "abc")?;
    let expected = source("dep", 1, "memory:dep", "x")?;
    for stored in [
        None,
        Some(source("dep", 1, "memory:dep", "y")?),
        Some(source("dep", 1, "memory:other", "x")?),
        Some(source("dep", 2, "memory:dep", "x")?),
    ] {
        let mut store = SourceStore::default();
        store.insert(a.clone())?;
        let error = if let Some(s) = stored {
            store.insert(s)?;
            GuardedEditError::ChangedDependency { dependency: 0 }
        } else {
            GuardedEditError::MissingDependency { dependency: 0 }
        };
        let before = store.snapshots().to_vec();
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        assert_eq!(
            store.apply_guarded(
                &[edit(&a)?],
                &[&expected],
                &[a.identity().clone()],
                &mut b,
                &mut admission
            ),
            Err(error)
        );
        assert_eq!(store.snapshots(), before);
        assert_eq!(b.usage().source_bytes, 0);
    }
    Ok(())
}
#[test]
fn late_denial_and_late_invalid_edit_leave_all_sources_unchanged() -> Result<(), GuardedEditError> {
    let a = source("a", 0, "memory:a", "abc")?;
    let z = source("z", 0, "memory:z", "abc")?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    store.insert(z.clone())?;
    let before = store.snapshots().to_vec();
    let edits = [edit(&a)?, edit(&z)?];
    assert_eq!(
        store.apply_guarded(
            &edits,
            &[&a, &z],
            &[a.identity().clone()],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::NotWritable { edit: 1 })
    );
    assert_eq!(store.snapshots(), before);
    let mut invalid = edits.clone();
    invalid[1].expected_digest = Digest::of(b"wrong");
    assert_eq!(
        store.apply_guarded(
            &invalid,
            &[&a, &z],
            &[a.identity().clone(), z.identity().clone()],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::Source(SourceError::ExpectedDigest))
    );
    assert_eq!(store.snapshots(), before);
    Ok(())
}
#[test]
fn cancellation_and_guard_budget_exhaustion_are_sticky_and_atomic() -> Result<(), GuardedEditError>
{
    let a = source("a", 0, "memory:a", "abc")?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    for work in [0, 1, 2, 10] {
        let mut limits = budget().limits();
        limits.work = work;
        let mut b = Budget::new(limits);
        let result = store.apply_guarded(
            &[edit(&a)?],
            &[&a],
            &[a.identity().clone()],
            &mut b,
            &mut SourceAdmission::default(),
        );
        assert_eq!(
            result,
            Err(GuardedEditError::Source(SourceError::Stopped(
                StopReason::WorkLimit
            )))
        );
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        assert_eq!(store.snapshots(), core::slice::from_ref(&a));
    }
    let mut b = budget();
    b.stop(StopReason::Cancelled);
    assert_eq!(
        store.apply_guarded(&[], &[], &[], &mut b, &mut SourceAdmission::default()),
        Err(GuardedEditError::Source(SourceError::Stopped(
            StopReason::Cancelled
        )))
    );
    Ok(())
}
#[test]
fn empty_edits_still_check_dependencies_and_empty_grants_allow_nothing()
-> Result<(), GuardedEditError> {
    let a = source("a", 0, "memory:a", "abc")?;
    let mut store = SourceStore::default();
    assert_eq!(
        store.apply_guarded(
            &[],
            &[&a],
            &[],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::MissingDependency { dependency: 0 })
    );
    store.insert(a.clone())?;
    assert_eq!(
        store.apply_guarded(
            &[edit(&a)?],
            &[],
            &[],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::NotWritable { edit: 0 })
    );
    assert!(
        store
            .apply_guarded(
                &[],
                &[&a],
                &[],
                &mut budget(),
                &mut SourceAdmission::default()
            )?
            .is_empty()
    );
    Ok(())
}

#[test]
fn guard_compares_independent_values_and_rejects_historical_or_false_grants()
-> Result<(), GuardedEditError> {
    let a = source("a", 0, "memory:a", "abc")?;
    let copy = source("a", 0, "memory:a", "abc")?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    assert!(
        store
            .apply_guarded(
                &[],
                &[&copy],
                &[],
                &mut budget(),
                &mut SourceAdmission::default()
            )?
            .is_empty()
    );
    let mut false_grant = a.identity().clone();
    false_grant.digest = Digest::of(b"different");
    assert_eq!(
        store.apply_guarded(
            &[edit(&a)?],
            &[],
            &[false_grant],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::NotWritable { edit: 0 })
    );
    let older = source("dep", 1, "memory:dep", "x")?;
    let newer = source("dep", 3, "memory:dep", "x")?;
    store.insert(newer)?;
    store.insert(older.clone())?;
    let before = store.snapshots().to_vec();
    assert_eq!(
        store.apply_guarded(
            &[edit(&a)?],
            &[&older],
            &[a.identity().clone()],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::ChangedDependency { dependency: 0 })
    );
    assert_eq!(store.snapshots(), before);
    Ok(())
}
#[test]
fn every_guard_work_boundary_preserves_store_and_uses_no_admission_or_allocation()
-> Result<(), GuardedEditError> {
    let a = source(&"a".repeat(256), 0, "memory:a", "abc")?;
    let copy = source(&"a".repeat(256), 0, "memory:a", "abc")?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    let edits = [edit(&a)?];
    let grants: Vec<_> = (1..8)
        .map(|revision| {
            let mut id = a.identity().clone();
            id.revision = revision;
            id
        })
        .collect();
    let mut complete = budget();
    assert_eq!(
        store.apply_guarded(
            &edits,
            &[&copy],
            &grants,
            &mut complete,
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::NotWritable { edit: 0 })
    );
    let required = complete.usage().work;
    // Sweep every limit through lookup, full equality and each rejected grant.
    for work in 0..required {
        let mut limits = budget().limits();
        limits.work = work;
        limits.allocation_units = 0;
        limits.source_bytes = 0;
        let mut b = Budget::new(limits);
        let error = store.apply_guarded(
            &edits,
            &[&copy],
            &grants,
            &mut b,
            &mut SourceAdmission::default(),
        );
        assert_eq!(
            error,
            Err(GuardedEditError::Source(SourceError::Stopped(
                StopReason::WorkLimit
            )))
        );
        assert_eq!(b.usage().allocation_units, 0);
        assert_eq!(b.usage().source_bytes, 0);
        assert_eq!(store.snapshots(), core::slice::from_ref(&a));
    }
    Ok(())
}

#[test]
fn late_dependency_indices_and_stale_edits_do_not_publish_partial_results()
-> Result<(), GuardedEditError> {
    let a = source("a", 0, "memory:a", "abc")?;
    let missing = source("missing", 0, "memory:missing", "x")?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    let before = store.snapshots().to_vec();
    assert_eq!(
        store.apply_guarded(
            &[edit(&a)?],
            &[&a, &missing],
            &[a.identity().clone()],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::MissingDependency { dependency: 1 })
    );
    assert_eq!(store.snapshots(), before);
    store.insert(source("a", 1, "memory:a", "abc")?)?;
    let before = store.snapshots().to_vec();
    // An explicit old grant does not bypass apply's latest-revision rule.
    assert_eq!(
        store.apply_guarded(
            &[edit(&a)?],
            &[],
            &[a.identity().clone()],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GuardedEditError::Source(SourceError::SnapshotMismatch))
    );
    assert_eq!(store.snapshots(), before);
    Ok(())
}
