use super::*;

/// Standalone guest syntax with the explicitly selected owner environment.
/// Owner origins retain their original arena order and IDs; guest origins stay
/// in `syntax.bundle`. Keeping the complete owner origin table avoids changing
/// the selected environment's canonical digest. No owner AST is copied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignClosure {
    pub syntax: ForeignSyntax,
    pub owner_environment: EnvironmentEntry,
    pub owner_origins: Vec<Origin>,
    pub owner_sources: Vec<SourceSnapshot>,
    pub owner_source_maps: Vec<Mapping>,
}
pub struct ValidatedForeignClosure<'a> {
    value: &'a ForeignClosure,
}
impl<'a> ValidatedForeignClosure<'a> {
    pub fn value(&self) -> &'a ForeignClosure {
        self.value
    }
}
impl ForeignClosure {
    /// Checks the two distinct origin arenas and declared source closures.
    /// Like SyntaxBundle's native graph proof, this checks environment identity
    /// equality; the portable codec additionally recomputes its canonical hash.
    pub fn validate<'a>(
        &'a self,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedForeignClosure<'a>, SyntaxError> {
        b.poll()?;
        self.syntax
            .bundle
            .validate_with_sources(registry, b, admission)?;
        require_schema(registry, &self.syntax.schema)?;
        b.charge(Resource::Work, self.syntax.category.len() as u64 + 40)?;
        if self.syntax.category.is_empty()
            || self.syntax.root != self.syntax.bundle.root
            || self.syntax.bundle.node(self.syntax.root)?.schema != self.syntax.schema
        {
            return Err(SyntaxError::ForeignRoot);
        }
        if self.syntax.environment.id != self.owner_environment.id
            || self.syntax.environment.digest != self.owner_environment.digest
        {
            return Err(SyntaxError::Environment);
        }
        let mut store = SourceStore::default();
        for (index, source) in self.owner_sources.iter().enumerate() {
            admission.admit_existing(source, b)?;
            for prior in &self.owner_sources[..index] {
                b.charge(
                    Resource::Work,
                    (prior.identity().source.0.len() + source.identity().source.0.len()) as u64
                        + 40,
                )?;
                if prior.identity().source == source.identity().source
                    && prior.identity().revision == source.identity().revision
                {
                    return Err(SyntaxError::DuplicateSource);
                }
            }
            store.insert_with_budget(source.clone_with_budget(b)?, b)?;
        }
        OriginGraph::validate_origins(&self.owner_origins, &store, b)?;
        SourceMap::validate_mappings(&self.owner_source_maps, &store, b)?;
        environment(
            &self.owner_environment.value,
            self.owner_origins.len(),
            registry,
            b,
        )?;
        Ok(ValidatedForeignClosure { value: self })
    }
    /// Capture a foreign field of this exact checked owner. Tables retain owner
    /// IDs and are copied with a budget; the owner's syntax nodes are not copied.
    pub fn capture(
        syntax: &ForeignSyntax,
        owner: &ValidatedSyntaxBundle<'_>,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, SyntaxError> {
        let owner = owner.bundle();
        let mut belongs = false;
        for node in &owner.nodes {
            for field in &node.fields {
                b.charge(Resource::Work, 1)?;
                if let FieldValue::Foreign(value) = field
                    && core::ptr::eq(value.as_ref(), syntax)
                {
                    belongs = true;
                }
            }
        }
        if !belongs {
            return Err(SyntaxError::Reference);
        }
        b.charge(Resource::Work, owner.environments.len() as u64 + 1)?;
        let environment = owner
            .environments
            .iter()
            .find(|e| e.id == syntax.environment.id && e.digest == syntax.environment.digest)
            .ok_or(SyntaxError::Environment)?;
        let mut origins = Vec::new();
        for origin in &owner.origins {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Origin>() as u64,
            )?;
            origins.push(origin.clone_with_budget(b)?);
        }
        let mut sources = Vec::new();
        for source in &owner.sources {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<SourceSnapshot>() as u64,
            )?;
            sources.push(source.clone_with_budget(b)?);
        }
        let mut maps = Vec::new();
        for map in &owner.source_maps {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Mapping>() as u64,
            )?;
            maps.push(map.clone_with_budget(b)?);
        }
        let bytes = (syntax.schema.package.len() + syntax.category.len()) as u64;
        b.charge(Resource::Work, bytes + 40)?;
        b.charge(
            Resource::AllocationUnits,
            bytes + core::mem::size_of::<Self>() as u64,
        )?;
        let result = Self {
            syntax: ForeignSyntax {
                schema: syntax.schema.clone(),
                category: syntax.category.clone(),
                root: syntax.root,
                bundle: syntax.bundle.clone_with_budget(b)?,
                environment: syntax.environment.clone(),
            },
            owner_environment: environment.clone_with_budget(b)?,
            owner_origins: origins,
            owner_sources: sources,
            owner_source_maps: maps,
        };
        result.validate(registry, b, admission)?;
        Ok(result)
    }
}
pub(super) fn environment(
    value: &Environment,
    origins: usize,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), SyntaxError> {
    for (index, binding) in value.bindings.iter().enumerate() {
        b.charge(
            Resource::Work,
            (binding.name.len() + binding.namespace.name.len()) as u64 + 1,
        )?;
        require_schema(registry, &binding.namespace.schema)?;
        if binding.name.is_empty() || binding.namespace.name.is_empty() {
            return Err(SyntaxError::Environment);
        }
        for prior in &value.bindings[..index] {
            b.charge(
                Resource::Work,
                (prior.name.len()
                    + binding.name.len()
                    + prior.namespace.name.len()
                    + binding.namespace.name.len()
                    + prior.namespace.schema.package.len()
                    + binding.namespace.schema.package.len()) as u64
                    + 40,
            )?;
            if prior.namespace == binding.namespace && prior.name == binding.name {
                return Err(SyntaxError::Environment);
            }
        }
        if binding.origin.is_some_and(|id| id.0 >= origins as u64) {
            return Err(SyntaxError::Reference);
        }
        registry.validate_typed(&binding.value, b)?;
    }
    for (index, resource) in value.resources.iter().enumerate() {
        b.charge(
            Resource::Work,
            (resource.bytes.len() + resource.id.len()) as u64 + 1,
        )?;
        if resource.id.is_empty() || resource.digest != Digest::of(&resource.bytes) {
            return Err(SyntaxError::ResourceDigest);
        }
        for prior in &value.resources[..index] {
            b.charge(
                Resource::Work,
                (prior.id.len() + resource.id.len()) as u64 + 1,
            )?;
            if prior.id == resource.id {
                return Err(SyntaxError::ResourceDigest);
            }
        }
    }
    Ok(())
}
