use super::*;
use crate::runtime::copy::{CopyCost, slot};
use nepl3_core::{
    budget::{Budget, StopReason},
    view::Trivia,
};
impl CopyCost for Trivia {
    fn charge(&self, budget: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        self.span.charge(budget)
    }
}
impl CopyCost for ReservationRequest {
    fn charge(&self, budget: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        self.session_id.charge(budget)?;
        self.snapshot.source_id.0.charge(budget)
    }
}
impl CopyCost for TokenTarget {
    fn charge(&self, budget: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        if let Self::Builtin { token_kind, .. } = self {
            token_kind.schema.charge(budget)?;
        }
        Ok(())
    }
}
impl CopyCost for TokenizationWait {
    fn charge(&self, budget: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        match self {
            Self::Reservation { request } => request.charge(budget),
            Self::Provider { continuation } => continuation.charge(budget),
        }
    }
}
impl CopyCost for TokenizationContinuation {
    fn charge(&self, budget: &mut Budget) -> Result<(), StopReason> {
        self.scope.charge(budget)?;
        slot::<Self>(budget)?;
        self.session_id.charge(budget)?;
        self.reader_schema.charge(budget)?;
        self.request.charge(budget)?;
        self.mode.charge(budget)?;
        self.target.charge(budget)?;
        self.current.charge(budget)?;
        self.trivia.charge(budget)?;
        self.expected.charge(budget)?;
        self.pending.charge(budget)?;
        self.report.charge(budget)
    }
}

impl CopyCost for TokenizationScope {
    fn charge(&self, budget: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(budget)?;
        self.operation_id.charge(budget)?;
        self.snapshot.source_id.0.charge(budget)
    }
}

impl TokenizationContinuation {
    pub fn charge_clone(&self, budget: &mut Budget) -> Result<(), StopReason> {
        self.charge(budget)
    }
    pub fn clone_with_budget(&self, budget: &mut Budget) -> Result<Self, StopReason> {
        self.charge(budget)?;
        Ok(self.clone())
    }
}
