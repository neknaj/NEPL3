use crate::check::ValidatedDocShape;
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource, StopReason};

/// Maximum owner depth for each embed, indexed by `EmbedRef`. The root has
/// depth one. A host generating a shared guest once must inherit its deepest
/// occurrence, not merely the first path encountered. These offsets are not
/// source, identity, binding, or guest semantic proofs.
///
/// O(nodes + edges + embeds) time and O(nodes + embeds) auxiliary space.
/// The input proof guarantees references and that every embed is used.
pub fn guest_depths(shape: &ValidatedDocShape<'_>, b: &mut Budget) -> Result<Vec<u64>, StopReason> {
    let value = shape.value();
    let count = (value.nodes.len() as u64).saturating_add(value.embeds.len() as u64);
    b.charge(Resource::Work, count)?;
    b.charge(Resource::AllocationUnits, count.saturating_mul(8))?;
    let mut depths = Vec::new();
    let mut guests = Vec::new();
    depths
        .try_reserve_exact(value.nodes.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    guests
        .try_reserve_exact(value.embeds.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    depths.resize(value.nodes.len(), 0u64);
    guests.resize(value.embeds.len(), 0u64);
    depths[crate::check::edges::root(value.root).0 as usize] = 1;
    for &id in shape.postorder().iter().rev() {
        b.charge(Resource::Work, 1)?;
        let depth = depths[id];
        b.observe_depth(depth)?;
        let kind = &value.nodes[id].kind;
        if let Some((embed, _)) = kind.embedded() {
            guests[embed.0 as usize] = guests[embed.0 as usize].max(depth);
        }
        let mut index = 0;
        while let Some((child, _)) = crate::check::edges::edge(kind, index) {
            b.charge(Resource::Work, 1)?;
            depths[child as usize] = depths[child as usize].max(depth.saturating_add(1));
            index += 1;
        }
    }
    Ok(guests)
}
