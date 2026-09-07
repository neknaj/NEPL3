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
    for (index, field) in fields.iter().enumerate() {
        budget.charge(Resource::Work, (field.name.len() + name.len()) as u64 + 1)?;
        if field.name == name {
            return Ok((index, field));
        }
    }
    Err(PackageError::InvalidSelector)
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
    subject: &mut Option<PackageSubject>,
) -> Result<(), PackageError> {
    // Reject every direct binding cycle, including currently unused declarations.
    for root in 0..package.bindings.len() {
        let mut pending = Vec::new();
        let mut path = Vec::new();
        push(&mut pending, (BindingId(root as u64), false), budget)?;
        while let Some((id, exit)) = pending.pop() {
            *subject = Some(PackageSubject::Binding {
                owner: None,
                binding: Some(id),
            });
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
    for owner in (0..package.forms.len())
        .map(|i| BindingOwner::Form(i as u64))
        .chain((0..package.leaves.len()).map(|i| BindingOwner::Leaf(i as u64)))
    {
        *subject = Some(PackageSubject::Binding {
            owner: Some(owner),
            binding: None,
        });
        if let Err(failure) = package.check_binding_owner(owner, registry, budget) {
            *subject = Some(PackageSubject::Binding {
                owner: Some(owner),
                binding: failure.binding,
            });
            return Err(failure.error);
        }
    }
    Ok(())
}
/// The concrete declaration whose existing binding rules are being checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingOwner {
    Form(u64),
    Leaf(u64),
}
#[derive(Debug, Eq, PartialEq)]
pub struct BindingFailure {
    pub error: PackageError,
    pub binding: Option<BindingId>,
}
struct Owner<'a> {
    fields: &'a [FieldSpec],
    self_type: Option<&'a TypeDescriptor>,
    root: BindingId,
    styles: &'a [StyleRule],
    selection_rules: &'a [SelectionRule],
}
impl LanguagePackage {
    /// Check one presentation selector using the same owner rules as package validation.
    /// Compilers may use this entry to attribute a failure to its source operand.
    pub fn check_presentation_selector(
        &self,
        owner: BindingOwner,
        selector: &StyleSelector,
        budget: &mut Budget,
    ) -> Result<(), PackageError> {
        let fields = match owner {
            BindingOwner::Form(i) => {
                &self
                    .forms
                    .get(usize::try_from(i).map_err(|_| PackageError::InvalidSelector)?)
                    .ok_or(PackageError::InvalidSelector)?
                    .fields[..]
            }
            BindingOwner::Leaf(i) => {
                self.leaves
                    .get(usize::try_from(i).map_err(|_| PackageError::InvalidSelector)?)
                    .ok_or(PackageError::InvalidSelector)?;
                &[]
            }
        };
        check_selector(self, fields, selector, budget)
    }
    /// Run the same binding validator used by `check`, retaining the failing
    /// binding arena index so source compilers can attribute their own AST.
    pub fn check_binding_owner(
        &self,
        owner: BindingOwner,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), BindingFailure> {
        let selected = match owner {
            BindingOwner::Form(index) => self
                .forms
                .get(usize::try_from(index).map_err(|_| BindingFailure {
                    error: PackageError::InvalidBinding,
                    binding: None,
                })?)
                .map(|v| Owner {
                    fields: &v.fields,
                    self_type: None,
                    root: v.binding,
                    styles: &v.styles,
                    selection_rules: &v.selection_rules,
                }),
            BindingOwner::Leaf(index) => self
                .leaves
                .get(usize::try_from(index).map_err(|_| BindingFailure {
                    error: PackageError::InvalidBinding,
                    binding: None,
                })?)
                .map(|v| Owner {
                    fields: &[],
                    self_type: Some(&v.payload),
                    root: v.binding,
                    styles: &v.styles,
                    selection_rules: &v.selection_rules,
                }),
        }
        .ok_or(BindingFailure {
            error: PackageError::InvalidBinding,
            binding: None,
        })?;
        detailed(self, selected, registry, budget)
    }
}
fn detailed(
    package: &LanguagePackage,
    owner: Owner<'_>,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), BindingFailure> {
    let mut binding = None;
    owner_inner(package, owner, registry, budget, &mut binding)
        .map_err(|error| BindingFailure { error, binding })
}
pub(super) fn owner(
    package: &LanguagePackage,
    fields: &[FieldSpec],
    self_type: Option<&TypeDescriptor>,
    root: BindingId,
    presentation: (&[StyleRule], &[SelectionRule]),
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), PackageError> {
    detailed(
        package,
        Owner {
            fields,
            self_type,
            root,
            styles: presentation.0,
            selection_rules: presentation.1,
        },
        registry,
        budget,
    )
    .map_err(|failure| failure.error)
}
fn owner_inner(
    package: &LanguagePackage,
    owner: Owner<'_>,
    registry: &SchemaRegistry,
    budget: &mut Budget,
    at: &mut Option<BindingId>,
) -> Result<(), PackageError> {
    let Owner {
        fields,
        self_type,
        root,
        styles,
        selection_rules,
    } = owner;
    budget.charge(Resource::AllocationUnits, fields.len() as u64)?;
    let mut visited = alloc::vec![false;fields.len()];
    let mut pending = Vec::new();
    push(&mut pending, root, budget)?;
    while let Some(id) = pending.pop() {
        *at = Some(id);
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
    *at = None;
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
        check_selector(package, fields, &style.selector, budget)?;
    }
    for (index, rule) in selection_rules.iter().enumerate() {
        check_selector(package, fields, &rule.selector, budget)?;
        for prior in &selection_rules[..index] {
            let size = |v: &StyleSelector| match v {
                StyleSelector::Field(v) | StyleSelector::Capture(v) => v.len(),
                _ => 0,
            };
            budget.charge(
                Resource::Work,
                (size(&prior.selector) + size(&rule.selector)) as u64 + 1,
            )?;
            if prior.selector == rule.selector {
                return Err(PackageError::InvalidSelector);
            }
        }
    }
    Ok(())
}

fn check_selector(
    package: &LanguagePackage,
    fields: &[FieldSpec],
    selector: &StyleSelector,
    budget: &mut Budget,
) -> Result<(), PackageError> {
    budget.charge(Resource::Work, 1)?;
    match selector {
        StyleSelector::Head | StyleSelector::SelfValue => {}
        StyleSelector::Field(name) => {
            field(fields, name, budget)?;
        }
        StyleSelector::Capture(name) => {
            let mut found = false;
            for expression in &package.reader.expressions {
                budget.charge(Resource::Work, 1)?;
                if let ReaderExpr::Capture { name: declared, .. } = expression {
                    budget.charge(Resource::Work, (name.len() + declared.len()) as u64)?;
                    found |= name == declared;
                }
            }
            if !found {
                return Err(PackageError::InvalidSelector);
            }
        }
    }
    Ok(())
}
