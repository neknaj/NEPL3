//! Host-selected output contracts for terminal provider results.
use super::*;
use crate::{
    diagnostic::validation::{DiagnosticSourceResolver, ReportValidationError},
    schema::{OperationDescriptor, SchemaDescriptor, SchemaError, SchemaRegistry},
    source::SourceStore,
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
        self.validate_contract(operation, registry, sources, budget)?;
        Ok(())
    }

    fn validate_contract<'a>(
        &self,
        operation: &OperationRef,
        registry: &'a SchemaRegistry,
        sources: &impl DiagnosticSourceResolver,
        budget: &mut Budget,
    ) -> Result<(&'a SchemaDescriptor, &'a OperationDescriptor), ResultValidationError> {
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
        Ok((descriptor, selected))
    }
}

/// Owns an immutable terminal value validated against a borrowed registry and
/// exact SourceStore. Both scopes remain immutably borrowed for its lifetime.
/// Request ID, execution context and lifetime transitions require host checks.
/// Custom resolvers are excluded: their grants may use interior mutable state.
pub struct ValidatedResult<'a> {
    result: OperationResult<TypedValue>,
    registry: &'a SchemaRegistry,
    sources: &'a SourceStore,
    schema: &'a SchemaDescriptor,
    schema_digest: Digest,
    operation: &'a OperationDescriptor,
}

impl<'a> ValidatedResult<'a> {
    /// Perform complete value/report validation once. A failed construction
    /// consumes the input and publishes no validation proof.
    pub fn new(
        result: OperationResult<TypedValue>,
        operation: &OperationRef,
        registry: &'a SchemaRegistry,
        sources: &'a SourceStore,
        budget: &mut Budget,
    ) -> Result<Self, ResultValidationError> {
        let schema_digest = operation.schema.digest;
        let (schema, operation) = result.validate_contract(operation, registry, sources, budget)?;
        Ok(Self {
            result,
            registry,
            sources,
            schema,
            schema_digest,
            operation,
        })
    }

    pub fn result(&self) -> &OperationResult<TypedValue> {
        &self.result
    }

    /// Reuse the proof only for the same registry, source grant collection and
    /// operation. Other scopes receive full validation and do not change this
    /// proof. Pointer comparisons identify live borrowed objects, not caches of
    /// addresses whose owners may have been dropped or mutated.
    pub fn validate_for(
        &self,
        operation: &OperationRef,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(), ResultValidationError> {
        budget.charge(Resource::Work, 1)?;
        if core::ptr::eq(self.registry, registry) && core::ptr::eq(self.sources, sources) {
            budget.charge(
                Resource::Work,
                (operation.name.len() as u64)
                    .saturating_add(self.operation.name.len() as u64)
                    .saturating_add(operation.schema.package.len() as u64)
                    .saturating_add(self.schema.package.len() as u64)
                    .saturating_add(40),
            )?;
            if operation.name == self.operation.name
                && operation.schema.package == self.schema.package
                && operation.schema.revision == self.schema.revision
                && operation.schema.digest == self.schema_digest
            {
                return Ok(());
            }
        }
        self.result
            .validate_for(operation, registry, sources, budget)
    }

    /// Consume the proof. Mutating the returned value requires fresh validation.
    pub fn into_inner(self) -> OperationResult<TypedValue> {
        self.result
    }
}
