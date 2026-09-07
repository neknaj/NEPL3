//! Explicit Math schema projection; tags never depend on native enum order.
use nepl3_core::{budget::Budget, schema::*};
mod descriptor;
pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    descriptor::descriptor(budget)
}
