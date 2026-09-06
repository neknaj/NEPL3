//! Structured operation reports. Hosts supply translated messages, clocks and output sinks.
use crate::budget::{Budget, Resource};
use crate::{
    budget::{StopReason, Usage},
    source::{Span, TextEdit},
    value::{SchemaRef, TypedValue},
};
use alloc::{string::String, vec::Vec};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Information,
    Hint,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Related {
    pub span: Option<Span>,
    pub code: String,
    pub arguments: TypedValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fix {
    pub id: String,
    pub edits: Vec<TextEdit>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub schema: SchemaRef,
    pub code: String,
    pub severity: Severity,
    pub stage: String,
    pub arguments: TypedValue,
    pub primary: Option<Span>,
    pub related: Vec<Related>,
    pub fixes: Vec<Fix>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub schema: SchemaRef,
    pub kind: String,
    pub operation_path: Vec<u64>,
    pub span: Option<Span>,
    pub payload: TypedValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceOverflow {
    pub dropped: u64,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub events: Vec<Event>,
    pub trace_overflow: Option<TraceOverflow>,
    pub usage: Usage,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationResult<T> {
    Complete {
        value: T,
        report: Report,
    },
    Invalid {
        partial: Option<T>,
        report: Report,
    },
    Stopped {
        reason: StopReason,
        partial: Option<T>,
        report: Report,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct ReportCheckpoint {
    diagnostics: usize,
    events: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportError {
    Stopped(StopReason),
    Checkpoint,
}
/// Collects formal diagnostics and events with shared monotonic budget charges.
#[derive(Debug, Default)]
pub struct ReportBuilder {
    report: Report,
    stopped: Option<StopReason>,
}
impl ReportBuilder {
    pub fn checkpoint(&self) -> ReportCheckpoint {
        ReportCheckpoint {
            diagnostics: self.report.diagnostics.len(),
            events: self.report.events.len(),
        }
    }
    /// Candidate facts are removed; resource charges and a stop condition are never rolled back.
    pub fn rollback(&mut self, checkpoint: ReportCheckpoint) -> Result<(), ReportError> {
        if checkpoint.diagnostics > self.report.diagnostics.len()
            || checkpoint.events > self.report.events.len()
        {
            return Err(ReportError::Checkpoint);
        }
        self.report.diagnostics.truncate(checkpoint.diagnostics);
        self.report.events.truncate(checkpoint.events);
        Ok(())
    }
    pub fn diagnostic(
        &mut self,
        diagnostic: Diagnostic,
        budget: &mut Budget,
    ) -> Result<(), ReportError> {
        self.allocate(core::mem::size_of::<Diagnostic>() as u64, budget)?;
        self.charge(Resource::Diagnostics, budget)?;
        self.report.diagnostics.push(diagnostic);
        Ok(())
    }
    pub fn event(&mut self, event: Event, budget: &mut Budget) -> Result<(), ReportError> {
        self.allocate(core::mem::size_of::<Event>() as u64, budget)?;
        self.charge(Resource::Events, budget)?;
        self.report.events.push(event);
        Ok(())
    }
    fn allocate(&mut self, amount: u64, budget: &mut Budget) -> Result<(), ReportError> {
        if let Some(reason) = self.stopped {
            return Err(ReportError::Stopped(reason));
        }
        if let Err(reason) = budget.charge(Resource::AllocationUnits, amount) {
            self.stopped = Some(reason);
            return Err(ReportError::Stopped(reason));
        }
        Ok(())
    }
    fn charge(&mut self, resource: Resource, budget: &mut Budget) -> Result<(), ReportError> {
        if let Some(reason) = self.stopped {
            return Err(ReportError::Stopped(reason));
        }
        if let Err(reason) = budget.charge(resource, 1) {
            if reason == StopReason::EventLimit {
                self.report.trace_overflow = Some(TraceOverflow { dropped: 1 });
            }
            self.stopped = Some(reason);
            return Err(ReportError::Stopped(reason));
        }
        Ok(())
    }
    pub fn finish(mut self, budget: &Budget) -> Report {
        self.report.usage = budget.usage();
        self.report
    }
}
