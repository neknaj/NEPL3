//! Explicitly generated language-neutral Grammar constructor descriptor.
use nepl3_core::{budget::Budget, schema::*};
mod descriptor;
pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    descriptor::descriptor(budget)
}
