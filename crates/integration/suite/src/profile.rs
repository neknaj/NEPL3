//! Native operation admission using the existing resolved Profile authority.
//!
//! The host supplies callbacks, grants and context identity. This connection
//! checks declared operations and executable identities before scheduling. Full
//! suite bridge selection, environment projection and process capabilities are
//! separate contracts; a resolved parsing projection does not authorize I/O.
use crate::scheduler;
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason},
    diagnostic::{OperationResult, Report},
    operation::Invoke,
    value::TypedValue,
};
use nepl3_engine::profile::{ProfileError, ResolvedParseProfile};

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Profile(ProfileError),
    Implementation,
    Registration,
    Duplicate,
    Limits,
    Execution(scheduler::Failure),
}

impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Borrowed, immutable native bindings for an independently resolved Profile.
/// Additional allowed operations, such as reader callbacks, may be registered
/// through their own dispatch paths. Scheduling can use only these bindings.
/// Every invocation context is SHA-256 of `NEPL3.Suite.Context.v1\0`, the
/// resolved Profile digest (32 bytes), and the host context digest (32 bytes),
/// in that order. Root, dependencies and continuations share this binding.
pub struct NativeOperations<'a> {
    profile: &'a ResolvedParseProfile<'a>,
    registrations: &'a [scheduler::Registration<'a>],
}

impl<'a> NativeOperations<'a> {
    pub fn new(
        profile: &'a ResolvedParseProfile<'a>,
        registrations: &'a [scheduler::Registration<'a>],
        validation: &mut Budget,
    ) -> Result<Self, Error> {
        validation.poll()?;
        for (index, registration) in registrations.iter().enumerate() {
            let operation = registration.invoke.operation;
            let width = (operation.name.len() as u64)
                .saturating_add(operation.schema.package.len() as u64)
                .saturating_add(112);
            validation.charge(Resource::Work, width)?;
            if registration.resume.operation != operation
                || registration.resume.implementation != registration.invoke.implementation
            {
                return Err(Error::Registration);
            }
            let required =
                profile
                    .provider(operation, validation)
                    .map_err(|error| match error {
                        ProfileError::Stopped(reason) => Error::Stopped(reason),
                        error => Error::Profile(error),
                    })?;
            if required.implementation_digest != registration.invoke.implementation {
                return Err(Error::Implementation);
            }
            for prior in &registrations[..index] {
                validation.charge(Resource::Work, width)?;
                if prior.invoke.operation == operation {
                    return Err(Error::Duplicate);
                }
            }
        }
        Ok(Self {
            profile,
            registrations,
        })
    }

    /// Run with Profile ceilings, host grants and the existing shared execution
    /// Budget. Admission failure executes no callback. Scheduler failures retain
    /// accepted child outcomes through Error::Execution without cloning them.
    pub fn run(
        &self,
        root: &Invoke,
        execution: &mut Budget,
        validation: &mut Budget,
        report: impl FnMut(u64, Report),
        cancel: impl FnMut(u64),
    ) -> Result<OperationResult<TypedValue>, Error> {
        execution.poll()?;
        validation.charge(Resource::Work, 16)?;
        let ceiling = self.profile.profile().limits;
        if !within(execution.limits(), ceiling) || !within(root.limits, ceiling) {
            return Err(Error::Limits);
        }
        scheduler::run_scoped(
            self.registrations,
            root,
            self.profile.registry(),
            execution,
            validation,
            report,
            cancel,
            Some(self.profile.digest()),
        )
        .map_err(Error::Execution)
    }
}

fn within(value: Limits, ceiling: Limits) -> bool {
    value.source_bytes <= ceiling.source_bytes
        && value.work <= ceiling.work
        && value.depth <= ceiling.depth
        && value.nodes <= ceiling.nodes
        && value.allocation_units <= ceiling.allocation_units
        && value.output_bytes <= ceiling.output_bytes
        && value.diagnostics <= ceiling.diagnostics
        && value.events <= ceiling.events
}
