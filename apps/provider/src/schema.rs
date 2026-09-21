//! Admit a descriptor batch selected by the host before operation dispatch.
//! Transport negotiation supplies the bytes; it never chooses trusted identities.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaDescriptor, SchemaError, SchemaRegistry},
    value::SchemaRef,
    value_codec::FoundationCodecError,
};
use nepl3_wire::WireError;

#[derive(Debug)]
pub enum SchemaAdmissionError {
    Stopped(StopReason),
    Schema(SchemaError),
    Wire(WireError),
    Count,
    DuplicateSelection,
}
impl From<StopReason> for SchemaAdmissionError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<SchemaError> for SchemaAdmissionError {
    fn from(error: SchemaError) -> Self {
        match error {
            SchemaError::Stopped(reason) => Self::Stopped(reason),
            e => Self::Schema(e),
        }
    }
}
impl From<WireError> for SchemaAdmissionError {
    fn from(error: WireError) -> Self {
        match error.stop_reason() {
            Some(reason) => Self::Stopped(reason),
            None => Self::Wire(error),
        }
    }
}

/// Decode one descriptor per host-selected identity against an immutable,
/// finalized bootstrap registry, then resolve their symbolic dependency closure.
/// Order permits forward and recursive symbolic references. A failed batch
/// exposes no partially registered registry. The consumed bootstrap is replaced
/// only by a successful return; callers retain their active registry separately.
///
/// Payload order follows `expected`, which must come from trusted host policy.
/// This grants schema knowledge only: implementations and resource permissions
/// require their own host registration. Framing/handshake state is external.
pub fn admit(
    mut bootstrap: SchemaRegistry,
    expected: &[SchemaRef],
    payloads: &[&[u8]],
    budget: &mut Budget,
) -> Result<SchemaRegistry, SchemaAdmissionError> {
    budget.poll()?;
    if !bootstrap.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    if expected.len() != payloads.len() {
        return Err(SchemaAdmissionError::Count);
    }
    // Bound allocation before allocating or decoding attacker-controlled bytes.
    let size = expected
        .len()
        .checked_mul(core::mem::size_of::<SchemaDescriptor>())
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, size as u64)?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(expected.len())
        .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    for (index, (identity, payload)) in expected.iter().zip(payloads).enumerate() {
        for prior in &expected[..index] {
            budget.charge(
                Resource::Work,
                (identity.package.len() as u64)
                    .saturating_add(prior.package.len() as u64)
                    .saturating_add(8),
            )?;
            if identity.package == prior.package && identity.revision == prior.revision {
                return Err(SchemaAdmissionError::DuplicateSelection);
            }
        }
        decoded.push(nepl3_wire::schema::decode(
            payload, identity, &bootstrap, budget,
        )?);
    }
    for (identity, descriptor) in expected.iter().zip(decoded) {
        budget.charge(Resource::AllocationUnits, identity.package.len() as u64)?;
        budget.charge(Resource::Work, identity.package.len() as u64 + 40)?;
        bootstrap.register(identity.clone(), descriptor, budget)?;
    }
    bootstrap.finalize(budget)?;
    Ok(bootstrap)
}
