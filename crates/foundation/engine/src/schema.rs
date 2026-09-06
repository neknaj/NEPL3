//! Registered language-neutral package descriptor, generated before ordinary builds.
use nepl3_core::{budget::Budget, schema::*};
mod descriptor;
pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    descriptor::descriptor(budget)
}
