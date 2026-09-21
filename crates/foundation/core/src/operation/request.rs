//! Input admission against an operation selected independently by the host.
use super::*;
use crate::schema::{SchemaError, SchemaRegistry};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputValidationError {
    Stopped(StopReason),
    Schema(SchemaError),
    OperationMismatch,
    UnknownOperation,
}
impl From<StopReason> for InputValidationError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<SchemaError> for InputValidationError {
    fn from(error: SchemaError) -> Self {
        match error {
            SchemaError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Schema(error),
        }
    }
}
impl Invoke {
    /// Check the saved dispatch selection and the descriptor's input contract.
    /// Environment receives structural schema validation; its domain semantics,
    /// capability grants, source/resource admission and request lifetime belong
    /// to the host. This method neither executes a callback nor trusts supplied
    /// Limits as permission to widen the caller's budget.
    pub fn validate_input(
        &self,
        selected: &OperationRef,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), InputValidationError> {
        budget.charge(
            Resource::Work,
            (self.operation.name.len() as u64)
                .saturating_add(selected.name.len() as u64)
                .saturating_add(self.operation.schema.package.len() as u64)
                .saturating_add(selected.schema.package.len() as u64)
                .saturating_add(80),
        )?;
        if self.operation != *selected {
            return Err(InputValidationError::OperationMismatch);
        }
        if !registry.is_finalized() {
            return Err(SchemaError::Unfinalized.into());
        }
        let descriptor = registry
            .descriptor(&selected.schema)
            .ok_or(SchemaError::UnknownSchema)?;
        for candidate in &descriptor.operations {
            budget.charge(
                Resource::Work,
                (candidate.name.len() as u64)
                    .saturating_add(selected.name.len() as u64)
                    .saturating_add(1),
            )?;
            if candidate.name == selected.name {
                registry.validate_typed_as(&candidate.input, &self.input, budget)?;
                registry.validate_typed(&self.environment, budget)?;
                return Ok(());
            }
        }
        Err(InputValidationError::UnknownOperation)
    }
}
