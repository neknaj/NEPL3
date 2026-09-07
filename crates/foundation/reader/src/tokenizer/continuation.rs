use super::*;
use crate::runtime::copy::{CopyCost, CopyPurpose, slot};
use nepl3_core::{
    budget::{Budget, StopReason},
    view::Trivia,
};
impl CopyCost for Trivia {
    fn charge_for(&self, budget: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        self.span.charge_for(budget, purpose)
    }
}
impl CopyCost for ReservationRequest {
    fn charge_for(&self, budget: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        self.session_id.charge_for(budget, purpose)?;
        self.snapshot.source_id.0.charge_for(budget, purpose)
    }
}
impl CopyCost for TokenTarget {
    fn charge_for(&self, budget: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        if let Self::Builtin { token_kind, .. } = self {
            token_kind.schema.charge_for(budget, purpose)?;
        }
        Ok(())
    }
}
impl CopyCost for TokenizationWait {
    fn charge_for(&self, budget: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        match self {
            Self::Reservation { request } => request.charge_for(budget, purpose),
            Self::Provider { continuation } => continuation.charge_for(budget, purpose),
        }
    }
}
impl CopyCost for TokenizationContinuation {
    fn charge_for(&self, budget: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        self.scope.charge_for(budget, purpose)?;
        slot::<Self>(budget)?;
        self.session_id.charge_for(budget, purpose)?;
        self.reader_schema.charge_for(budget, purpose)?;
        self.request.charge_for(budget, purpose)?;
        self.mode.charge_for(budget, purpose)?;
        self.target.charge_for(budget, purpose)?;
        self.current.charge_for(budget, purpose)?;
        self.trivia.charge_for(budget, purpose)?;
        self.expected.charge_for(budget, purpose)?;
        self.pending.charge_for(budget, purpose)?;
        self.report.charge_for(budget, purpose)
    }
}

impl CopyCost for TokenizationScope {
    fn charge_for(&self, budget: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        self.operation_id.charge_for(budget, purpose)?;
        self.snapshot.source_id.0.charge_for(budget, purpose)
    }
}

impl TokenizationContinuation {
    pub fn charge_clone(&self, budget: &mut Budget) -> Result<(), StopReason> {
        self.charge(budget)
    }
    pub fn clone_with_budget(&self, budget: &mut Budget) -> Result<Self, StopReason> {
        self.charge_copy(budget)?;
        Ok(self.clone())
    }
}
