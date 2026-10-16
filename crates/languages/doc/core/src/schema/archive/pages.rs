//! Opt-in PageSet archive transport, independent of legacy PageSet identity.
use alloc::boxed::Box;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::*,
};
pub const PACKAGE: &str = "nepl3.doc-pages-archive";
pub const REVISION: u64 = 1;
pub const ROOT: &str = "PageSetArchive";
pub const PAGE: &str = "ArchivedPage";
pub fn descriptor(b: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    let fields = [
        ("registration", "nepl3.doc", "PageRegistration", false),
        ("document", "nepl3.doc-archive", "DocumentArchive", false),
        ("pages", PACKAGE, PAGE, true),
        ("files", "nepl3.doc", "PageFile", true),
    ];
    let strings = PACKAGE.len()
        + ROOT.len()
        + PAGE.len()
        + fields
            .iter()
            .map(|(f, p, n, _)| f.len() + p.len() + n.len())
            .sum::<usize>();
    b.charge(
        Resource::AllocationUnits,
        (strings
            + 2 * core::mem::size_of::<NamedType>()
            + 4 * core::mem::size_of::<FieldDescriptor>()
            + 2 * core::mem::size_of::<TypeDescriptor>()) as u64,
    )?;
    b.charge(Resource::Work, strings as u64 + 12)?;
    let field = |(name, package, ty, list): (&str, &str, &str, bool)| {
        let named = TypeDescriptor::Named(TypeRef {
            package: package.into(),
            revision: 1,
            name: ty.into(),
        });
        FieldDescriptor {
            name: name.into(),
            ty: if list {
                TypeDescriptor::List(Box::new(named))
            } else {
                named
            },
        }
    };
    Ok(SchemaDescriptor {
        package: PACKAGE.into(),
        revision: REVISION,
        operations: alloc::vec![],
        types: alloc::vec![
            NamedType {
                name: PAGE.into(),
                constraints: alloc::vec![],
                shape: TypeShape::Record {
                    fields: alloc::vec![field(fields[0]), field(fields[1])]
                }
            },
            NamedType {
                name: ROOT.into(),
                constraints: alloc::vec![],
                shape: TypeShape::Record {
                    fields: alloc::vec![field(fields[2]), field(fields[3])]
                }
            }
        ],
    })
}
