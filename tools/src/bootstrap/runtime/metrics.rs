//! Host-only timing and logical usage deltas; clocks never enter a core crate.
use nepl3_core::budget::{Budget, Usage};
use std::time::{Duration, Instant};
#[derive(Debug, Default)]
pub struct Cost {
    pub calls: u64,
    pub elapsed: Duration,
    pub usage: Usage,
}
impl Cost {
    pub fn measure<T>(&mut self, budget: &mut Budget, run: impl FnOnce(&mut Budget) -> T) -> T {
        let before = budget.usage();
        let started = Instant::now();
        let result = run(budget);
        let after = budget.usage();
        self.calls += 1;
        self.elapsed += started.elapsed();
        self.usage.source_bytes += after.source_bytes - before.source_bytes;
        self.usage.work += after.work - before.work;
        self.usage.allocation_units += after.allocation_units - before.allocation_units;
        self.usage.nodes += after.nodes - before.nodes;
        self.usage.output_bytes += after.output_bytes - before.output_bytes;
        self.usage.diagnostics += after.diagnostics - before.diagnostics;
        self.usage.events += after.events - before.events;
        self.usage.depth = self.usage.depth.max(after.depth);
        result
    }
}
#[derive(Debug, Default)]
pub struct Metrics {
    pub inline_host: bool,
    /// Profiling only: isolated budgets measure one copy of actual owned values.
    /// These costs overlap and are not added to the operation's real usage.
    pub probe_continuations: bool,
    pub reader_continuation_copy: Cost,
    pub tokenizer_continuation_copy: Cost,
    pub initial: Cost,
    pub provider_sources: Cost,
    pub provider_context: Cost,
    pub provider_read: Cost,
    pub resume: Cost,
    pub reservation: Cost,
}
