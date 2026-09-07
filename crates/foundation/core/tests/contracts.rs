use nepl3_core::{budget::*, source::*, value::*};
use num_bigint::{BigInt, BigUint};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 100,
        nodes: 1_000_000,
        allocation_units: 1_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn source(id: &str, revision: u64, text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(id.into()),
        revision,
        "memory:document".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}

#[test]
fn snapshot_metadata_copy_is_bounded_but_independent_comparison_is_not_free()
-> Result<(), SourceError> {
    let id = SourceId("名".repeat(20_000));
    let uri = format!("memory:{}", "u".repeat(60_000));
    let original =
        SourceSnapshot::new(id.clone(), 1, uri.clone(), b"text".to_vec(), &mut budget())?;
    let limits = Limits {
        work: 1000,
        allocation_units: 1024,
        ..budget().limits()
    };
    let copy = original.clone_with_budget(&mut Budget::new(limits));
    #[cfg(target_has_atomic = "ptr")]
    {
        let copy = copy.map_err(SourceError::Stopped)?;
        assert!(original.eq_with_budget(&copy, &mut Budget::new(limits))?);
        drop(original);
        assert_eq!(copy.identity().source, id);
        assert_eq!(copy.uri(), uri);
        assert_eq!(copy.text(), "text");
        assert_eq!(
            copy.charge_clone(&mut Budget::new(limits)),
            Err(StopReason::WorkLimit)
        );
        let independently_built = SourceSnapshot::new(id, 1, uri, b"text".to_vec(), &mut budget())?;
        assert_eq!(
            copy.eq_with_budget(&independently_built, &mut Budget::new(limits)),
            Err(StopReason::WorkLimit)
        );
        assert!(copy.eq_with_budget(&independently_built, &mut budget())?);
    }
    #[cfg(not(target_has_atomic = "ptr"))]
    assert_eq!(copy, Err(StopReason::WorkLimit));
    Ok(())
}
#[test]
fn borrowed_store_insert_does_not_clone_duplicates_or_publish_on_stop() -> Result<(), SourceError> {
    let original = source("日本語", 7, &"x".repeat(10_000))?;
    let mut store = SourceStore::default();
    store.insert_ref_with_budget(&original, &mut budget())?;
    let mut no_allocation = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    store.insert_ref_with_budget(&original, &mut no_allocation)?;
    assert_eq!(no_allocation.usage().allocation_units, 0);
    assert_eq!(store.snapshots(), core::slice::from_ref(&original));
    let independently_built = source("日本語", 7, &"x".repeat(10_000))?;
    let mut too_little_comparison = Budget::new(Limits {
        work: 1000,
        ..budget().limits()
    });
    assert_eq!(
        store.insert_ref_with_budget(&independently_built, &mut too_little_comparison),
        Err(SourceError::Stopped(StopReason::WorkLimit))
    );
    let changed = source("日本語", 7, "changed")?;
    assert_eq!(
        store.insert_ref_with_budget(&changed, &mut budget()),
        Err(SourceError::IdentityConflict)
    );
    let next = source("a", 0, "new")?;
    let mut stopped = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    assert_eq!(
        store.insert_ref_with_budget(&next, &mut stopped),
        Err(SourceError::Stopped(StopReason::AllocationLimit))
    );
    assert!(store.get_ref(next.identity()).is_none());
    store.insert_ref_with_budget(&next, &mut budget())?;
    assert_eq!(store.snapshots(), &[original, next]);
    Ok(())
}
#[test]
fn source_digest_is_sha256_of_exact_original_bytes() -> Result<(), SourceError> {
    let empty = source("a", 0, "")?;
    // FIPS SHA-256 empty-message vector, independent of our implementation.
    assert_eq!(
        empty.id().digest.0,
        [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55
        ]
    );
    let text = "\u{feff}日🙂\r\n";
    let snapshot = source("a", 1, text)?;
    assert!(snapshot.has_bom());
    assert_eq!(snapshot.text().as_bytes(), text.as_bytes());
    assert_ne!(
        source("a", 1, "a\r\n")?.id().digest,
        source("a", 1, "a\n")?.id().digest
    );
    Ok(())
}

