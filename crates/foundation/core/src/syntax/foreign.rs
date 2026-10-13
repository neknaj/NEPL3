use super::*;
pub mod group;

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
/// Borrowed closed components. A view owns no data and carries no validation
/// proof; standalone and grouped owners expose the same explicit boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForeignClosureView<'a> {
    syntax: &'a ForeignSyntax,
    environment: &'a EnvironmentEntry,
    origins: &'a [Origin],
    sources: &'a [SourceSnapshot],
    source_maps: &'a [Mapping],
}
impl<'a> ForeignClosureView<'a> {
    pub fn syntax(&self) -> &'a ForeignSyntax {
        self.syntax
    }
    pub fn owner_environment(&self) -> &'a EnvironmentEntry {
        self.environment
    }
    pub fn owner_origins(&self) -> &'a [Origin] {
        self.origins
    }
    pub fn owner_sources(&self) -> &'a [SourceSnapshot] {
        self.sources
    }
    pub fn owner_source_maps(&self) -> &'a [Mapping] {
        self.source_maps
    }
    /// Fresh receiving-operation validation under the actual caller depth and
    /// current limits. A borrowed view itself carries no validation authority.
    pub fn validate(
        &self,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), SyntaxError> {
        b.poll()?;
        validate_syntax(self.syntax, registry, b, admission)?;
        association(self.syntax, self.environment)?;
        validate_provenance(self.origins, self.sources, self.source_maps, b, admission)?;
        environment(&self.environment.value, self.origins.len(), registry, b)?;
        Ok(())
    }
    /// Explicitly copy a standalone closure. No source/semantic validity is
    /// inferred here; callers validate at their receiving operation boundary.
    pub fn materialize(&self, b: &mut Budget) -> Result<ForeignClosure, StopReason> {
        b.poll()?;
        let owner = group::copy_provenance(self.origins, self.sources, self.source_maps, b)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<ForeignClosure>() as u64,
        )?;
        Ok(ForeignClosure {
            syntax: clone_syntax(self.syntax, b)?,
            owner_environment: self.environment.clone_with_budget(b)?,
            owner_origins: owner.origins,
            owner_sources: owner.sources,
            owner_source_maps: owner.source_maps,
        })
    }
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
    pub fn view(&self) -> ForeignClosureView<'_> {
        ForeignClosureView {
            syntax: &self.syntax,
            environment: &self.owner_environment,
            origins: &self.owner_origins,
            sources: &self.owner_sources,
            source_maps: &self.owner_source_maps,
        }
    }

    /// Checks the two distinct origin arenas and declared source closures.
    /// Like SyntaxBundle's native graph proof, this checks environment identity
    /// equality; the portable codec additionally recomputes its canonical hash.
    pub fn validate<'a>(
        &'a self,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedForeignClosure<'a>, SyntaxError> {
        self.view().validate(registry, b, admission)?;
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
        Self::capture_selected(syntax, owner, registry, b, admission)
    }
    /// Capture a foreign field by its position in this checked owner. Selection
    /// uses bounded lookups; owner provenance and guest validation are preserved.
    pub fn capture_at(
        owner: &ValidatedSyntaxBundle<'_>,
        node: NodeRef,
        field: usize,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, SyntaxError> {
        b.charge(Resource::Work, 1)?;
        let owner = owner.bundle();
        let Some(FieldValue::Foreign(syntax)) = owner.node(node)?.fields.get(field) else {
            return Err(SyntaxError::Reference);
        };
        Self::capture_selected(syntax, owner, registry, b, admission)
    }
    fn capture_selected(
        syntax: &ForeignSyntax,
        owner: &SyntaxBundle,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, SyntaxError> {
        b.charge(Resource::Work, owner.environments.len() as u64 + 1)?;
        let environment = owner
            .environments
            .iter()
            .find(|e| e.id == syntax.environment.id && e.digest == syntax.environment.digest)
            .ok_or(SyntaxError::Environment)?;
        let provenance =
            group::copy_provenance(&owner.origins, &owner.sources, &owner.source_maps, b)?;
        let result = Self {
            syntax: clone_syntax_with_allocation(syntax, core::mem::size_of::<Self>() as u64, b)?,
            owner_environment: environment.clone_with_budget(b)?,
            owner_origins: provenance.origins,
            owner_sources: provenance.sources,
            owner_source_maps: provenance.source_maps,
        };
        result.validate(registry, b, admission)?;
        Ok(result)
    }
}
fn clone_syntax(syntax: &ForeignSyntax, b: &mut Budget) -> Result<ForeignSyntax, StopReason> {
    clone_syntax_with_allocation(syntax, 0, b)
}
fn clone_syntax_with_allocation(
    syntax: &ForeignSyntax,
    extra_allocation: u64,
    b: &mut Budget,
) -> Result<ForeignSyntax, StopReason> {
    let bytes = (syntax.schema.package.len() + syntax.category.len()) as u64;
    b.charge(Resource::Work, bytes + 40)?;
    b.charge(Resource::AllocationUnits, bytes + extra_allocation)?;
    Ok(ForeignSyntax {
        schema: syntax.schema.clone(),
        category: syntax.category.clone(),
        root: syntax.root,
        bundle: syntax.bundle.clone_with_budget(b)?,
        environment: syntax.environment.clone(),
    })
}
fn association(syntax: &ForeignSyntax, owner: &EnvironmentEntry) -> Result<(), SyntaxError> {
    if syntax.environment.id != owner.id || syntax.environment.digest != owner.digest {
        return Err(SyntaxError::Environment);
    }
    Ok(())
}
fn validate_syntax(
    syntax: &ForeignSyntax,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), SyntaxError> {
    syntax
        .bundle
        .validate_with_sources(registry, b, admission)?;
    require_schema(registry, &syntax.schema)?;
    b.charge(Resource::Work, syntax.category.len() as u64 + 40)?;
    if syntax.category.is_empty()
        || syntax.root != syntax.bundle.root
        || syntax.bundle.node(syntax.root)?.schema != syntax.schema
    {
        return Err(SyntaxError::ForeignRoot);
    }
    Ok(())
}
fn validate_provenance(
    origins: &[Origin],
    sources: &[SourceSnapshot],
    maps: &[Mapping],
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), SyntaxError> {
    let mut store = SourceStore::default();
    for source in sources {
        admission.admit_existing(source, b)?;
        if store
            .get_revision_with_budget(&source.identity().source, source.identity().revision, b)?
            .is_some()
        {
            return Err(SyntaxError::DuplicateSource);
        }
        store.insert_with_budget(source.clone_with_budget(b)?, b)?;
    }
    OriginGraph::validate_origins(origins, &store, b)?;
    SourceMap::validate_mappings(maps, &store, b)?;
    Ok(())
}
pub(super) fn environment(
    value: &Environment,
    origins: usize,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<super::environment::EnvironmentIndex, SyntaxError> {
    b.poll()?;
    for binding in &value.bindings {
        b.charge(
            Resource::Work,
            (binding.name.len() + binding.namespace.name.len()) as u64 + 1,
        )?;
        require_schema(registry, &binding.namespace.schema)?;
        if binding.name.is_empty() || binding.namespace.name.is_empty() {
            return Err(SyntaxError::Environment);
        }
        if binding.origin.is_some_and(|id| id.0 >= origins as u64) {
            return Err(SyntaxError::Reference);
        }
        registry.validate_typed(&binding.value, b)?;
    }
    resource_contents(&value.resources, b)?;
    super::environment::EnvironmentIndex::new(value, b)
}

/// Verify content digests and unique, nonempty resource identities.
pub fn resources(values: &[ResourceContent], b: &mut Budget) -> Result<(), SyntaxError> {
    resource_contents(values, b)?;
    super::environment::resource_index(values, b)?;
    Ok(())
}

fn resource_contents(values: &[ResourceContent], b: &mut Budget) -> Result<(), SyntaxError> {
    b.poll()?;
    for resource in values {
        b.charge(
            Resource::Work,
            (resource.bytes.len() + resource.id.len()) as u64 + 1,
        )?;
        if resource.id.is_empty() || resource.digest != Digest::of(&resource.bytes) {
            return Err(SyntaxError::ResourceDigest);
        }
    }
    Ok(())
}
