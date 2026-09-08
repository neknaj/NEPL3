//! Generated from interfaces/doc-reader.json.
//! Regenerate with `cargo run --locked -p nepl3-tools -- doc --write`.
//! Registers structural shapes; named semantic constraints require their owning validators.

#[rustfmt::skip]
pub fn descriptor(budget: &mut nepl3_core::budget::Budget) -> Result<super::SchemaDescriptor, super::SchemaError> {
budget.charge(nepl3_core::budget::Resource::AllocationUnits, (157usize + core::mem::size_of::<super::NamedType>() + 2 * core::mem::size_of::<super::OperationDescriptor>()) as u64)?;
budget.charge(nepl3_core::budget::Resource::Work, 160)?;
Ok(super::SchemaDescriptor { package: "nepl3.doc.reader".into(), revision: 1, types: std::vec![super::NamedType { name: "SentenceDiagnosticArguments".into(), shape: super::TypeShape::Record { fields: std::vec![] }, constraints: std::vec![] }], operations: std::vec![super::OperationDescriptor { name: "sentence".into(), input: super::TypeDescriptor::Named(super::TypeRef { package: "nepl3.reader".into(), revision: 1, name: "ReadRequest".into() }), output: super::TypeDescriptor::Named(super::TypeRef { package: "nepl3.reader".into(), revision: 1, name: "ReadReply".into() }), pure: true },
super::OperationDescriptor { name: "sentenceReferenced".into(), input: super::TypeDescriptor::Named(super::TypeRef { package: "nepl3.reader".into(), revision: 1, name: "ReadRequest".into() }), output: super::TypeDescriptor::Named(super::TypeRef { package: "nepl3.reader".into(), revision: 1, name: "ReadReply".into() }), pure: true }] })
}
