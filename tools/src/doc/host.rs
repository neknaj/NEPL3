//! Explicit standard Doc composition; core semantics do not select host code.
use crate::source::host::{NativeHost, NativeReader};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    source::Digest,
};
use nepl3_engine::parse::ParseError;
use nepl3_reader::builtin::{BuiltinReader, provider};
pub fn native<'a>(
    registry: &'a SchemaRegistry,
    implementation: Digest,
    prefix: String,
    b: &mut Budget,
) -> Result<NativeHost<'a>, ParseError> {
    b.charge(
        Resource::AllocationUnits,
        4 * core::mem::size_of::<NativeReader>() as u64,
    )?;
    let mut readers = Vec::with_capacity(4);
    for kind in [
        BuiltinReader::Name,
        BuiltinReader::Number,
        BuiltinReader::Trivia,
    ] {
        readers.push(NativeReader {
            operation: provider::operation(kind, registry, b)?,
            read: provider::read,
        });
    }
    readers.push(NativeReader {
        operation: crate::sentence::reader::signature(registry, b)?.operation,
        read: crate::sentence::reader::read,
    });
    NativeHost::new(registry, implementation, prefix, readers, b)
}
