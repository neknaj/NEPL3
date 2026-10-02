use super::*;
use nepl3_core::{budget::Resource, source::Digest};

pub(super) fn check(
    old: &InsertionInput<'_, '_, '_>,
    new: &InsertionInput<'_, '_, '_>,
    request: &ExpectedReadRequest,
    edit: &TextEdit,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), InsertionError> {
    b.charge(Resource::Work, 1)?;
    for value in [old, new] {
        if !core::ptr::eq(value.parsed.execution().tree(), value.prepared.tree.tree())
            || !core::ptr::eq(value.parsed.seed().profile(), value.prepared.profile)
        {
            return Err(InsertionError::ProofMismatch);
        }
    }
    if request.key != old.prepared.key() {
        return Err(InsertionError::Access(BindingAccessError::StaleAnalysis));
    }
    let a = old.parsed.seed();
    let c = new.parsed.seed();
    if !core::ptr::eq(a.profile(), c.profile())
        || !core::ptr::eq(a.environments(), c.environments())
    {
        return Err(InsertionError::ContextMismatch);
    }
    let a = a.request();
    let c = c.request();
    if !crate::selection::entry_equal(a.entry, c.entry, b)?
        || a.start != c.start
        || a.final_input != c.final_input
        || a.states.len() != c.states.len()
    {
        return Err(InsertionError::ContextMismatch);
    }
    for (a, c) in a.states.iter().zip(c.states) {
        b.charge(
            Resource::Work,
            (a.alias.len() as u64)
                .saturating_add(c.alias.len() as u64)
                .saturating_add(1),
        )?;
        if a.alias != c.alias || !a.state.equal_with_budget(&c.state, b)? {
            return Err(InsertionError::ContextMismatch);
        }
    }
    if edit
        .span
        .snapshot_ref()
        .compare_with_budget(a.snapshot.identity(), b)?
        != core::cmp::Ordering::Equal
    {
        return Err(InsertionError::EditMismatch);
    }
    b.charge(
        Resource::Work,
        (request.source.source_id.0.len() as u64)
            .saturating_add(a.snapshot.identity().source.0.len() as u64)
            .saturating_add(41),
    )?;
    if request.source.source_id != a.snapshot.identity().source
        || request.source.revision != a.snapshot.identity().revision
        || request.source.digest != a.snapshot.identity().digest
        || request.offset != edit.span.start()
    {
        return Err(InsertionError::EditMismatch);
    }
    a.snapshot.check_range(edit.span.start(), edit.span.end())?;
    if edit.span.start() != edit.span.end()
        || edit.replacement.is_empty()
        || edit.expected_digest != Digest::of(b"")
        || edit.span.start() < a.start
        || edit.span.end() > a.limit
    {
        return Err(InsertionError::EditMismatch);
    }
    let bytes = edit.replacement.len() as u64;
    if a.limit.checked_add(bytes) != Some(c.limit)
        || a.snapshot.identity().revision.checked_add(1) != Some(c.snapshot.identity().revision)
    {
        return Err(InsertionError::EditMismatch);
    }
    b.charge(
        Resource::Work,
        (a.snapshot.identity().source.0.len() as u64)
            .saturating_add(c.snapshot.identity().source.0.len() as u64)
            .saturating_add(a.snapshot.uri().len() as u64)
            .saturating_add(c.snapshot.uri().len() as u64)
            .saturating_add(1),
    )?;
    if a.snapshot.identity().source != c.snapshot.identity().source
        || a.snapshot.uri() != c.snapshot.uri()
    {
        return Err(InsertionError::EditMismatch);
    }
    let old = a.snapshot.text().as_bytes();
    let new = c.snapshot.text().as_bytes();
    b.charge(
        Resource::Work,
        (old.len() as u64)
            .saturating_add(new.len() as u64)
            .saturating_add(bytes)
            .saturating_add(1),
    )?;
    let at = usize::try_from(edit.span.start()).map_err(|_| InsertionError::EditMismatch)?;
    let end = at
        .checked_add(edit.replacement.len())
        .ok_or(InsertionError::EditMismatch)?;
    if old.len().checked_add(edit.replacement.len()) != Some(new.len())
        || new.get(..at) != old.get(..at)
        || new.get(at..end) != Some(edit.replacement.as_bytes())
        || new.get(end..) != old.get(at..)
    {
        return Err(InsertionError::EditMismatch);
    }
    admission.admit_existing(a.snapshot, b)?;
    admission.admit_existing(c.snapshot, b)?;
    Ok(())
}
