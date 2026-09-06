//! Recovery remains explicit and bundle-local; these nodes are never domain-checked values.
use crate::package::EntryContext;
use crate::package::{LanguagePackage, PackageError};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
};
use nepl3_core::{
    source::Span,
    syntax::{NodeRef, SyntaxBundle, TokenRef},
    value::KindRef,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnexpectedPolicy {
    ConsumeToken,
    PreserveRemainder,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncToken {
    pub ancestor_category: String,
    pub kind: KindRef,
    pub spelling: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryRule {
    pub category: String,
    pub unexpected: UnexpectedPolicy,
    pub synchronization: Vec<SyncToken>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryPlan {
    pub default_unexpected: UnexpectedPolicy,
    pub rules: Vec<RecoveryRule>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnparsedReason {
    UnknownHead,
    CategoryMismatch,
    ProviderFailure,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryKind {
    Missing {
        expected: EntryContext,
        anchor: Span,
    },
    Unexpected {
        token: TokenRef,
    },
    Unparsed {
        span: Span,
        reason: UnparsedReason,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryEntry {
    pub node: NodeRef,
    pub kind: RecoveryKind,
}
/// `node` belongs to the bundle reached before this step; `field` selects its ForeignSyntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignStep {
    pub node: NodeRef,
    pub field: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleRecovery {
    pub path: Vec<ForeignStep>,
    pub entries: Vec<RecoveryEntry>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseTree {
    pub profile_digest: nepl3_core::source::Digest,
    pub bundle: SyntaxBundle,
    pub recovery: Vec<BundleRecovery>,
    pub contexts: Vec<crate::selection::BundleContext>,
}

impl RecoveryPlan {
    pub fn validate(
        &self,
        package: &LanguagePackage,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), PackageError> {
        budget.charge(Resource::Work, 1)?;
        for (i, rule) in self.rules.iter().enumerate() {
            budget.charge(
                Resource::Work,
                ((i + package.categories.len()) as u64)
                    .saturating_mul(rule.category.len() as u64 + 1),
            )?;
            package.category(&rule.category)?;
            if self.rules[..i]
                .iter()
                .any(|prior| prior.category == rule.category)
            {
                return Err(PackageError::DuplicateName);
            }
            for sync in &rule.synchronization {
                budget.charge(
                    Resource::Work,
                    (package.categories.len() as u64)
                        .saturating_mul(sync.ancestor_category.len() as u64 + 1),
                )?;
                package.category(&sync.ancestor_category)?;
                if sync.kind.schema != package.schema {
                    return Err(PackageError::KindShape);
                }
                registry.kind_name(&sync.kind.schema, sync.kind.local_kind)?;
                if sync.spelling.as_ref().is_some_and(|v| v.is_empty()) {
                    return Err(PackageError::EmptyName);
                }
            }
        }
        Ok(())
    }
}
