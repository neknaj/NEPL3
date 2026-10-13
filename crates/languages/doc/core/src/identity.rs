//! Operation domains wrap a bounded-size Foundation component identity.
use crate::portable::{PortableError, boundary};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::Digest,
    syntax::group::ForeignClosureGroup,
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
pub(crate) fn guests<C: FoundationValueCodec>(
    group: &ForeignClosureGroup,
    domain: &[u8],
    c: &mut C,
    b: &mut Budget,
) -> Result<Vec<Digest>, PortableError<C::Error>> {
    let components = c.foreign_group_entry_digests(group, b).map_err(boundary)?;
    if components.len() != group.entries.len() {
        return Err(PortableError::Shape);
    }
    let bytes = components
        .len()
        .checked_mul(core::mem::size_of::<Digest>())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut result = Vec::with_capacity(components.len());
    for component in components {
        b.charge(Resource::Work, 32)?;
        b.charge(Resource::AllocationUnits, 32)?;
        let value = NdfValue::Bytes(component.0.to_vec());
        result.push(
            c.canonical_value_digest(domain, &value, b)
                .map_err(boundary)?,
        );
    }
    Ok(result)
}
