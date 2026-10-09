//! Sealed owned native launch. Preparation and actual transport state are paired
//! at launch, never supplied independently at completion. Not a CLI supervisor,
//! artifact admission, or epoch/profile/resource identity qualification.
use super::{Cleanup, Config, Failure, State, StatePoll, reply};
use crate::doc::math::display::{
    PreparedDisplay,
    request::{
        self, Controls,
        owned::{OwnedRequest, Preparation},
    },
};
use nepl3_core::budget::{Budget, StopReason};
use std::{io, process::ExitStatus};

// Keep the admitted native owner inline: boxing after spawn would add a
// fallible allocation at the ownership-transfer boundary. Native stack size is
// not represented by the logical AllocationUnits ledger.
#[allow(clippy::large_enum_variant)]
#[must_use]
pub enum Start<'c> {
    Running(Launch<'c>),
    NotRequested(PreparedDisplay),
    Rejected {
        prepared: PreparedDisplay,
        error: request::Error,
    },
    Failed {
        prepared: PreparedDisplay,
        failure: Failure,
    },
    Stopped(StopReason),
}
struct Running {
    input: OwnedRequest,
    state: State,
}
/// Owns its original request and native resources without self-reference.
/// Keep polling or transfer cleanup; Drop remains best effort. Callers must use
/// the original cumulative Budget, including after cancellation or deadlines.
#[must_use]
pub struct Launch<'c> {
    running: Option<Running>,
    config: Config<'c>,
}
#[must_use]
pub enum Poll<'c> {
    Pending {
        failure: Option<Failure>,
        termination_error: Option<io::ErrorKind>,
    },
    Complete(Completed<'c>),
    Failed(Failed<'c>),
    Consumed,
}
/// No public construction or reassociation of completion bytes is permitted.
/// ```compile_fail
/// use nepl3_tools::doc::math::display::process::{Config, owned::Completed};
/// fn replace(c: &mut Completed<'_>, config: Config<'_>) { c.config = config; }
/// ```
/// ```compile_fail
/// use nepl3_tools::doc::math::display::process::owned::Completed;
/// fn replace(c: &mut Completed<'_>) { c.bytes = Vec::new(); }
/// ```
pub struct Completed<'c> {
    input: OwnedRequest,
    config: Config<'c>,
    bytes: Vec<u8>,
    exit: ExitStatus,
}
pub struct Failed<'c> {
    prepared: PreparedDisplay,
    controls: Controls,
    config: Config<'c>,
    failure: Failure,
    exit: ExitStatus,
}
type Data = (reply::Outcome, Option<reply::Observations>, bool);
/// Decoded attempt data retains its exact original owner and actual launch
/// configuration. It is not a projected/admitted visual or successful artifact.
/// ```compile_fail
/// use nepl3_tools::doc::math::display::{PreparedDisplay, process::owned::Decoded};
/// fn replace(d: &mut Decoded<'_>, p: PreparedDisplay) { d.prepared = p; }
/// ```
#[must_use]
pub struct Decoded<'c> {
    prepared: PreparedDisplay,
    controls: Controls,
    config: Config<'c>,
    exit: ExitStatus,
    result: Result<Data, reply::Error>,
}

