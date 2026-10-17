use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    source::{SourceAdmission, SourceError, SourceId, SourceSetError, SourceSnapshot},
};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 100,
        nodes: 1_000_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn snapshot(id: &str, uri: &str, text: &[u8]) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(id.into()),
        1,
        uri.into(),
        text.to_vec(),
        &mut budget(),
    )
}
#[test]
fn public_checked_set_is_exact_local_declarations_not_the_admission_union()
-> Result<(), SourceSetError> {
    let a = snapshot("a", "memory:a", b"a")?;
    let z = snapshot("z", "memory:z", b"z")?;
    let mut ledger = SourceAdmission::default();
    let mut b = budget();
    ledger.admit_existing(&z, &mut b)?;
    assert_eq!(
        ledger
            .checked_source_set(core::slice::from_ref(&a), &mut b)?
            .snapshots(),
        core::slice::from_ref(&a)
    );
    let independent = snapshot("a", "memory:a", b"a")?;
    assert_eq!(
        ledger
            .checked_source_set(&[independent], &mut b)?
            .snapshots(),
        core::slice::from_ref(&a)
    );
    assert_eq!(
        ledger
            .checked_source_set(&[z.clone(), a.clone()], &mut b)?
            .snapshots(),
        &[z.clone(), a.clone()]
    );
    assert!(
        ledger
            .checked_source_set(&[], &mut b)?
            .snapshots()
            .is_empty()
    );
    assert_eq!(
        ledger
            .checked_source_set(&[a.clone(), a.clone()], &mut b)
            .err(),
        Some(SourceSetError::Duplicate)
    );
    let conflict = snapshot("a", "other:a", b"a")?;
    assert_eq!(
        ledger.checked_source_set(&[conflict], &mut b).err(),
        Some(SourceSetError::Source(SourceError::IdentityConflict))
    );
    assert_eq!(
        ledger
            .checked_source_set(&[a.clone(), z.clone()], &mut b)?
            .snapshots(),
        &[a, z]
    );
    Ok(())
}
#[test]
fn public_checked_set_cold_warm_limits_and_sticky_failure() -> Result<(), SourceSetError> {
    let sources = [
        snapshot("a", "memory:a", b"a")?,
        snapshot("z", "memory:z", b"z")?,
    ];
    for warm in [false, true] {
        let run = |ledger: &mut SourceAdmission, b: &mut Budget| -> Result<(), SourceSetError> {
            if warm {
                ledger.checked_source_set(&sources, b)?;
            }
            ledger.checked_source_set(&sources, b)?;
            Ok(())
        };
        let mut measured = budget();
        run(&mut SourceAdmission::default(), &mut measured)?;
        let used = measured.usage();
        for kind in 0..4 {
            let mut limits = budget().limits();
            limits.work = used.work;
            limits.allocation_units = used.allocation_units;
            limits.source_bytes = used.source_bytes;
            let reason = match kind {
                0 => None,
                1 => {
                    limits.work -= 1;
                    Some(StopReason::WorkLimit)
                }
                2 => {
                    limits.allocation_units -= 1;
                    Some(StopReason::AllocationLimit)
                }
                _ => {
                    limits.source_bytes -= 1;
                    Some(StopReason::SourceLimit)
                }
            };
            let mut b = Budget::new(limits);
            let mut ledger = SourceAdmission::default();
            let result = run(&mut ledger, &mut b);
            if let Some(reason) = reason {
                assert_eq!(
                    result,
                    Err(SourceSetError::Source(SourceError::Stopped(reason)))
                );
                assert_eq!(b.poll(), Err(reason));
                let saved = b.usage();
                assert_eq!(
                    ledger.checked_source_set(&sources, &mut b).err(),
                    Some(SourceSetError::Source(SourceError::Stopped(reason)))
                );
                assert_eq!(b.usage(), saved);
            } else {
                result?;
                assert_eq!(b.usage(), used);
            }
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert_eq!(
            SourceAdmission::default()
                .checked_source_set(&[], &mut cancelled)
                .err(),
            Some(SourceSetError::Source(SourceError::Stopped(
                StopReason::Cancelled
            )))
        );
        assert_eq!(used.source_bytes, 2);
    }
    Ok(())
}
