//! Native Invoke implementations which may request dependency operations.
use super::*;
use crate::suspension::{self, AwaitError};
use nepl3_core::operation::OperationReply;

pub type Operation =
    fn(&Invoke, Digest, &SchemaRegistry, &mut Budget) -> Result<OperationReply, StopReason>;
/// Borrowed implementation whose typed captures remain alive for the entire
/// registration. Dispatch requires no heap allocation for the callback.
pub type Callback<'a> = dyn Fn(&Invoke, Digest, &SchemaRegistry, &mut Budget) -> Result<OperationReply, StopReason>
    + 'a;
pub struct Registration<'a> {
    pub operation: &'a OperationRef,
    pub implementation: Digest,
    pub invoke: &'a Callback<'a>,
}
#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Dispatch(DispatchError),
    Await(AwaitError),
    Preparation(suspension::host::ActivationError),
}

// This short-lived value moves straight into a scheduler frame. Keep admission
// free of an additional fallible heap allocation solely for enum indirection.
#[allow(clippy::large_enum_variant)]
pub(crate) enum PreparedReply<'a, S> {
    Result(OperationResult<TypedValue>),
    Await(suspension::host::PreparedAwait<'a, S>),
}

pub(crate) fn prepare_reply<'a, S: DiagnosticSourceResolver>(
    reply: OperationReply,
    request: &'a Invoke,
    context: Digest,
    registry: &'a SchemaRegistry,
    sources: &'a S,
    validation: &mut Budget,
) -> Result<PreparedReply<'a, S>, Error> {
    match reply {
        OperationReply::Result(result) => {
            result
                .validate_for(&request.operation, registry, sources, validation)
                .map_err(|e| match e {
                    ResultValidationError::Stopped(s) => Error::Stopped(s),
                    e => Error::Dispatch(DispatchError::Output(e)),
                })?;
            Ok(PreparedReply::Result(result))
        }
        reply => {
            suspension::host::prepare_owned(request, context, reply, registry, sources, validation)
                .map(PreparedReply::Await)
                .map_err(|error| match error {
                    suspension::host::ActivationError::Stopped(reason) => Error::Stopped(reason),
                    suspension::host::ActivationError::Await(error) => Error::Await(error),
                    error => Error::Preparation(error),
                })
        }
    }
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// The host selects this registration and computes context from admitted inputs.
/// Returned Await data has passed structural admission. Its dependency grants,
/// graph registration and scheduling still require explicit host decisions.
#[allow(clippy::too_many_arguments)]
pub fn invoke(
    registration: &Registration<'_>,
    implementation: Digest,
    request: &Invoke,
    context: Digest,
    registry: &SchemaRegistry,
    sources: &impl DiagnosticSourceResolver,
    execution: &mut Budget,
    validation: &mut Budget,
) -> Result<OperationReply, Error> {
    invoke_with(
        registration,
        implementation,
        request,
        context,
        registry,
        execution,
        validation,
        |reply, validation| {
            validate_reply(&reply, request, context, registry, sources, validation)?;
            Ok(reply)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn invoke_with<T>(
    registration: &Registration<'_>,
    implementation: Digest,
    request: &Invoke,
    context: Digest,
    registry: &SchemaRegistry,
    execution: &mut Budget,
    validation: &mut Budget,
    admit: impl FnOnce(OperationReply, &mut Budget) -> Result<T, Error>,
) -> Result<T, Error> {
    execution.poll()?;
    validation.charge(Resource::Work, 32)?;
    if registration.implementation != implementation {
        return Err(Error::Dispatch(DispatchError::MissingImplementation));
    }
    request
        .validate_input(registration.operation, registry, validation)
        .map_err(|e| match e {
            InputValidationError::Stopped(s) => Error::Stopped(s),
            e => Error::Dispatch(DispatchError::Input(e)),
        })?;
    let reply = run_with_limits(
        request.limits,
        execution,
        |budget| (registration.invoke)(request, context, registry, budget),
        |reply| match reply {
            OperationReply::Result(OperationResult::Stopped { reason, .. }) => Some(*reason),
            _ => None,
        },
    )?;
    admit(reply, validation)
}

/// Validate a native or decoded portable reply against the immutable request
/// and context retained by the host. Source permissions are supplied separately.
/// This admits structural data; dispatch, grants and lifetime transitions remain
/// the host's responsibility. Report usage never replaces host-side accounting.
pub fn validate_reply(
    reply: &OperationReply,
    request: &Invoke,
    context: Digest,
    registry: &SchemaRegistry,
    sources: &impl DiagnosticSourceResolver,
    validation: &mut Budget,
) -> Result<(), Error> {
    match reply {
        OperationReply::Result(result) => result
            .validate_for(&request.operation, registry, sources, validation)
            .map_err(|e| match e {
                ResultValidationError::Stopped(s) => Error::Stopped(s),
                e => Error::Dispatch(DispatchError::Output(e)),
            })?,
        OperationReply::Await {
            continuation,
            calls,
            report,
        } => {
            suspension::prepare(
                request,
                context,
                continuation,
                calls,
                report,
                registry,
                sources,
                validation,
            )
            .map_err(|e| match e {
                AwaitError::Stopped(s) => Error::Stopped(s),
                e => Error::Await(e),
            })?;
        }
    }
    Ok(())
}
