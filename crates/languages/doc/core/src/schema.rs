//! Descriptor generated from the explicit Doc contract, never Rust enum order.
use nepl3_core::{budget::Budget, schema::*};
pub mod archive;
mod descriptor;
pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    descriptor::descriptor(budget)
}
