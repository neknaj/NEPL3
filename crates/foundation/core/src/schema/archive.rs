//! Explicit versioned source-set archive schema; foundation v1 stays unchanged.
pub mod syntax;
use super::{
    FieldDescriptor, NamedType, SchemaDescriptor, SchemaError, TypeDescriptor, TypeRef, TypeShape,
};
use crate::budget::{Budget, Resource};
use alloc::boxed::Box;

pub const PACKAGE: &str = "nepl3.source-archive";
pub const REVISION: u64 = 1;
pub const ROOT: &str = "SourceSets";

/// Shape contract mirrored by interfaces/source-archive.json. Native membership
/// checks remain mandatory after structural decoding, as with foundation sources.
pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    let string_bytes = PACKAGE.len()
        + ROOT.len()
        + "sources".len()
        + "memberships".len()
        + 2 * "nepl3.foundation".len()
        + "SourceContent".len()
        + "SourceRef".len();
    let allocation = string_bytes
        + core::mem::size_of::<NamedType>()
        + 2 * core::mem::size_of::<FieldDescriptor>()
        + 3 * core::mem::size_of::<TypeDescriptor>();
    budget.charge(Resource::AllocationUnits, allocation as u64)?;
    budget.charge(Resource::Work, string_bytes as u64 + 8)?;
    let named = |name: &str| {
        TypeDescriptor::Named(TypeRef {
            package: "nepl3.foundation".into(),
            revision: 1,
            name: name.into(),
        })
    };
    Ok(SchemaDescriptor {
        package: PACKAGE.into(),
        revision: REVISION,
        types: alloc::vec![NamedType {
            name: ROOT.into(),
            constraints: alloc::vec![],
            shape: TypeShape::Record {
                fields: alloc::vec![
                    FieldDescriptor {
                        name: "sources".into(),
                        ty: TypeDescriptor::List(Box::new(named("SourceContent")))
                    },
                    FieldDescriptor {
                        name: "memberships".into(),
                        ty: TypeDescriptor::List(Box::new(TypeDescriptor::List(Box::new(named(
                            "SourceRef"
                        )))))
                    },
                ]
            },
        }],
        operations: alloc::vec![],
    })
}
