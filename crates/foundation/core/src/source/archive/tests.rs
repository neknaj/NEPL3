use super::*;
use crate::{budget::Limits, source::SourceId};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: u64::MAX,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
    })
}
fn source(id: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(id.into()),
        0,
        alloc::format!("memory:{id}"),
        b"same content".to_vec(),
        &mut budget(),
    )
}
#[test]
fn membership_is_local_and_equal_content_keeps_distinct_identity() -> Result<(), ArchiveSourceError>
{
    let sources = [source("a")?, source("b")?];
    let mut b = budget();
    let mut table = ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut b)?;
    assert!(table.resolve(&[], &mut b)?.is_empty());
    let first = table.resolve(&[sources[0].reference()], &mut b)?;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].identity(), sources[0].identity());
    let second = table.resolve(&[sources[1].reference()], &mut b)?;
    assert_ne!(first[0].identity(), second[0].identity());
    table.finish(&mut b)
}
#[test]
fn rejects_unused_missing_duplicate_and_reordered_members() -> Result<(), ArchiveSourceError> {
    let sources = [source("a")?, source("b")?];
    let mut b = budget();
    let mut table = ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut b)?;
    let a = sources[0].reference();
    let z = source("z")?.reference();
    assert_eq!(
        table.resolve(&[a.clone(), z], &mut b).err(),
        Some(ArchiveSourceError::MissingMembership)
    );
    assert_eq!(
        table.resolve(&[a.clone(), a], &mut b).err(),
        Some(ArchiveSourceError::NonCanonical)
    );
    assert_eq!(
        table
            .resolve(&[sources[1].reference(), sources[0].reference()], &mut b)
            .err(),
        Some(ArchiveSourceError::NonCanonical)
    );
    assert_eq!(table.finish(&mut b), Err(ArchiveSourceError::UnusedSource));
    Ok(())
}
#[test]
fn rejects_noncanonical_table_and_sticky_cancellation() -> Result<(), ArchiveSourceError> {
    let sources = [source("b")?, source("a")?];
    assert_eq!(
        ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut budget()).err(),
        Some(ArchiveSourceError::NonCanonical)
    );
    let mut b = budget();
    b.stop(StopReason::Cancelled);
    assert_eq!(
        ArchiveSources::new(&[], &mut SourceAdmission::default(), &mut b).err(),
        Some(ArchiveSourceError::Source(SourceError::Stopped(
            StopReason::Cancelled
        )))
    );
    Ok(())
}

fn resolve_usage(ceiling: Limits) -> Result<crate::budget::Usage, ArchiveSourceError> {
    let sources = [source("a")?, source("b")?];
    let mut table = ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut budget())?;
    let mut b = Budget::new(ceiling);
    table.resolve(&[sources[0].reference(), sources[1].reference()], &mut b)?;
    table.finish(&mut b)?;
    Ok(b.usage())
}
#[test]
fn exact_and_one_short_resolution_budgets() -> Result<(), ArchiveSourceError> {
    let maximum = budget().limits();
    let measured = resolve_usage(maximum)?;
    let exact = Limits {
        work: measured.work,
        allocation_units: measured.allocation_units,
        ..maximum
    };
    assert_eq!(resolve_usage(exact)?, measured);
    for (limits, reason) in [
        (
            Limits {
                work: measured.work - 1,
                ..exact
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: measured.allocation_units - 1,
                ..exact
            },
            StopReason::AllocationLimit,
        ),
    ] {
        assert_eq!(
            resolve_usage(limits).err(),
            Some(ArchiveSourceError::Source(SourceError::Stopped(reason)))
        );
    }
    Ok(())
}
#[test]
fn wrong_digest_and_conflicting_table_are_rejected() -> Result<(), ArchiveSourceError> {
    let a = source("a")?;
    let sources = [a.clone()];
    let mut b = budget();
    let mut table = ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut b)?;
    let mut wrong = a.reference();
    wrong.digest.0[0] ^= 1;
    assert_eq!(
        table.resolve(&[wrong], &mut b).err(),
        Some(ArchiveSourceError::MissingMembership)
    );
    let conflicting = SourceSnapshot::new(
        SourceId("a".into()),
        0,
        "memory:other".into(),
        b"same content".to_vec(),
        &mut b,
    )?;
    assert_eq!(
        ArchiveSources::new(&[a, conflicting], &mut SourceAdmission::default(), &mut b).err(),
        Some(ArchiveSourceError::NonCanonical)
    );
    Ok(())
}
#[test]
fn failed_resolution_does_not_mark_earlier_members_used() -> Result<(), ArchiveSourceError> {
    let sources = [source("a")?, source("b")?];
    let mut table = ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut budget())?;
    assert_eq!(
        table
            .resolve(
                &[sources[0].reference(), source("z")?.reference()],
                &mut budget()
            )
            .err(),
        Some(ArchiveSourceError::MissingMembership)
    );
    table.resolve(&[sources[1].reference()], &mut budget())?;
    assert_eq!(
        table.finish(&mut budget()),
        Err(ArchiveSourceError::UnusedSource)
    );
    Ok(())
}

