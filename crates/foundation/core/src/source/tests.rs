//! Native admission-ledger interruption checks over fixed validated snapshots.
use super::*;
use crate::budget::{Limits, Usage};

const ABC: Digest = Digest([
    186, 120, 22, 191, 143, 1, 207, 234, 65, 65, 64, 222, 93, 174, 34, 35, 176, 3, 97, 163, 150,
    23, 122, 156, 180, 16, 255, 97, 242, 0, 21, 173,
]);
const ABD: Digest = Digest([
    165, 45, 21, 159, 38, 43, 44, 109, 219, 114, 74, 97, 132, 11, 239, 195, 110, 179, 12, 136, 135,
    122, 64, 48, 182, 92, 190, 134, 41, 132, 73, 201,
]);

fn limits() -> Limits {
    Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: u64::MAX,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
    }
}

// Fixed valid fixtures keep construction outside the measured admission.
// A normal test independently checks these digests and locator validity.
fn fixture(id: &str, uri: &str, changed: bool) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::from_parts(
        SnapshotId {
            source: SourceId(id.into()),
            revision: 0,
            digest: if changed { ABD } else { ABC },
        },
        uri.into(),
        if changed { "abd" } else { "abc" }.into(),
        &mut Budget::new(limits()),
    )
}

fn reason(case: u8) -> StopReason {
    match case % 9 {
        0 => StopReason::Cancelled,
        1 => StopReason::SourceLimit,
        2 => StopReason::WorkLimit,
        3 => StopReason::DepthLimit,
        4 => StopReason::NodeLimit,
        5 => StopReason::AllocationLimit,
        6 => StopReason::OutputLimit,
        7 => StopReason::DiagnosticLimit,
        _ => StopReason::EventLimit,
    }
}

fn same_usage(actual: Usage, expected: Usage) {
    assert_eq!(actual.source_bytes, expected.source_bytes);
    assert_eq!(actual.work, expected.work);
    assert_eq!(actual.depth, expected.depth);
    assert_eq!(actual.nodes, expected.nodes);
    assert_eq!(actual.allocation_units, expected.allocation_units);
    assert_eq!(actual.output_bytes, expected.output_bytes);
    assert_eq!(actual.diagnostics, expected.diagnostics);
    assert_eq!(actual.events, expected.events);
}

fn seed_entries(ledger: &SourceAdmission, first: &SourceSnapshot, last: &SourceSnapshot) {
    assert_eq!(&ledger.admitted[0].0, first.identity());
    assert_eq!(ledger.admitted[0].1, first.uri());
    assert_eq!(&ledger.admitted[1].0, last.identity());
    assert_eq!(ledger.admitted[1].1, last.uri());
}

