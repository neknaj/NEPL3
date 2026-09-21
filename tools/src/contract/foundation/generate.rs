//! Explicit, checked-in production projection; no build script or runtime JSON parser.
use super::*;

pub(super) const PATH: &str = "crates/foundation/core/src/schema/foundation.rs";

/// Trusted generator settings, separate from schema string values.
#[derive(Clone, Copy)]
pub(crate) struct Output {
    pub provenance: &'static str,
    pub command: Option<&'static str>,
    pub budget: &'static str,
    pub allocator: &'static str,
}

impl Output {
    pub(crate) const fn domain(provenance: &'static str, command: &'static str) -> Self {
        Self {
            provenance,
            command: Some(command),
            budget: "nepl3_core::budget",
            allocator: "alloc",
        }
    }

    pub(crate) const fn host(mut self) -> Self {
        self.allocator = "std";
        self
    }
}

#[derive(Default)]
struct Cost {
    bytes: usize,
    fields: usize,
    variants: usize,
    boxes: usize,
    strings: usize,
    allocator: &'static str,
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
                let allocator = self.allocator;
                format!(
                    "super::TypeDescriptor::{name}({allocator}::boxed::Box::new({}))",
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
        format!("{}::vec![{}]", self.allocator, items.join(","))
    }
}

pub(crate) fn source(descriptor: &SchemaDescriptor) -> Result<String> {
    source_with(
        descriptor,
        Output {
            provenance: "interfaces/contracts.json via interfaces/foundation.json",
            command: Some("foundation"),
            budget: "crate::budget",
            allocator: "alloc",
        },
    )
}

pub(crate) fn source_with(descriptor: &SchemaDescriptor, output: Output) -> Result<String> {
    let mut cost = Cost {
        allocator: output.allocator,
        ..Cost::default()
    };
    let allocator = output.allocator;
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
                    "super::TypeShape::Variant {{ variants: {allocator}::vec![{}] }}",
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
        types.push(format!("super::NamedType {{ name: {name}, shape: {shape}, constraints: {allocator}::vec![{constraints}] }}"));
    }
    let mut operations = Vec::new();
    for operation in &descriptor.operations {
        let name = cost.string(&operation.name);
        let input = cost.ty(&operation.input);
        let output = cost.ty(&operation.output);
        operations.push(format!("super::OperationDescriptor {{ name: {name}, input: {input}, output: {output}, pure: {} }}",operation.pure));
    }
    let regeneration = output.command.map_or_else(String::new, |command| {
        format!("//! Regenerate with `cargo run --locked -p nepl3-tools -- {command} --write`.\n")
    });
    Ok(format!(
        "//! Generated from {provenance}.\n\
         {regeneration}\
         //! Registers structural shapes; named semantic constraints require their owning validators.\n\n\
         #[rustfmt::skip]\n\
         pub fn descriptor(budget: &mut {budget}::Budget) -> Result<super::SchemaDescriptor, super::SchemaError> {{\n\
         budget.charge({budget}::Resource::AllocationUnits, ({bytes}usize{named}{fields}{variants}{boxes}{strings}{operation_cost}) as u64)?;\n\
         budget.charge({budget}::Resource::Work, {work})?;\n\
         Ok(super::SchemaDescriptor {{ package: {package}, revision: {revision}, types: {allocator}::vec![{types}], operations: {allocator}::vec![{operations}] }})\n\
         }}\n",
        provenance = output.provenance,
        budget = output.budget,
        bytes = cost.bytes,
        named = storage(descriptor.types.len(), "super::NamedType"),
        fields = storage(cost.fields, "super::FieldDescriptor"),
        variants = storage(cost.variants, "super::VariantDescriptor"),
        boxes = storage(cost.boxes, "super::TypeDescriptor"),
        strings = storage(cost.strings, &format!("{allocator}::string::String")),
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

#[cfg(test)]
mod tests;
