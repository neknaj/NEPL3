//! Checked continuation delivery to a host-selected native resume callback.
use super::*;
use nepl3_core::operation::{
    Continuation, ContinuationError, OperationReply, Resume,
    lifetime::{LifetimeError, RequestLifetimes},
};

pub type TerminalResume = fn(
    &Invoke,
    &Resume,
    &SchemaRegistry,
    &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason>;

pub struct Registration<'a> {
    pub operation: &'a OperationRef,
    pub implementation: Digest,
    pub resume: TerminalResume,
}

/// Immutable data retained by the host for this Await generation. Each source
/// resolver contains only the grants for its corresponding dependency call.
pub struct SavedAwait<'a, S> {
    pub parent: &'a Invoke,
    pub context: Digest,
    pub continuation: &'a Continuation,
    pub calls: &'a [Invoke],
    pub sources: &'a [&'a S],
}

#[derive(Debug)]
pub enum ResumeError {
    Dispatch(DispatchError),
    Stopped(StopReason),
    Binding(ContinuationError),
    Lifetime(LifetimeError),
    SourceCount,
    NonterminalDependency,
}
impl From<DispatchError> for ResumeError {
    fn from(error: DispatchError) -> Self {
        match error {
            DispatchError::Stopped(s) => Self::Stopped(s),
            e => Self::Dispatch(e),
        }
    }
}
impl From<StopReason> for ResumeError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
fn binding(error: ContinuationError) -> ResumeError {
    match error {
        ContinuationError::Stopped(s) => ResumeError::Stopped(s),
        e => ResumeError::Binding(e),
    }
}
fn lifetime(error: LifetimeError) -> ResumeError {
    match error {
        LifetimeError::Stopped(s) => ResumeError::Stopped(s),
        e => ResumeError::Lifetime(e),
    }
}

/// All correlation and dependency result checks precede consumption of the
/// lifetime's Await phase. Once the callback begins, it owns execution failure:
/// the generation has been consumed and cannot be retried as a fresh Resume.
/// Provider-specific continuation-state semantics remain the callback's duty.
#[allow(clippy::too_many_arguments)]
pub fn execute<S: DiagnosticSourceResolver>(
    registration: &Registration<'_>,
    implementation: Digest,
    saved: &SavedAwait<'_, S>,
    resume: &Resume,
    lifetimes: &mut RequestLifetimes,
    registry: &SchemaRegistry,
    output_sources: &impl DiagnosticSourceResolver,
    execution: &mut Budget,
    validation: &mut Budget,
) -> Result<OperationResult<TypedValue>, ResumeError> {
    execution.poll()?;
    validation.charge(Resource::Work, 32)?;
    if registration.implementation != implementation {
        return Err(DispatchError::MissingImplementation.into());
    }
    saved
        .parent
        .validate_input(registration.operation, registry, validation)
        .map_err(|e| match e {
            InputValidationError::Stopped(s) => ResumeError::Stopped(s),
            e => ResumeError::Dispatch(DispatchError::Input(e)),
        })?;
    saved
        .continuation
        .check_binding(
            registration.operation,
            saved.parent.request_id,
            saved.context,
            validation,
        )
        .map_err(binding)?;
    resume
        .check_binding(saved.continuation, saved.calls.len(), validation)
        .map_err(binding)?;
    lifetimes
        .check_resume(resume, validation)
        .map_err(lifetime)?;
    if saved.sources.len() != saved.calls.len() {
        return Err(ResumeError::SourceCount);
    }
    for ((reply, call), sources) in resume
        .dependency_results
        .iter()
        .zip(saved.calls)
        .zip(saved.sources)
    {
        let OperationReply::Result(result) = reply else {
            return Err(ResumeError::NonterminalDependency);
        };
        result
            .validate_for(&call.operation, registry, *sources, validation)
            .map_err(|e| match e {
                ResultValidationError::Stopped(s) => ResumeError::Stopped(s),
                e => ResumeError::Dispatch(DispatchError::Output(e)),
            })?;
    }
    // Check the execution ceiling before consuming the saved generation.
    execution.with_ceiling(saved.parent.limits, |_| Ok::<(), StopReason>(()))?;
    lifetimes.resume(resume, validation).map_err(lifetime)?;
    run_terminal(
        saved.parent,
        registration.operation,
        registry,
        output_sources,
        execution,
        validation,
        |budget| (registration.resume)(saved.parent, resume, registry, budget),
    )
    .map_err(Into::into)
}
