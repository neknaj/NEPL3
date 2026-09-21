mod session;
use super::*;
use alloc::vec;
use nepl3_core::{
    budget::{Resource, StopReason},
    source::{Digest, SourceAdmission, SourceError, SourceId},
};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10000,
        work: 100000,
        depth: 100,
        nodes: 10000,
        allocation_units: 100000,
        output_bytes: 10000,
        diagnostics: 10,
        events: 10,
    })
}

#[test]
fn admission_scope_is_owned_and_foreign_ledgers_recheck() -> Result<(), crate::runtime::ReaderError>
{
    fn send_sync<T: Send + Sync>() {}
    send_sync::<SourceAdmission>();
    let mut b = budget();
    let source = SourceSnapshot::new(
        SourceId("s".into()),
        0,
        "memory:s".into(),
        vec![b'x'],
        &mut budget(),
    )?;
    let mut ledger = SourceAdmission::default();
    ledger.admit_existing(&source, &mut b)?;
    let mut accepted = AcceptedTokenizationReport::empty(
        TokenizationScope {
            operation_id: "test".into(),
            profile_digest: Digest([0; 32]),
            snapshot: source.reference(),
        },
        &mut b,
    )?;
    accepted.sources.push(source.clone());
    accepted.admission_scope = ledger.scope_with_budget(&mut b)?;
    let before = b.usage();
    accepted.admit_sources(&mut ledger, &mut b)?;
    assert_eq!(b.usage().source_bytes, before.source_bytes);
    if cfg!(target_has_atomic = "ptr") {
        assert_eq!(
            b.usage().work,
            before.work,
            "owned admission needs no source traversal"
        );
    }
    let checkpoint = accepted.checkpoint(&mut b)?;
    checkpoint.admit_sources(&mut ledger, &mut b)?;
    let mut foreign = SourceAdmission::default();
    let conflicting = SourceSnapshot::new(
        SourceId("s".into()),
        0,
        "memory:s".into(),
        vec![b'y'],
        &mut budget(),
    )?;
    foreign.admit_existing(&conflicting, &mut b)?;
    assert_eq!(
        checkpoint.admit_sources(&mut foreign, &mut b),
        Err(crate::runtime::ReaderError::Source(
            SourceError::IdentityConflict
        ))
    );
    let mut new_ledger = SourceAdmission::default();
    let before = b.usage().source_bytes;
    accepted.admit_sources(&mut new_ledger, &mut b)?;
    assert_eq!(b.usage().source_bytes, before + 1);
    accepted.retain_admission_scope(&new_ledger);
    assert!(accepted.admission_scope.is_none());
    b.cancel();
    assert_eq!(
        checkpoint.admit_sources(&mut ledger, &mut b),
        Err(crate::runtime::ReaderError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn admission_marker_allocation_failure_does_not_publish_a_scope() -> Result<(), SourceError> {
    let mut ledger = SourceAdmission::default();
    let mut limited = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    let result = ledger.scope_with_budget(&mut limited);
    if cfg!(target_has_atomic = "ptr") {
        assert!(matches!(
            result,
            Err(SourceError::Stopped(StopReason::AllocationLimit))
        ));
    } else {
        assert!(result?.is_none());
    }
    let mut b = budget();
    let scope = ledger.scope_with_budget(&mut b)?;
    if let Some(scope) = scope {
        assert!(ledger.matches_scope(&scope));
        let original = core::mem::take(&mut ledger);
        assert!(!ledger.matches_scope(&scope));
        assert!(original.matches_scope(&scope));
        b.charge(Resource::Work, 1)?;
    }
    Ok(())
}
