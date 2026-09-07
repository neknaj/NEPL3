//! Source regions preserve syntax ownership and reader sidecars separately from
//! binding resolutions. Raw sidecar validation is not provider authentication.
use super::{AnalysisKey, BindingAccessError, PreparedBindingRequest};
use crate::parse::ReaderFactBatch;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    source::{Digest, SourceAdmission, SourceError, SourceRef, SourceSnapshot, Span},
    view::PresentationClass,
};
pub(crate) mod check;
mod mapping;
pub mod query;
mod run;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegionKey {
    pub analysis: AnalysisKey,
    pub reader_facts_digest: Digest,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionCapability {
    SyntaxOnly,
    ReaderFacts,
}
/// Field/child indices form a root-first path in the token's ViewBundle.
/// They are declaration positions, never raw ViewRef arena indices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewStep {
    pub field: u64,
    pub child: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegionPart {
    Node,
    Head,
    Field { field: u64, element: Option<u64> },
    View { root: u64, path: Vec<ViewStep> },
    Capture { batch: u64, fact: u64 },
    Presentation { batch: u64, fact: u64 },
    Recovery,
}
/// Bundle and node use the canonical root/field DFS numbering of the tree in
/// RegionKey.analysis, independent of native storage order and Foreign aliases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionTarget {
    pub bundle: u64,
    /// None retains a trailing/skip-only reader batch without inventing a node.
    pub node: Option<u64>,
    pub part: RegionPart,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionMapping {
    Direct,
    Exact,
    /// Only a portion of the logical region has an exact source correspondence.
    ExactFragment,
    Transformed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRegion {
    pub target: RegionTarget,
    pub span: Span,
    pub logical_span: Span,
    pub mapping: RegionMapping,
    pub priority: u64,
    pub depth: u64,
    pub declaration_order: u64,
    pub classes: Vec<PresentationClass>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionRequest {
    pub key: RegionKey,
    pub source: SourceRef,
    pub offset: u64,
}
pub struct PreparedRegionInput<'a, 't, 'p> {
    pub(crate) binding: &'a PreparedBindingRequest<'t, 'p>,
    pub(crate) facts: Option<&'a [ReaderFactBatch]>,
    pub(crate) key: RegionKey,
}
impl PreparedRegionInput<'_, '_, '_> {
    pub fn key(&self) -> RegionKey {
        self.key
    }
    pub fn capability(&self) -> RegionCapability {
        if self.facts.is_some() {
            RegionCapability::ReaderFacts
        } else {
            RegionCapability::SyntaxOnly
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegionError {
    Access(BindingAccessError),
    Source(SourceError),
    Owner,
    Sidecar,
    Mapping,
}
impl From<StopReason> for RegionError {
    fn from(v: StopReason) -> Self {
        Self::Access(BindingAccessError::Stopped(v))
    }
}
impl From<SourceError> for RegionError {
    fn from(v: SourceError) -> Self {
        Self::Source(v)
    }
}
impl RegionError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Access(BindingAccessError::Stopped(v))
            | Self::Source(SourceError::Stopped(v)) => Some(*v),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegionOutcome {
    Complete {
        selection: Option<u64>,
        regions: Vec<SourceRegion>,
    },
    Invalid(RegionError),
    Stopped(StopReason),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionReply {
    pub key: RegionKey,
    pub capability: RegionCapability,
    pub outcome: RegionOutcome,
    pub report: Report,
    pub sources: Vec<SourceSnapshot>,
}
/// Returns source regions with original overlaps/classes plus a position
/// selection. LSP line splitting and overlap normalization belong to its adapter.
pub fn regions(
    input: &PreparedRegionInput<'_, '_, '_>,
    request: &RegionRequest,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> RegionReply {
    let mut sources = Vec::new();
    let outcome = match b.with_depth(|b| run::run(input, request, &mut sources, b, admission)) {
        Ok(v) => v,
        Err(e) => {
            sources.clear();
            match e.stop_reason() {
                Some(v) => RegionOutcome::Stopped(v),
                None => RegionOutcome::Invalid(e),
            }
        }
    };
    RegionReply {
        key: request.key,
        capability: input.capability(),
        outcome,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
        sources,
    }
}
fn push<T>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), RegionError> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    v.push(item);
    Ok(())
}
fn span(v: &Span, b: &mut Budget) -> Result<Span, RegionError> {
    b.charge(Resource::Work, v.snapshot_ref().source.0.len() as u64 + 40)?;
    b.charge(
        Resource::AllocationUnits,
        v.snapshot_ref().source.0.len() as u64,
    )?;
    Ok(v.clone())
}
