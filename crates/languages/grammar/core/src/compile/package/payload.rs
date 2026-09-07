use super::*;
fn add<'a>(
    result: &mut Vec<&'a SchemaRef>,
    value: &'a SchemaRef,
    surface: &SchemaRef,
    budget: &mut Budget,
) -> Result<(), CompileError> {
    for previous in result.iter().copied().chain(core::iter::once(surface)) {
        budget.charge(
            Resource::Work,
            previous.package.len().min(value.package.len()) as u64 + 41,
        )?;
        if previous == value {
            return Ok(());
        }
    }
    push(result, value, budget)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn schemas(
    registry: &SchemaRegistry,
    surface: &SchemaRef,
    reader: &nepl3_reader::plan::ReaderPlan,
    forms: &[p::Form],
    leaves: &[p::Leaf],
    extensions: &[p::ExtensionRequirement],
    budget: &mut Budget,
) -> Result<Vec<SchemaRef>, CompileError> {
    let mut result = Vec::new();
    let mut pending = Vec::new();
    let mut visited = Vec::new();
    push(&mut pending, &reader.state_type, budget)?;
    for rule in &reader.rules {
        push(&mut pending, &rule.output, budget)?;
    }
    for provider in &reader.providers {
        for ty in [
            &provider.value_input,
            &provider.value_output,
            &provider.state_type,
            &provider.continuation_type,
        ] {
            push(&mut pending, ty, budget)?;
        }
    }
    for leaf in leaves {
        push(&mut pending, &leaf.payload, budget)?;
    }
    for extension in extensions {
        add(&mut result, &extension.operation.schema, surface, budget)?;
        push(&mut pending, &extension.input, budget)?;
        push(&mut pending, &extension.output, budget)?;
    }
    // Only annotations actually referenced by this package contribute their owners.
    for expression in &reader.expressions {
        match expression {
            nepl3_reader::plan::ReaderExpr::Region { class, .. } => {
                add(&mut result, &class.schema, surface, budget)?
            }
            nepl3_reader::plan::ReaderExpr::Node { kind, .. } => {
                add(&mut result, &kind.schema, surface, budget)?
            }
            _ => {}
        }
    }
    for style in forms
        .iter()
        .flat_map(|f| &f.styles)
        .chain(leaves.iter().flat_map(|f| &f.styles))
    {
        add(&mut result, &style.class.schema, surface, budget)?;
    }
    while let Some(ty) = pending.pop() {
        budget.charge(Resource::Work, 1)?;
        match ty {
            TypeDescriptor::List(inner) | TypeDescriptor::Option(inner) => {
                push(&mut pending, inner.as_ref(), budget)?
            }
            TypeDescriptor::Named(reference) => {
                let mut found = false;
                for previous in &visited {
                    let previous: &&TypeRef = previous;
                    budget.charge(
                        Resource::Work,
                        (reference.package.len() + reference.name.len()) as u64 + 9,
                    )?;
                    if *previous == reference {
                        found = true;
                        break;
                    }
                }
                if found {
                    continue;
                }
                push(&mut visited, reference, budget)?;
                let schema = registry
                    .selected(&reference.package, reference.revision)
                    .ok_or(SchemaError::UnknownSchema)?;
                add(&mut result, schema, surface, budget)?;
                let descriptor = registry
                    .descriptor(schema)
                    .ok_or(SchemaError::UnknownSchema)?;
                budget.charge(
                    Resource::Work,
                    (descriptor.types.len() as u64).saturating_mul(reference.name.len() as u64 + 1),
                )?;
                let definition = descriptor
                    .types
                    .iter()
                    .find(|v| v.name == reference.name)
                    .ok_or(SchemaError::UnknownType)?;
                match &definition.shape {
                    TypeShape::Record { fields } => {
                        for field in fields {
                            push(&mut pending, &field.ty, budget)?;
                        }
                    }
                    TypeShape::Variant { variants } => {
                        for variant in variants {
                            for field in &variant.fields {
                                push(&mut pending, &field.ty, budget)?;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut owned = Vec::new();
    for schema in result {
        budget.charge(Resource::AllocationUnits, schema.package.len() as u64)?;
        push(&mut owned, schema.clone(), budget)?;
    }
    Ok(owned)
}
