//! Shared, monotonic logical resource accounting. This does not intercept physical OOM.

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
        fn sum(a: u64, b: u64, limit: u64, reason: StopReason) -> Result<u64, StopReason> {
            a.checked_add(b).filter(|v| *v <= limit).ok_or(reason)
        }
        let next = (|| {
            Ok(Usage {
                source_bytes: sum(
                    self.usage.source_bytes,
                    observed.source_bytes,
                    self.limits.source_bytes,
                    StopReason::SourceLimit,
                )?,
                work: sum(
                    self.usage.work,
                    observed.work,
                    self.limits.work,
                    StopReason::WorkLimit,
                )?,
                depth: {
                    let depth = self.usage.depth.max(observed.depth);
                    if depth > self.limits.depth {
                        return Err(StopReason::DepthLimit);
                    }
                    depth
                },
                nodes: sum(
                    self.usage.nodes,
                    observed.nodes,
                    self.limits.nodes,
                    StopReason::NodeLimit,
                )?,
                allocation_units: sum(
                    self.usage.allocation_units,
                    observed.allocation_units,
                    self.limits.allocation_units,
                    StopReason::AllocationLimit,
                )?,
                output_bytes: sum(
                    self.usage.output_bytes,
                    observed.output_bytes,
                    self.limits.output_bytes,
                    StopReason::OutputLimit,
                )?,
                diagnostics: sum(
                    self.usage.diagnostics,
                    observed.diagnostics,
                    self.limits.diagnostics,
                    StopReason::DiagnosticLimit,
                )?,
                events: sum(
                    self.usage.events,
                    observed.events,
                    self.limits.events,
                    StopReason::EventLimit,
                )?,
            })
        })();
        match next {
            Ok(next) => {
                self.usage = next;
                self.poll()
            }
            Err(reason) => Err(self.stop(reason)),
        }
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
        self.poll()?;
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
        let Some(next) = used.checked_add(amount).filter(|v| *v <= limit) else {
            self.stopped = Some(reason);
            return Err(reason);
        };
        *used = next;
        Ok(())
    }
    /// Checks an iterative traversal depth relative to the active caller, recording its high-water mark.
    pub fn observe_depth(&mut self, relative: u64) -> Result<(), StopReason> {
        self.poll()?;
        let Some(depth) = self
            .depth
            .checked_add(relative)
            .filter(|v| *v <= self.limits.depth)
        else {
            self.stopped = Some(StopReason::DepthLimit);
            return Err(StopReason::DepthLimit);
        };
        self.usage.depth = self.usage.depth.max(depth);
        Ok(())
    }
    /// Restores current depth on either normal success or failure; cumulative work remains charged.
    pub fn with_depth<T, E: From<StopReason>>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        self.poll()?;
        self.observe_depth(1)?;
        let next = self.depth.checked_add(1).ok_or(StopReason::DepthLimit)?;
        self.depth = next;
        self.usage.depth = self.usage.depth.max(next);
        let result = operation(self);
        self.depth -= 1;
        result
    }
}
