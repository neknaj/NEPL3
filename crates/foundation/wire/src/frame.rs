//! Pure length-prefixed NDF framing for host transports (spec 09, section 5).
//! Hosts own I/O and request state. Payload schema and domain checks remain explicit.
use crate::{StructuralValue, WireError};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{SchemaRegistry, TypeDescriptor},
    value::NdfValue,
};

/// Inspect an eight-byte unsigned big-endian length before allocating a payload.
/// SourceBytes covers the complete frame, including its eight-byte header.
/// This preflight consumes Work; accepted bytes are charged by `decode_checked`.
pub fn payload_length(header: &[u8; 8], budget: &mut Budget) -> Result<usize, WireError> {
    budget.charge(Resource::Work, 8)?;
    let payload = u64::from_be_bytes(*header);
    let total = payload.checked_add(8).ok_or(WireError::InvalidLength)?;
    let remaining = budget
        .limits()
        .source_bytes
        .saturating_sub(budget.usage().source_bytes);
    if total > remaining {
        budget.charge(Resource::SourceBytes, total)?;
    }
    usize::try_from(payload).map_err(|_| WireError::InvalidLength)
}

/// Decode the first frame and return the unconsumed suffix.
/// An incomplete buffer yields None; final input instead reports UnexpectedEnd.
/// A complete payload must contain exactly one NDF value of the expected schema.
pub fn decode_checked<'a>(
    bytes: &'a [u8],
    final_input: bool,
    expected: &TypeDescriptor,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Option<(StructuralValue, &'a [u8])>, WireError> {
    budget.poll()?;
    let incomplete = || {
        if final_input {
            Err(WireError::UnexpectedEnd)
        } else {
            Ok(None)
        }
    };
    let Some(header) = bytes.get(..8) else {
        return incomplete();
    };
    let header: [u8; 8] = header.try_into().map_err(|_| WireError::InvalidLength)?;
    let payload = payload_length(&header, budget)?;
    let total = payload.checked_add(8).ok_or(WireError::InvalidLength)?;
    let Some(frame) = bytes.get(..total) else {
        return incomplete();
    };
    budget.charge(Resource::SourceBytes, total as u64)?;
    let value = crate::decode_checked(&frame[8..], expected, registry, budget)?;
    Ok(Some((value, &bytes[total..])))
}

/// Encode one schema-checked value with its length prefix.
/// OutputBytes includes the header. Allocation/Work include the payload copy.
pub fn encode_checked(
    value: &NdfValue,
    expected: &TypeDescriptor,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    budget.charge(Resource::OutputBytes, 8)?;
    let payload = crate::encode_checked(value, expected, registry, budget)?;
    let length = u64::try_from(payload.len()).map_err(|_| WireError::InvalidLength)?;
    let total = payload
        .len()
        .checked_add(8)
        .ok_or(WireError::InvalidLength)?;
    budget.charge(Resource::AllocationUnits, total as u64)?;
    budget.charge(Resource::Work, total as u64)?;
    let mut frame = Vec::with_capacity(total);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}
