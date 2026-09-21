//! Generated from interfaces/fixture.json.
//! Regenerate with `cargo run --locked -p nepl3-tools -- fixture --write`.
//! Registers structural shapes; named semantic constraints require their owning validators.

#[rustfmt::skip]
pub fn descriptor(budget: &mut nepl3_core::budget::Budget) -> Result<super::SchemaDescriptor, super::SchemaError> {
budget.charge(nepl3_core::budget::Resource::AllocationUnits, (133usize + core::mem::size_of::<super::NamedType>() + core::mem::size_of::<std::string::String>()) as u64)?;
budget.charge(nepl3_core::budget::Resource::Work, 134)?;
Ok(super::SchemaDescriptor { package: "fixture".into(), revision: 1, types: std::vec![super::NamedType { name: "alloc::Marker".into(), shape: super::TypeShape::Record { fields: std::vec![] }, constraints: std::vec!["keep crate::budget:: foundation --write; Generated from interfaces/contracts.json via interfaces/foundation.json.".into()] }], operations: std::vec![] })
}
