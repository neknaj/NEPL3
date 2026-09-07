//! Shared analysis facts. IDs belong to an explicit analysis, never a spelling hash.
//! Grammar binding plans and domain algorithms produce these same core-owned values.
use crate::{
    budget::{Budget, Resource, StopReason},
    origin::{Mapping, Origin, OriginId},
    source::{SourceSnapshot, Span},
    value::{SchemaRef, TypedValue},
};
use alloc::{string::String, vec::Vec};
mod check;
pub use check::{CheckedFactDelta, CheckedFactSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceRef(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamespacePolicy {
    Lexical,
    Global,
    Open,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactNamespace {
    pub schema: SchemaRef,
    pub name: String,
    pub policy: NamespacePolicy,
    pub root: ScopeId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scope {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub origin: Option<OriginId>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entity {
    pub id: EntityId,
    pub scope: ScopeId,
    pub namespace: NamespaceRef,
    pub name: String,
    pub definition: Option<Span>,
    pub selection: Option<Span>,
    pub origin: Option<OriginId>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OccurrenceRole {
    Definition,
    Reference,
    Import,
    Export,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceResolution {
    Resolved(EntityId),
    Unresolved(String),
    Ambiguous(Vec<EntityId>),
    Deferred(Vec<TypedValue>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Occurrence {
    pub id: OccurrenceId,
    pub scope: ScopeId,
    pub namespace: NamespaceRef,
    pub name: String,
    pub role: OccurrenceRole,
    pub span: Span,
    pub origin: Option<OriginId>,
    pub resolution: ReferenceResolution,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FactTarget {
    Scope(ScopeId),
    Entity(EntityId),
    Occurrence(OccurrenceId),
    Source(Span),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relation {
    pub id: RelationId,
    pub source: FactTarget,
    pub target: FactTarget,
    pub payload: TypedValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScopeEdge {
    Import {
        from: ScopeId,
        to: ScopeId,
        namespace: NamespaceRef,
    },
    Export {
        from: ScopeId,
        to: ScopeId,
        entity: EntityId,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactSet {
    pub analysis_id: String,
    pub namespaces: Vec<FactNamespace>,
    pub scopes: Vec<Scope>,
    pub entities: Vec<Entity>,
    pub occurrences: Vec<Occurrence>,
    pub relations: Vec<Relation>,
    pub edges: Vec<ScopeEdge>,
    pub sources: Vec<SourceSnapshot>,
    pub origins: Vec<Origin>,
    pub source_maps: Vec<Mapping>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdRange {
    pub start: u64,
    pub end: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FactReservation {
    pub scopes: IdRange,
    pub entities: IdRange,
    pub occurrences: IdRange,
    pub relations: IdRange,
}
/// Host-issued authority. New scopes may only descend from current_scope or
/// another created scope. Existing writable scopes are explicit descendants;
/// import targets are separately authorized reads, never implied write grants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactAuthority {
    pub analysis_id: String,
    pub current_scope: ScopeId,
    pub namespaces: Vec<NamespaceRef>,
    pub writable_scopes: Vec<ScopeId>,
    pub import_scopes: Vec<ScopeId>,
    pub resolution_updates: Vec<OccurrenceId>,
    pub relation_sources: Vec<FactTarget>,
    pub reservation: FactReservation,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionUpdate {
    pub occurrence: OccurrenceId,
    pub resolution: ReferenceResolution,
}
/// Source and origin arrays append to the request tables. Origin IDs in the
/// added facts use that explicit joint table; a provider cannot reinterpret ID 0.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactDelta {
    pub analysis_id: String,
    pub origin_base: u64,
    pub scopes: Vec<Scope>,
    pub entities: Vec<Entity>,
    pub occurrences: Vec<Occurrence>,
    pub relations: Vec<Relation>,
    pub edges: Vec<ScopeEdge>,
    pub resolutions: Vec<ResolutionUpdate>,
    pub sources: Vec<SourceSnapshot>,
    pub origins: Vec<Origin>,
    pub source_maps: Vec<Mapping>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FactError {
    Stopped(StopReason),
    Source(crate::source::SourceError),
    Origin(crate::origin::OriginError),
    Schema(crate::schema::SchemaError),
    Analysis,
    DuplicateId,
    MissingScope,
    MissingNamespace,
    MissingEntity,
    MissingOccurrence,
    MissingOrigin,
    Cycle,
    Name,
    Span,
    Resolution,
    Authority,
    Reservation,
}
impl From<StopReason> for FactError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<crate::source::SourceError> for FactError {
    fn from(v: crate::source::SourceError) -> Self {
        match v {
            crate::source::SourceError::Stopped(r) => Self::Stopped(r),
            v => Self::Source(v),
        }
    }
}
impl From<crate::origin::OriginError> for FactError {
    fn from(v: crate::origin::OriginError) -> Self {
        match v {
            crate::origin::OriginError::Stopped(r) => Self::Stopped(r),
            v => Self::Origin(v),
        }
    }
}
impl From<crate::schema::SchemaError> for FactError {
    fn from(v: crate::schema::SchemaError) -> Self {
        match v {
            crate::schema::SchemaError::Stopped(r) => Self::Stopped(r),
            v => Self::Schema(v),
        }
    }
}
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut Budget) -> Result<(), FactError> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    values.push(value);
    Ok(())
}
