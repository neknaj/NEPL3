use nepl3_core::{budget::*, source::*};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 20_000_000,
        depth: 100,
        nodes: 1000,
        allocation_units: 20_000_000,
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
fn fixture() -> Result<(SourceStore, Vec<TextEdit>), SourceError> {
    let mut store = SourceStore::default();
    let mut edits = Vec::new();
    for id in ["a", "b"] {
        let input = source(id, "memory:edit", "abc")?;
        edits.push(TextEdit {
            span: input.span(0, 1)?,
            expected_digest: Digest::of(b"a"),
            replacement: "A".into(),
        });
        store.insert(input)?;
    }
    Ok((store, edits))
}
#[test]
fn deletion_admits_original_and_generated_snapshots_once() -> Result<(), SourceError> {
    let input = source("a", "memory:a", "abc")?;
    let edit = TextEdit {
        span: input.span(0, 3)?,
        expected_digest: Digest::of(b"abc"),
        replacement: String::new(),
    };
    let mut store = SourceStore::default();
    store.insert(input.clone())?;
    let mut limits = budget().limits();
    limits.source_bytes = 0;
    let mut stopped = Budget::new(limits);
    assert_eq!(
        store.apply(
            core::slice::from_ref(&edit),
            &mut stopped,
            &mut SourceAdmission::default()
        ),
        Err(SourceError::Stopped(StopReason::SourceLimit))
    );
    assert_eq!(store.snapshots(), core::slice::from_ref(&input));
    let mut operation = budget();
    let mut admission = SourceAdmission::default();
    admission.admit_existing(&input, &mut operation)?;
    let ids = store.apply(&[edit], &mut operation, &mut admission)?;
    let generated = store.get_ref(&ids[0]).ok_or(SourceError::MissingSnapshot)?;
    assert_eq!(generated.text(), "");
    assert_eq!(generated.identity().digest, Digest::of(b""));
    assert_eq!(generated.identity().revision, 1);
    admission.admit_existing(generated, &mut operation)?;
    assert_eq!(operation.usage().source_bytes, 3);
    let edit = TextEdit {
        span: generated.span(0, 0)?,
        expected_digest: Digest::of(b""),
        replacement: "x".into(),
    };
    store.apply(&[edit], &mut operation, &mut admission)?;
    assert_eq!(operation.usage().source_bytes, 4);
    Ok(())
}
#[test]
fn semantic_failure_does_not_reserve_an_unpublished_output_identity() -> Result<(), SourceError> {
    let (mut store, mut edits) = fixture()?;
    let mut operation = budget();
    let mut admission = SourceAdmission::default();
    edits[1].expected_digest = Digest::of(b"wrong");
    assert_eq!(
        store.apply(&edits, &mut operation, &mut admission),
        Err(SourceError::ExpectedDigest)
    );
    let spent = operation.usage();
    assert_eq!(store.snapshots().len(), 2);
    edits[0].replacement = "Q".into();
    edits[1].expected_digest = Digest::of(b"a");
    let ids = store.apply(&edits, &mut operation, &mut admission)?;
    assert_eq!(
        store.get_ref(&ids[0]).map(SourceSnapshot::text),
        Some("Qbc")
    );
    assert_eq!(
        store.get_ref(&ids[1]).map(SourceSnapshot::text),
        Some("Abc")
    );
    assert_eq!(operation.usage().source_bytes, 12);
    assert!(operation.usage().work > spent.work);
    assert!(operation.usage().allocation_units > spent.allocation_units);
    Ok(())
}
#[test]
fn every_allocation_stop_is_atomic_sticky_and_retryable_in_a_fresh_operation()
-> Result<(), SourceError> {
    let (mut successful, edits) = fixture()?;
    let mut full = budget();
    successful.apply(&edits, &mut full, &mut SourceAdmission::default())?;
    let required = full.usage().allocation_units;
    for cap in 0..required {
        let (mut store, edits) = fixture()?;
        let mut limits = budget().limits();
        limits.allocation_units = cap;
        let mut operation = Budget::new(limits);
        let mut admission = SourceAdmission::default();
        assert_eq!(
            store.apply(&edits, &mut operation, &mut admission),
            Err(SourceError::Stopped(StopReason::AllocationLimit)),
            "cap={cap}"
        );
        assert_eq!(store.snapshots().len(), 2, "cap={cap}");
        assert!(store.snapshots().iter().all(|s| s.identity().revision == 0));
        assert_eq!(
            store.apply(&[], &mut operation, &mut admission),
            Err(SourceError::Stopped(StopReason::AllocationLimit))
        );
    }
    let (mut store, edits) = fixture()?;
    let mut limits = budget().limits();
    limits.allocation_units = required - 1;
    let mut failed = Budget::new(limits);
    assert!(
        store
            .apply(&edits, &mut failed, &mut SourceAdmission::default())
            .is_err()
    );
    // A fresh operation has its own admission and cumulative usage.
    store.apply(&edits, &mut budget(), &mut SourceAdmission::default())?;
    assert_eq!(store.snapshots(), successful.snapshots());
    Ok(())
}
#[test]
fn work_source_and_locator_limits_stop_before_unbounded_owned_copies() -> Result<(), SourceError> {
    for (id, uri) in [
        ("x".repeat(100_000), "memory:short".into()),
        ("short".into(), format!("memory:{}", "x".repeat(100_000))),
    ] {
        let input = source(&id, &uri, "abc")?;
        let edit = TextEdit {
            span: input.span(0, 1)?,
            expected_digest: Digest::of(b"a"),
            replacement: "A".into(),
        };
        let mut store = SourceStore::default();
        store.insert(input)?;
        let mut limits = budget().limits();
        limits.allocation_units = 0;
        let mut limited = Budget::new(limits);
        assert_eq!(
            store.apply(
                core::slice::from_ref(&edit),
                &mut limited,
                &mut SourceAdmission::default()
            ),
            Err(SourceError::Stopped(StopReason::AllocationLimit))
        );
        assert_eq!(limited.usage().allocation_units, 0);
        limits = budget().limits();
        limits.work = 2000;
        assert_eq!(
            store.apply(
                core::slice::from_ref(&edit),
                &mut Budget::new(limits),
                &mut SourceAdmission::default()
            ),
            Err(SourceError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(store.snapshots().len(), 1);
        let ids = store.apply(&[edit], &mut budget(), &mut SourceAdmission::default())?;
        let output = store.get_ref(&ids[0]).ok_or(SourceError::MissingSnapshot)?;
        assert_eq!(output.text(), "Abc");
        assert_eq!(output.uri(), uri);
        assert_eq!(output.identity().source.0, id);
        assert_eq!(output.identity().digest, Digest::of(b"Abc"));
    }
    for cap in [0, 6, 11] {
        let (mut store, edits) = fixture()?;
        let mut limits = budget().limits();
        limits.source_bytes = cap;
        assert_eq!(
            store.apply(
                &edits,
                &mut Budget::new(limits),
                &mut SourceAdmission::default()
            ),
            Err(SourceError::Stopped(StopReason::SourceLimit))
        );
        assert_eq!(store.snapshots().len(), 2);
    }
    let (mut store, edits) = fixture()?;
    let mut stopped = budget();
    stopped.cancel();
    assert_eq!(
        store.apply(&edits, &mut stopped, &mut SourceAdmission::default()),
        Err(SourceError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(stopped.usage(), Usage::default());
    Ok(())
}

#[test]
fn a_conflicting_preexisting_output_does_not_publish_other_transaction_outputs()
-> Result<(), SourceError> {
    let (mut store, mut edits) = fixture()?;
    let mut operation = budget();
    let mut admission = SourceAdmission::default();
    let reserved = SourceSnapshot::new(
        SourceId("b".into()),
        1,
        "memory:edit".into(),
        b"Zbc".to_vec(),
        &mut budget(),
    )?;
    admission.admit_existing(&reserved, &mut operation)?;
    assert_eq!(
        store.apply(&edits, &mut operation, &mut admission),
        Err(SourceError::IdentityConflict)
    );
    assert_eq!(store.snapshots().len(), 2);
    edits[0].replacement = "Q".into();
    edits[1].replacement = "Z".into();
    let ids = store.apply(&edits, &mut operation, &mut admission)?;
    assert_eq!(
        store.get_ref(&ids[0]).map(SourceSnapshot::text),
        Some("Qbc")
    );
    assert_eq!(store.get_ref(&ids[1]), Some(&reserved));
    assert_eq!(operation.usage().source_bytes, 12);
    Ok(())
}
