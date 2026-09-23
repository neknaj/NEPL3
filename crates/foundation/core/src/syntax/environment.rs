use super::*;

/// Environment values checked against an immutable registry and origin/source
/// context. The proof retains all inputs; origin IDs belong to this exact arena.
#[derive(Clone, Copy)]
pub struct ValidatedEnvironment<'a> {
    value: &'a Environment,
    origins: &'a [Origin],
    sources: &'a SourceStore,
    registry: &'a SchemaRegistry,
}

impl<'a> ValidatedEnvironment<'a> {
    pub fn value(&self) -> &'a Environment {
        self.value
    }
    pub fn origins(&self) -> &'a [Origin] {
        self.origins
    }
    pub fn sources(&self) -> &'a SourceStore {
        self.sources
    }
    pub fn registry(&self) -> &'a SchemaRegistry {
        self.registry
    }
    /// Validate another value within this exact immutable provenance context.
    /// Binding references and resources are checked again; the retained origin
    /// graph and source store need no repeated traversal.
    pub fn validate_value<'b>(
        &'b self,
        value: &'b Environment,
        budget: &mut Budget,
    ) -> Result<ValidatedEnvironment<'b>, SyntaxError> {
        budget.poll()?;
        super::foreign::environment(value, self.origins.len(), self.registry, budget)?;
        Ok(ValidatedEnvironment {
            value,
            origins: self.origins,
            sources: self.sources,
            registry: self.registry,
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
        super::foreign::environment(self, origins.len(), registry, budget)?;
        Ok(ValidatedEnvironment {
            value: self,
            origins,
            sources,
            registry,
        })
    }
}
