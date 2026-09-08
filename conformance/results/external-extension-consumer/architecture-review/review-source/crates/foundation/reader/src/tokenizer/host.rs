//! Synchronous host transport, separate from portable tokenizer continuations.
use super::{AcceptedTokenizationReply, ReservationRequest};
use crate::{
    model::ProviderCall,
    runtime::{ProviderReply, ReaderError},
};
use nepl3_core::{
    budget::Budget,
    source::{SourceAdmission, SourceReservation},
};

/// The host selects its registered implementation. None leaves the ordinary
/// owned suspension available. Replies use the existing reader validation.
pub trait TokenizationHost {
    fn provider(
        &mut self,
        call: &ProviderCall,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<ProviderReply>, ReaderError>;
    fn reservation(
        &mut self,
        request: &ReservationRequest,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<SourceReservation>, ReaderError>;
}

pub struct TokenizationHostReply {
    pub reply: AcceptedTokenizationReply,
    /// Non-stopping callback errors and rejected reader replies keep an owned
    /// retry boundary. Invalid supplied reservations return the ordinary
    /// tokenizer error instead; they are not callback transport errors.
    pub host_error: Option<ReaderError>,
}

/// A later Stopped reply cannot conceal replacement of the operation budget.
pub(super) struct IntegrityHost<'a, H> {
    pub inner: &'a mut H,
    pub violated: bool,
}
impl<H: TokenizationHost> TokenizationHost for IntegrityHost<'_, H> {
    fn provider(
        &mut self,
        call: &ProviderCall,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<ProviderReply>, ReaderError> {
        let before = (
            budget.limits(),
            budget.usage(),
            budget.current_depth(),
            budget.poll(),
        );
        let result = self.inner.provider(call, budget, admission);
        self.violated |= budget.limits() != before.0
            || !crate::runtime::usage_at_least(budget.usage(), before.1)
            || budget.current_depth() != before.2
            || before.3.is_err() && budget.poll() != before.3;
        result
    }
    fn reservation(
        &mut self,
        request: &ReservationRequest,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<SourceReservation>, ReaderError> {
        let before = (
            budget.limits(),
            budget.usage(),
            budget.current_depth(),
            budget.poll(),
        );
        let result = self.inner.reservation(request, budget, admission);
        self.violated |= budget.limits() != before.0
            || !crate::runtime::usage_at_least(budget.usage(), before.1)
            || budget.current_depth() != before.2
            || before.3.is_err() && budget.poll() != before.3;
        result
    }
}
