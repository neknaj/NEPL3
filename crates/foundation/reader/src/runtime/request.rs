//! Stable request source union; the temporary index grants no source authority.
mod index;
use super::copy::{copy, slot};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::{SourceRef, SourceSnapshot},
};

/// SourceRef owns its name even when SourceSnapshot storage is shared.
/// The enclosing request accounts for inline storage; this copy owns the bytes.
pub(super) fn copy_reference(
    snapshot: &SourceSnapshot,
    budget: &mut Budget,
) -> Result<SourceRef, StopReason> {
    let bytes = snapshot.identity().source.0.len() as u64;
    budget.charge(Resource::AllocationUnits, bytes)?;
    budget.charge(Resource::Work, bytes)?;
    Ok(snapshot.reference())
}

pub(super) fn copy_sources<'a>(
    sources: impl IntoIterator<Item = &'a SourceSnapshot>,
    budget: &mut Budget,
) -> Result<Vec<SourceSnapshot>, StopReason> {
    let sources = sources.into_iter();
    // A tiny bounded union needs no table. This is only an algorithm choice,
    // never a uniqueness/admission assumption; equal keys still compare IDs.
    if sources.size_hint().1.is_some_and(|count| count <= 2) {
        budget.poll()?;
        slot::<Vec<SourceSnapshot>>(budget)?;
        let mut output: Vec<SourceSnapshot> = Vec::new();
        for source in sources {
            budget.charge(Resource::Work, 1)?;
            let key = source.identity_lookup_hash();
            let mut present = false;
            for old in &output {
                budget.charge(Resource::Work, 1)?;
                if old.identity_lookup_hash() == key
                    && old
                        .identity()
                        .compare_with_budget(source.identity(), budget)?
                        .is_eq()
                {
                    present = true;
                    break;
                }
            }
            if !present {
                output.push(copy(source, budget)?);
            }
        }
        return Ok(output);
    }
    copy_sources_with_hash(sources, SourceSnapshot::identity_lookup_hash, budget)
}

fn copy_sources_with_hash<'a>(
    sources: impl IntoIterator<Item = &'a SourceSnapshot>,
    hash: impl Fn(&SourceSnapshot) -> u64,
    budget: &mut Budget,
) -> Result<Vec<SourceSnapshot>, StopReason> {
    budget.poll()?;
    slot::<index::Index>(budget)?;
    let mut index = index::Index::new();
    let sources = sources.into_iter();
    // A finite upper bound is not a uniqueness proof. All reserved slots,
    // including those left unused by duplicate inputs, are charged upfront.
    if let Some(count) = sources.size_hint().1 {
        index.reserve_hint(count, budget)?;
    }
    let mut output: Vec<SourceSnapshot> = Vec::new();
    for source in sources {
        budget.charge(Resource::Work, 1)?;
        let key = hash(source);
        let vacant = match index.lookup(&output, source, key, &hash, budget)? {
            index::Lookup::Present => continue,
            index::Lookup::Vacant(at) => at,
        };
        let grew = index.reserve_one(&output, &hash, budget)?;
        let at = match (vacant, grew) {
            (Some(at), false) => at,
            _ => index.vacant(key, budget)?,
        };
        let owned = copy(source, budget)?;
        index.insert_at(at, output.len(), budget)?;
        output.push(owned);
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
