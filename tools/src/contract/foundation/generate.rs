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
         budget.charge(crate::budget::Resource::AllocationUnits, ({bytes}usize{named}{fields}{variants}{boxes}{strings}{operation_cost}) as u64)?;\n\
         budget.charge(crate::budget::Resource::Work, {work})?;\n\
         Ok(super::SchemaDescriptor {{ package: {package}, revision: {revision}, types: alloc::vec![{types}], operations: alloc::vec![{operations}] }})\n\
         }}\n",
        bytes = cost.bytes,
        named = storage(descriptor.types.len(), "super::NamedType"),
        fields = storage(cost.fields, "super::FieldDescriptor"),
        variants = storage(cost.variants, "super::VariantDescriptor"),
        boxes = storage(cost.boxes, "super::TypeDescriptor"),
        strings = storage(cost.strings, "alloc::string::String"),
        work = cost.bytes
            + cost.fields
            + cost.variants
            + cost.boxes
            + descriptor.types.len()
            + descriptor.operations.len(),
        operation_cost = storage(descriptor.operations.len(), "super::OperationDescriptor"),
        operations = operations.join(",\n"),
        revision = descriptor.revision,
        types = types.join(",\n"),
    ))
}
fn storage(count: usize, ty: &str) -> String {
    match count {
        0 => String::new(),
        1 => format!(" + core::mem::size_of::<{ty}>()"),
        count => format!(" + {count} * core::mem::size_of::<{ty}>()"),
    }
}
