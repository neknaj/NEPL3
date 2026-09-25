//! Immutable document and guest proofs shared by native consumers.
use super::{StructureError, ValidatedDocumentSyntax};
use crate::model::{DocContent, DocumentSyntax};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::SourceAdmission,
    syntax::RegistryValidatedSyntaxBundle,
};

/// Complete Doc structure validation bound to its immutable registry. Guest
/// syntax proofs follow Syntax-embed order; typed-value embeds have no entry.
/// This grants no language semantics, label resolution or rendering authority.
/// Receiving operations call `validate_for` before reusing structural checks.
pub struct RegistryValidatedDocumentSyntax<'d, 'r> {
    pub(crate) structure: ValidatedDocumentSyntax<'d>,
    pub(crate) registry: &'r SchemaRegistry,
    pub(crate) syntax: Vec<RegistryValidatedSyntaxBundle<'d, 'r>>,
    depth: u64,
}

impl<'d: 'r, 'r> RegistryValidatedDocumentSyntax<'d, 'r> {
    pub fn new(
        document: &'d DocumentSyntax,
        registry: &'r SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, StructureError> {
        let mut syntax = Vec::new();
        let (structure, depth) = budget.measure_depth(|budget| {
            document.validate_structure_with_syntax(registry, budget, admission, |proof, budget| {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<RegistryValidatedSyntaxBundle<'_, '_>>() as u64,
                )?;
                syntax.try_reserve(1).map_err(|_| {
                    StructureError::Stopped(budget.stop(StopReason::AllocationLimit))
                })?;
                syntax.push(proof);
                Ok(())
            })
        })?;
        Ok(Self {
            structure,
            registry,
            syntax,
            depth,
        })
    }

    pub fn structure(&self) -> &ValidatedDocumentSyntax<'d> {
        &self.structure
    }

    pub fn syntax(&self) -> &[RegistryValidatedSyntaxBundle<'d, 'r>] {
        &self.syntax
    }

    /// Apply the receiver's source admission and relative depth. A different
    /// registry performs complete validation. No ambient source completes a
    /// missing declaration, and all owner and nested guest sources are admitted.
    pub fn validate_for(
        &self,
        registry: &SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), StructureError> {
        budget.charge(Resource::Work, 1)?;
        let document = self.structure.document();
        if !core::ptr::eq(registry, self.registry) {
            return document
                .validate_structure(registry, budget, admission)
                .map(|_| ());
        }
        budget.observe_depth(self.depth)?;
        for source in &document.sources {
            budget.charge(Resource::Work, 1)?;
            admission.admit_existing(source, budget)?;
        }
        for embed in &document.value.embeds {
            budget.charge(Resource::Work, 1)?;
            if let DocContent::Syntax { closure } = &embed.content {
                for source in closure.provenance.sources() {
                    budget.charge(Resource::Work, 1)?;
                    admission.admit_existing(source, budget)?;
                }
            }
        }
        for syntax in &self.syntax {
            syntax.validate_for(registry, budget, admission)?;
        }
        budget.poll()?;
        Ok(())
    }
}
