//! One native attempt at a time, with an externally retained cleanup slot.
//! This is an ownership boundary, not an event loop or CLI success policy.
use super::{
    Cleanup, CleanupPoll, Config,
    driver::{self, Driver, DriverError},
};
use crate::doc::math::display::request::PreparedRequest;
use nepl3_core::budget::{Budget, StopReason};

#[derive(Debug)]
pub enum StartError {
    CleanupPending,
    Driver(DriverError),
}
/// Retain this owner outside temporary generation callbacks and their results.
/// Dropping the supervisor while occupied remains best effort, not reclamation.
#[derive(Default)]
pub struct Supervisor {
    cleanup: Option<Cleanup>,
}
pub struct Reclamation {
    pub parent_stop: Option<StopReason>,
    pub status: CleanupPoll,
}
impl Supervisor {
    pub fn has_pending_cleanup(&self) -> bool {
        self.cleanup.is_some()
    }
    /// Refuses an occupied slot before admitting any new native resources.
    /// The same cumulative Budget is borrowed for the entire scoped attempt.
    pub fn start<'s, 'a>(
        &'s mut self,
        request: &'a PreparedRequest<'_>,
        config: Config<'a>,
        budget: &'s mut Budget,
    ) -> Result<Session<'s, 'a>, StartError> {
        if self.cleanup.is_some() {
            return Err(StartError::CleanupPending);
        }
        let driver = driver::start(request, config, budget).map_err(StartError::Driver)?;
        Ok(Session {
            supervisor: self,
            budget,
            driver: Some(driver),
        })
    }
    /// One nonblocking application poll; no fresh execution Budget or decoder.
    /// None means no retained cleanup. Reclaimed concerns direct-child/pipes only.
    pub fn poll_cleanup(&mut self) -> Option<Reclamation> {
        let cleanup = self.cleanup.as_mut()?;
        let parent_stop = cleanup.parent_stop();
        let status = cleanup.poll();
        if matches!(
            status,
            CleanupPoll::Reclaimed { .. } | CleanupPoll::Consumed
        ) {
            self.cleanup = None;
        }
        Some(Reclamation {
            parent_stop,
            status,
        })
    }
}
/// A pending attempt automatically transfers into its reserved supervisor slot
/// on scope exit, including callback errors and unwinding. This does not resume
/// abandoned generation and cannot prevent explicit memory leaks or process exit.
#[must_use]
pub struct Session<'s, 'a> {
    supervisor: &'s mut Supervisor,
    budget: &'s mut Budget,
    driver: Option<Driver<'a>>,
}
impl<'a> Session<'_, 'a> {
    pub fn cancel(&mut self) {
        if let Some(driver) = &mut self.driver {
            driver.cancel();
        }
    }
    /// Record cancellation; the next poll or scope exit requests termination.
    pub fn cancel_parent(&mut self) {
        self.budget.cancel();
    }
    pub fn poll(&mut self) -> driver::Poll<'a> {
        let Some(driver) = &mut self.driver else {
            return driver::Poll::Consumed;
        };
        let result = driver.poll(self.budget);
        if matches!(result, driver::Poll::Finished(_) | driver::Poll::Consumed) {
            self.driver = None;
        }
        result
    }
}
impl Drop for Session<'_, '_> {
    fn drop(&mut self) {
        if let Some(driver) = self.driver.take() {
            // Exclusive borrowing plus start's empty-slot check prevents overwrite.
            self.supervisor.cleanup = Some(driver.into_cleanup(self.budget));
        }
    }
}

mod owned;
pub use owned::{OwnedFinished, OwnedPoll, OwnedSession, OwnedStart};

mod run;
pub use run::{RunControl, RunError};
