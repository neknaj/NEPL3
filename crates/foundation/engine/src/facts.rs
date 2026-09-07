//! Custom binding transport. Analysis algorithms remain separate operations.
use crate::recovery::{ForeignStep, ParseTree};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    budget::StopReason,
    diagnostic::Report,
    facts::{EntityId, FactAuthority, FactDelta, FactSet, ScopeId},
    syntax::NodeRef,
};
pub(crate) mod check;
mod emit;
pub(crate) mod phase;
pub use check::{CheckedFactsRequest, CheckedFactsView, FactsError};
pub use emit::FactsEmitter;
pub(crate) fn signature(
    input: &nepl3_core::schema::TypeDescriptor,
    output: &nepl3_core::schema::TypeDescriptor,
    pure: bool,
) -> bool {
    let named = |value: &nepl3_core::schema::TypeDescriptor, name: &str| matches!(value,nepl3_core::schema::TypeDescriptor::Named(v) if v.package=="nepl3.engine"&&v.revision==1&&v.name==name);
    pure && named(input, "FactsRequest") && named(output, "FactsReply")
}
/// A header receipt names accepted declarations by ID, never by spelling or
/// source position. Raw transport validation does not authenticate issuance.
#[derive(Debug, Eq, PartialEq)]
pub struct FactsHeader {
    pub group: ScopeId,
    pub provider: crate::profile::ProviderRequirement,
    pub target: crate::binding::CanonicalBindingTarget,
    pub entities: Vec<EntityId>,
    pub exports: Vec<EntityId>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum FactsPhase {
    Ordinary,
    Header { group: ScopeId },
    Body { header: Box<FactsHeader> },
}
pub struct FactsRequest {
    pub tree: ParseTree,
    pub path: Vec<ForeignStep>,
    pub node: NodeRef,
    pub existing: FactSet,
    pub authority: FactAuthority,
    pub phase: FactsPhase,
}
/// The same logical request as FactsRequest, borrowed from a native analysis.
/// This carries no authority proof until the host explicitly issues it.
#[derive(Clone, Copy)]
pub struct FactsRequestView<'a> {
    pub tree: &'a ParseTree,
    pub path: &'a [ForeignStep],
    pub node: NodeRef,
    pub existing: &'a FactSet,
    pub authority: &'a FactAuthority,
    pub phase: &'a FactsPhase,
}
impl FactsRequest {
    pub fn view(&self) -> FactsRequestView<'_> {
        FactsRequestView {
            tree: &self.tree,
            path: &self.path,
            node: self.node,
            existing: &self.existing,
            authority: &self.authority,
            phase: &self.phase,
        }
    }
}
pub enum FactsReply {
    Complete {
        delta: FactDelta,
        report: Report,
        sources: Vec<nepl3_core::source::SourceSnapshot>,
        source_maps: Vec<nepl3_core::origin::Mapping>,
    },
    Invalid {
        partial: Option<FactDelta>,
        report: Report,
        sources: Vec<nepl3_core::source::SourceSnapshot>,
        source_maps: Vec<nepl3_core::origin::Mapping>,
    },
    Stopped {
        reason: StopReason,
        partial: Option<FactDelta>,
        report: Report,
        sources: Vec<nepl3_core::source::SourceSnapshot>,
        source_maps: Vec<nepl3_core::origin::Mapping>,
    },
}
