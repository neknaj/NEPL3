//! Checked continuation delivery to a host-selected native resume callback.
use super::*;
use nepl3_core::operation::{
    Continuation, ContinuationError, OperationReply, Resume,
    lifetime::{LifetimeError, RequestLifetimes},
};

pub type ResumeOperation =
    fn(&Invoke, &Resume, &SchemaRegistry, &mut Budget) -> Result<OperationReply, StopReason>;

pub struct Registration<'a> {
    pub operation: &'a OperationRef,
    pub implementation: Digest,
    pub resume: ResumeOperation,
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
    Await(crate::suspension::AwaitError),
    Preparation(crate::suspension::host::ActivationError),
}
impl From<super::suspending::Error> for ResumeError {
    fn from(error: super::suspending::Error) -> Self {
        match error {
            super::suspending::Error::Stopped(s) => Self::Stopped(s),
            super::suspending::Error::Dispatch(e) => Self::Dispatch(e),
            super::suspending::Error::Await(e) => Self::Await(e),
            super::suspending::Error::Preparation(e) => Self::Preparation(e),
        }
    }
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
/// A returned Await is structurally admitted; the host registers its next
/// generation and authorizes dependencies before scheduling further execution.
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
) -> Result<OperationReply, ResumeError> {
    execute_with(
        registration,
        implementation,
        saved,
        resume,
        lifetimes,
        registry,
        execution,
        validation,
        |reply, validation| {
            super::suspending::validate_reply(
                &reply,
                saved.parent,
                saved.context,
                registry,
                output_sources,
                validation,
            )?;
            Ok(reply)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_with<S: DiagnosticSourceResolver, T>(
    registration: &Registration<'_>,
    implementation: Digest,
    saved: &SavedAwait<'_, S>,
    resume: &Resume,
    lifetimes: &mut RequestLifetimes,
    registry: &SchemaRegistry,
    execution: &mut Budget,
    validation: &mut Budget,
    admit: impl FnOnce(OperationReply, &mut Budget) -> Result<T, ResumeError>,
) -> Result<T, ResumeError> {
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
    let reply = run_with_limits(
        saved.parent.limits,
        execution,
        |budget| (registration.resume)(saved.parent, resume, registry, budget),
        |reply| match reply {
            OperationReply::Result(OperationResult::Stopped { reason, .. }) => Some(*reason),
            _ => None,
        },
    )?;
    admit(reply, validation)
}
