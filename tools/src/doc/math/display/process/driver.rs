//! Borrowed poll-driven native transport and reply decoding.
//! Pending retains Process ownership, including after cancellation, deadline or
//! failed termination. No synchronous finish helper or scheduling watchdog can
//! convert pending cleanup into completion. The borrowed Driver cannot escape
//! generate_owned's temporary request; its cleanup-only owner can be transferred
//! to an externally reserved slot before the synchronous HRTB callback returns.
//! Callers retain the original cumulative Budget for polling and decoding.
use super::{
    Config, Failure, Process,
    reply::{self, Reply},
};
use crate::doc::math::display::request::PreparedRequest;
use nepl3_core::budget::Budget;
use std::io;

#[derive(Debug)]
pub enum DriverError {
    Transport(Failure),
    Decode(reply::Error),
}
// Keep the decoded reply inline: boxing would add a new allocation at a
// completion boundary that must also report exhausted budgets faithfully.
#[allow(clippy::large_enum_variant)]
#[must_use]
pub enum Poll<'a> {
    Pending {
        failure: Option<Failure>,
        termination_error: Option<io::ErrorKind>,
    },
    Finished(Result<Reply<'a>, DriverError>),
    Consumed,
}
/// Keep this owner through Pending. Dropping it inherits Process's best-effort
/// cleanup and possible detached I/O; it never certifies cleanup success.
/// Each poll is one transport poll and, only after Complete, one bounded decode.
/// This is not a hard wall-clock guarantee for OS calls or renderer termination.
/// ```compile_fail
/// use nepl3_tools::doc::math::display::{PreparedDisplay, process::{Config,driver::{self,Driver}}, request::{self,Controls}};
/// fn escape<'a>(prepared: &'a PreparedDisplay, config: Config<'a>, controls: Controls, b: &mut nepl3_core::budget::Budget) -> Option<Driver<'a>> {
///     let request = request::prepare(prepared, controls, 4096, b).ok()??;
///     driver::start(&request, config, b).ok()
/// }
/// ```
#[must_use]
pub struct Driver<'a> {
    process: Process<'a>,
}
pub fn start<'a>(
    request: &'a PreparedRequest<'_>,
    config: Config<'a>,
    b: &mut Budget,
) -> Result<Driver<'a>, DriverError> {
    super::start(request, config, b)
        .map(|process| Driver { process })
        .map_err(DriverError::Transport)
}
impl<'a> Driver<'a> {
    pub fn cancel(&mut self) {
        self.process.cancel();
    }
    /// Move native cleanup into an external, already-available supervisor slot
    /// before returning from a temporary request scope. This cannot retain or
    /// recreate a reply; terminal driver/decode outcomes remain with their caller.
    pub fn into_cleanup(self, b: &Budget) -> super::Cleanup {
        self.process.into_cleanup(b)
    }
    /// Do not early-return on a stopped budget: the process must still be
    /// polled for cleanup. Internal termination_failure remains on Reply and
    /// is separate from direct-child/pipe transport completion.
    pub fn poll(&mut self, b: &mut Budget) -> Poll<'a> {
        match self.process.poll(b) {
            super::Poll::Pending {
                failure,
                termination_error,
            } => Poll::Pending {
                failure,
                termination_error,
            },
            super::Poll::Complete(completed) => {
                Poll::Finished(reply::decode(completed, b).map_err(DriverError::Decode))
            }
            super::Poll::Failed(failure) => Poll::Finished(Err(DriverError::Transport(failure))),
            super::Poll::Consumed => Poll::Consumed,
        }
    }
}
