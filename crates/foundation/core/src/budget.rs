//! Shared, monotonic logical resource accounting. This does not intercept physical OOM.

mod ceiling;
mod charge;
mod depth;
mod observed;
mod scope;
#[cfg(kani)]
mod verification;

/// Logical limits for one operation and every nested operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Limits {
    pub source_bytes: u64,
    pub work: u64,
    pub depth: u64,
    pub nodes: u64,
    pub allocation_units: u64,
    pub output_bytes: u64,
    pub diagnostics: u64,
    pub events: u64,
}

/// Actual cumulative charges; depth is the maximum concurrently entered depth.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Usage {
    pub source_bytes: u64,
    pub work: u64,
    pub depth: u64,
    pub nodes: u64,
    pub allocation_units: u64,
    pub output_bytes: u64,
    pub diagnostics: u64,
    pub events: u64,
}

/// A stopped computation is not a successful partial value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    Cancelled,
    SourceLimit,
    WorkLimit,
    DepthLimit,
    NodeLimit,
    AllocationLimit,
    OutputLimit,
    DiagnosticLimit,
    EventLimit,
}

/// Cumulative resources; depth uses [`Budget::with_depth`] instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resource {
    SourceBytes,
    Work,
    Nodes,
    AllocationUnits,
    OutputBytes,
    Diagnostics,
    Events,
}

/// Observation of one scoped callback, including a callback that stopped its
/// budget. Retaining output is not certification of semantic success.
#[derive(Debug, Eq, PartialEq)]
#[must_use = "inspect both callback output and stopped before admitting a result"]
pub struct DepthObservation<T> {
    pub output: T,
    pub relative_depth: u64,
    pub stopped: Option<StopReason>,
}

/// Passed by mutable borrow through nested operations; never cloned or rolled back.
#[derive(Debug)]
pub struct Budget {
    limits: Limits,
    usage: Usage,
    depth: u64,
    stopped: Option<StopReason>,
    observed_depth: u64,
}

