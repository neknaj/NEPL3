//! Explicit prepared binding requests and keyed access to completed analysis.
//! Portable data alone never creates a completed name-resolution proof.
use crate::{
    binding::{BindingAnalysis, BindingOutcome, BindingReply},
    profile::ResolvedParseProfile,
    recovery::ParseTree,
    tree::ValidatedParseTree,
};
use alloc::string::String;
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason},
    source::{Digest, SourceAdmission, SourceRef},
};

/// Revision 1 has no optional semantic switches. Adding a switch requires a
/// declared schema change and inclusion in the request identity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BindingOptions;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnalysisKey {
    pub tree_digest: Digest,
    pub profile_digest: Digest,
    pub execution_digest: Digest,
    pub request_digest: Digest,
}

/// Owned transport data. Its claimed key is rederived when preparing execution.
pub struct BindingRequest {
    pub analysis_id: String,
    pub tree: ParseTree,
    pub options: BindingOptions,
    pub limits: Limits,
    pub key: AnalysisKey,
}

/// Issued only after validating the source/tree/profile and deriving the key.
/// Holding this borrow prevents mutation of the concrete input during execution.
pub struct PreparedBindingRequest<'a, 'p> {
    pub(crate) analysis_id: &'a str,
    pub(crate) tree: ValidatedParseTree<'a>,
    pub(crate) profile: &'a ResolvedParseProfile<'p>,
    pub(crate) options: BindingOptions,
    pub(crate) limits: Limits,
    pub(crate) key: AnalysisKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingAccessError {
    Stopped(StopReason),
    LimitsMismatch,
    StaleAnalysis,
    MissingSource,
    Incomplete,
}
impl From<StopReason> for BindingAccessError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// The original native reply remains available for every failure and stop.
/// Only actual plan execution can produce this wrapper's completed proof.
pub struct BoundBindingReply {
    key: AnalysisKey,
    reply: BindingReply,
}
impl PreparedBindingRequest<'_, '_> {
    pub fn key(&self) -> AnalysisKey {
        self.key
    }
    pub fn execute(
        &self,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<BoundBindingReply, BindingAccessError> {
        if budget.limits() != self.limits {
            return Err(BindingAccessError::LimitsMismatch);
        }
        let reply = crate::binding::analyze(
            self.analysis_id,
            &self.tree,
            self.profile,
            budget,
            admission,
        );
        Ok(BoundBindingReply {
            key: self.key,
            reply,
        })
    }
}
impl BoundBindingReply {
    pub fn key(&self) -> AnalysisKey {
        self.key
    }
    pub fn reply(&self) -> &BindingReply {
        &self.reply
    }
    pub fn into_reply(self) -> BindingReply {
        self.reply
    }
    /// Gate for a source-specific query. The host supplies the key of its current
    /// requested analysis and the exact source revision, not just a file name.
    pub fn for_source(
        &self,
        expected: &AnalysisKey,
        source: &SourceRef,
        budget: &mut Budget,
    ) -> Result<&BindingAnalysis, BindingAccessError> {
        budget.charge(Resource::Work, 128)?;
        if self.key != *expected {
            return Err(BindingAccessError::StaleAnalysis);
        }
        let BindingOutcome::Complete(analysis) = &self.reply.outcome else {
            return Err(BindingAccessError::Incomplete);
        };
        for snapshot in self.reply.sources() {
            budget.charge(
                Resource::Work,
                (source.source_id.0.len() as u64)
                    .saturating_add(snapshot.identity().source.0.len() as u64)
                    .saturating_add(40),
            )?;
            let identity = snapshot.identity();
            if identity.source == source.source_id
                && identity.revision == source.revision
                && identity.digest == source.digest
            {
                return Ok(analysis);
            }
        }
        Err(BindingAccessError::MissingSource)
    }
}