#[test]
fn marking_failure_is_transactional_and_old_budget_stays_stopped() -> Result<(), ArchiveSourceError>
{
    let sources = [source("a")?, source("b")?];
    let members = [sources[0].reference(), sources[1].reference()];
    let mut probe = ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut budget())?;
    let mut cost = budget();
    probe.resolve(&members, &mut cost)?;
    let mut table = ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut budget())?;
    let mut short = Budget::new(Limits {
        work: cost.usage().work - 1,
        ..budget().limits()
    });
    // The last charge marks two entries; one-short fails after both clones.
    assert_eq!(
        table.resolve(&members, &mut short).err(),
        Some(ArchiveSourceError::Source(SourceError::Stopped(
            StopReason::WorkLimit
        )))
    );
    assert_eq!(
        table.resolve(&[], &mut short).err(),
        Some(ArchiveSourceError::Source(SourceError::Stopped(
            StopReason::WorkLimit
        )))
    );
    table.resolve(&[sources[1].reference()], &mut budget())?;
    assert_eq!(
        table.finish(&mut budget()),
        Err(ArchiveSourceError::UnusedSource)
    );
    Ok(())
}
#[test]
fn admission_is_not_membership_and_repeated_resolution_is_allowed() -> Result<(), ArchiveSourceError>
{
    let sources = [source("a")?];
    let external = source("outside")?;
    let mut admission = SourceAdmission::default();
    let mut b = budget();
    admission.admit_existing(&external, &mut b)?;
    let mut table = ArchiveSources::new(&sources, &mut admission, &mut b)?;
    assert_eq!(
        table.resolve(&[external.reference()], &mut b).err(),
        Some(ArchiveSourceError::MissingMembership)
    );
    let admitted_bytes = b.usage().source_bytes;
    for _ in 0..2 {
        table.resolve(&[sources[0].reference()], &mut b)?;
    }
    assert_eq!(b.usage().source_bytes, admitted_bytes);
    table.finish(&mut b)
}
#[test]
fn cancellation_observed_by_empty_resolution_and_finish() -> Result<(), ArchiveSourceError> {
    let mut table = ArchiveSources::new(&[], &mut SourceAdmission::default(), &mut budget())?;
    let mut b = budget();
    b.stop(StopReason::Cancelled);
    let cancelled = ArchiveSourceError::Source(SourceError::Stopped(StopReason::Cancelled));
    assert_eq!(table.resolve(&[], &mut b).err(), Some(cancelled.clone()));
    assert_eq!(table.finish(&mut b), Err(cancelled));
    Ok(())
}
#[test]
fn duplicate_table_rejected_but_distinct_revisions_allowed() -> Result<(), ArchiveSourceError> {
    let a = source("a")?;
    assert_eq!(
        ArchiveSources::new(
            &[a.clone(), a.clone()],
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .err(),
        Some(ArchiveSourceError::NonCanonical)
    );
    let revision = SourceSnapshot::new(
        SourceId("a".into()),
        1,
        "memory:a".into(),
        b"new".to_vec(),
        &mut budget(),
    )?;
    let sources = [a, revision];
    let mut table = ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut budget())?;
    let resolved = table.resolve(
        &[sources[0].reference(), sources[1].reference()],
        &mut budget(),
    )?;
    assert_eq!(resolved[0].identity().revision, 0);
    assert_eq!(resolved[1].identity().revision, 1);
    table.finish(&mut budget())
}

#[test]
fn clones_charge_native_handles_or_owned_text_as_target_requires() -> Result<(), ArchiveSourceError>
{
    let sources = [source("a")?];
    let mut table = ArchiveSources::new(&sources, &mut SourceAdmission::default(), &mut budget())?;
    let mut b = budget();
    let output = table.resolve(&[sources[0].reference()], &mut b)?;
    let handles = (core::mem::size_of::<usize>() + core::mem::size_of::<SourceSnapshot>()) as u64;
    #[cfg(target_has_atomic = "ptr")]
    let copied = 0u64;
    #[cfg(not(target_has_atomic = "ptr"))]
    let copied = (sources[0].identity().source.0.len()
        + sources[0].uri().len()
        + sources[0].text().len()) as u64;
    assert_eq!(b.usage().allocation_units, handles + copied);
    assert_eq!(b.usage().source_bytes, 0);
    #[cfg(target_has_atomic = "ptr")]
    assert!(core::ptr::eq(
        output[0].text().as_ptr(),
        sources[0].text().as_ptr()
    ));
    assert_eq!(output[0].text(), sources[0].text());
    table.finish(&mut b)
}
