//! Host-owned request correlation. Transport, schema, capability and dependency
//! result validation are performed by the host before committing transitions.
use super::*;
use alloc::boxed::Box;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifetimeError {
    Stopped(StopReason),
    Binding(ContinuationError),
    Closed,
    DuplicateRequest,
    UnknownRequest,
    Phase,
    Capacity,
}
impl From<StopReason> for LifetimeError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<ContinuationError> for LifetimeError {
    fn from(v: ContinuationError) -> Self {
        Self::Binding(v)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestPhase {
    Running,
    Awaiting,
    Finished,
    Cancelled,
}

enum State {
    Running,
    Awaiting {
        continuation: Box<Continuation>,
        dependencies: usize,
    },
    Finished,
    Cancelled,
}
struct Entry {
    id: u64,
    provider: OperationRef,
    snapshot: Digest,
    state: State,
}

/// IDs remain reserved until this connection is discarded. This prevents late
/// replies from being attributed to a different request that reused an ID.
#[derive(Default)]
pub struct RequestLifetimes {
    entries: Vec<Entry>,
    closed: bool,
}
impl RequestLifetimes {
    fn locate(&self, id: u64, b: &mut Budget) -> Result<Result<usize, usize>, LifetimeError> {
        b.poll()?;
        let (mut lo, mut hi) = (0, self.entries.len());
        while lo < hi {
            b.charge(Resource::Work, 1)?;
            let mid = lo + (hi - lo) / 2;
            match self.entries[mid].id.cmp(&id) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => return Ok(Ok(mid)),
            }
        }
        Ok(Err(lo))
    }
    fn active(&self, id: u64, b: &mut Budget) -> Result<usize, LifetimeError> {
        b.poll()?;
        if self.closed {
            return Err(LifetimeError::Closed);
        }
        self.locate(id, b)?
            .map_err(|_| LifetimeError::UnknownRequest)
    }
    /// Register only after the host accepts the Invoke and its resource grants.
    pub fn begin(
        &mut self,
        id: u64,
        provider: OperationRef,
        snapshot: Digest,
        b: &mut Budget,
    ) -> Result<(), LifetimeError> {
        b.poll()?;
        if self.closed {
            return Err(LifetimeError::Closed);
        }
        let index = self
            .locate(id, b)?
            .err()
            .ok_or(LifetimeError::DuplicateRequest)?;
        b.charge(Resource::Work, (self.entries.len() - index) as u64 + 1)?;
        if self.entries.len() == self.entries.capacity() {
            let capacity = self
                .entries
                .capacity()
                .checked_mul(2)
                .ok_or(LifetimeError::Capacity)?
                .max(1);
            let extra = capacity - self.entries.len();
            let bytes = extra
                .checked_mul(core::mem::size_of::<Entry>())
                .ok_or(LifetimeError::Capacity)?;
            b.charge(Resource::AllocationUnits, bytes as u64)?;
            // Capacity growth may move every existing entry.
            b.charge(Resource::Work, self.entries.len() as u64)?;
            self.entries
                .try_reserve_exact(extra)
                .map_err(|_| LifetimeError::Capacity)?;
        }
        self.entries.insert(
            index,
            Entry {
                id,
                provider,
                snapshot,
                state: State::Running,
            },
        );
        Ok(())
    }
    /// The caller has checked the Await report, dependency allowlist and graph.
    pub fn suspend(
        &mut self,
        id: u64,
        continuation: Continuation,
        dependencies: usize,
        b: &mut Budget,
    ) -> Result<(), LifetimeError> {
        let index = self.active(id, b)?;
        let entry = &mut self.entries[index];
        if !matches!(entry.state, State::Running) {
            return Err(LifetimeError::Phase);
        }
        continuation.check_binding(&entry.provider, id, entry.snapshot, b)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Continuation>() as u64,
        )?;
        entry.state = State::Awaiting {
            continuation: Box::new(continuation),
            dependencies,
        };
        Ok(())
    }
    /// Inspect correlation without consuming the saved Await. The host performs
    /// dependency output/source/budget checks before calling `resume`.
    pub fn check_resume(&self, resume: &Resume, b: &mut Budget) -> Result<(), LifetimeError> {
        let index = self.active(resume.request_id, b)?;
        let State::Awaiting {
            continuation,
            dependencies,
        } = &self.entries[index].state
        else {
            return Err(LifetimeError::Phase);
        };
        resume.check_binding(continuation, *dependencies, b)?;
        Ok(())
    }
    /// Consume a checked Await exactly once, immediately before provider resume.
    pub fn resume(&mut self, resume: &Resume, b: &mut Budget) -> Result<(), LifetimeError> {
        self.check_resume(resume, b)?;
        let index = self.active(resume.request_id, b)?;
        self.entries[index].state = State::Running;
        Ok(())
    }
    /// Finish after validating the terminal reply against its saved invocation.
    pub fn finish(&mut self, id: u64, b: &mut Budget) -> Result<(), LifetimeError> {
        let index = self.active(id, b)?;
        if !matches!(self.entries[index].state, State::Running) {
            return Err(LifetimeError::Phase);
        }
        self.entries[index].state = State::Finished;
        Ok(())
    }
    pub fn cancel(&mut self, id: u64, b: &mut Budget) -> Result<(), LifetimeError> {
        let index = self.active(id, b)?;
        if !matches!(
            self.entries[index].state,
            State::Running | State::Awaiting { .. }
        ) {
            return Err(LifetimeError::Phase);
        }
        self.entries[index].state = State::Cancelled;
        Ok(())
    }
    pub fn phase(&self, id: u64, b: &mut Budget) -> Result<RequestPhase, LifetimeError> {
        let index = self
            .locate(id, b)?
            .map_err(|_| LifetimeError::UnknownRequest)?;
        Ok(match self.entries[index].state {
            State::Running => RequestPhase::Running,
            State::Awaiting { .. } => RequestPhase::Awaiting,
            State::Finished => RequestPhase::Finished,
            State::Cancelled => RequestPhase::Cancelled,
        })
    }
    /// Allocation-free shutdown remains available after resource exhaustion.
    /// Notify the host of each pending ID to interrupt execution/transport.
    pub fn close(&mut self, mut cancel: impl FnMut(u64)) {
        self.closed = true;
        for entry in &mut self.entries {
            if matches!(entry.state, State::Running | State::Awaiting { .. }) {
                entry.state = State::Cancelled;
                cancel(entry.id);
            }
        }
    }
}