/// Two previously admitted keys (a,c), one attempt: fresh b, independent a,
/// shared a, or a with different bytes/locator. All state is operation-local.
fn exercise(case: u8, ceiling: Limits, prior: Option<StopReason>) -> Result<Usage, SourceError> {
    let first = fixture("a", "m:a", false)?;
    let last = fixture("c", "m:c", false)?;
    let candidate = match case {
        0 => fixture("b", "m:b", false)?,
        1 => fixture("a", "m:a", false)?,
        2 => first.clone(),
        3 => fixture("a", "m:a", true)?,
        _ => fixture("a", "m:x", false)?,
    };
    let mut ledger = SourceAdmission::default();
    let mut b = Budget::new(limits());
    ledger.admit_existing(&first, &mut b)?;
    ledger.admit_existing(&last, &mut b)?;
    let before = b.usage();
    #[cfg(target_has_atomic = "ptr")]
    let shared = [
        Arc::as_ptr(&ledger.shared[0]),
        Arc::as_ptr(&ledger.shared[1]),
    ];
    if let Some(stop) = prior {
        b.stop(stop);
    }
    let result = b.with_ceiling(ceiling, |b| ledger.admit_existing(&candidate, b));
    let after = b.usage();
    if ceiling.source_bytes == u64::MAX
        && ceiling.work == u64::MAX
        && ceiling.allocation_units == u64::MAX
        && prior.is_none()
    {
        assert_eq!(
            result,
            if case < 3 {
                Ok(())
            } else {
                Err(SourceError::IdentityConflict)
            }
        );
    }
    assert!(after.work >= before.work);
    assert!(after.allocation_units >= before.allocation_units);
    assert_eq!(after.nodes, before.nodes);
    assert_eq!(after.depth, before.depth);
    assert_eq!(after.output_bytes, before.output_bytes);
    assert_eq!(after.diagnostics, before.diagnostics);
    assert_eq!(after.events, before.events);
    assert_eq!(b.current_depth(), 0);
    if case == 0 {
        // A later allocation/work stop retains an earlier SourceBytes charge.
        assert!(
            after.source_bytes == before.source_bytes
                || after.source_bytes == before.source_bytes + 3
        );
    } else {
        assert_eq!(after.source_bytes, before.source_bytes);
    }
    if let Some(stop) = prior {
        assert_eq!(result, Err(SourceError::Stopped(stop)));
        same_usage(after, before);
    }
    match result {
        Ok(()) => {
            assert!(case < 3);
            assert!(b.poll().is_ok());
            if case == 0 {
                assert_eq!(ledger.admitted.len(), 3);
                seed_entries(&ledger, &first, &last);
                assert_eq!(&ledger.admitted[2].0, candidate.identity());
                assert_eq!(ledger.admitted[2].1, candidate.uri());
                assert_eq!(ledger.index.as_slice(), &[0, 2, 1]);
                assert_eq!(after.source_bytes, before.source_bytes + 3);
            } else {
                assert_eq!(ledger.admitted.len(), 2);
                seed_entries(&ledger, &first, &last);
                assert_eq!(ledger.index.as_slice(), &[0, 1]);
            }
            #[cfg(target_has_atomic = "ptr")]
            {
                assert_eq!(ledger.shared.len(), if case == 2 { 2 } else { 3 });
                for pair in ledger.shared.windows(2) {
                    assert!(Arc::as_ptr(&pair[0]) < Arc::as_ptr(&pair[1]));
                }
                for old in &shared {
                    assert!(ledger.shared.iter().any(|new| *old == Arc::as_ptr(new)));
                }
                assert!(
                    ledger
                        .shared
                        .iter()
                        .any(|new| Arc::ptr_eq(&candidate.storage, new))
                );
            }
        }
        Err(error) => {
            assert_eq!(ledger.admitted.len(), 2);
            seed_entries(&ledger, &first, &last);
            assert_eq!(ledger.index.as_slice(), &[0, 1]);
            #[cfg(target_has_atomic = "ptr")]
            {
                assert_eq!(ledger.shared.len(), shared.len());
                for (old, new) in shared.iter().zip(&ledger.shared) {
                    assert!(*old == Arc::as_ptr(new));
                }
            }
            match error {
                SourceError::Stopped(stop) => {
                    assert_eq!(b.poll(), Err(stop));
                    assert_eq!(
                        ledger.admit_existing(&candidate, &mut b),
                        Err(SourceError::Stopped(stop))
                    );
                    same_usage(b.usage(), after);
                    assert_eq!(ledger.admitted.len(), 2);
                    seed_entries(&ledger, &first, &last);
                    assert_eq!(ledger.index.as_slice(), &[0, 1]);
                    #[cfg(target_has_atomic = "ptr")]
                    {
                        assert_eq!(ledger.shared.len(), shared.len());
                        for (old, new) in shared.iter().zip(&ledger.shared) {
                            assert!(*old == Arc::as_ptr(new));
                        }
                    }
                }
                SourceError::IdentityConflict => {
                    assert!(case >= 3);
                    assert!(prior.is_none());
                    assert!(b.poll().is_ok());
                }
                unexpected => {
                    assert_eq!(unexpected, SourceError::IdentityConflict, "unrelated error")
                }
            }
        }
    }
    Ok(after)
}

#[test]
fn fixed_admission_fixtures_have_valid_digests_and_locators() {
    assert_eq!(Digest::of(b"abc"), ABC);
    assert_eq!(Digest::of(b"abd"), ABD);
    for uri in ["m:a", "m:b", "m:c", "m:x"] {
        assert!(valid_locator(uri));
    }
}

#[test]
fn admission_preserves_ledger_at_every_relevant_ceiling() -> Result<(), SourceError> {
    for case in 0..5 {
        let completed = exercise(case, limits(), None)?;
        for resource in [
            Resource::SourceBytes,
            Resource::Work,
            Resource::AllocationUnits,
        ] {
            let maximum = match resource {
                Resource::SourceBytes => completed.source_bytes,
                Resource::Work => completed.work,
                _ => completed.allocation_units,
            };
            for cap in 0..=maximum {
                let mut ceiling = limits();
                match resource {
                    Resource::SourceBytes => ceiling.source_bytes = cap,
                    Resource::Work => ceiling.work = cap,
                    _ => ceiling.allocation_units = cap,
                }
                exercise(case, ceiling, None)?;
            }
        }
        for stop in 0..9 {
            exercise(case, limits(), Some(reason(stop)))?;
        }
    }
    Ok(())
}

