//! Prepared-request issuance of keyed native Reference traces.
use super::{AnalysisKey, BindingAccessError, PreparedBindingRequest};
use crate::binding::{
    BindingHost,
    trace::{FinalReference, ReferenceTrace, TraceAccessError},
};
use nepl3_core::{
    budget::{Budget, Limits, Resource},
    source::SourceAdmission,
};

pub struct BoundReferenceTrace<'a, 'p> {
    key: AnalysisKey,
    limits: Limits,
    trace: ReferenceTrace<'a, 'p>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum ReferenceTraceAccessError {
    Access(BindingAccessError),
    Trace(TraceAccessError),
}
impl PreparedBindingRequest<'_, '_> {
    pub fn trace_references(
        &self,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<BoundReferenceTrace<'_, '_>, BindingAccessError> {
        self.trace_references_inner(None, budget, admission)
    }
    pub fn trace_references_with_host(
        &self,
        host: &mut dyn BindingHost,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<BoundReferenceTrace<'_, '_>, BindingAccessError> {
        self.trace_references_inner(Some(host), budget, admission)
    }
    fn trace_references_inner(
        &self,
        host: Option<&mut dyn BindingHost>,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<BoundReferenceTrace<'_, '_>, BindingAccessError> {
        if budget.limits() != self.limits {
            return Err(BindingAccessError::LimitsMismatch);
        }
        let trace = crate::binding::trace::analyze(
            self.analysis_id,
            &self.tree,
            self.profile,
            host,
            budget,
            admission,
        );
        Ok(BoundReferenceTrace {
            key: self.key,
            limits: self.limits,
            trace,
        })
    }
}
impl<'a, 'p> BoundReferenceTrace<'a, 'p> {
    pub fn key(&self) -> AnalysisKey {
        self.key
    }
    /// Native data retains its own provenance; this getter checks no claimed key.
    pub fn trace(&self) -> &ReferenceTrace<'a, 'p> {
        &self.trace
    }
    /// Consuming this wrapper discards the prepared request's key association.
    pub fn into_trace(self) -> ReferenceTrace<'a, 'p> {
        self.trace
    }
    /// A local gate never changes the native outcome or its original Report.
    pub fn for_key(
        &self,
        expected: &AnalysisKey,
        budget: &mut Budget,
    ) -> Result<&ReferenceTrace<'a, 'p>, BindingAccessError> {
        if budget.limits() != self.limits {
            return Err(BindingAccessError::LimitsMismatch);
        }
        budget.charge(Resource::Work, 128)?;
        if self.key != *expected {
            return Err(BindingAccessError::StaleAnalysis);
        }
        Ok(&self.trace)
    }
    pub fn final_reference(
        &self,
        expected: &AnalysisKey,
        index: usize,
        budget: &mut Budget,
    ) -> Result<FinalReference<'_>, ReferenceTraceAccessError> {
        self.for_key(expected, budget)
            .map_err(ReferenceTraceAccessError::Access)?
            .final_reference(index, budget)
            .map_err(ReferenceTraceAccessError::Trace)
    }
}

pub mod named;
