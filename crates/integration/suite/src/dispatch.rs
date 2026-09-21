//! Borrowed native registration and checked terminal-operation dispatch.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::{OperationResult, validation::DiagnosticSourceResolver},
    operation::{Invoke, request::InputValidationError, validation::ResultValidationError},
    schema::SchemaRegistry,
    source::Digest,
    value::{OperationRef, TypedValue},
};

/// A native implementation of a terminal operation. Operations which suspend
/// use the separate Await/Resume host path; this callback owns no continuation.
pub type TerminalOperation =
    fn(&Invoke, &SchemaRegistry, &mut Budget) -> Result<OperationResult<TypedValue>, StopReason>;

pub struct Registration<'a> {
    pub operation: &'a OperationRef,
    /// Identity established from executable assets by the host.
    pub implementation: Digest,
    pub invoke: TerminalOperation,
}

#[derive(Debug)]
pub enum DispatchError {
    Stopped(StopReason),
    Input(InputValidationError),
    Output(ResultValidationError),
    MissingImplementation,
    AmbiguousImplementation,
}
impl From<StopReason> for DispatchError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Invoke an explicitly selected native implementation after input validation.
/// The host authorizes the operation and admits source/resource grants before
/// calling this function. Validation has its own host budget so a provider's
/// Stopped result and partial value can still be checked after execution stops.
/// Report Usage is a claim; the caller observes actual execution Usage directly.
#[allow(clippy::too_many_arguments)]
pub fn invoke_terminal(
    registrations: &[Registration<'_>],
    selected: &OperationRef,
    implementation: Digest,
    request: &Invoke,
    registry: &SchemaRegistry,
    sources: &impl DiagnosticSourceResolver,
    execution: &mut Budget,
    validation: &mut Budget,
) -> Result<OperationResult<TypedValue>, DispatchError> {
    execution.poll()?;
    let mut found = None;
    for registration in registrations {
        validation.charge(
            Resource::Work,
            (registration.operation.name.len() as u64)
                .saturating_add(selected.name.len() as u64)
                .saturating_add(registration.operation.schema.package.len() as u64)
                .saturating_add(selected.schema.package.len() as u64)
                .saturating_add(112),
        )?;
        if registration.operation == selected && registration.implementation == implementation {
            if found.is_some() {
                return Err(DispatchError::AmbiguousImplementation);
            }
            found = Some(registration.invoke);
        }
    }
    let invoke = found.ok_or(DispatchError::MissingImplementation)?;
    request
        .validate_input(selected, registry, validation)
        .map_err(|e| match e {
            InputValidationError::Stopped(s) => DispatchError::Stopped(s),
            e => DispatchError::Input(e),
        })?;
    let result = execution.with_ceiling(request.limits, |budget| {
        let result = invoke(request, registry, budget)?;
        if let Err(reason) = budget.poll()
            && !matches!(&result, OperationResult::Stopped { reason: reported, .. } if *reported == reason) {
            return Err(reason);
        }
        Ok(result)
    })?;
    result
        .validate_for(selected, registry, sources, validation)
        .map_err(|e| match e {
            ResultValidationError::Stopped(s) => DispatchError::Stopped(s),
            e => DispatchError::Output(e),
        })?;
    Ok(result)
}
