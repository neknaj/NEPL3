//! Explicit host selection and authority for a native Custom Facts call.
use super::BindingError;
use crate::{
    facts::{CheckedFactsView, FactsEmitter},
    profile::ProviderRequirement,
    recovery::{ForeignStep, ParseTree},
};
use nepl3_core::{
    budget::{Budget, StopReason},
    facts::{FactAuthority, FactDelta, FactSet, ScopeId},
    syntax::NodeRef,
};

/// Read-only call site. An authority returned by the host is checked against
/// these existing facts; the provider's response cannot expand that authority.
pub struct BindingCall<'a> {
    pub provider: &'a ProviderRequirement,
    pub tree: &'a ParseTree,
    pub path: &'a [ForeignStep],
    pub node: NodeRef,
    pub scope: ScopeId,
    pub existing: &'a FactSet,
    pub phase: &'a crate::facts::FactsPhase,
}
/// Formal diagnostics belong to FactsEmitter, independently of this unchecked
/// delta. A stopped parent never accepts new raw values from this outcome.
pub enum CustomOutcome {
    Complete(FactDelta),
    Invalid(Option<FactDelta>),
    Stopped {
        reason: StopReason,
        partial: Option<FactDelta>,
    },
}
pub trait BindingHost {
    /// None means that the selected implementation is not registered here.
    fn authorize(
        &mut self,
        call: &BindingCall<'_>,
        budget: &mut Budget,
    ) -> Result<Option<FactAuthority>, BindingError>;
    fn facts(
        &mut self,
        provider: &ProviderRequirement,
        request: &CheckedFactsView<'_, '_>,
        emit: &mut FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError>;
}
