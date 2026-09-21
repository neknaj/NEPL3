//! Deterministic identity of the context actually admitted by the host.
use super::*;
use nepl3_core::{budget::Resource, source::Digest};

fn hash(value: &NdfValue, b: &mut Budget) -> Result<Digest, WireError> {
    let encoded = crate::encode(value, b)?;
    b.charge(Resource::Work, encoded.len() as u64)?;
    Ok(Digest::of(&encoded))
}
fn sift(values: &mut [Digest], mut root: usize, b: &mut Budget) -> Result<(), WireError> {
    while root < values.len() / 2 {
        b.charge(Resource::Work, 1)?;
        let mut child = root * 2 + 1;
        if child + 1 < values.len() {
            b.charge(Resource::Work, 32)?;
            if values[child].0 < values[child + 1].0 {
                child += 1;
            }
        }
        b.charge(Resource::Work, 32)?;
        if values[root].0 >= values[child].0 {
            break;
        }
        b.charge(Resource::Work, 1)?;
        values.swap(root, child);
        root = child;
    }
    Ok(())
}
fn ordered(values: &mut Vec<Digest>, b: &mut Budget) -> Result<NdfValue, WireError> {
    for root in (0..values.len() / 2).rev() {
        sift(values, root, b)?;
    }
    for end in (1..values.len()).rev() {
        b.charge(Resource::Work, 1)?;
        values.swap(0, end);
        sift(&mut values[..end], 0, b)?;
    }
    b.charge(Resource::Work, (values.len() as u64).saturating_mul(32))?;
    values.dedup();
    sequence(values, b, |digest, b| bytes(&digest.0, b))
}

/// Compute the snapshot identity from the host-admitted environment and source /
/// resource closure. `configuration` identifies all remaining immutable host
/// policy and provider configuration. Authorization precedes this call; hashing
/// remotely supplied capabilities does not grant access to them.
///
/// Source/resource ordering does not change identity. Equal repeated snapshots
/// denote the same source; conflicting identities and duplicate resources fail.
/// Request ID, input and Limits are excluded: input is compared separately by
/// call-graph admission, while IDs/limits must not conceal recursive cycles.
/// The domain version identifies this host-side digest recipe, not a wire type.
pub fn context_digest(
    call: &Invoke,
    configuration: Digest,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<Digest, WireError> {
    b.poll()?;
    registry.validate_typed(&call.environment, b)?;
    validate_resources(&call.resources, b)?;
    let schema = schema(registry)?;
    let mut admission = SourceAdmission::default();
    let mut sources = Vec::new();
    b.charge(
        Resource::AllocationUnits,
        (call.sources.len() as u64).saturating_mul(32),
    )?;
    sources
        .try_reserve_exact(call.sources.len())
        .map_err(|_| b.stop(nepl3_core::budget::StopReason::AllocationLimit))?;
    for source in &call.sources {
        admission.admit_existing(source, b)?;
        let identity = source.identity();
        let fields = [
            text(&identity.source.0, b)?,
            NdfValue::U64(identity.revision),
            bytes(&identity.digest.0, b)?,
            text(source.uri(), b)?,
        ];
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of_val(&fields) as u64,
        )?;
        sources.push(hash(&NdfValue::List(fields.into()), b)?);
    }
    let mut resources = Vec::new();
    b.charge(
        Resource::AllocationUnits,
        (call.resources.len() as u64).saturating_mul(32),
    )?;
    resources
        .try_reserve_exact(call.resources.len())
        .map_err(|_| b.stop(nepl3_core::budget::StopReason::AllocationLimit))?;
    for resource in &call.resources {
        let fields = [text(&resource.id, b)?, bytes(&resource.digest.0, b)?];
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of_val(&fields) as u64,
        )?;
        resources.push(hash(&NdfValue::List(fields.into()), b)?);
    }
    let fields = [
        text("nepl3.operation-context/1", b)?,
        bytes(&configuration.0, b)?,
        call.environment.value(schema, b)?,
        ordered(&mut sources, b)?,
        ordered(&mut resources, b)?,
    ];
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of_val(&fields) as u64,
    )?;
    hash(&NdfValue::List(fields.into()), b)
}
