//! Synchronous host scheduling over the existing scoped transport owner.
use super::{StartError, Supervisor};
use crate::doc::math::display::{
    process::{Config, Failure, driver, reply::Reply},
    request::PreparedRequest,
};
use nepl3_core::budget::Budget;
use std::io;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunControl {
    Continue,
    /// Abandon this attempt; retained cleanup cannot resume generation.
    Abort,
    /// Stop the original operation ledger as well as this attempt.
    CancelParent,
}
#[derive(Debug)]
pub enum RunError {
    Start(StartError),
    Driver(driver::DriverError),
    /// Transport is not complete. The external supervisor retains cleanup.
    PendingCleanup {
        failure: Option<Failure>,
        termination_error: Option<io::ErrorKind>,
    },
    Aborted,
    ParentCancelled,
    Consumed,
}
impl Supervisor {
    /// Drive a single synchronous host attempt. `wait` schedules healthy pending
    /// work (for example by sleeping) and may abandon the attempt. There is no
    /// built-in busy wait, fresh Budget, cleanup deadline or background executor.
    /// The caller must retain this Supervisor and poll_cleanup after any error
    /// while has_pending_cleanup is true. Abort is not resumable generation.
    ///
    /// Busy refuses native admission before start validates or charges this
    /// request; upstream source/request preparation may already have charged it.
    /// Reply means terminal direct-child/pipe transport and decoded attempt data,
    /// not visual success, worker cleanup, or artifact acceptance. Inspect the
    /// reply's outcome and termination_failure through the generation policy.
    pub fn run<'a>(
        &mut self,
        request: &'a PreparedRequest<'_>,
        config: Config<'a>,
        budget: &mut Budget,
        wait: &mut impl FnMut() -> RunControl,
    ) -> Result<Reply<'a>, RunError> {
        let mut session = self
            .start(request, config, budget)
            .map_err(RunError::Start)?;
        loop {
            match session.poll() {
                driver::Poll::Finished(result) => return result.map_err(RunError::Driver),
                driver::Poll::Consumed => return Err(RunError::Consumed),
                driver::Poll::Pending {
                    failure,
                    termination_error,
                } if failure.is_some() || termination_error.is_some() => {
                    return Err(RunError::PendingCleanup {
                        failure,
                        termination_error,
                    });
                }
                driver::Poll::Pending { .. } => match wait() {
                    RunControl::Continue => {}
                    RunControl::Abort => return Err(RunError::Aborted),
                    RunControl::CancelParent => {
                        session.cancel_parent();
                        return Err(RunError::ParentCancelled);
                    }
                },
            }
        }
    }
}
