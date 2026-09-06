use super::*;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{SchemaRegistry, TypeDescriptor},
};
use nepl3_reader::plan::ReaderExpr;

fn field<'a>(
    fields: &'a [FieldSpec],
    name: &str,
    budget: &mut Budget,
) -> Result<(usize, &'a FieldSpec), PackageError> {
    budget.charge(Resource::Work, fields.len() as u64 + 1)?;
    fields
        .iter()
        .enumerate()
        .find(|(_, v)| v.name == name)
        .ok_or(PackageError::InvalidSelector)
}
fn push<T>(items: &mut Vec<T>, value: T, budget: &mut Budget) -> Result<(), PackageError> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    items.push(value);
    Ok(())
}
pub(super) fn check(
    package: &LanguagePackage,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), PackageError> {
    // Reject every direct binding cycle, including currently unused declarations.
    for root in 0..package.bindings.len() {
        let mut pending = Vec::new();
        let mut path = Vec::new();
        push(&mut pending, (BindingId(root as u64), false), budget)?;
        while let Some((id, exit)) = pending.pop() {
            budget.charge(Resource::Work, path.len() as u64 + 1)?;
            if exit {
                path.pop();
                continue;
            }
            budget.observe_depth(path.len() as u64 + 1)?;
            if path.contains(&id) {
                return Err(PackageError::DirectCycle);
            }
            let binding = usize::try_from(id.0)
                .ok()
                .and_then(|i| package.bindings.get(i))
                .ok_or(PackageError::InvalidBinding)?;
            push(&mut path, id, budget)?;
            push(&mut pending, (id, true), budget)?;
            if let Binding::Group(children) | Binding::Scope(children) = binding {
                for child in children.iter().rev() {
                    push(&mut pending, (*child, false), budget)?;
                }
            }
        }
    }
    for form in &package.forms {
        owner(
            package,
            &form.fields,
            None,
            form.binding,
            &form.styles,
            registry,
            budget,
        )?;
    }
    for leaf in &package.leaves {
        owner(
            package,
            &[],
            Some(&leaf.payload),
            leaf.binding,
            &leaf.styles,
            registry,
            budget,
        )?;
    }
    Ok(())
}
fn owner(
    package: &LanguagePackage,
    fields: &[FieldSpec],
    self_type: Option<&TypeDescriptor>,
    root: BindingId,
    styles: &[StyleRule],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), PackageError> {
    budget.charge(Resource::AllocationUnits, fields.len() as u64)?;
    let mut visited = alloc::vec![false;fields.len()];
    let mut pending = Vec::new();
    push(&mut pending, root, budget)?;
    while let Some(id) = pending.pop() {
        budget.charge(Resource::Work, 1)?;
        let binding = usize::try_from(id.0)
            .ok()
            .and_then(|i| package.bindings.get(i))
            .ok_or(PackageError::InvalidBinding)?;
        match binding {
            Binding::None => {}
            Binding::Group(children) | Binding::Scope(children) => {
                for child in children.iter().rev() {
                    push(&mut pending, *child, budget)?;
                }
            }
            Binding::Visit(name) | Binding::Import(name) | Binding::Propagate(name) => {
                let (i, _) = field(fields, name, budget)?;
                visited[i] = true;
            }
            Binding::Bind { namespace, name }
            | Binding::Reference { namespace, name }
            | Binding::Export { namespace, name } => {
                budget.charge(Resource::Work, package.namespaces.len() as u64 + 1)?;
                if !package.namespaces.iter().any(|v| &v.name == namespace) {
                    return Err(PackageError::MissingNamespace);
                }
                match name {
                    NameSelector::SelfValue => {
                        if self_type != Some(&TypeDescriptor::Text) {
                            return Err(PackageError::InvalidBinding);
                        }
                    }
                    NameSelector::Field(name) => {
                        let (i, field) = field(fields, name, budget)?;
                        if !super::shape::name_read(package, field.read, budget)? {
                            return Err(PackageError::InvalidBinding);
                        }
                        visited[i] = true;
                    }
                }
            }
            Binding::Sequential { declarations, body }
            | Binding::Recursive { declarations, body } => {
                let (i, decl) = field(fields, declarations, budget)?;
                let (j, _) = field(fields, body, budget)?;
                if i == j
                    || !matches!(
                        super::shape::terminal(package, decl.read, budget)?,
                        ReadSpec::ListOf { .. }
                    )
                {
                    return Err(PackageError::InvalidBinding);
                }
                visited[i] = true;
                visited[j] = true;
            }
            Binding::Custom(operation) => {
                budget.charge(Resource::Work, package.extensions.len() as u64 + 1)?;
                if !package
                    .extensions
                    .iter()
                    .any(|v| &v.operation == operation && v.signature == "facts/v1")
                {
                    return Err(PackageError::MissingExtension);
                }
                visited.fill(true);
            }
        }
    }
    for (i, field) in fields.iter().enumerate() {
        if !visited[i] && !super::shape::literal(package, field.read, budget)? {
            return Err(PackageError::UnvisitedField);
        }
    }
    for style in styles {
        budget.charge(Resource::Work, 1)?;
        if style.class.name.is_empty() || registry.descriptor(&style.class.schema).is_none() {
            return Err(PackageError::InvalidSelector);
        }
        match &style.selector {
            StyleSelector::Head | StyleSelector::SelfValue => {}
            StyleSelector::Field(name) => {
                field(fields, name, budget)?;
            }
            StyleSelector::Capture(name) => {
                budget.charge(Resource::Work, package.reader.expressions.len() as u64)?;
                if !package.reader.expressions.iter().any(
                    |expr| matches!(expr,ReaderExpr::Capture{name:declared,..} if declared==name),
                ) {
                    return Err(PackageError::InvalidSelector);
                }
            }
        }
    }
    Ok(())
}
