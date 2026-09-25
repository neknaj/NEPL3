//! Owned structural and selection proofs, scoped to an immutable profile.
use super::{TreeError, validate_selections};
use crate::{profile::ResolvedParseProfile, recovery::*, selection::BundleContext};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    source::{Digest, SourceAdmission},
    syntax::{OwnedValidatedSyntaxBundle, ValidatedSyntaxBundle},
};

/// Retains structural and selection validation without copying the syntax graph.
/// This certifies the same conditions as `ParseTree::validate`. Domain semantics
/// and source admission into another operation's ledger remain separate checks.
pub struct OwnedValidatedParseTree<'p> {
    syntax: OwnedValidatedSyntaxBundle<'p>,
    profile: &'p ResolvedParseProfile<'p>,
    profile_digest: Digest,
    contexts: Vec<BundleContext>,
    recovery: Vec<BundleRecovery>,
}

/// A rejected conversion retains its complete input, including on resource stop.
#[derive(Debug)]
pub struct ParseTreeValidationFailure {
    pub error: TreeError,
    pub tree: ParseTree,
}

impl ParseTree {
    /// Validate and retain the graph and its selection indices. Failure preserves
    /// the input; the supplied budget and admission ledger retain their progress.
    #[allow(clippy::result_large_err)] // Recover storage without allocating on stop.
    pub fn try_into_validated<'p>(
        self,
        profile: &'p ResolvedParseProfile<'p>,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<OwnedValidatedParseTree<'p>, ParseTreeValidationFailure> {
        if let Err(error) = budget.charge(Resource::Work, 1) {
            return Err(ParseTreeValidationFailure {
                error: error.into(),
                tree: self,
            });
        }
        if self.profile_digest != profile.digest() {
            return Err(ParseTreeValidationFailure {
                error: TreeError::Selection,
                tree: self,
            });
        }
        let ParseTree {
            profile_digest,
            bundle,
            contexts,
            recovery,
        } = self;
        let syntax = match bundle.try_into_validated(profile.registry(), budget, admission) {
            Ok(syntax) => syntax,
            Err(failure) => {
                return Err(ParseTreeValidationFailure {
                    error: failure.error.into(),
                    tree: ParseTree {
                        profile_digest,
                        bundle: failure.bundle,
                        contexts,
                        recovery,
                    },
                });
            }
        };
        if let Err(error) =
            validate_selections(syntax.bundle(), &contexts, &recovery, profile, budget)
        {
            return Err(ParseTreeValidationFailure {
                error,
                tree: ParseTree {
                    profile_digest,
                    bundle: syntax.into_inner(),
                    contexts,
                    recovery,
                },
            });
        }
        Ok(OwnedValidatedParseTree {
            syntax,
            profile,
            profile_digest,
            contexts,
            recovery,
        })
    }
}

impl<'p> OwnedValidatedParseTree<'p> {
    pub fn syntax(&self) -> ValidatedSyntaxBundle<'_> {
        self.syntax.as_validated()
    }
    pub fn profile(&self) -> &'p ResolvedParseProfile<'p> {
        self.profile
    }
    pub fn contexts(&self) -> &[BundleContext] {
        &self.contexts
    }
    pub fn recovery(&self) -> &[BundleRecovery] {
        &self.recovery
    }
    pub fn is_recovered(&self) -> bool {
        self.recovery.iter().any(|entry| !entry.entries.is_empty())
    }
    /// Consuming the proof restores the raw, mutable representation.
    pub fn into_inner(self) -> ParseTree {
        ParseTree {
            profile_digest: self.profile_digest,
            bundle: self.syntax.into_inner(),
            contexts: self.contexts,
            recovery: self.recovery,
        }
    }
}
