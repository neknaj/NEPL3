//! Scoped owned-input attempts sharing the supervisor's one cleanup slot.
use super::Supervisor;
use crate::doc::math::display::{
    PreparedDisplay,
    process::{Config, Failure, owned as transport},
    request::{self, Controls},
};
use nepl3_core::budget::{Budget, StopReason};
use std::io;

// Keep native ownership inline; no new allocation is introduced after launch.
#[allow(clippy::large_enum_variant)]
#[must_use]
pub enum OwnedStart<'s, 'c> {
    Running(OwnedSession<'s, 'c>),
    Busy(PreparedDisplay),
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
#[allow(clippy::large_enum_variant)]
#[must_use]
pub enum OwnedFinished<'c> {
    Decoded(transport::Decoded<'c>),
    Failed(transport::Failed<'c>),
}
// Terminal data stays inline rather than allocating at completion/stop.
#[allow(clippy::large_enum_variant)]
#[must_use]
pub enum OwnedPoll<'c> {
    Pending {
        failure: Option<Failure>,
        termination_error: Option<io::ErrorKind>,
    },
    Finished(OwnedFinished<'c>),
    Consumed,
}
/// Retains the exact cumulative Budget through transport and decoding. Source
/// preparation is owned; Config still borrows the caller's trusted selection.
/// Keep the outer Supervisor alive after a pending session leaves its scope.
/// ```compile_fail
/// use nepl3_tools::doc::math::display::{PreparedDisplay, request::Controls, process::{Config, supervisor::{Supervisor, OwnedStart}}};
/// use nepl3_core::budget::Budget;
/// fn replace(s: &mut Supervisor, p: PreparedDisplay, controls: Controls, config: Config<'_>, b: &mut Budget) {
///     if let OwnedStart::Running(mut session) = s.start_owned(p, controls, 4096, config, b) {
///         b.cancel();
///         let _ = session.poll();
///     }
/// }
/// ```
#[must_use]
pub struct OwnedSession<'s, 'c> {
    supervisor: &'s mut Supervisor,
    budget: &'s mut Budget,
    launch: Option<transport::Launch<'c>>,
}
impl Supervisor {
    /// Busy preserves the original source before validating or charging a new
    /// request. It does not change the prior cleanup or the supplied Budget.
    pub fn start_owned<'s, 'c>(
        &'s mut self,
        prepared: PreparedDisplay,
        controls: Controls,
        request_cap: usize,
        config: Config<'c>,
        budget: &'s mut Budget,
    ) -> OwnedStart<'s, 'c> {
        if self.cleanup.is_some() {
            return OwnedStart::Busy(prepared);
        }
        match transport::start(prepared, controls, request_cap, config, budget) {
            transport::Start::Running(launch) => OwnedStart::Running(OwnedSession {
                supervisor: self,
                budget,
                launch: Some(launch),
            }),
            transport::Start::NotRequested(prepared) => OwnedStart::NotRequested(prepared),
            transport::Start::Rejected { prepared, error } => {
                OwnedStart::Rejected { prepared, error }
            }
            transport::Start::Failed { prepared, failure } => {
                OwnedStart::Failed { prepared, failure }
            }
            transport::Start::Stopped(stop) => OwnedStart::Stopped(stop),
        }
    }
}
impl<'c> OwnedSession<'_, 'c> {
    pub fn cancel(&mut self) {
        if let Some(launch) = &mut self.launch {
            launch.cancel();
        }
    }
    /// Record the parent stop; the next poll or scope exit requests termination.
    pub fn cancel_parent(&mut self) {
        self.budget.cancel();
    }
    /// One transport poll, then only after completion one bounded decode using
    /// the same Budget. Finished is attempt data, not an artifact-success gate.
    pub fn poll(&mut self) -> OwnedPoll<'c> {
        let Some(launch) = &mut self.launch else {
            return OwnedPoll::Consumed;
        };
        match launch.poll(self.budget) {
            transport::Poll::Pending {
                failure,
                termination_error,
            } => OwnedPoll::Pending {
                failure,
                termination_error,
            },
            transport::Poll::Complete(completed) => {
                self.launch = None;
                OwnedPoll::Finished(OwnedFinished::Decoded(completed.decode(self.budget)))
            }
            transport::Poll::Failed(failed) => {
                self.launch = None;
                OwnedPoll::Finished(OwnedFinished::Failed(failed))
            }
            transport::Poll::Consumed => {
                self.launch = None;
                OwnedPoll::Consumed
            }
        }
    }
}
impl Drop for OwnedSession<'_, '_> {
    fn drop(&mut self) {
        if let Some(launch) = self.launch.take() {
            // Empty-slot admission plus exclusive borrowing prevents overwrite.
            self.supervisor.cleanup = launch.into_cleanup(self.budget);
        }
    }
}
