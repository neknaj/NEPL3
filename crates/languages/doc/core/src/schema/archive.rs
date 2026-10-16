//! Explicit document transport; Doc v1 descriptor identity is unchanged.
use nepl3_core::{
    budget::{Budget, Resource},
    schema::*,
};
pub const PACKAGE: &str = "nepl3.doc-archive";
pub const REVISION: u64 = 1;
pub const ROOT: &str = "DocumentArchive";
pub fn descriptor(b: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    let strings = PACKAGE.len()
        + ROOT.len()
        + "skeleton".len()
        + "group".len()
        + "nepl3.doc".len()
        + "DocumentSyntax".len();
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
                        name: "skeleton".into(),
                        ty: TypeDescriptor::Named(TypeRef {
                            package: "nepl3.doc".into(),
                            revision: 1,
                            name: "DocumentSyntax".into()
                        })
                    },
                    FieldDescriptor {
                        name: "group".into(),
                        ty: TypeDescriptor::Bytes
                    },
                ]
            }
        }],
    })
}

pub mod pages;
