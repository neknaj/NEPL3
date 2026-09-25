//! Owned structural validation without cloning the source or syntax graph.
use super::*;

/// An immutable syntax graph validated against the borrowed registry.
///
/// This proves the same reference/source geometry contract as
/// `ValidatedSyntaxBundle`. Domain semantics and environment content digests
/// remain the consumer's responsibility. This native proof has no wire form.
/// Taking the raw bundle consumes the proof; further edits require validation.
/// Admission and resource charges belong to the validation operation. A caller
/// starting a separate operation must admit the sources into its own ledger.
#[derive(Debug)]
pub struct OwnedValidatedSyntaxBundle<'r> {
    bundle: SyntaxBundle,
    registry: &'r SchemaRegistry,
    owner_depth: u64,
}

/// Validation failure retains the exact owned graph, including partial source
/// information. Returning it requires no allocation after a resource stop.
#[derive(Debug)]
pub struct SyntaxValidationFailure {
    pub error: SyntaxError,
    pub bundle: SyntaxBundle,
}

impl SyntaxBundle {
    /// Validate once, then transfer the unchanged graph into an immutable owner.
    /// Failure leaves all input vectors available to the caller and preserves
    /// charges already made to the supplied Budget and SourceAdmission.
    #[allow(clippy::result_large_err)] // Keep the owned graph without boxing on a stop.
    pub fn try_into_validated<'r>(
        self,
        registry: &'r SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<OwnedValidatedSyntaxBundle<'r>, SyntaxValidationFailure> {
        match self.validate_with_sources(registry, budget, admission) {
            Ok(proof) => {
                let owner_depth = proof.owner_depth;
                Ok(OwnedValidatedSyntaxBundle {
                    bundle: self,
                    registry,
                    owner_depth,
                })
            }
            Err(error) => Err(SyntaxValidationFailure {
                error,
                bundle: self,
            }),
        }
    }
}

impl OwnedValidatedSyntaxBundle<'_> {
    /// Borrow the established proof. No table is copied or revalidated.
    pub fn as_validated(&self) -> ValidatedSyntaxBundle<'_> {
        ValidatedSyntaxBundle {
            bundle: &self.bundle,
            owner_depth: self.owner_depth,
        }
    }

    /// Registry held immutable for the complete lifetime of this owner.
    pub fn registry(&self) -> &SchemaRegistry {
        self.registry
    }

    pub fn bundle(&self) -> &SyntaxBundle {
        &self.bundle
    }

    /// Consume the proof to recover editable/transportable raw data.
    pub fn into_inner(self) -> SyntaxBundle {
        self.bundle
    }
}
