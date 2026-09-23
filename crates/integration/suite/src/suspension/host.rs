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

/// Owned generation and its validated provider report. The host retains reports
/// across suspension and accounts actual execution independently of claimed Usage.
/// Dropping this state requires the same lifetime cleanup as `ActiveAwait`.
pub struct OwnedActiveAwait {
    pub pending: PendingDependencies<'static>,
    pub report: Report,
}

/// Structurally checked, owned Await awaiting host authorization. The original
/// parent, schema registry and source resolver remain borrowed until activation.
/// Dependency IDs and result slots are prepared once and moved into the active
/// generation. No lifetime has been published at this stage.
pub struct PreparedAwait<'a, S> {
    parent: &'a Invoke,
    _registry: &'a SchemaRegistry,
    _sources: &'a S,
    pending: PendingDependencies<'static>,
    report: Report,
}

impl<S> PreparedAwait<'_, S> {
    pub fn calls(&self) -> &[Invoke] {
        self.pending.calls()
    }

    /// Authorize against the current policy and publish the complete generation
    /// atomically. Failure consumes the preparation without changing lifetimes.
    pub fn activate(
        self,
        policy: &[OperationGrant<'_>],
        contexts: &[Digest],
        lifetimes: &mut RequestLifetimes,
        budget: &mut Budget,
    ) -> Result<OwnedActiveAwait, ActivationError> {
        budget.poll()?;
        if contexts.len() != self.pending.calls().len() {
            return Err(ActivationError::ContextCount);
        }
        dependencies::authorize(self.pending.calls(), policy, budget).map_err(|e| match e {
            DependencyGrantError::Stopped(s) => ActivationError::Stopped(s),
            e => ActivationError::Grants(e),
        })?;
        publish(
            self.parent,
            self.pending.continuation(),
            self.pending.calls(),
            contexts,
            lifetimes,
            budget,
        )?;
        Ok(OwnedActiveAwait {
            pending: self.pending,
            report: self.report,
        })
    }
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
    publish(parent, continuation, calls, contexts, lifetimes, budget)?;
    Ok(ActiveAwait {
        pending,
        authorized,
    })
}

/// Admit and retain an owned Await for a host frame. All checks and allocations
/// precede lifetime publication; a rejected reply is consumed with no table
/// change. This result retains no authorization proof: dispatch must use the
/// current host policy to authorize each borrowed call before invoking it.
/// Execution budgets and cancellation remain the scheduler's responsibility.
#[allow(clippy::too_many_arguments)]
pub fn activate_owned(
    parent: &Invoke,
    context: Digest,
    reply: OperationReply,
    policy: &[OperationGrant<'_>],
    contexts: &[Digest],
    registry: &SchemaRegistry,
    sources: &impl DiagnosticSourceResolver,
    lifetimes: &mut RequestLifetimes,
    budget: &mut Budget,
) -> Result<OwnedActiveAwait, ActivationError> {
    prepare_owned(parent, context, reply, registry, sources, budget)?
        .activate(policy, contexts, lifetimes, budget)
}

/// Validate an owned Await and retain its dependency index and result slots.
/// Callers may inspect immutable calls to compute host contexts before consuming
/// this value with `activate`. Registry and source permissions remain associated
/// with this exact preparation; raw mutable reply access is not exposed.
pub fn prepare_owned<'a, S: DiagnosticSourceResolver>(
    parent: &'a Invoke,
    context: Digest,
    reply: OperationReply,
    registry: &'a SchemaRegistry,
    sources: &'a S,
    budget: &mut Budget,
) -> Result<PreparedAwait<'a, S>, ActivationError> {
    budget.poll()?;
    let OperationReply::Await {
        continuation,
        calls,
        report,
    } = reply
    else {
        return Err(ActivationError::NotAwait);
    };
    validate(
        parent,
        context,
        &continuation,
        &calls,
        &report,
        registry,
        sources,
        budget,
    )
    .map_err(|e| match e {
        AwaitError::Stopped(s) => ActivationError::Stopped(s),
        e => ActivationError::Await(e),
    })?;
    let pending =
        PendingDependencies::from_owned(continuation, calls, budget).map_err(|e| match e {
            DependencyError::Stopped(s) => ActivationError::Stopped(s),
            e => ActivationError::Await(AwaitError::Dependencies(e)),
        })?;
    Ok(PreparedAwait {
        parent,
        _registry: registry,
        _sources: sources,
        pending,
        report,
    })
}

fn publish(
    parent: &Invoke,
    continuation: &Continuation,
    calls: &[Invoke],
    contexts: &[Digest],
    lifetimes: &mut RequestLifetimes,
    budget: &mut Budget,
) -> Result<(), ActivationError> {
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
    Ok(())
}
