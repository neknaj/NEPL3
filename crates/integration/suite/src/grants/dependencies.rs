//! Authorize a complete dependency batch before registering or dispatching it.
use super::*;
use alloc::vec::Vec;
use nepl3_core::value::OperationRef;

/// One operation in the host allowlist, with its exact context projection.
pub struct OperationGrant<'a> {
    pub operation: &'a OperationRef,
    pub grants: &'a Grants<'a>,
}
pub struct AuthorizedDependency<'a> {
    operation: &'a OperationRef,
    invocation: AuthorizedInvoke<'a>,
}
impl AuthorizedDependency<'_> {
    pub fn operation(&self) -> &OperationRef {
        self.operation
    }
    pub fn invocation(&self) -> &AuthorizedInvoke<'_> {
        &self.invocation
    }
}
#[derive(Debug)]
pub enum DependencyGrantError {
    Stopped(StopReason),
    NotAllowed,
    Ambiguous,
    Context(GrantError),
}
impl From<StopReason> for DependencyGrantError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// No callback or lifetime mutation occurs. A later failure returns no partial
/// batch. Schema, call-ID/ancestry and execution budgets remain separate gates.
/// The immutable proof borrows both selected policy and received call data.
pub fn authorize<'a>(
    calls: &'a [Invoke],
    policy: &'a [OperationGrant<'a>],
    budget: &mut Budget,
) -> Result<Vec<AuthorizedDependency<'a>>, DependencyGrantError> {
    budget.poll()?;
    let bytes = calls
        .len()
        .checked_mul(core::mem::size_of::<AuthorizedDependency<'a>>())
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(calls.len())
        .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    for call in calls {
        let mut selected = None;
        for candidate in policy {
            budget.charge(
                Resource::Work,
                (call.operation.name.len() as u64)
                    .saturating_add(candidate.operation.name.len() as u64)
                    .saturating_add(call.operation.schema.package.len() as u64)
                    .saturating_add(candidate.operation.schema.package.len() as u64)
                    .saturating_add(40),
            )?;
            if call.operation == *candidate.operation {
                if selected.is_some() {
                    return Err(DependencyGrantError::Ambiguous);
                }
                selected = Some(candidate);
            }
        }
        let selected = selected.ok_or(DependencyGrantError::NotAllowed)?;
        let invocation = selected.grants.admit(call, budget).map_err(|e| match e {
            GrantError::Stopped(reason) => DependencyGrantError::Stopped(reason),
            error => DependencyGrantError::Context(error),
        })?;
        result.push(AuthorizedDependency {
            operation: selected.operation,
            invocation,
        });
    }
    Ok(result)
}
