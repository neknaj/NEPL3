//! Host-selected output contracts for terminal provider results.
use super::*;
use crate::{
    diagnostic::validation::{DiagnosticSourceResolver, ReportValidationError},
    schema::{SchemaError, SchemaRegistry},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResultValidationError {
    Stopped(StopReason),
    Schema(SchemaError),
    Report(ReportValidationError),
    UnknownOperation,
    TraceOverflow,
}
impl From<StopReason> for ResultValidationError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<SchemaError> for ResultValidationError {
    fn from(error: SchemaError) -> Self {
        match error {
            SchemaError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Schema(error),
        }
    }
}
impl From<ReportValidationError> for ResultValidationError {
    fn from(error: ReportValidationError) -> Self {
        match error {
            ReportValidationError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Report(error),
        }
    }
}

impl OperationResult<TypedValue> {
    /// Validate against the operation saved by the host and its authorized
    /// diagnostic source resolver. Partial results retain the same output type.
    /// Invalid and Stopped remain terminal outcomes, including their partials.
    /// This checks structural output/report contracts; domain invariants,
    /// request correlation and accounting of actual remote work belong to the
    /// host. Claimed report Usage is never absorbed into the caller's budget.
    pub fn validate_for(
        &self,
        operation: &OperationRef,
        registry: &SchemaRegistry,
        sources: &impl DiagnosticSourceResolver,
        budget: &mut Budget,
    ) -> Result<(), ResultValidationError> {
        budget.poll()?;
        if !registry.is_finalized() {
            return Err(SchemaError::Unfinalized.into());
        }
        let descriptor = registry
            .descriptor(&operation.schema)
            .ok_or(SchemaError::UnknownSchema)?;
        let mut selected = None;
        for candidate in &descriptor.operations {
            budget.charge(
                Resource::Work,
                (candidate.name.len() as u64)
                    .saturating_add(operation.name.len() as u64)
                    .saturating_add(1),
            )?;
            if candidate.name == operation.name {
                selected = Some(candidate);
                break;
            }
        }
        let selected = selected.ok_or(ResultValidationError::UnknownOperation)?;
        let (value, report, stopped) = match self {
            Self::Complete { value, report } => (Some(value), report, false),
            Self::Invalid { partial, report } => (partial.as_ref(), report, false),
            Self::Stopped {
                partial, report, ..
            } => (partial.as_ref(), report, true),
        };
        if report.trace_overflow.is_some() && !stopped {
            return Err(ResultValidationError::TraceOverflow);
        }
        if let Some(value) = value {
            registry.validate_typed_as(&selected.output, value, budget)?;
        }
        report.validate_with_sources(sources, registry, budget)?;
        Ok(())
    }
}
