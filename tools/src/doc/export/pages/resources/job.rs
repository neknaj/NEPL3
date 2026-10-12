//! Host-owned cumulative allowance across explicitly admitted atomic attempts.
//! This retains accounting, not a parser/renderer continuation. A caller must
//! separately retain a valid phase checkpoint and rerun interrupted work.
use nepl3_core::budget::{Budget, Limits, Resource, StopReason, Usage};

/// Unique running authority. No Clone or caller-supplied usage restoration.
#[derive(Debug)]
pub struct Running {
    limits: Limits,
    usage: Usage,
}
#[derive(Debug)]
pub struct Paused {
    running: Running,
    reason: StopReason,
}
#[derive(Debug)]
pub struct Cancelled {
    /// The resource stop, if cancellation followed one.
    pub previous_stop: Option<StopReason>,
    pub limits: Limits,
    pub usage: Usage,
}
#[derive(Debug)]
pub struct RejectedGrant {
    paused: Paused,
}
impl RejectedGrant {
    pub fn into_paused(self) -> Paused {
        self.paused
    }
}
#[derive(Debug)]
pub enum Attempt<T, E> {
    Complete {
        value: T,
        running: Running,
    },
    /// No partial value is admitted, even if the callback returned Ok after a stop.
    Paused {
        paused: Paused,
        error: Option<E>,
    },
    Failed {
        error: E,
        limits: Limits,
        usage: Usage,
    },
    Cancelled(Cancelled),
}

/// A non-replaceable view of one admitted attempt. The underlying Budget is
/// owned by Running::run and cannot be reset or replaced by the callback.
pub struct Meter<'a> {
    budget: &'a mut Budget,
    cancellation: &'a mut bool,
}
impl Meter<'_> {
    pub fn limits(&self) -> Limits {
        self.budget.limits()
    }
    pub fn usage(&self) -> Usage {
        self.budget.usage()
    }
    pub fn poll(&self) -> Result<(), StopReason> {
        if *self.cancellation {
            Err(StopReason::Cancelled)
        } else {
            self.budget.poll()
        }
    }
    pub fn charge(&mut self, resource: Resource, amount: u64) -> Result<(), StopReason> {
        self.poll()?;
        self.budget.charge(resource, amount)
    }
    pub fn observe_depth(&mut self, depth: u64) -> Result<(), StopReason> {
        self.poll()?;
        self.budget.observe_depth(depth)
    }
    pub fn cancel(&mut self) {
        *self.cancellation = true;
        self.budget.cancel();
    }
}

