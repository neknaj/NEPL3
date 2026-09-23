//! Explicit native environment selection for a host-owned bridge.
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    origin::Origin,
    schema::{SchemaError, TypeDescriptor},
    source::SourceStore,
    syntax::{
        Environment, EnvironmentBinding, NamespaceRef, ResourceContent, SyntaxError,
        ValidatedEnvironment,
    },
    value::SchemaRef,
};

/// Select one source binding and publish its unchanged typed value under an
/// explicit guest name. Conversion of language values is a separate operation.
pub struct BindingProjection<'a> {
    pub namespace: &'a NamespaceRef,
    pub name: &'a str,
    pub target_namespace: &'a NamespaceRef,
    pub target_name: &'a str,
    pub expected: &'a TypeDescriptor,
}

#[derive(Debug)]
pub enum ProjectionError {
    Stopped(StopReason),
    Schema(SchemaError),
    Syntax(SyntaxError),
    MissingBinding,
    MissingResource,
}
impl From<StopReason> for ProjectionError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
impl From<SchemaError> for ProjectionError {
    fn from(value: SchemaError) -> Self {
        match value {
            SchemaError::Stopped(reason) => Self::Stopped(reason),
            value => Self::Schema(value),
        }
    }
}
impl From<SyntaxError> for ProjectionError {
    fn from(value: SyntaxError) -> Self {
        match value.stop_reason() {
            Some(reason) => Self::Stopped(reason),
            None => Self::Syntax(value),
        }
    }
}

/// Owned selected values with their original immutable provenance context.
/// Unselected origin entries remain in the borrowed arena so IDs stay stable.
/// A host must explicitly grant sources for each operation through Grants.
pub struct ProjectedEnvironment<'a> {
    value: Environment,
    context: ValidatedEnvironment<'a>,
}
impl ProjectedEnvironment<'_> {
    pub fn value(&self) -> &Environment {
        &self.value
    }
    pub fn origins(&self) -> &[Origin] {
        self.context.origins()
    }
    pub fn sources(&self) -> &SourceStore {
        self.context.sources()
    }
    pub fn validate(&self, budget: &mut Budget) -> Result<ValidatedEnvironment<'_>, SyntaxError> {
        self.context.validate_value(&self.value, budget)
    }
}

fn text(value: &str, budget: &mut Budget) -> Result<String, StopReason> {
    budget.charge(Resource::Work, value.len() as u64)?;
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(value.into())
}

/// Project only explicitly named bindings/resources. Empty selections produce
/// an empty guest environment. Failure leaves the source unchanged and returns
/// no partial environment. All lookup, copy and validation work is budgeted.
pub fn project<'a>(
    source: &ValidatedEnvironment<'a>,
    bindings: &[BindingProjection<'_>],
    resources: &[&str],
    budget: &mut Budget,
) -> Result<ProjectedEnvironment<'a>, ProjectionError> {
    budget.poll()?;
    let mut selected = Vec::new();
    for rule in bindings {
        let mut found = None;
        for binding in &source.value().bindings {
            let width = (binding.name.len() as u64)
                .saturating_add(rule.name.len() as u64)
                .saturating_add(binding.namespace.name.len() as u64)
                .saturating_add(rule.namespace.name.len() as u64)
                .saturating_add(binding.namespace.schema.package.len() as u64)
                .saturating_add(rule.namespace.schema.package.len() as u64)
                .saturating_add(40);
            budget.charge(Resource::Work, width)?;
            if binding.namespace == *rule.namespace && binding.name == rule.name {
                found = Some(binding);
                break;
            }
        }
        let binding = found.ok_or(ProjectionError::MissingBinding)?;
        source
            .registry()
            .validate_typed_as(rule.expected, &binding.value, budget)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<EnvironmentBinding>() as u64,
        )?;
        let schema = SchemaRef {
            package: text(&rule.target_namespace.schema.package, budget)?,
            revision: rule.target_namespace.schema.revision,
            digest: rule.target_namespace.schema.digest,
        };
        selected.push(EnvironmentBinding {
            namespace: NamespaceRef {
                schema,
                name: text(&rule.target_namespace.name, budget)?,
            },
            name: text(rule.target_name, budget)?,
            value: binding.value.clone_with_budget(budget)?,
            origin: binding.origin,
        });
    }
    let mut selected_resources = Vec::new();
    for id in resources {
        let mut found = None;
        for resource in &source.value().resources {
            budget.charge(
                Resource::Work,
                (resource.id.len() as u64)
                    .saturating_add(id.len() as u64)
                    .saturating_add(1),
            )?;
            if resource.id == *id {
                found = Some(resource);
                break;
            }
        }
        let resource = found.ok_or(ProjectionError::MissingResource)?;
        budget.charge(Resource::Work, resource.bytes.len() as u64)?;
        budget.charge(
            Resource::AllocationUnits,
            (core::mem::size_of::<ResourceContent>() as u64)
                .saturating_add(resource.bytes.len() as u64),
        )?;
        selected_resources.push(ResourceContent {
            id: text(&resource.id, budget)?,
            digest: resource.digest,
            bytes: resource.bytes.clone(),
        });
    }
    let value = Environment {
        bindings: selected,
        resources: selected_resources,
    };
    source.validate_value(&value, budget)?;
    Ok(ProjectedEnvironment {
        value,
        context: *source,
    })
}
