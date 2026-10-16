//! Syntax transport has a separate identity from the source-set archive schema.
use crate::{
    budget::{Budget, Resource},
    schema::{
        FieldDescriptor, NamedType, SchemaDescriptor, SchemaError, TypeDescriptor, TypeRef,
        TypeShape,
    },
};
pub const PACKAGE: &str = "nepl3.syntax-archive";
pub const ROOT: &str = "SyntaxArchive";
pub const REVISION: u64 = 1;
pub fn descriptor(b: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    let strings = PACKAGE.len()
        + ROOT.len()
        + "sources".len()
        + "skeleton".len()
        + super::PACKAGE.len()
        + super::ROOT.len();
    b.charge(
        Resource::AllocationUnits,
        (strings + core::mem::size_of::<NamedType>() + 2 * core::mem::size_of::<FieldDescriptor>())
            as u64,
    )?;
    b.charge(Resource::Work, strings as u64 + 6)?;
    Ok(SchemaDescriptor {
        package: PACKAGE.into(),
        revision: REVISION,
        operations: alloc::vec![],
        types: alloc::vec![NamedType {
            name: ROOT.into(),
            constraints: alloc::vec![],
            shape: TypeShape::Record {
                fields: alloc::vec![
                    FieldDescriptor {
                        name: "sources".into(),
                        ty: TypeDescriptor::Named(TypeRef {
                            package: super::PACKAGE.into(),
                            revision: super::REVISION,
                            name: super::ROOT.into()
                        })
                    },
                    FieldDescriptor {
                        name: "skeleton".into(),
                        ty: TypeDescriptor::Bytes
                    },
                ]
            }
        }],
    })
}
