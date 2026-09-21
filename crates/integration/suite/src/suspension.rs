//! Admission of a provider's Await before dependency execution.
pub mod host;
use nepl3_core::{
    budget::{Budget, StopReason},
    diagnostic::{
        Report,
        validation::{DiagnosticSourceResolver, ReportValidationError},
    },
    operation::{
        Continuation, ContinuationError, Invoke,
        dependencies::{DependencyError, PendingDependencies},
        request::InputValidationError,
    },
    schema::{SchemaError, SchemaRegistry},
    source::Digest,
};

#[derive(Debug)]
pub enum AwaitError {
    Stopped(StopReason),
    Binding(ContinuationError),
    State(SchemaError),
    Report(ReportValidationError),
    Call(InputValidationError),
    Dependencies(DependencyError),
    TraceOverflow,
}

/// Bind an Await to the host-saved call and context, then prepare ordered result
/// collection. This checks continuation/state/report and dependency signatures.
/// The host subsequently grants each dependency its resources, registers its
/// connection-wide ID and ancestry, and dispatches within the parent's budget.
/// No callback, source admission or capability grant occurs here. Borrowed call
/// data remains immutable throughout collection and Resume construction.
#[allow(clippy::too_many_arguments)]
pub fn prepare<'a>(
    parent: &Invoke,
    context: Digest,
    continuation: &'a Continuation,
    calls: &'a [Invoke],
    report: &Report,
    registry: &SchemaRegistry,
    sources: &impl DiagnosticSourceResolver,
    budget: &mut Budget,
) -> Result<PendingDependencies<'a>, AwaitError> {
    continuation
        .check_binding(&parent.operation, parent.request_id, context, budget)
        .map_err(|e| match e {
            ContinuationError::Stopped(s) => AwaitError::Stopped(s),
            e => AwaitError::Binding(e),
        })?;
    registry
        .validate_typed(&continuation.state, budget)
        .map_err(|e| match e {
            SchemaError::Stopped(s) => AwaitError::Stopped(s),
            e => AwaitError::State(e),
        })?;
    if report.trace_overflow.is_some() {
        return Err(AwaitError::TraceOverflow);
    }
    report
        .validate_with_sources(sources, registry, budget)
        .map_err(|e| match e {
            ReportValidationError::Stopped(s) => AwaitError::Stopped(s),
            e => AwaitError::Report(e),
        })?;
    for call in calls {
        call.validate_input(&call.operation, registry, budget)
            .map_err(|e| match e {
                InputValidationError::Stopped(s) => AwaitError::Stopped(s),
                e => AwaitError::Call(e),
            })?;
    }
    PendingDependencies::new(continuation, calls, budget).map_err(|e| match e {
        DependencyError::Stopped(s) => AwaitError::Stopped(s),
        e => AwaitError::Dependencies(e),
    })
}