impl Running {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            usage: Usage::default(),
        }
    }
    pub fn usage(&self) -> Usage {
        self.usage
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    pub fn cancel(self) -> Cancelled {
        Cancelled {
            previous_stop: None,
            limits: self.limits,
            usage: self.usage,
        }
    }
    /// Admit at most the remaining job resources before entering the callback.
    /// Depth is a peak, so it is bounded rather than subtracted. Retried work
    /// consumes another attempt and is charged again to this same ledger.
    pub fn run<T, E>(
        mut self,
        phase: Limits,
        operation: impl FnOnce(&mut Meter<'_>) -> Result<T, E>,
    ) -> Attempt<T, E> {
        let mut b = Budget::new(remaining(self.limits, self.usage, phase));
        let mut cancellation = false;
        let result = operation(&mut Meter {
            budget: &mut b,
            cancellation: &mut cancellation,
        });
        // Every charge was bounded by remaining(), so these additions cannot
        // exceed the finite parent limits or overflow. No failed observation
        // can discard already-performed work.
        self.usage = add(self.usage, b.usage());
        if cancellation {
            let mut cancelled = self.cancel();
            cancelled.previous_stop = b.poll().err().filter(|r| *r != StopReason::Cancelled);
            return Attempt::Cancelled(cancelled);
        }
        match b.poll() {
            Err(StopReason::Cancelled) => Attempt::Cancelled(self.cancel()),
            Err(reason) => Attempt::Paused {
                paused: Paused {
                    running: self,
                    reason,
                },
                error: result.err(),
            },
            Ok(()) => match result {
                Ok(value) => Attempt::Complete {
                    value,
                    running: self,
                },
                Err(error) => Attempt::Failed {
                    error,
                    limits: self.limits,
                    usage: self.usage,
                },
            },
        }
    }
}
impl Paused {
    pub fn usage(&self) -> Usage {
        self.running.usage
    }
    pub fn limits(&self) -> Limits {
        self.running.limits
    }
    pub fn reason(&self) -> StopReason {
        self.reason
    }
    /// Consumes this exact paused authority. Rejection returns it unchanged.
    /// An unchanged cumulative ceiling is valid when the next attempt selects
    /// a larger per-phase allowance; increasing only one cap need not suffice.
    pub fn resume(mut self, limits: Limits) -> Result<Running, Box<RejectedGrant>> {
        if !covers(limits, self.running.limits) {
            return Err(Box::new(RejectedGrant { paused: self }));
        }
        self.running.limits = limits;
        Ok(self.running)
    }
    /// Cancellation remains terminal even after an earlier resource stop.
    pub fn cancel(self) -> Cancelled {
        let mut cancelled = self.running.cancel();
        cancelled.previous_stop = Some(self.reason);
        cancelled
    }
}

fn remaining(l: Limits, u: Usage, p: Limits) -> Limits {
    Limits {
        source_bytes: p.source_bytes.min(l.source_bytes - u.source_bytes),
        work: p.work.min(l.work - u.work),
        depth: p.depth.min(l.depth),
        nodes: p.nodes.min(l.nodes - u.nodes),
        allocation_units: p
            .allocation_units
            .min(l.allocation_units - u.allocation_units),
        output_bytes: p.output_bytes.min(l.output_bytes - u.output_bytes),
        diagnostics: p.diagnostics.min(l.diagnostics - u.diagnostics),
        events: p.events.min(l.events - u.events),
    }
}
fn add(a: Usage, b: Usage) -> Usage {
    Usage {
        source_bytes: a.source_bytes + b.source_bytes,
        work: a.work + b.work,
        depth: a.depth.max(b.depth),
        nodes: a.nodes + b.nodes,
        allocation_units: a.allocation_units + b.allocation_units,
        output_bytes: a.output_bytes + b.output_bytes,
        diagnostics: a.diagnostics + b.diagnostics,
        events: a.events + b.events,
    }
}
fn covers(a: Limits, b: Limits) -> bool {
    a.source_bytes >= b.source_bytes
        && a.work >= b.work
        && a.depth >= b.depth
        && a.nodes >= b.nodes
        && a.allocation_units >= b.allocation_units
        && a.output_bytes >= b.output_bytes
        && a.diagnostics >= b.diagnostics
        && a.events >= b.events
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::budget::Resource;
    fn limits(work: u64) -> Limits {
        Limits {
            work,
            depth: 20,
            ..Limits::default()
        }
    }
    #[test]
    fn resume_preserves_usage_and_enforces_remaining_before_work() -> Result<(), String> {
        let running = Running::new(limits(10));
        let Attempt::Paused { paused, .. } = running.run(limits(4), |b| {
            b.charge(Resource::Work, 3)?;
            b.charge(Resource::Work, 2)
        }) else {
            return Err("phase must pause".into());
        };
        assert_eq!(paused.usage().work, 3);
        assert_eq!(paused.reason(), StopReason::WorkLimit);
        // Same job cap, larger explicit phase cap: retry is charged again.
        let running = paused
            .resume(limits(10))
            .map_err(|_| "explicit grant".to_string())?;
        let Attempt::Complete { running, .. } =
            running.run(limits(9), |b| b.charge(Resource::Work, 5))
        else {
            return Err("retry fits".into());
        };
        assert_eq!(running.usage().work, 8);
        let Attempt::Paused { paused, .. } = running.run(limits(9), |b| {
            assert_eq!(b.limits().work, 2);
            b.charge(Resource::Work, 2)?;
            b.charge(Resource::Work, 1)
        }) else {
            return Err("cumulative cap must pause".into());
        };
        assert_eq!(paused.usage().work, 10);
        let paused = paused
            .resume(limits(9))
            .err()
            .ok_or("no shrinking grants")?
            .into_paused();
        assert_eq!(paused.usage().work, 10);
        let running = paused
            .resume(limits(11))
            .map_err(|_| "explicit additional allowance".to_string())?;
        let Attempt::Complete { running, .. } =
            running.run(limits(9), |b| b.charge(Resource::Work, 1))
        else {
            return Err("one extra unit fits".into());
        };
        assert_eq!(running.usage().work, 11);
        Ok(())
    }
    #[test]
    fn stop_cannot_be_reported_as_success_and_cancel_after_stop_is_terminal() -> Result<(), String>
    {
        let Attempt::Paused { paused, error } = Running::new(limits(0)).run(limits(9), |b| {
            assert!(b.charge(Resource::Work, 1).is_err());
            Ok::<_, ()>("not a partial success")
        }) else {
            return Err("stopped Ok must pause".into());
        };
        assert!(error.is_none());
        let cancelled = paused.cancel();
        assert_eq!(cancelled.usage.work, 0);
        // Cancelled deliberately exposes no resume/run operation.
        let Attempt::Cancelled(cancelled) = Running::new(limits(10)).run(limits(10), |b| {
            b.charge(Resource::Work, 2)?;
            b.cancel();
            assert_eq!(b.poll(), Err(StopReason::Cancelled));
            assert_eq!(b.charge(Resource::Work, 0), Err(StopReason::Cancelled));
            assert_eq!(b.observe_depth(0), Err(StopReason::Cancelled));
            Ok::<_, StopReason>(())
        }) else {
            return Err("cancel must be terminal".into());
        };
        assert_eq!(cancelled.usage.work, 2);
        Ok(())
    }
    #[test]
    fn cancellation_after_resource_stop_preserves_cause_and_charges() -> Result<(), String> {
        let Attempt::Cancelled(cancelled) = Running::new(limits(3)).run(limits(3), |b| {
            b.charge(Resource::Work, 2)?;
            assert_eq!(b.charge(Resource::Work, 2), Err(StopReason::WorkLimit));
            b.cancel();
            assert_eq!(b.poll(), Err(StopReason::Cancelled));
            assert_eq!(b.charge(Resource::Work, 0), Err(StopReason::Cancelled));
            assert_eq!(b.observe_depth(0), Err(StopReason::Cancelled));
            Ok::<_, StopReason>(())
        }) else {
            return Err("explicit cancellation must be terminal".into());
        };
        assert_eq!(cancelled.usage.work, 2);
        assert_eq!(cancelled.previous_stop, Some(StopReason::WorkLimit));
        Ok(())
    }
    #[test]
    fn ordinary_failure_retains_usage() -> Result<(), String> {
        let Attempt::Failed { error, usage, .. } = Running::new(limits(3)).run(limits(3), |b| {
            b.charge(Resource::Work, 2)
                .map_err(|_| "fits".to_string())?;
            Err::<(), _>("invalid input".to_string())
        }) else {
            return Err("ordinary failure".into());
        };
        assert_eq!(error, "invalid input");
        assert_eq!(usage.work, 2);
        Ok(())
    }
    #[test]
    fn all_cumulative_resources_keep_their_remaining_allowance() -> Result<(), String> {
        let limits = Limits {
            source_bytes: 3,
            work: 3,
            depth: 3,
            nodes: 3,
            allocation_units: 3,
            output_bytes: 3,
            diagnostics: 3,
            events: 3,
        };
        for resource in [
            Resource::SourceBytes,
            Resource::Work,
            Resource::Nodes,
            Resource::AllocationUnits,
            Resource::OutputBytes,
            Resource::Diagnostics,
            Resource::Events,
        ] {
            let Attempt::Complete { running, .. } =
                Running::new(limits).run(limits, |b| b.charge(resource, 2))
            else {
                return Err("first charge fits".into());
            };
            let Attempt::Paused { paused, .. } = running.run(limits, |b| {
                b.charge(resource, 1)?;
                b.charge(resource, 1)
            }) else {
                return Err("cumulative ceiling".into());
            };
            let mut expected = Budget::new(limits);
            expected
                .charge(resource, 3)
                .map_err(|_| "fits".to_string())?;
            assert_eq!(paused.usage(), expected.usage());
        }
        Ok(())
    }
    #[test]
    fn depth_pause_resume_preserves_peak_including_maximum() -> Result<(), String> {
        let mut cap = limits(10);
        cap.depth = 7;
        let Attempt::Paused { paused, .. } = Running::new(cap).run(cap, |b| {
            b.observe_depth(7)?;
            b.observe_depth(8)
        }) else {
            return Err("depth must pause".into());
        };
        assert_eq!(paused.reason(), StopReason::DepthLimit);
        assert_eq!(paused.usage().depth, 7);
        cap.depth = u64::MAX;
        let running = paused
            .resume(cap)
            .map_err(|_| "expanded depth".to_string())?;
        let Attempt::Complete { running, .. } = running.run(cap, |b| b.observe_depth(u64::MAX))
        else {
            return Err("maximum peak fits".into());
        };
        let Attempt::Complete { running, .. } = running.run(cap, |b| b.observe_depth(1)) else {
            return Err("peak is not cumulative".into());
        };
        assert_eq!(running.usage().depth, u64::MAX);
        Ok(())
    }
    #[test]
    fn every_shrinking_limit_returns_the_original_paused_authority() -> Result<(), String> {
        let cap = Limits {
            source_bytes: 3,
            work: 3,
            depth: 3,
            nodes: 3,
            allocation_units: 3,
            output_bytes: 3,
            diagnostics: 3,
            events: 3,
        };
        for field in 0..8 {
            let Attempt::Paused { paused, .. } = Running::new(cap).run(cap, |b| {
                b.charge(Resource::Work, 2)?;
                b.charge(Resource::Work, 2)
            }) else {
                return Err("pause".into());
            };
            let mut smaller = cap;
            match field {
                0 => smaller.source_bytes = 2,
                1 => smaller.work = 2,
                2 => smaller.depth = 2,
                3 => smaller.nodes = 2,
                4 => smaller.allocation_units = 2,
                5 => smaller.output_bytes = 2,
                6 => smaller.diagnostics = 2,
                _ => smaller.events = 2,
            }
            let paused = paused
                .resume(smaller)
                .err()
                .ok_or("shrinking rejected")?
                .into_paused();
            assert_eq!(paused.limits(), cap);
            assert_eq!(paused.usage().work, 2);
            assert_eq!(paused.reason(), StopReason::WorkLimit);
        }
        Ok(())
    }
    #[test]
    fn maximum_counter_and_depth_are_not_wrapped_or_summed() -> Result<(), String> {
        let Attempt::Complete { running, .. } =
            Running::new(limits(u64::MAX)).run(limits(u64::MAX), |b| {
                b.charge(Resource::Work, u64::MAX)?;
                b.observe_depth(7)
            })
        else {
            return Err("exact max fits".into());
        };
        let Attempt::Paused { paused, .. } = running.run(limits(1), |b| {
            b.observe_depth(8)?;
            b.charge(Resource::Work, 1)
        }) else {
            return Err("no wrap".into());
        };
        assert_eq!(paused.usage().work, u64::MAX);
        assert_eq!(paused.usage().depth, 8);
        assert_eq!(
            paused
                .resume(limits(u64::MAX))
                .map_err(|_| "same cap".to_string())?
                .usage()
                .work,
            u64::MAX
        );
        Ok(())
    }
}
