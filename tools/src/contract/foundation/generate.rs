//! Explicit, checked-in production projection; no build script or runtime JSON parser.
use super::*;

pub(super) const PATH: &str = "crates/foundation/core/src/schema/foundation.rs";

#[derive(Default)]
struct Cost {
    bytes: usize,
    fields: usize,
    variants: usize,
    boxes: usize,
    strings: usize,
}
impl Cost {
    fn string(&mut self, text: &str) -> String {
        self.bytes += text.len();
        format!("{text:?}.into()")
    }
    fn ty(&mut self, ty: &TypeDescriptor) -> String {
        match ty {
            TypeDescriptor::List(inner) | TypeDescriptor::Option(inner) => {
                self.boxes += 1;
                let name = if matches!(ty, TypeDescriptor::List(_)) {
                    "List"
                } else {
                    "Option"
                };
                format!(
                    "super::TypeDescriptor::{name}(alloc::boxed::Box::new({}))",
                    self.ty(inner)
                )
            }
            TypeDescriptor::Named(reference) => {
                let package = self.string(&reference.package);
                let name = self.string(&reference.name);
                format!(
                    "super::TypeDescriptor::Named(super::TypeRef {{ package: {package}, revision: {}, name: {name} }})",
                    reference.revision
                )
            }
            primitive => format!("super::TypeDescriptor::{primitive:?}"),
        }
    }
    fn fields(&mut self, fields: &[FieldDescriptor]) -> String {
        self.fields += fields.len();
        let items = fields
            .iter()
            .map(|field| {
                let name = self.string(&field.name);
                let ty = self.ty(&field.ty);
                format!("super::FieldDescriptor {{ name: {name}, ty: {ty} }}")
            })
            .collect::<Vec<_>>();
        format!("alloc::vec![{}]", items.join(","))
    }
}

pub(crate) fn source(descriptor: &SchemaDescriptor) -> Result<String> {
    let mut cost = Cost::default();
    let package = cost.string(&descriptor.package);
    let mut types = Vec::new();
    for ty in &descriptor.types {
        let name = cost.string(&ty.name);
        let shape = match &ty.shape {
            TypeShape::Record { fields } => format!(
                "super::TypeShape::Record {{ fields: {} }}",
                cost.fields(fields)
            ),
            TypeShape::Variant { variants } => {
                cost.variants += variants.len();
                let mut items = Vec::new();
                for variant in variants {
                    let name = cost.string(&variant.name);
                    let fields = cost.fields(&variant.fields);
                    items.push(format!(
                        "super::VariantDescriptor {{ name: {name}, fields: {fields} }}"
                    ));
                }
                format!(
                    "super::TypeShape::Variant {{ variants: alloc::vec![{}] }}",
                    items.join(",")
                )
            }
        };
        cost.strings += ty.constraints.len();
        let constraints = ty
            .constraints
            .iter()
            .map(|id| cost.string(id))
            .collect::<Vec<_>>()
            .join(",");
        types.push(format!("super::NamedType {{ name: {name}, shape: {shape}, constraints: alloc::vec![{constraints}] }}"));
    }
    let mut operations = Vec::new();
    for operation in &descriptor.operations {
        let name = cost.string(&operation.name);
        let input = cost.ty(&operation.input);
        let output = cost.ty(&operation.output);
        operations.push(format!("super::OperationDescriptor {{ name: {name}, input: {input}, output: {output}, pure: {} }}",operation.pure));
    }
    Ok(format!(
        "//! Generated from interfaces/contracts.json via interfaces/foundation.json.\n\
         //! Regenerate with `cargo run --locked -p nepl3-tools -- foundation --write`.\n\
         //! Registers structural shapes; named semantic constraints require their owning validators.\n\n\
         #[rustfmt::skip]\n\
         pub fn descriptor(budget: &mut crate::budget::Budget) -> Result<super::SchemaDescriptor, super::SchemaError> {{\n\
         budget.charge(crate::budget::Resource::AllocationUnits, ({bytes}usize + {named} * core::mem::size_of::<super::NamedType>() + {fields} * core::mem::size_of::<super::FieldDescriptor>() + {variants} * core::mem::size_of::<super::VariantDescriptor>() + {boxes} * core::mem::size_of::<super::TypeDescriptor>() + {strings} * core::mem::size_of::<alloc::string::String>(){operation_cost}) as u64)?;\n\
         budget.charge(crate::budget::Resource::Work, {work})?;\n\
         Ok(super::SchemaDescriptor {{ package: {package}, revision: {revision}, types: alloc::vec![{types}], operations: alloc::vec![{operations}] }})\n\
         }}\n",
        bytes = cost.bytes,
        named = descriptor.types.len(),
        fields = cost.fields,
        variants = cost.variants,
        boxes = cost.boxes,
        strings = cost.strings,
        work = cost.bytes
            + cost.fields
            + cost.variants
            + cost.boxes
            + descriptor.types.len()
            + descriptor.operations.len(),
        operation_cost = if descriptor.operations.is_empty() {
            String::new()
        } else if descriptor.operations.len() == 1 {
            " + core::mem::size_of::<super::OperationDescriptor>()".into()
        } else {
            format!(
                " + {} * core::mem::size_of::<super::OperationDescriptor>()",
                descriptor.operations.len()
            )
        },
        operations = operations.join(",\n"),
        revision = descriptor.revision,
        types = types.join(",\n"),
    ))
}