pub fn start<'c>(
    prepared: PreparedDisplay,
    controls: Controls,
    request_cap: usize,
    config: Config<'c>,
    b: &mut Budget,
) -> Start<'c> {
    let input = request::owned::prepare(prepared, controls, request_cap, b);
    if let Err(stop) = b.poll() {
        return Start::Stopped(stop);
    }
    match input {
        Preparation::NotRequested(prepared) => Start::NotRequested(prepared),
        Preparation::Rejected {
            owner: prepared,
            error,
        } => Start::Rejected { prepared, error },
        Preparation::Ready(input) => match super::start_state(input.wire(), config, b) {
            Ok(state) => Start::Running(Launch {
                running: Some(Running { input, state }),
                config,
            }),
            Err(failure) => {
                if let Err(stop) = b.poll() {
                    return Start::Stopped(stop);
                }
                if let Failure::Budget(stop) = failure {
                    return Start::Stopped(stop);
                }
                let (prepared, ()) = input.consume(|_| ());
                Start::Failed { prepared, failure }
            }
        },
    }
}
impl<'c> Launch<'c> {
    pub fn cancel(&mut self) {
        if let Some(running) = &mut self.running {
            running.state.cancel();
        }
    }
    pub fn into_cleanup(mut self, b: &Budget) -> Option<Cleanup> {
        self.running
            .take()
            .map(|running| running.state.into_cleanup(b))
    }
    pub fn poll(&mut self, b: &mut Budget) -> Poll<'c> {
        let Some(running) = &mut self.running else {
            return Poll::Consumed;
        };
        match running.state.poll(Some(b)) {
            StatePoll::Pending {
                failure,
                termination_error,
            } => Poll::Pending {
                failure,
                termination_error,
            },
            StatePoll::Complete { bytes, exit } => {
                let Some(Running { input, state: _ }) = self.running.take() else {
                    return Poll::Consumed;
                };
                Poll::Complete(Completed {
                    input,
                    config: self.config,
                    bytes,
                    exit,
                })
            }
            StatePoll::Failed { failure, exit } => {
                let Some(Running { input, state: _ }) = self.running.take() else {
                    return Poll::Consumed;
                };
                let controls = input.controls();
                let (prepared, ()) = input.consume(|_| ());
                Poll::Failed(Failed {
                    prepared,
                    controls,
                    config: self.config,
                    failure,
                    exit,
                })
            }
            StatePoll::Consumed => {
                self.running = None;
                Poll::Consumed
            }
        }
    }
}
impl<'c> Completed<'c> {
    /// Decode only the wire completion sealed to this original launch. The
    /// temporary borrowed request never escapes; no replacement input is accepted.
    /// Use the original cumulative execution Budget, never a fresh ledger.
    pub fn decode(self, b: &mut Budget) -> Decoded<'c> {
        let Self {
            input,
            config,
            bytes,
            exit,
        } = self;
        let controls = input.controls();
        let (prepared, result) = input.consume(|request| {
            reply::decode(
                super::Completed {
                    request,
                    config,
                    bytes,
                    exit,
                },
                b,
            )
            .map(|reply| reply.into_owned_data())
        });
        Decoded {
            prepared,
            controls,
            config,
            exit,
            result,
        }
    }
}
impl<'c> Failed<'c> {
    pub(in crate::doc::math::display) fn into_parts(
        self,
    ) -> (PreparedDisplay, Controls, Config<'c>, Failure, ExitStatus) {
        (
            self.prepared,
            self.controls,
            self.config,
            self.failure,
            self.exit,
        )
    }
    pub fn prepared(&self) -> &PreparedDisplay {
        &self.prepared
    }
    pub fn controls(&self) -> Controls {
        self.controls
    }
    pub fn config(&self) -> Config<'_> {
        self.config
    }
    pub fn failure(&self) -> Failure {
        self.failure
    }
    pub fn exit(&self) -> ExitStatus {
        self.exit
    }
}
impl<'c> Decoded<'c> {
    pub(in crate::doc::math::display) fn into_parts(
        self,
    ) -> (
        PreparedDisplay,
        Controls,
        Config<'c>,
        Result<Data, reply::Error>,
        ExitStatus,
    ) {
        (
            self.prepared,
            self.controls,
            self.config,
            self.result,
            self.exit,
        )
    }
    pub fn prepared(&self) -> &PreparedDisplay {
        &self.prepared
    }
    pub fn controls(&self) -> Controls {
        self.controls
    }
    pub fn config(&self) -> Config<'_> {
        self.config
    }
    pub fn exit(&self) -> ExitStatus {
        self.exit
    }
    pub fn result(
        &self,
    ) -> Result<(&reply::Outcome, Option<&reply::Observations>, bool), &reply::Error> {
        self.result
            .as_ref()
            .map(|(outcome, observations, termination_failure)| {
                (outcome, observations.as_ref(), *termination_failure)
            })
    }
}
