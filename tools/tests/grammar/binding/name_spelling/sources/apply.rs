//! Exercise the store primitive with an actual strictly checked insertion.
use super::super::*;
use nepl3_core::source::{GuardedEditError, SourceAdmission, SourceStore};
use nepl3_engine::analysis::insertion::quality::StrictNameInsertion;

pub(super) fn verify(
    strict: &StrictNameInsertion<'_, '_, '_, '_>,
    b: &Budget,
) -> Result<(), String> {
    let checked = strict.declaration().reference().insertion().checked();
    let original = checked.original().seed().request().snapshot;
    let candidate = checked.candidate().seed().request().snapshot;
    let edits = core::slice::from_ref(checked.edit());
    let mut store = SourceStore::default();
    store.insert(original.clone()).map_err(err)?;
    let grants = [original.identity().clone()];
    // This is an explicitly new host transaction, with its own operation ledger.
    // Only its edited primary is selected as a current dependency here. This
    // fixture does not assert complete external dependency/permission discovery.
    let mut operation = Budget::new(b.limits());
    let mut admission = SourceAdmission::default();
    assert_eq!(
        store.apply_guarded(edits, &[original], &[], &mut operation, &mut admission),
        Err(GuardedEditError::NotWritable { edit: 0 })
    );
    assert_eq!(store.snapshots(), core::slice::from_ref(original));
    let result = store
        .apply_guarded(edits, &[original], &grants, &mut operation, &mut admission)
        .map_err(err)?;
    assert_eq!(
        result.as_slice(),
        core::slice::from_ref(candidate.identity())
    );
    assert_eq!(store.get_ref(&result[0]), Some(candidate));
    assert_eq!(checked.original().seed().request().snapshot, original);
    // Reusing the old source/declaration proof after application cannot replay
    // the same edit over the new retained revision, even with the old grant.
    let before = store.snapshots().to_vec();
    assert_eq!(
        store.apply_guarded(edits, &[original], &grants, &mut operation, &mut admission),
        Err(GuardedEditError::ChangedDependency { dependency: 0 })
    );
    assert_eq!(store.snapshots(), before);
    Ok(())
}
