use nepl3_core::diagnostic::validation::{DiagnosticSourceResolver, ReportValidationError};
use nepl3_core::{budget::*, source::*};

#[test]
fn diagnostic_source_lookup_uses_index_and_checks_digest() -> Result<(), ReportValidationError> {
    let mut store = SourceStore::default();
    // Sorted IDs put the target at the end of the old linear scan.
    for i in 0..512 {
        store.insert(source(&format!("s-{i:04}"), "abc")?)?;
    }
    let target = source("s-0511", "abc")?;
    let span = target.span(1, 3)?;
    let mut limited = Budget::new(Limits {
        work: 256,
        ..budget().limits()
    });
    assert_eq!(
        DiagnosticSourceResolver::slice(&store, &span, &mut limited)?,
        "bc"
    );
    // Same ID/revision alone is insufficient; a different digest is rejected.
    let wrong = source("s-0511", "xyz")?;
    assert_eq!(
        DiagnosticSourceResolver::slice(&store, &wrong.span(0, 1)?, &mut budget()),
        Err(ReportValidationError::Source(SourceError::MissingSnapshot))
    );
    let absent = source("absent", "x")?;
    assert_eq!(
        DiagnosticSourceResolver::slice(&store, &absent.span(0, 1)?, &mut budget()),
        Err(ReportValidationError::Source(SourceError::MissingSnapshot))
    );
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert_eq!(
        DiagnosticSourceResolver::slice(&store, &span, &mut stopped),
        Err(ReportValidationError::Stopped(StopReason::WorkLimit))
    );
    Ok(())
}

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 100,
        nodes: 1000,
        allocation_units: 1_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn source(id: &str, text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(id.into()),
        0,
        "memory:test".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}
fn matches(store: &SourceStore, scope: &Option<SourceStoreScope>) -> bool {
    scope
        .as_ref()
        .is_some_and(|scope| store.matches_scope(scope))
}

#[test]
fn collection_scope_covers_each_insertion_and_failed_mutation() -> Result<(), SourceError> {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<SourceStore>();
    send_sync::<SourceStoreScope>();
    let mut store = SourceStore::default();
    let a = source("a", "a")?;
    store.insert(a.clone())?;
    for variant in 0..3 {
        store.prepare_scope(&mut budget())?;
        let old = store.scope();
        assert_eq!(matches(&store, &old), cfg!(target_has_atomic = "ptr"));
        store.insert(a.clone())?;
        store.insert_with_budget(a.clone(), &mut budget())?;
        store.insert_ref_with_budget(&a, &mut budget())?;
        assert_eq!(matches(&store, &old), cfg!(target_has_atomic = "ptr"));
        assert_eq!(
            store.insert(source("a", "conflict")?),
            Err(SourceError::IdentityConflict)
        );
        let next = source(&format!("new-{variant}"), "b")?;
        let mut stopped = Budget::new(Limits {
            work: 0,
            ..budget().limits()
        });
        assert!(store.insert_ref_with_budget(&next, &mut stopped).is_err());
        assert_eq!(matches(&store, &old), cfg!(target_has_atomic = "ptr"));
        match variant {
            0 => store.insert(next)?,
            1 => store.insert_with_budget(next, &mut budget())?,
            _ => store.insert_ref_with_budget(&next, &mut budget())?,
        }
        assert!(!matches(&store, &old));
        store.prepare_scope(&mut budget())?;
        assert!(!matches(&store, &old));
    }
    let scope = store.scope();
    let mut other = SourceStore::default();
    for snapshot in store.snapshots() {
        other.insert(snapshot.clone())?;
    }
    other.prepare_scope(&mut budget())?;
    assert!(!matches(&other, &scope));
    drop(store);
    let mut recreated = SourceStore::default();
    recreated.prepare_scope(&mut budget())?;
    assert!(!matches(&recreated, &scope));
    Ok(())
}

#[test]
fn edits_invalidate_only_on_commit_and_preparation_is_metered() -> Result<(), SourceError> {
    let mut store = SourceStore::default();
    let a = source("a", "a")?;
    store.insert(a.clone())?;
    let mut no_alloc = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    if cfg!(target_has_atomic = "ptr") {
        assert!(store.prepare_scope(&mut no_alloc).is_err());
        assert!(store.scope().is_none());
    }
    store.prepare_scope(&mut budget())?;
    let scope = store.scope();
    // A stopped Budget remains stopped; reuse needs a fresh operation budget.
    let mut no_alloc = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    store.prepare_scope(&mut no_alloc)?;
    store.apply(&[], &mut budget(), &mut SourceAdmission::default())?;
    assert_eq!(matches(&store, &scope), cfg!(target_has_atomic = "ptr"));
    let edit = TextEdit {
        span: a.span(0, 1)?,
        expected_digest: Digest::of(b"a"),
        replacement: "a".into(),
    };
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(
        store
            .apply(
                core::slice::from_ref(&edit),
                &mut stopped,
                &mut SourceAdmission::default()
            )
            .is_err()
    );
    assert_eq!(matches(&store, &scope), cfg!(target_has_atomic = "ptr"));
    // Identical text still creates a new revision, hence a new collection.
    let mut control = SourceStore::default();
    control.insert(a.clone())?;
    let mut measured = budget();
    control.apply(
        core::slice::from_ref(&edit),
        &mut measured,
        &mut SourceAdmission::default(),
    )?;
    // The final allocation charge is also before the commit point.
    let mut stopped = Budget::new(Limits {
        allocation_units: measured.usage().allocation_units - 1,
        ..budget().limits()
    });
    assert_eq!(
        store.apply(
            core::slice::from_ref(&edit),
            &mut stopped,
            &mut SourceAdmission::default()
        ),
        Err(SourceError::Stopped(StopReason::AllocationLimit))
    );
    assert!(stopped.usage().work > 0);
    assert_eq!(store.snapshots(), core::slice::from_ref(&a));
    assert_eq!(matches(&store, &scope), cfg!(target_has_atomic = "ptr"));
    store.apply(&[edit], &mut budget(), &mut SourceAdmission::default())?;
    assert!(!matches(&store, &scope));
    assert_eq!(store.snapshots().len(), 2);
    Ok(())
}
