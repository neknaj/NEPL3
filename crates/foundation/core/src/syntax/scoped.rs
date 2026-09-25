//! Structural proof bound to the immutable registry used by validation.
use super::*;

/// Structural validation of an unchanged graph against an immutable registry.
/// Each receiving operation still admits the complete source closure and applies
/// the validation's relative depth to its current Budget. Environment digests
/// and language semantics remain the codec and language owner's responsibility.
pub struct RegistryValidatedSyntaxBundle<'s, 'r> {
    pub(super) syntax: ValidatedSyntaxBundle<'s>,
    pub(super) registry: &'r SchemaRegistry,
}

impl SyntaxBundle {
    /// Validate and retain the registry borrow required for later proof reuse.
    pub fn validate_in_registry<'s, 'r>(
        &'s self,
        registry: &'r SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<RegistryValidatedSyntaxBundle<'s, 'r>, SyntaxError> {
        Ok(RegistryValidatedSyntaxBundle {
            syntax: self.validate_with_sources(registry, budget, admission)?,
            registry,
        })
    }
}

impl<'s> RegistryValidatedSyntaxBundle<'s, '_> {
    pub fn bundle(&self) -> &'s SyntaxBundle {
        self.syntax.bundle()
    }

    /// Reuse graph checks only for the identical borrowed registry. A different
    /// registry receives full structural validation. Both paths charge this
    /// operation's source admission and depth; neither supplies ambient sources.
    pub fn validate_for(
        &self,
        registry: &SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), SyntaxError> {
        self.checked_for(registry, budget, admission).map(|_| ())
    }

    /// Obtain the structural proof for this receiving operation after applying
    /// its registry, depth and source admission. A changed registry validates
    /// afresh and returns that validation's own relative depth.
    pub fn checked_for(
        &self,
        registry: &SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedSyntaxBundle<'s>, SyntaxError> {
        budget.charge(Resource::Work, 1)?;
        if !core::ptr::eq(registry, self.registry) {
            return self
                .bundle()
                .validate_with_sources(registry, budget, admission);
        }
        budget.observe_depth(self.syntax.validation_depth)?;
        let mut pending = Vec::new();
        push_bundle(&mut pending, self.bundle(), budget)?;
        while let Some(bundle) = pending.pop() {
            budget.charge(Resource::Work, 1)?;
            for source in &bundle.sources {
                budget.charge(Resource::Work, 1)?;
                admission.admit_existing(source, budget)?;
            }
            // Validation covers every declared node, including disconnected
            // nodes. Their foreign bundles must retain the same admission scope.
            for node in &bundle.nodes {
                budget.charge(Resource::Work, 1)?;
                for field in &node.fields {
                    budget.charge(Resource::Work, 1)?;
                    if let FieldValue::Foreign(foreign) = field {
                        push_bundle(&mut pending, &foreign.bundle, budget)?;
                    }
                }
            }
        }
        Ok(ValidatedSyntaxBundle {
            bundle: self.syntax.bundle,
            owner_depth: self.syntax.owner_depth,
            validation_depth: self.syntax.validation_depth,
        })
    }
}

fn push_bundle<'s>(
    pending: &mut Vec<&'s SyntaxBundle>,
    bundle: &'s SyntaxBundle,
    budget: &mut Budget,
) -> Result<(), SyntaxError> {
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<&SyntaxBundle>() as u64,
    )?;
    pending
        .try_reserve(1)
        .map_err(|_| SyntaxError::Stopped(budget.stop(StopReason::AllocationLimit)))?;
    pending.push(bundle);
    Ok(())
}
