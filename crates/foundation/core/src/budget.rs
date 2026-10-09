//! Shared, monotonic logical resource accounting. This does not intercept physical OOM.

mod charge;
mod depth;
mod observed;
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
        self.poll()?;
        let previous = self.depth;
        let target = previous.max(base);
        if target > self.limits.depth {
            self.stopped = Some(StopReason::DepthLimit);
            return Err(StopReason::DepthLimit.into());
        }
        self.depth = target;
        self.usage.depth = self.usage.depth.max(target);
        self.observed_depth = self.observed_depth.max(target);
        let result = operation(self);
        self.depth = previous;
        result
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
    /// Nested calls cannot widen the current ceiling. Every Result path restores
    /// the outer limits, while a stop remains sticky and is never rolled back.
    pub fn with_ceiling<T, E: From<StopReason>>(
        &mut self,
        ceiling: Limits,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        self.poll()?;
        let outer = self.limits;
        self.limits = Limits {
            source_bytes: outer.source_bytes.min(ceiling.source_bytes),
            work: outer.work.min(ceiling.work),
            depth: outer.depth.min(ceiling.depth),
            nodes: outer.nodes.min(ceiling.nodes),
            allocation_units: outer.allocation_units.min(ceiling.allocation_units),
            output_bytes: outer.output_bytes.min(ceiling.output_bytes),
            diagnostics: outer.diagnostics.min(ceiling.diagnostics),
            events: outer.events.min(ceiling.events),
        };
        let result = (|| {
            for resource in [
                Resource::SourceBytes,
                Resource::Work,
                Resource::Nodes,
                Resource::AllocationUnits,
                Resource::OutputBytes,
                Resource::Diagnostics,
                Resource::Events,
            ] {
                self.charge(resource, 0)?;
            }
            if self.usage.depth > self.limits.depth {
                return Err(self.stop(StopReason::DepthLimit).into());
            }
            operation(self)
        })();
        self.limits = outer;
        result
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
    /// Measure a validation operation's relative depth without resetting Usage.
    /// Nested measurements contribute to their enclosing measurement. Callers
    /// use pure validation operations which never replace the Budget itself.
    pub fn measure_depth<T, E: From<StopReason>>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<(T, u64), E> {
        self.poll()?;
        let base = self.depth;
        let previous = self.observed_depth;
        self.observed_depth = base;
        let result = operation(self);
        let measured = self.observed_depth;
        self.observed_depth = previous.max(measured);
        let value = result?;
        self.poll()?;
        Ok((value, measured.saturating_sub(base)))
    }
    /// Restores current depth on either normal success or failure; cumulative work remains charged.
    pub fn with_depth<T, E: From<StopReason>>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        self.poll()?;
        self.observe_depth(1)?;
        let previous = self.depth;
        let next = previous.checked_add(1).ok_or(StopReason::DepthLimit)?;
        self.depth = next;
        self.usage.depth = self.usage.depth.max(next);
        let result = operation(self);
        // A host callback can replace the budget. Restore the caller's depth
        // rather than subtracting from an untrusted post-callback value.
        self.depth = previous;
        result
    }
}

#[cfg(test)]
mod tests;
