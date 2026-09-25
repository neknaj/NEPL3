//! Explicit versioned descriptor; generated from interfaces/sentence.json.
use nepl3_core::{budget::Budget, schema::*};
mod descriptor;

pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    descriptor::descriptor(budget)
}

/// Compare the complete generated identity. This establishes neither registry
/// finalization nor validity of a value or its source closure.
pub fn matches(
    reference: &nepl3_core::value::SchemaRef,
    budget: &mut Budget,
) -> Result<bool, SchemaError> {
    budget.charge(
        nepl3_core::budget::Resource::Work,
        reference.package.len() as u64 + descriptor::EXPECTED_PACKAGE.len() as u64 + 40,
    )?;
    Ok(reference.package == descriptor::EXPECTED_PACKAGE
        && reference.revision == descriptor::EXPECTED_REVISION
        && reference.digest.0 == descriptor::EXPECTED_DIGEST)
}