impl Budget {
    pub fn current_depth(&self) -> u64 {
        self.depth
    }
    /// Hosts restore a saved, validated provider depth while resolving a suspended call.
    pub fn with_depth_at_least<T, E: From<StopReason>>(
        &mut self,
        base: u64,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        let previous = self.depth;
        self.enter_depth(depth::entry::Target::AtLeast(base))?;
        let guard = scope::Depth {
            budget: self,
            previous,
        };
        operation(guard.budget)
    }
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            usage: Usage::default(),
            depth: 0,
            stopped: None,
            observed_depth: 0,
        }
    }
    pub fn usage(&self) -> Usage {
        self.usage
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    /// Temporarily lower resource ceilings without changing observed Usage.
    /// Nested calls cannot widen the current ceiling. Result paths and Rust
    /// unwinding restore outer limits; stops and charges are never rolled back.
    pub fn with_ceiling<T, E: From<StopReason>>(
        &mut self,
        ceiling: Limits,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        let outer = self.limits;
        match ceiling::enter(outer, ceiling, self.usage, self.stopped) {
            Ok(limits) => self.limits = limits,
            Err(reason) => return Err(self.stop(reason).into()),
        }
        let guard = scope::Ceiling {
            budget: self,
            previous: outer,
        };
        operation(guard.budget)
    }
    /// Record already completed work observed by an authorized host, including
    /// a delegated operation that finished after this budget stopped locally.
    /// This grants no permission to execute more work and does not authenticate
    /// a remote claim. The caller must verify the saved grant and observation.
    /// All bounds are checked before recording; an existing stop is preserved.
    pub fn record_observed_usage(&mut self, observed: Usage) -> Result<(), StopReason> {
        let next = observed::record(
            self.usage,
            observed,
            self.limits,
            self.observed_depth,
            self.stopped,
        );
        self.usage = next.usage;
        self.observed_depth = next.measured;
        self.stopped = next.stopped;
        self.poll()
    }

    /// Records a validated nested operation's stop without replacing an earlier cause.
    pub fn stop(&mut self, reason: StopReason) -> StopReason {
        *self.stopped.get_or_insert(reason)
    }
    pub fn cancel(&mut self) {
        if self.stopped.is_none() {
            self.stopped = Some(StopReason::Cancelled);
        }
    }
    pub fn poll(&self) -> Result<(), StopReason> {
        match self.stopped {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }
    /// Overflow is treated as exceeding the corresponding limit. Failed charges do not mutate usage.
    pub fn charge(&mut self, resource: Resource, amount: u64) -> Result<(), StopReason> {
        let (used, limit, reason) = match resource {
            Resource::SourceBytes => (
                &mut self.usage.source_bytes,
                self.limits.source_bytes,
                StopReason::SourceLimit,
            ),
            Resource::Work => (
                &mut self.usage.work,
                self.limits.work,
                StopReason::WorkLimit,
            ),
            Resource::Nodes => (
                &mut self.usage.nodes,
                self.limits.nodes,
                StopReason::NodeLimit,
            ),
            Resource::AllocationUnits => (
                &mut self.usage.allocation_units,
                self.limits.allocation_units,
                StopReason::AllocationLimit,
            ),
            Resource::OutputBytes => (
                &mut self.usage.output_bytes,
                self.limits.output_bytes,
                StopReason::OutputLimit,
            ),
            Resource::Diagnostics => (
                &mut self.usage.diagnostics,
                self.limits.diagnostics,
                StopReason::DiagnosticLimit,
            ),
            Resource::Events => (
                &mut self.usage.events,
                self.limits.events,
                StopReason::EventLimit,
            ),
        };
        match charge::transition(*used, limit, amount, self.stopped, reason) {
            charge::Charge::Charged { used: next } => {
                *used = next;
                Ok(())
            }
            charge::Charge::Stopped { reason } => {
                self.stopped = Some(reason);
                Err(reason)
            }
        }
    }
    /// Checks an iterative traversal depth relative to the active caller, recording its high-water mark.
    pub fn observe_depth(&mut self, relative: u64) -> Result<(), StopReason> {
        match depth::observe(
            self.depth,
            relative,
            self.limits.depth,
            self.usage.depth,
            self.observed_depth,
            self.stopped,
        ) {
            depth::Observation::Observed { usage, measured } => {
                self.usage.depth = usage;
                self.observed_depth = measured;
                Ok(())
            }
            depth::Observation::Stopped { reason } => {
                self.stopped = Some(reason);
                Err(reason)
            }
        }
    }
    fn enter_depth(&mut self, target: depth::entry::Target) -> Result<(), StopReason> {
        match depth::entry::enter(
            self.depth,
            target,
            self.limits.depth,
            self.usage.depth,
            self.observed_depth,
            self.stopped,
        ) {
            depth::entry::Entry::Entered {
                active,
                usage,
                measured,
            } => {
                self.depth = active;
                self.usage.depth = usage;
                self.observed_depth = measured;
                Ok(())
            }
            depth::entry::Entry::Stopped(reason) => {
                self.stopped = Some(reason);
                Err(reason)
            }
        }
    }
    /// Measure a validation operation's relative depth without resetting Usage.
    /// Nested measurements contribute to their enclosing measurement. Callers
    /// use pure validation operations which never replace the Budget itself.
    pub fn measure_depth<T, E: From<StopReason>>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<(T, u64), E> {
        let observed = self.observe_depth_scope(operation)?;
        let value = observed.output?;
        self.poll()?;
        Ok((value, observed.relative_depth))
    }
    /// Observe fresh relative depth while retaining callback output and its stop.
    /// Historical Usage is unchanged and does not enter this interval's peak.
    /// A preexisting stop prevents the callback; a stop reached by the callback
    /// is returned in the observation rather than discarding already produced
    /// output. This does not admit that output or authorize resuming a budget.
    /// The trusted callback must not replace the Budget itself.
    pub fn observe_depth_scope<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> Result<DepthObservation<T>, StopReason> {
        self.poll()?;
        let base = self.depth;
        let previous = self.observed_depth;
        self.observed_depth = base;
        let guard = scope::Measurement {
            budget: self,
            base,
            previous,
        };
        let output = operation(guard.budget);
        let completed = depth::measurement::finish(base, previous, guard.budget.observed_depth);
        drop(guard);
        Ok(DepthObservation {
            output,
            relative_depth: completed.relative,
            stopped: self.poll().err(),
        })
    }
    /// Restores current depth on success, failure, or Rust unwinding; work remains charged.
    pub fn with_depth<T, E: From<StopReason>>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        let previous = self.depth;
        self.enter_depth(depth::entry::Target::Next)?;
        // A host callback can replace the budget. Restore the caller's depth
        // rather than subtracting from an untrusted post-callback value.
        let guard = scope::Depth {
            budget: self,
            previous,
        };
        operation(guard.budget)
    }
}

#[cfg(test)]
mod tests;