#[test]
fn late_allocation_stop_retains_source_charge_without_publishing() -> Result<(), SourceError> {
    let completed = exercise(0, limits(), None)?;
    let first = fixture("a", "m:a", false)?;
    let last = fixture("c", "m:c", false)?;
    let candidate = fixture("b", "m:b", false)?;
    let mut ledger = SourceAdmission::default();
    let mut b = Budget::new(limits());
    ledger.admit_existing(&first, &mut b)?;
    ledger.admit_existing(&last, &mut b)?;
    let admitted = ledger.admitted.clone();
    let index = ledger.index.clone();
    #[cfg(target_has_atomic = "ptr")]
    let shared = ledger.shared.clone();
    let before = b.usage();
    let mut ceiling = limits();
    ceiling.allocation_units = completed.allocation_units - 1;
    assert!(ceiling.allocation_units >= before.allocation_units);
    assert_eq!(
        b.with_ceiling(ceiling, |b| ledger.admit_existing(&candidate, b)),
        Err(SourceError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(b.usage().source_bytes, before.source_bytes + 3);
    assert_eq!(ledger.admitted, admitted);
    assert_eq!(ledger.index, index);
    #[cfg(target_has_atomic = "ptr")]
    {
        assert_eq!(ledger.shared.len(), shared.len());
        for (old, new) in shared.iter().zip(&ledger.shared) {
            assert!(Arc::ptr_eq(old, new));
        }
    }
    Ok(())
}

// First-admission boundaries complement the two-entry ledger cases above.
fn first_admission(ceiling: Limits) -> Result<(), SourceError> {
    let candidate = fixture("a", "m:a", false)?;
    let mut ledger = SourceAdmission::default();
    let mut b = Budget::new(ceiling);
    let result = ledger.admit_existing(&candidate, &mut b);
    if ceiling.source_bytes == u64::MAX
        && ceiling.work == u64::MAX
        && ceiling.allocation_units == u64::MAX
    {
        assert_eq!(result, Ok(()));
    }
    let after = b.usage();
    assert!(after.source_bytes == 0 || after.source_bytes == 3);
    assert_eq!(after.depth, 0);
    assert_eq!(after.nodes, 0);
    assert_eq!(after.output_bytes, 0);
    assert_eq!(after.diagnostics, 0);
    assert_eq!(after.events, 0);
    assert_eq!(b.current_depth(), 0);
    match result {
        Ok(()) => {
            assert_eq!(ledger.admitted.len(), 1);
            assert_eq!(&ledger.admitted[0].0, candidate.identity());
            assert_eq!(ledger.admitted[0].1, candidate.uri());
            assert_eq!(ledger.index.as_slice(), &[0]);
            assert_eq!(after.source_bytes, 3);
            assert!(b.poll().is_ok());
            #[cfg(target_has_atomic = "ptr")]
            {
                assert_eq!(ledger.shared.len(), 1);
                assert!(Arc::ptr_eq(&ledger.shared[0], &candidate.storage));
            }
        }
        Err(SourceError::Stopped(stop)) => {
            assert!(ledger.admitted.is_empty());
            assert!(ledger.index.is_empty());
            #[cfg(target_has_atomic = "ptr")]
            assert!(ledger.shared.is_empty());
            assert_eq!(b.poll(), Err(stop));
            assert_eq!(
                ledger.admit_existing(&candidate, &mut b),
                Err(SourceError::Stopped(stop))
            );
            same_usage(b.usage(), after);
            assert!(ledger.admitted.is_empty());
            assert!(ledger.index.is_empty());
            #[cfg(target_has_atomic = "ptr")]
            assert!(ledger.shared.is_empty());
        }
        other => assert_eq!(other, Ok(()), "unexpected first admission error"),
    }
    Ok(())
}

#[test]
fn first_admission_matches_native_boundaries() -> Result<(), SourceError> {
    first_admission(limits())?;
    let candidate = fixture("a", "m:a", false)?;
    let mut baseline = Budget::new(limits());
    SourceAdmission::default().admit_existing(&candidate, &mut baseline)?;
    let completed = baseline.usage();
    for resource in [
        Resource::SourceBytes,
        Resource::Work,
        Resource::AllocationUnits,
    ] {
        let maximum = match resource {
            Resource::SourceBytes => completed.source_bytes,
            Resource::Work => completed.work,
            _ => completed.allocation_units,
        };
        for cap in 0..=maximum {
            let mut ceiling = limits();
            match resource {
                Resource::SourceBytes => ceiling.source_bytes = cap,
                Resource::Work => ceiling.work = cap,
                _ => ceiling.allocation_units = cap,
            }
            first_admission(ceiling)?;
        }
    }
    Ok(())
}
