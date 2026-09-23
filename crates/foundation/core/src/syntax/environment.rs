use super::*;
mod index;
pub(super) use index::{EnvironmentIndex, resource_index};

/// Environment values checked against an immutable registry and origin/source
/// context. The proof retains all inputs; origin IDs belong to this exact arena.
pub struct ValidatedEnvironment<'a> {
    value: &'a Environment,
    context: EnvironmentContext<'a>,
    index: EnvironmentIndex,
}

/// Immutable, checked provenance and schema context. Values admitted through
/// this context still require binding, typed-value and resource validation.
#[derive(Clone, Copy)]
pub struct EnvironmentContext<'a> {
    origins: &'a [Origin],
    sources: &'a SourceStore,
    registry: &'a SchemaRegistry,
}

impl<'a> ValidatedEnvironment<'a> {
    pub fn value(&self) -> &'a Environment {
        self.value
    }
    pub fn origins(&self) -> &'a [Origin] {
        self.context.origins
    }
    pub fn sources(&self) -> &'a SourceStore {
        self.context.sources
    }
    pub fn registry(&self) -> &'a SchemaRegistry {
        self.context.registry
    }
    pub fn context(&self) -> EnvironmentContext<'a> {
        self.context
    }
    /// Search the retained index without changing declaration order.
    pub fn binding(
        &self,
        namespace: &NamespaceRef,
        name: &str,
        budget: &mut Budget,
    ) -> Result<Option<&'a EnvironmentBinding>, SyntaxError> {
        self.index.binding(self.value, namespace, name, budget)
    }
    pub fn resource(
        &self,
        id: &str,
        budget: &mut Budget,
    ) -> Result<Option<&'a ResourceContent>, SyntaxError> {
        self.index.resource(self.value, id, budget)
    }
    /// Validate another value within this exact immutable provenance context.
    /// Binding references and resources are checked again; the retained origin
    /// graph and source store need no repeated traversal.
    pub fn validate_value<'b>(
        &'b self,
        value: &'b Environment,
        budget: &mut Budget,
    ) -> Result<ValidatedEnvironment<'b>, SyntaxError> {
        self.context.validate_value(value, budget)
    }
}

impl<'a> EnvironmentContext<'a> {
    pub fn origins(&self) -> &'a [Origin] {
        self.origins
    }
    pub fn sources(&self) -> &'a SourceStore {
        self.sources
    }
    pub fn validate_value<'b>(
        &'b self,
        value: &'b Environment,
        budget: &mut Budget,
    ) -> Result<ValidatedEnvironment<'b>, SyntaxError> {
        budget.poll()?;
        let index = super::foreign::environment(value, self.origins.len(), self.registry, budget)?;
        Ok(ValidatedEnvironment {
            value,
            context: *self,
            index,
        })
    }
}

impl Environment {
    /// Check binding uniqueness, namespace identities, typed values, resource
    /// content and origin references. No environment digest is asserted here;
    /// the existing portable codec computes that identity when publishing it.
    pub fn validate<'a>(
        &'a self,
        origins: &'a [Origin],
        sources: &'a SourceStore,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<ValidatedEnvironment<'a>, SyntaxError> {
        budget.poll()?;
        if !registry.is_finalized() {
            return Err(SchemaError::Unfinalized.into());
        }
        OriginGraph::validate_origins(origins, sources, budget)?;
        let index = super::foreign::environment(self, origins.len(), registry, budget)?;
        Ok(ValidatedEnvironment {
            value: self,
            context: EnvironmentContext {
                origins,
                sources,
                registry,
            },
            index,
        })
    }
}