#[test]
fn source_limits_precede_decode_and_decode_does_not_replace_bytes() {
    let mut limited = Budget::new(Limits::default());
    assert_eq!(
        SourceSnapshot::new(
            SourceId("a".into()),
            0,
            "memory:a".into(),
            vec![255],
            &mut limited
        ),
        Err(SourceError::Stopped(StopReason::SourceLimit))
    );
    assert_eq!(
        SourceSnapshot::new(
            SourceId("a".into()),
            0,
            "memory:a".into(),
            vec![255],
            &mut budget()
        ),
        Err(SourceError::Decode {
            valid_up_to: 0,
            error_len: Some(1)
        })
    );
    assert_eq!(
        SourceSnapshot::new(
            SourceId("a".into()),
            0,
            "memory:a".into(),
            vec![0xe3, 0x81],
            &mut budget()
        ),
        Err(SourceError::Decode {
            valid_up_to: 0,
            error_len: None
        })
    );
}

#[test]
fn span_rejects_scalar_interiors_bounds_and_other_identity() -> Result<(), SourceError> {
    let a = source("a", 0, "日🙂\r\n")?;
    let b = source("b", 0, "日🙂\r\n")?;
    assert!(a.span(1, 3).is_err());
    assert!(a.span(3, 5).is_err());
    assert!(a.span(1, 0).is_err());
    assert!(a.span(0, u64::MAX).is_err());
    assert!(a.span(8, 8).is_ok()); // Between CR and LF is a valid raw UTF-8 scalar boundary.
    assert_eq!(a.slice(&a.span(9, 9)?)?, "");
    assert_eq!(b.slice(&a.span(0, 3)?), Err(SourceError::SnapshotMismatch));
    assert_eq!(
        source("a", 1, a.text())?.slice(&a.span(0, 3)?),
        Err(SourceError::SnapshotMismatch)
    );
    assert_eq!(
        source("a", 0, "changed")?.slice(&a.span(0, 3)?),
        Err(SourceError::SnapshotMismatch)
    );
    Ok(())
}

#[test]
fn line_positions_roundtrip_unicode_and_all_line_endings() -> Result<(), SourceError> {
    let snapshot = source("a", 0, "\u{feff}日🙂\r\nx\ry\nz")?;
    let index = LineIndex::new(&snapshot, &mut budget())?;
    assert_eq!(index.line_count(), 4);
    for encoding in [
        PositionEncoding::Utf8,
        PositionEncoding::Utf16,
        PositionEncoding::Utf32,
    ] {
        for offset in 0..=snapshot.text().len() as u64 {
            if snapshot.span(offset, offset).is_err() {
                continue;
            }
            if let Ok(position) = index.position(&snapshot, offset, encoding) {
                assert_eq!(index.offset(&snapshot, position, encoding)?, offset);
            }
        }
    }
    assert_eq!(
        index.position(&snapshot, 10, PositionEncoding::Utf16)?,
        Position {
            line: 0,
            character: 4
        }
    );
    assert_eq!(
        index.position(&snapshot, 11, PositionEncoding::Utf16),
        Err(SourceError::LineTerminator)
    );
    assert_eq!(
        index.offset(
            &snapshot,
            Position {
                line: 0,
                character: 3
            },
            PositionEncoding::Utf16
        ),
        Err(SourceError::ScalarBoundary)
    );
    assert_eq!(
        index.offset(
            &snapshot,
            Position {
                line: u64::MAX,
                character: 0
            },
            PositionEncoding::Utf8
        ),
        Err(SourceError::Position)
    );
    assert_eq!(
        index.position(&source("b", 0, snapshot.text())?, 0, PositionEncoding::Utf8),
        Err(SourceError::SnapshotMismatch)
    );
    Ok(())
}

