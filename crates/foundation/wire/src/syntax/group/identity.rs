//! Called only after this operation encoded and validated the exact native group.
//! Hash disjoint components separately: no quadratic batch-request index and no
//! repeated provenance hashing for each consuming guest.
use super::*;
use nepl3_core::source::Digest;

const PROVENANCE: &[u8] = b"NEPL3.Foreign.Provenance.v1\0";
const ENVIRONMENT: &[u8] = b"NEPL3.Foreign.Environment.v1\0";
const SYNTAX: &[u8] = b"NEPL3.Foreign.Syntax.v1\0";
const ENTRY: &[u8] = b"NEPL3.Foreign.Entry.v1\0";

pub(crate) fn from_encoded(
    group: &ForeignClosureGroup,
    encoded: &NdfValue,
    schema: &SchemaRef,
    b: &mut Budget,
) -> Result<Vec<Digest>, WireError> {
    b.poll()?;
    let f = fields(encoded, schema, "ForeignClosureGroup", 3)?;
    let provenances = list(&f[0])?;
    let environments = list(&f[1])?;
    let entries = list(&f[2])?;
    if provenances.len() != group.provenances.len()
        || environments.len() != group.environments.len()
        || entries.len() != group.entries.len()
    {
        return Err(WireError::InvalidLength);
    }
    let mut provenance_digests = Vec::new();
    for provenance in provenances {
        b.charge(Resource::Work, 1)?;
        let digest = crate::encode::digest(PROVENANCE, provenance, b)?;
        push(&mut provenance_digests, digest, b)?;
    }
    let mut environment_digests = Vec::new();
    for environment in environments {
        b.charge(Resource::Work, 1)?;
        let f = fields(environment, schema, "ForeignOwnerEnvironment", 2)?;
        let digest = crate::encode::digest(ENVIRONMENT, &f[1], b)?;
        push(&mut environment_digests, digest, b)?;
    }
    let mut result = Vec::new();
    for (entry, encoded) in group.entries.iter().zip(entries) {
        b.charge(Resource::Work, 1)?;
        let owner_index = usize::try_from(entry.owner.0).map_err(|_| WireError::InvalidLength)?;
        let owner = group
            .environments
            .get(owner_index)
            .ok_or(WireError::InvalidLength)?;
        let provenance_index =
            usize::try_from(owner.provenance.0).map_err(|_| WireError::InvalidLength)?;
        let provenance = provenance_digests
            .get(provenance_index)
            .ok_or(WireError::InvalidLength)?;
        let environment = environment_digests
            .get(owner_index)
            .ok_or(WireError::InvalidLength)?;
        let f = fields(encoded, schema, "ForeignGroupEntry", 2)?;
        let syntax = crate::encode::digest(SYNTAX, &f[1], b)?;
        // Canonical NDF list of three Bytes32 values, in this fixed order.
        b.charge(Resource::Work, 96)?;
        b.charge(
            Resource::AllocationUnits,
            96 + 3 * core::mem::size_of::<NdfValue>() as u64,
        )?;
        let value = NdfValue::List(alloc::vec![
            NdfValue::Bytes(provenance.0.to_vec()),
            NdfValue::Bytes(environment.0.to_vec()),
            NdfValue::Bytes(syntax.0.to_vec()),
        ]);
        let digest = crate::encode::digest(ENTRY, &value, b)?;
        push(&mut result, digest, b)?;
    }
    Ok(result)
}
