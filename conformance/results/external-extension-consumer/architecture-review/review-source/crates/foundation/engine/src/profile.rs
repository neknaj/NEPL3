//! Immutable parsing projection of a suite profile; no bridge or provider executes here.
use crate::package::PackageIdentity;
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::Limits,
    source::Digest,
    value::{OperationRef, SchemaRef},
};

mod resolve;
pub use resolve::{ProfileError, ResolvedParseProfile, RuntimeCatalog};

/// Resolved root of one field read. Arena IDs remain owned by the parent package;
/// Foreign has no remaining local read ID and selects an explicit guest registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRead {
    pub entry: crate::package::EntryContext,
    pub read: Option<crate::package::ReadSpecId>,
    pub foreign: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageRegistration {
    pub alias: String,
    pub package: PackageIdentity,
    pub default_category: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryMode {
    pub alias: String,
    pub category: String,
    pub mode: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadRegistration {
    pub alias: String,
    pub category: String,
    pub provider: crate::selection::HeadProviderRef,
}
/// Identity asserted by a profile and checked against a separately supplied host catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRequirement {
    pub provider: String,
    pub revision: u64,
    pub implementation_digest: Digest,
    pub operation: OperationRef,
}
/// The host derives this identity from its real implementation/asset manifest.
/// An external profile does not manufacture registrations by repeating its own claims.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderImplementation {
    pub provider: String,
    pub revision: u64,
    pub implementation_digest: Digest,
    pub operations: Vec<OperationRef>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceIdentity {
    pub id: String,
    pub digest: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceSnapshot {
    pub id: String,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseProfile {
    pub id: String,
    pub languages: Vec<LanguageRegistration>,
    pub schemas: Vec<SchemaRef>,
    pub category_modes: Vec<CategoryMode>,
    pub head_providers: Vec<HeadRegistration>,
    pub providers: Vec<ProviderRequirement>,
    pub allowlist: Vec<OperationRef>,
    pub resources: Vec<ResourceIdentity>,
    pub limits: Limits,
}
