//! Native Invoke implementations which may request dependency operations.
use super::*;
use crate::suspension::{self, AwaitError};
use nepl3_core::operation::OperationReply;

pub type Operation =
    fn(&Invoke, Digest, &SchemaRegistry, &mut Budget) -> Result<OperationReply, StopReason>;
pub struct Registration<'a> {
    pub operation: &'a OperationRef,
    pub implementation: Digest,
    pub invoke: Operation,
}
#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Dispatch(DispatchError),
    Await(AwaitError),
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
    validate_reply(&reply, request, context, registry, sources, validation)?;
    Ok(reply)
}

pub(super) fn validate_reply(
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
