//! Explicit syntax transport identity, distinct from the source-set schema.
use crate::{
    budget::Budget,
    schema::{SchemaDescriptor, SchemaError},
};
pub const PACKAGE: &str = "nepl3.syntax-archive";
pub const ROOT: &str = "SyntaxArchive";
pub const REVISION: u64 = 1;
pub fn descriptor(b: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    super::envelope_descriptor(PACKAGE, ROOT, REVISION, b)
}
