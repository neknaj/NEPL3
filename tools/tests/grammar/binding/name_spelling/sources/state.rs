use super::*;
use nepl3_core::source::SourceStore;
use nepl3_engine::analysis::insertion::sources::{
    SourceInventory,
    state::{self, ComparisonError, StoredState},
};

pub(super) fn verify(inventory: &SourceInventory<'_>, b: &mut Budget) -> Result<(), String> {
    let before = inventory.report().clone();
    let empty = SourceStore::default();
    let absent = state::compare(inventory, &empty, &mut Budget::new(b.limits())).map_err(err)?;
    assert!(
        absent
            .observations()
            .iter()
            .all(|v| matches!(v.state, StoredState::Missing { latest: None }))
    );
    let mut store = SourceStore::default();
    for row in inventory.entries() {
        // Independently reconstruct equal storage to exercise metadata and byte
        // equality metering rather than only the Arc-sharing fast path.
        store
            .insert(changed_revision(
                row.source,
                row.source.identity().revision,
                b,
            )?)
            .map_err(err)?;
    }
    let first = inventory.entries()[0].source;
    let mut lookup = Budget::new(b.limits());
    let independently_stored = store
        .get_revision_with_budget(
            &first.identity().source,
            first.identity().revision,
            &mut lookup,
        )
        .map_err(err)?
        .ok_or("independent stored fixture")?;
    let prefix = 1
        + lookup.usage().work
        + 1
        + first.identity().source.0.len() as u64
        + independently_stored.identity().source.0.len() as u64
        + first.uri().len() as u64
        + independently_stored.uri().len() as u64
        + 34;
    let text_cost = (first.text().len() + independently_stored.text().len()) as u64;
    assert!(text_cost > 0);
    let mut text_short = Budget::new(b.limits());
    let prepaid = b.limits().work - (prefix + text_cost - 1);
    text_short.charge(Resource::Work, prepaid).map_err(err)?;
    let Err(error) = state::compare(inventory, &store, &mut text_short) else {
        return Err("unmetered text comparison".into());
    };
    assert_eq!(error.stop_reason(), Some(StopReason::WorkLimit));
    assert_eq!(text_short.usage().work, prepaid + prefix);
    let stored_before = store.snapshots().to_vec();
    let mut measure = Budget::new(b.limits());
    let compared = state::compare(inventory, &store, &mut measure).map_err(err)?;
    assert!(core::ptr::eq(compared.inventory(), inventory));
    assert!(core::ptr::eq(compared.store(), &store));
    assert_eq!(compared.observations().len(), inventory.entries().len());
    for (row, evidence) in compared.observations().iter().zip(inventory.entries()) {
        assert!(core::ptr::eq(row.evidence, evidence));
        // Independent linear maximum from the explicit fixture contents.
        let latest = store
            .snapshots()
            .iter()
            .filter(|s| s.identity().source == evidence.source.identity().source)
            .max_by_key(|s| s.identity().revision)
            .ok_or("missing fixture")?;
        let stored = match row.state {
            StoredState::Latest { stored } => {
                assert_eq!(latest.identity().revision, stored.identity().revision);
                stored
            }
            StoredState::Historical {
                stored,
                latest: actual,
            } => {
                assert!(latest.identity().revision > stored.identity().revision);
                assert!(core::ptr::eq(latest, actual));
                stored
            }
            _ => return Err("all retained source revisions must be present".into()),
        };
        assert_eq!(stored, evidence.source);
    }
    let usage = measure.usage();
    assert_eq!(compared.report().usage, usage);
    assert!(compared.report().diagnostics.is_empty());
    assert!(compared.report().events.is_empty());
    for (resource, cap, cost, reason) in [
        (
            Resource::Work,
            b.limits().work,
            usage.work,
            StopReason::WorkLimit,
        ),
        (
            Resource::Nodes,
            b.limits().nodes,
            usage.nodes,
            StopReason::NodeLimit,
        ),
        (
            Resource::AllocationUnits,
            b.limits().allocation_units,
            usage.allocation_units,
            StopReason::AllocationLimit,
        ),
    ] {
        let mut exact = Budget::new(b.limits());
        exact.charge(resource, cap - cost).map_err(err)?;
        assert_eq!(
            state::compare(inventory, &store, &mut exact)
                .map_err(err)?
                .observations()
                .len(),
            inventory.entries().len()
        );
        let mut short = Budget::new(b.limits());
        short.charge(resource, cap - cost + 1).map_err(err)?;
        let Err(error) = state::compare(inventory, &store, &mut short) else {
            return Err("short comparison budget accepted".into());
        };
        assert_eq!(error.stop_reason(), Some(reason));
        assert_eq!(short.poll(), Err(reason));
    }
    let mut cancelled = Budget::new(b.limits());
    cancelled.cancel();
    assert!(matches!(
        state::compare(inventory, &store, &mut cancelled),
        Err(ComparisonError::Stopped(StopReason::Cancelled))
    ));
    let mut limits = b.limits();
    limits.work -= 1;
    let mut mismatch = Budget::new(limits);
    mismatch.cancel();
    assert!(matches!(
        state::compare(inventory, &store, &mut mismatch),
        Err(ComparisonError::LimitsMismatch)
    ));
    assert_eq!(mismatch.usage(), Budget::new(limits).usage());
    assert_eq!(store.snapshots(), stored_before);

    let target = inventory.entries()[0].source;
    // Isolated source/revision fixtures exercise metadata conflict independently
    // of hash identity, and higher/lower revisions independently of collection order.
    for change_uri in [false, true] {
        let mut conflict = SourceStore::default();
        let replacement = SourceSnapshot::new(
            target.identity().source.clone(),
            target.identity().revision,
            if change_uri {
                "memory:changed-locator".into()
            } else {
                target.uri().into()
            },
            if change_uri {
                target.text().as_bytes().to_vec()
            } else {
                b"different bytes".to_vec()
            },
            &mut Budget::new(b.limits()),
        )
        .map_err(err)?;
        if change_uri {
            assert_eq!(replacement.identity(), target.identity());
        }
        conflict.insert(replacement).map_err(err)?;
        conflict
            .insert(changed_revision(
                target,
                target.identity().revision + 10,
                b,
            )?)
            .map_err(err)?;
        let compared =
            state::compare(inventory, &conflict, &mut Budget::new(b.limits())).map_err(err)?;
        assert!(matches!(
            compared.observations()[0].state,
            StoredState::Conflict { .. }
        ));
    }
    let mut revision_only = SourceStore::default();
    revision_only
        .insert(changed_revision(
            target,
            target.identity().revision + 10,
            b,
        )?)
        .map_err(err)?;
    let compared =
        state::compare(inventory, &revision_only, &mut Budget::new(b.limits())).map_err(err)?;
    assert!(
        matches!(compared.observations()[0].state, StoredState::Missing { latest: Some(latest) } if latest.identity().revision > target.identity().revision)
    );
    // A real candidate's newer primary revision is absent from the original seed.
    let candidate_newer = inventory
        .entries()
        .iter()
        .find(|row| {
            row.source.identity().source == target.identity().source
                && row.source.identity().revision > target.identity().revision
        })
        .ok_or("candidate newer fixture")?;
    let mut original_only = SourceStore::default();
    original_only.insert(target.clone()).map_err(err)?;
    let compared =
        state::compare(inventory, &original_only, &mut Budget::new(b.limits())).map_err(err)?;
    let row = compared
        .observations()
        .iter()
        .find(|row| core::ptr::eq(row.evidence, candidate_newer))
        .ok_or("candidate row")?;
    assert!(
        matches!(row.state, StoredState::Missing { latest: Some(latest) } if latest.identity().revision < candidate_newer.source.identity().revision)
    );
    // Drop the borrowed result before changing the observed store. A new query
    // must see the new revision; the prior report is not a reusable freshness token.
    drop(compared);
    original_only
        .insert(changed_revision(
            target,
            target.identity().revision + 10,
            b,
        )?)
        .map_err(err)?;
    let compared =
        state::compare(inventory, &original_only, &mut Budget::new(b.limits())).map_err(err)?;
    assert!(matches!(
        compared.observations()[0].state,
        StoredState::Historical { .. }
    ));
    let cumulative = state::compare(inventory, &store, b).map_err(err)?;
    assert_eq!(cumulative.report().usage, b.usage());
    assert_eq!(inventory.report(), &before);
    Ok(())
}
fn changed_revision(
    source: &SourceSnapshot,
    revision: u64,
    b: &Budget,
) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        source.identity().source.clone(),
        revision,
        source.uri().into(),
        source.text().as_bytes().to_vec(),
        &mut Budget::new(b.limits()),
    )
    .map_err(err)
}
