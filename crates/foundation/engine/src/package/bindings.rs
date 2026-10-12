mod graph;
use super::*;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    lexical::equal_with_budget,
    schema::{SchemaRegistry, TypeDescriptor},
};
use nepl3_reader::plan::ReaderExpr;

fn field<'a>(
    fields: &'a [FieldSpec],
    name: &str,
    budget: &mut Budget,
) -> Result<(usize, &'a FieldSpec), PackageError> {
    for (index, field) in fields.iter().enumerate() {
        if equal_with_budget(&field.name, name, budget)? {
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
    let mut at = None;
    let result = graph::Cache::new(package, budget, &mut at);
    let mut cache = result.inspect_err(|_| {
        *subject = Some(PackageSubject::Binding {
            owner: None,
            binding: at,
        });
    })?;
    graph::walk(
        package,
        (0..package.bindings.len()).map(|i| BindingId(i as u64)),
        &mut cache,
        budget,
        &mut at,
        |_, _| Ok(()),
    )
    .inspect_err(|_| {
        *subject = Some(PackageSubject::Binding {
            owner: None,
            binding: at,
        });
    })?;
    for owner in (0..package.forms.len())
        .map(|i| BindingOwner::Form(i as u64))
        .chain((0..package.leaves.len()).map(|i| BindingOwner::Leaf(i as u64)))
    {
        *subject = Some(PackageSubject::Binding {
            owner: Some(owner),
            binding: None,
        });
        if let Err(failure) =
            package.check_binding_owner_cached(owner, registry, budget, Some(&mut cache))
        {
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
    /// Only the selected root is traversed. Shared descendants are validated
    /// once for this owner; active cycles return DirectCycle and the common
    /// caller-relative depth ceiling applies even without prior package checks.
    pub fn check_binding_owner(
        &self,
        owner: BindingOwner,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), BindingFailure> {
        self.check_binding_owner_cached(owner, registry, budget, None)
    }
    fn check_binding_owner_cached(
        &self,
        owner: BindingOwner,
        registry: &SchemaRegistry,
        budget: &mut Budget,
        cache: Option<&mut graph::Cache>,
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
        match cache {
            Some(cache) => detailed_cached(self, selected, registry, budget, cache),
            None => detailed(self, selected, registry, budget),
        }
    }
}
fn detailed(
    package: &LanguagePackage,
    owner: Owner<'_>,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), BindingFailure> {
    let mut binding = None;
    let mut cache = graph::Cache::new(package, budget, &mut binding)
        .map_err(|error| BindingFailure { error, binding })?;
    detailed_cached(package, owner, registry, budget, &mut cache)
}
fn detailed_cached(
    package: &LanguagePackage,
    owner: Owner<'_>,
    registry: &SchemaRegistry,
    budget: &mut Budget,
    cache: &mut graph::Cache,
) -> Result<(), BindingFailure> {
    let mut binding = None;
    owner_inner(package, owner, registry, budget, &mut binding, cache)
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
    cache: &mut graph::Cache,
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
    graph::walk(
        package,
        core::iter::once(root),
        cache,
        budget,
        at,
        |binding, budget| {
            match binding {
                Binding::None => {}
                Binding::Group(_) | Binding::Scope(_) => {}
                Binding::Visit(name) | Binding::Import(name) | Binding::Propagate(name) => {
                    let (i, _) = field(fields, name, budget)?;
                    visited[i] = true;
                }
                Binding::Bind { namespace, name }
                | Binding::Reference { namespace, name }
                | Binding::Export { namespace, name } => {
                    let mut found = false;
                    for candidate in &package.namespaces {
                        if equal_with_budget(&candidate.name, namespace, budget)? {
                            found = true;
                            break;
                        }
                    }
                    if !found {
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
            Ok(())
        },
    )?;
    *at = None;
    for (i, field) in fields.iter().enumerate() {
        if !visited[i] && !super::shape::literal(package, field.read, budget)? {
            return Err(PackageError::UnvisitedField);
        }
    }
    for style in styles {
        budget.charge(Resource::Work, 1)?;
        if style.class.name.is_empty()
            || registry
                .descriptor_with_budget(&style.class.schema, budget)?
                .is_none()
        {
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
