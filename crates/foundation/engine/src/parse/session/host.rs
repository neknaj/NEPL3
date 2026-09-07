//! Connect native tokenizer calls to the resolved parser provider catalog.
use super::*;
use nepl3_reader::{model::ProviderCall, runtime::ReaderError};

pub(super) struct TokenizerHost<'h, 'p, 'a> {
    pub host: &'h mut dyn super::super::ParseHost,
    pub profile: &'p ResolvedParseProfile<'a>,
    pub error: &'h mut Option<ParseError>,
}
impl TokenizationHost for TokenizerHost<'_, '_, '_> {
    fn provider(
        &mut self,
        call: &ProviderCall,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<ProviderReply>, ReaderError> {
        let operation = match call {
            ProviderCall::Read { operation, .. }
            | ProviderCall::Transform { operation, .. }
            | ProviderCall::Dependent { operation, .. } => operation,
        };
        let result = (|| {
            let requirement = self.profile.provider(operation, budget)?;
            self.host.provider(call, requirement, budget, admission)
        })();
        result.map_err(|error| {
            if let Some(reason) = error.stop_reason() {
                budget.stop(reason);
            }
            *self.error = Some(error);
            ReaderError::Context
        })
    }
    fn reservation(
        &mut self,
        request: &ReservationRequest,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<SourceReservation>, ReaderError> {
        self.host
            .reservation(request, budget, admission)
            .map_err(|error| {
                if let Some(reason) = error.stop_reason() {
                    budget.stop(reason);
                }
                *self.error = Some(error);
                ReaderError::Context
            })
    }
}
