//! Host preparation and atomic publication of one authorized Await generation.
use super::*;
use crate::grants::dependencies::{
    self, AuthorizedDependency, DependencyGrantError, OperationGrant,
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::Resource,
    operation::{
        OperationReply,
        lifetime::{LifetimeError, RequestLifetimes},
    },
};

/// Dropping this collection does not cancel registered lifetimes. On later
/// execution/transport failure the host cancels the subtree or closes its table.
pub struct ActiveAwait<'a> {
    pub pending: PendingDependencies<'a>,
    pub authorized: Vec<AuthorizedDependency<'a>>,
}

#[derive(Debug)]
pub enum ActivationError {
    Stopped(StopReason),
    NotAwait,
    ContextCount,
    Await(AwaitError),
    Grants(DependencyGrantError),
    Lifetime(LifetimeError),
}
impl From<StopReason> for ActivationError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Validate the full reply, authorize every dependency and prepare result slots
/// before publishing IDs and the parent's Await phase. No provider callback runs.
/// `contexts` are host-computed snapshots for authorized children in call order;
/// they must include provider configuration and exact environment/resource grants.
/// The host retains responsibility for execution budgets and transport cleanup.
#[allow(clippy::too_many_arguments)]
pub fn activate<'a>(
    parent: &Invoke,
    context: Digest,
    reply: &'a OperationReply,
    policy: &'a [OperationGrant<'a>],
    contexts: &[Digest],
    registry: &SchemaRegistry,
    sources: &impl DiagnosticSourceResolver,
    lifetimes: &mut RequestLifetimes,
    budget: &mut Budget,
) -> Result<ActiveAwait<'a>, ActivationError> {
    budget.poll()?;
    let OperationReply::Await {
        continuation,
        calls,
        report,
    } = reply
    else {
        return Err(ActivationError::NotAwait);
    };
    if contexts.len() != calls.len() {
        return Err(ActivationError::ContextCount);
    }
    let pending = prepare(
        parent,
        context,
        continuation,
        calls,
        report,
        registry,
        sources,
        budget,
    )
    .map_err(|e| match e {
        AwaitError::Stopped(s) => ActivationError::Stopped(s),
        e => ActivationError::Await(e),
    })?;
    let authorized = dependencies::authorize(calls, policy, budget).map_err(|e| match e {
        DependencyGrantError::Stopped(s) => ActivationError::Stopped(s),
        e => ActivationError::Grants(e),
    })?;
    let bytes = calls
        .len()
        .checked_mul(core::mem::size_of::<(&Invoke, Digest)>())
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut bindings = Vec::new();
    bindings
        .try_reserve_exact(calls.len())
        .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    for (call, snapshot) in calls.iter().zip(contexts) {
        budget.charge(Resource::Work, 1)?;
        bindings.push((call, *snapshot));
    }
    let strings = (continuation.provider.name.len() as u64)
        .saturating_add(continuation.provider.schema.package.len() as u64);
    budget.charge(Resource::Work, strings.saturating_add(72))?;
    budget.charge(Resource::AllocationUnits, strings)?;
    let saved = Continuation {
        provider: continuation.provider.clone(),
        parent_request: continuation.parent_request,
        snapshot_digest: continuation.snapshot_digest,
        state: continuation.state.clone_with_budget(budget)?,
    };
    lifetimes
        .suspend_calls(parent.request_id, saved, &bindings, budget)
        .map_err(|e| match e {
            LifetimeError::Stopped(s) => ActivationError::Stopped(s),
            e => ActivationError::Lifetime(e),
        })?;
    Ok(ActiveAwait {
        pending,
        authorized,
    })
}
