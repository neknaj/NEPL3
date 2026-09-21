//! Transport-independent operation requests and saved continuation identities.
//! Hosts own dispatch, capability checks and request lifetime.
use crate::{
    budget::{Budget, Limits, Resource, StopReason},
    diagnostic::{OperationResult, Report},
    source::{Digest, SourceSnapshot},
    syntax::ResourceContent,
    value::{OperationRef, TypedValue},
};
use alloc::vec::Vec;

/// The fields of the foundation Invoke record. The receiving operation must
/// validate its input/environment semantics and authorize supplied resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invoke {
    pub request_id: u64,
    pub operation: OperationRef,
    pub input: TypedValue,
    pub environment: TypedValue,
    pub sources: Vec<SourceSnapshot>,
    pub resources: Vec<ResourceContent>,
    pub limits: Limits,
}

/// Portable saved state bound to the exact provider, parent and snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Continuation {
    pub provider: OperationRef,
    pub parent_request: u64,
    pub snapshot_digest: Digest,
    pub state: TypedValue,
}

/// A terminal operation result or suspended calls with a saved continuation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationReply {
    Result(OperationResult<TypedValue>),
    Await {
        continuation: Continuation,
        calls: Vec<Invoke>,
        report: Report,
    },
}

/// Results are ordered as the dependency calls of the saved Await. The host
/// checks correspondence to those calls before passing this record to a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resume {
    pub request_id: u64,
    pub continuation: Continuation,
    pub dependency_results: Vec<OperationReply>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationError {
    Stopped(StopReason),
    Provider,
    ParentRequest,
    Snapshot,
}
impl From<StopReason> for ContinuationError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl Continuation {
    /// Compare against host-saved identity before resuming. The host must also
    /// recognize this continuation and validate its operation-specific state.
    pub fn check_binding(
        &self,
        provider: &OperationRef,
        parent_request: u64,
        snapshot_digest: Digest,
        budget: &mut Budget,
    ) -> Result<(), ContinuationError> {
        budget.charge(
            Resource::Work,
            (self.provider.schema.package.len()
                + provider.schema.package.len()
                + self.provider.name.len()
                + provider.name.len()) as u64
                + 80,
        )?;
        if &self.provider != provider {
            return Err(ContinuationError::Provider);
        }
        if self.parent_request != parent_request {
            return Err(ContinuationError::ParentRequest);
        }
        if self.snapshot_digest != snapshot_digest {
            return Err(ContinuationError::Snapshot);
        }
        Ok(())
    }
}
