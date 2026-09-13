//! Explicit versioned descriptor; generated from interfaces/sentence.json.
use nepl3_core::{budget::Budget, schema::*};
mod descriptor;

pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    descriptor::descriptor(budget)
}