#[test]
fn line_index_allocation_limit_is_enforced() -> Result<(), SourceError> {
    let snapshot = source("a", 0, "\n\n\n")?;
    let mut limits = budget().limits();
    limits.allocation_units = 0;
    assert!(matches!(
        LineIndex::new(&snapshot, &mut Budget::new(limits)),
        Err(SourceError::Stopped(StopReason::AllocationLimit))
    ));
    Ok(())
}

#[test]
fn store_preserves_distinct_ids_at_same_uri_and_rejects_conflicting_revision()
-> Result<(), SourceError> {
    let a = source("a", 0, "same")?;
    let b = source("b", 0, "same")?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    store.insert(b.clone())?;
    assert_eq!(store.resolve(&a.reference()), Some(&a));
    assert_eq!(store.resolve(&b.reference()), Some(&b));
    assert_eq!(
        store.insert(source("a", 0, "different")?),
        Err(SourceError::IdentityConflict)
    );
    Ok(())
}

#[test]
fn edits_are_atomic_across_sources_and_reject_stale_or_conflicting_input() -> Result<(), SourceError>
{
    let a = source("a", 0, "日abc")?;
    let b = source("b", 0, "xyz")?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    store.insert(b.clone())?;
    let edit = TextEdit {
        span: a.span(3, 4)?,
        expected_digest: Digest::of(b"a"),
        replacement: "🙂".into(),
    };
    let wrong = TextEdit {
        span: b.span(0, 1)?,
        expected_digest: Digest::of(b"wrong"),
        replacement: "X".into(),
    };
    assert_eq!(
        store.apply(
            &[edit.clone(), wrong],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(SourceError::ExpectedDigest)
    );
    assert_eq!(store.latest(&a.id().source), Some(&a));
    assert_eq!(
        store.apply(
            &[edit.clone(), edit.clone()],
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(SourceError::OverlappingEdits)
    );
    let ids = store.apply(
        core::slice::from_ref(&edit),
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(
        store.get(ids[0].clone()).map(SourceSnapshot::text),
        Some("日🙂bc")
    );
    assert_eq!(
        store.apply(&[edit], &mut budget(), &mut SourceAdmission::default()),
        Err(SourceError::SnapshotMismatch)
    );
    Ok(())
}

#[test]
fn edit_size_limit_checked_before_replacement_is_built() -> Result<(), SourceError> {
    let a = source("a", 0, "a")?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    let edit = TextEdit {
        span: a.span(0, 1)?,
        expected_digest: Digest::of(b"a"),
        replacement: "long".repeat(100),
    };
    let mut limits = budget().limits();
    limits.source_bytes = 1;
    let mut budget = Budget::new(limits);
    assert_eq!(
        store.apply(&[edit], &mut budget, &mut SourceAdmission::default()),
        Err(SourceError::Stopped(StopReason::SourceLimit))
    );
    // Borrowed edit ordering and the input admission ledger are real allocations.
    // The 400-byte output buffer has not been allocated when its source cap fails.
    assert!(budget.usage().allocation_units > 0);
    assert!(budget.usage().allocation_units < 400);
    assert_eq!(budget.usage().source_bytes, 1);
    assert_eq!(store.latest(&a.id().source), Some(&a));
    Ok(())
}

#[test]
fn snapshot_clones_preserve_owned_lifetime_and_independent_identity_checks()
-> Result<(), SourceError> {
    let content = "a".repeat(100_000);
    let original = source("shared", 3, &content)?;
    let mut b = budget();
    let cloned = original.clone_with_budget(&mut b)?;
    #[cfg(target_has_atomic = "ptr")]
    assert!(b.usage().allocation_units < 1024);
    #[cfg(target_has_atomic = "ptr")]
    assert!(b.usage().work < 1024);
    #[cfg(not(target_has_atomic = "ptr"))]
    assert!(b.usage().work >= content.len() as u64);
    let mut comparison = budget();
    original.charge_clone(&mut comparison)?;
    assert!(comparison.usage().work >= content.len() as u64);
    let independent = source("shared", 3, &content)?;
    assert_eq!(cloned, independent);
    assert_ne!(cloned, source("shared", 4, &content)?);
    let mut changed = content.clone();
    changed.replace_range(99_999..100_000, "b");
    assert_ne!(cloned, source("shared", 3, &changed)?);
    drop(original);
    assert_eq!(cloned.text(), content);
    assert_eq!(cloned.slice(&cloned.span(99_999, 100_000)?)?, "a");
    let mut limits = budget().limits();
    limits.allocation_units = 0;
    assert_eq!(
        cloned.clone_with_budget(&mut Budget::new(limits)),
        Err(StopReason::AllocationLimit)
    );
    #[cfg(target_has_atomic = "ptr")]
    {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<SourceSnapshot>();
    }
    Ok(())
}

#[test]
fn locator_profile_is_checked_without_os_or_scheme_normalization() {
    for uri in [
        "relative",
        "1x:a",
        "memory:",
        "memory:a b",
        "memory:%",
        "memory:%xz",
    ] {
        assert!(
            SourceSnapshot::new(SourceId("x".into()), 0, uri.into(), vec![], &mut budget())
                .is_err()
        );
    }
    assert!(
        SourceSnapshot::new(
            SourceId("x".into()),
            0,
            "memory:%e3%81%82".into(),
            vec![],
            &mut budget()
        )
        .is_ok()
    );
}

#[test]
fn canonical_integers_reject_negative_zero_and_leading_zero() {
    assert_eq!(
        Integer::from_canonical(true, &[]),
        Err(NumberError::NegativeZero)
    );
    assert_eq!(
        Integer::from_canonical(false, &[0, 1]),
        Err(NumberError::LeadingZero)
    );
    assert_eq!(Integer::from(0i64).canonical_parts(), (false, vec![]));
    let bytes = vec![255; 100];
    let value = Integer::from_canonical(true, &bytes);
    assert_eq!(value.map(|n| n.canonical_parts()), Ok((true, bytes)));
}

#[test]
fn rational_constructors_normalize_native_but_reject_noncanonical_wire() {
    let normalized = Rational::new(BigInt::from(6), BigInt::from(-8));
    assert_eq!(
        normalized.map(|r| (r.numerator().clone(), r.denominator().clone())),
        Ok((Integer::from(-3i64), BigUint::from(4u64)))
    );
    assert_eq!(
        Rational::from_canonical(Integer::from(2i64), &[4]),
        Err(NumberError::NotReduced)
    );
    assert_eq!(
        Rational::from_canonical(Integer::from(0i64), &[2]),
        Err(NumberError::NotReduced)
    );
    assert_eq!(
        Rational::from_canonical(Integer::from(1i64), &[]),
        Err(NumberError::ZeroDenominator)
    );
    assert_eq!(
        Rational::new(BigInt::from(1), BigInt::from(0)),
        Err(NumberError::ZeroDenominator)
    );
    assert_eq!(
        Rational::new(BigInt::from(0), BigInt::from(-55)).map(|r| r.denominator_bytes()),
        Ok(vec![1])
    );
}

#[test]
fn budget_exact_limits_overflow_zero_and_cancel_are_distinct() {
    let mut zero = Budget::new(Limits::default());
    assert_eq!(zero.charge(Resource::Work, 0), Ok(()));
    assert_eq!(zero.charge(Resource::Work, 1), Err(StopReason::WorkLimit));
    let mut limits = budget().limits();
    limits.work = u64::MAX;
    let mut budget = Budget::new(limits);
    assert_eq!(budget.charge(Resource::Work, u64::MAX), Ok(()));
    assert_eq!(budget.charge(Resource::Work, 1), Err(StopReason::WorkLimit));
    assert_eq!(budget.usage().work, u64::MAX);
    budget.cancel();
    assert_eq!(budget.charge(Resource::Work, 0), Err(StopReason::WorkLimit));
    let mut fresh = Budget::new(limits);
    fresh.cancel();
    assert_eq!(fresh.poll(), Err(StopReason::Cancelled));
}

#[test]
fn nested_budget_retains_work_and_peak_depth_after_failed_branch() -> Result<(), StopReason> {
    let mut budget = budget();
    let result: Result<(), StopReason> = budget.with_depth(|b| {
        b.with_depth(|b| {
            b.charge(Resource::Work, 7)?;
            Err(StopReason::NodeLimit)
        })
    });
    assert_eq!(result, Err(StopReason::NodeLimit));
    assert_eq!(budget.usage().depth, 2);
    assert_eq!(budget.usage().work, 7);
    budget.with_depth(|b| b.charge(Resource::Work, 1))?;
    assert_eq!(budget.usage().depth, 2);
    assert_eq!(budget.usage().work, 8);
    Ok(())
}

#[test]
fn source_admission_charges_unique_snapshots_once_across_shared_calls() -> Result<(), SourceError> {
    let mut admission = SourceAdmission::default();
    let mut budget = budget();
    let first = admission.create(
        SourceId("a".into()),
        0,
        "memory:a".into(),
        b"abc".to_vec(),
        &mut budget,
    )?;
    admission.admit_existing(&first, &mut budget)?;
    admission.admit_existing(&first, &mut budget)?;
    assert_eq!(budget.usage().source_bytes, 3);
    let second = source("b", 0, "abc")?;
    admission.admit_existing(&second, &mut budget)?;
    assert_eq!(budget.usage().source_bytes, 6);
    assert_eq!(
        admission.admit_existing(&source("a", 0, "xyz")?, &mut budget),
        Err(SourceError::IdentityConflict)
    );
    Ok(())
}

#[test]
fn first_budget_stop_is_sticky_across_smaller_charges_and_cancellation() {
    let mut budget = Budget::new(Limits::default());
    assert_eq!(budget.charge(Resource::Work, 1), Err(StopReason::WorkLimit));
    assert_eq!(
        budget.charge(Resource::Events, 0),
        Err(StopReason::WorkLimit)
    );
    budget.cancel();
    assert_eq!(budget.poll(), Err(StopReason::WorkLimit));
}

#[test]
fn source_admission_rejects_locator_changes_for_same_snapshot_identity() -> Result<(), SourceError>
{
    let mut admission = SourceAdmission::default();
    let mut budget = budget();
    let first = admission.create(
        SourceId("a".into()),
        0,
        "memory:first".into(),
        b"same".to_vec(),
        &mut budget,
    )?;
    let other = SourceSnapshot::new(
        SourceId("a".into()),
        0,
        "memory:other".into(),
        b"same".to_vec(),
        &mut Budget::new(budget.limits()),
    )?;
    assert_eq!(first.id(), other.id());
    assert_eq!(
        admission.admit_existing(&other, &mut budget),
        Err(SourceError::IdentityConflict)
    );
    Ok(())
}

#[test]
fn admission_accounts_for_copied_locator_payload() -> Result<(), SourceError> {
    let snapshot = SourceSnapshot::new(
        SourceId("x".into()),
        0,
        format!("memory:{}", "a".repeat(10_000)),
        vec![],
        &mut budget(),
    )?;
    let mut limits = budget().limits();
    limits.allocation_units = 200;
    assert_eq!(
        SourceAdmission::default().admit_existing(&snapshot, &mut Budget::new(limits)),
        Err(SourceError::Stopped(StopReason::AllocationLimit))
    );
    Ok(())
}

#[test]
fn generated_source_locator_scan_is_charged_before_validation() -> Result<(), SourceError> {
    let mut limits = budget().limits();
    limits.work = 1000;
    let uri = format!("memory:{}", "x".repeat(100_000));
    for value in [uri.clone(), format!("{uri} ")] {
        let mut limited = Budget::new(limits);
        assert_eq!(
            SourceSnapshot::new(
                SourceId("generated".into()),
                1,
                value,
                b"x".to_vec(),
                &mut limited
            ),
            Err(SourceError::Stopped(StopReason::WorkLimit))
        );
    }
    let mut setup = budget();
    let mut admission = SourceAdmission::default();
    admission.create(
        SourceId("generated".into()),
        1,
        uri.clone(),
        b"x".to_vec(),
        &mut setup,
    )?;
    let mut limited = Budget::new(limits);
    assert_eq!(
        admission.import(
            SourceId("generated".into()),
            1,
            uri,
            b"x".to_vec(),
            &mut limited
        ),
        Err(SourceError::Stopped(StopReason::WorkLimit))
    );
    Ok(())
}

#[test]
fn shared_copy_and_independent_comparison_have_separate_work_bounds() -> Result<(), SourceError> {
    let text = "x".repeat(100_000);
    let original = source("shared-cost", 0, &text)?;
    let mut copy_budget = budget();
    original.charge_shared_clone(&mut copy_budget)?;
    let cloned = original.clone();
    #[cfg(target_has_atomic = "ptr")]
    assert!(copy_budget.usage().work < 1024);
    #[cfg(not(target_has_atomic = "ptr"))]
    assert!(copy_budget.usage().work >= text.len() as u64);
    let mut limits = budget().limits();
    limits.work = 1024;
    #[cfg(target_has_atomic = "ptr")]
    assert!(original.eq_with_budget(&cloned, &mut Budget::new(limits))?);
    let independent = source("shared-cost", 0, &text)?;
    assert_eq!(
        original.eq_with_budget(&independent, &mut Budget::new(limits)),
        Err(StopReason::WorkLimit)
    );
    let mut store = SourceStore::default();
    store.insert(original)?;
    assert_eq!(
        store.insert_with_budget(independent, &mut Budget::new(limits)),
        Err(SourceError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(store.snapshots().len(), 1);
    assert_eq!(store.snapshots()[0], cloned);
    let conflict = SourceReservation {
        source_id: SourceId("shared-cost".into()),
        revision: 0,
        uri: "memory:different".into(),
    };
    let conflicting = SourceAdmission::default().create(
        conflict.source_id,
        conflict.revision,
        conflict.uri,
        text.into_bytes(),
        &mut budget(),
    )?;
    assert_eq!(
        store.insert_with_budget(conflicting, &mut budget()),
        Err(SourceError::IdentityConflict)
    );
    Ok(())
}

#[test]
fn indexed_source_insert_preserves_order_and_metered_duplicate_lookup() -> Result<(), SourceError> {
    let mut store = SourceStore::default();
    for index in (0..128).rev() {
        store.insert(source(&format!("source-{index:03}"), 0, "data")?)?;
    }
    assert_eq!(store.snapshots()[0].identity().source.0, "source-127");
    assert_eq!(store.snapshots()[127].identity().source.0, "source-000");
    let duplicate = store.snapshots()[127].clone();
    let mut b = budget();
    store.insert_with_budget(duplicate, &mut b)?;
    assert!(b.usage().work < 1024);
    assert_eq!(store.snapshots().len(), 128);
    let new = source("source-middle", 0, "other")?;
    let mut limits = budget().limits();
    limits.allocation_units = 0;
    assert_eq!(
        store.insert_with_budget(new.clone(), &mut Budget::new(limits)),
        Err(SourceError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(store.snapshots().len(), 128);
    store.insert_with_budget(new, &mut budget())?;
    assert_eq!(store.snapshots()[128].identity().source.0, "source-middle");
    // Edits must update the same index atomically before later duplicate insertion.
    let old = store.snapshots()[0].clone();
    let ids = store.apply(
        &[TextEdit {
            span: old.span(0, 4)?,
            expected_digest: old.identity().digest,
            replacement: "changed".into(),
        }],
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let changed = store
        .get_ref(&ids[0])
        .ok_or(SourceError::MissingSnapshot)?
        .clone();
    store.insert_with_budget(changed, &mut budget())?;
    assert_eq!(store.snapshots().len(), 130);
    Ok(())
}

#[test]
fn indexed_admission_keeps_unique_bytes_and_checks_empty_cancel() -> Result<(), SourceError> {
    let store = SourceStore::default();
    let mut stopped = budget();
    stopped.cancel();
    assert!(matches!(
        store.get_revision_with_budget(&SourceId("none".into()), 0, &mut stopped),
        Err(StopReason::Cancelled)
    ));
    let mut admission = SourceAdmission::default();
    let mut b = budget();
    let mut snapshots = Vec::new();
    for i in (0..128).rev() {
        snapshots.push(admission.create(
            SourceId(format!("source-{i:03}")),
            0,
            format!("memory:{i}"),
            b"data".to_vec(),
            &mut b,
        )?);
    }
    assert_eq!(b.usage().source_bytes, 512);
    let before = b.usage();
    admission.admit_existing(&snapshots[127], &mut b)?;
    assert_eq!(b.usage().source_bytes, 512);
    assert!(b.usage().work - before.work < 1024);
    let copy = admission.import(
        snapshots[127].identity().source.clone(),
        0,
        snapshots[127].uri().into(),
        b"data".to_vec(),
        &mut b,
    )?;
    assert_eq!(copy, snapshots[127]);
    assert_eq!(b.usage().source_bytes, 512);
    assert_eq!(
        admission.import(
            copy.identity().source.clone(),
            0,
            "memory:conflict".into(),
            b"data".to_vec(),
            &mut b
        ),
        Err(SourceError::IdentityConflict)
    );
    Ok(())
}
#[test]
fn shared_admission_is_operation_local_and_independent_storage_still_checked()
-> Result<(), SourceError> {
    let make = |uri: &str, text: &str| {
        SourceSnapshot::new(
            SourceId("shared".into()),
            0,
            uri.into(),
            text.as_bytes().to_vec(),
            &mut budget(),
        )
    };
    let original = make("memory:source", "\u{65e5}\u{672c}\r\n\u{1f600}")?;
    let mut admission = SourceAdmission::default();
    let mut b = budget();
    admission.admit_existing(&original, &mut b)?;
    let bytes = b.usage().source_bytes;
    let clone = original.clone();
    drop(original);
    for _ in 0..10 {
        admission.admit_existing(&clone, &mut b)?;
    }
    assert_eq!(b.usage().source_bytes, bytes);
    let independent = make("memory:source", clone.text())?;
    admission.admit_existing(&independent, &mut b)?;
    assert_eq!(b.usage().source_bytes, bytes);
    assert_eq!(
        admission.admit_existing(&make("memory:changed", clone.text())?, &mut b),
        Err(SourceError::IdentityConflict)
    );
    assert_eq!(
        admission.admit_existing(&make("memory:source", "changed")?, &mut b),
        Err(SourceError::IdentityConflict)
    );
    let mut fresh = SourceAdmission::default();
    let mut fresh_budget = budget();
    fresh.admit_existing(&clone, &mut fresh_budget)?;
    assert_eq!(fresh_budget.usage().source_bytes, bytes);
    b.stop(StopReason::Cancelled);
    assert_eq!(
        admission.admit_existing(&clone, &mut b),
        Err(SourceError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn admission_usage_does_not_depend_on_snapshot_allocation_order() -> Result<(), SourceError> {
    let run = |reverse: bool| -> Result<nepl3_core::budget::Usage, SourceError> {
        let mut snapshots = Vec::new();
        for at in 0..64 {
            let id = if reverse { 63 - at } else { at };
            snapshots.push(SourceSnapshot::new(
                SourceId(format!("source-{id:02}")),
                0,
                format!("memory:source-{id:02}"),
                b"abc".to_vec(),
                &mut budget(),
            )?);
        }
        if reverse {
            snapshots.reverse();
        }
        let mut admission = SourceAdmission::default();
        let mut b = budget();
        for snapshot in &snapshots {
            admission.admit_existing(snapshot, &mut b)?;
        }
        for snapshot in snapshots.iter().rev().chain(&snapshots) {
            admission.admit_existing(snapshot, &mut b)?;
        }
        assert_eq!(b.usage().source_bytes, 192);
        Ok(b.usage())
    };
    assert_eq!(run(false)?, run(true)?);
    Ok(())
}
