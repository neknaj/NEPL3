use super::*;
mod provenance;
pub use provenance::{ForeignCapture, OwnerProvenance, ValidatedOwnerProvenance};

/// Standalone guest syntax with the explicitly selected owner environment.
/// Owner origins retain their original arena order and IDs; guest origins stay
/// in `syntax.bundle`. Keeping the complete owner origin table avoids changing
/// the selected environment's canonical digest. No owner AST is copied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignClosure {
    pub syntax: ForeignSyntax,
    pub owner_environment: EnvironmentEntry,
    pub provenance: OwnerProvenance,
}
pub struct ValidatedForeignClosure<'a> {
    value: &'a ForeignClosure,
    syntax: ValidatedSyntaxBundle<'a>,
}
impl<'a> ValidatedForeignClosure<'a> {
    pub fn value(&self) -> &'a ForeignClosure {
        self.value
    }
    /// Reuse the guest graph proof established by this closure validation.
    /// The immutable closure borrow protects the guest and its source tables.
    /// Domain semantics and canonical environment hashes require their own checks.
    pub fn syntax(&self) -> &ValidatedSyntaxBundle<'a> {
        &self.syntax
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
        self.provenance.validate(registry, b, admission)?;
        self.validate_contents(registry, b, admission)
    }

    fn validate_contents<'a>(
        &'a self,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedForeignClosure<'a>, SyntaxError> {
        b.poll()?;
        let syntax = self
            .syntax
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
        environment(
            &self.owner_environment.value,
            self.provenance.origins().len(),
            registry,
            b,
        )?;
        Ok(ValidatedForeignClosure {
            value: self,
            syntax,
        })
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
        for node in &owner.bundle().nodes {
            for field in &node.fields {
                b.charge(Resource::Work, 1)?;
                if let FieldValue::Foreign(value) = field
                    && core::ptr::eq(value.as_ref(), syntax)
                {
                    let provenance = OwnerProvenance::capture(owner.bundle(), b)?;
                    return Self::capture_selected(
                        syntax,
                        owner.bundle(),
                        provenance,
                        registry,
                        b,
                        admission,
                    );
                }
            }
        }
        Err(SyntaxError::Reference)
    }
    /// Capture the foreign field at a checked owner's node and field position.
    /// Selection performs one bounded lookup; it does not scan unrelated nodes.
    /// The complete owner provenance and closure validation are unchanged.
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
        let provenance = OwnerProvenance::capture(owner, b)?;
        Self::capture_selected(syntax, owner, provenance, registry, b, admission)
    }
    fn capture_selected(
        syntax: &ForeignSyntax,
        owner: &SyntaxBundle,
        provenance: OwnerProvenance,
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
            provenance,
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
