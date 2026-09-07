//! Explicit synchronous dispatch of reader and projected head operations.
use super::{ParseError, ParseReply};
use crate::profile::ProviderRequirement;
use nepl3_core::{
    budget::Budget,
    source::{SourceAdmission, SourceReservation},
};
use nepl3_reader::{model::ProviderCall, runtime::ProviderReply, tokenizer::ReservationRequest};

/// An operation-local host adapter. The adapter must match `requirement` against
/// its own implementation catalog before invoking code. Returning `None` leaves
/// the request pending for the ordinary owned continuation boundary.
///
/// Replies always pass through the same reader/tokenizer validation as `resume`.
/// The supplied budget/admission and active caller depth belong to this operation.
pub trait ParseHost {
    /// Receives only the head/completed-child projection, never parser frames or
    /// an implicit full source store. Unsupported operations remain owned AwaitHead.
    fn head(
        &mut self,
        _call: &crate::head::HeadCall,
        _requirement: &ProviderRequirement,
        _budget: &mut Budget,
        _admission: &mut SourceAdmission,
    ) -> Result<Option<crate::head::HeadReply>, ParseError> {
        Ok(None)
    }
    fn provider(
        &mut self,
        call: &ProviderCall,
        requirement: &ProviderRequirement,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<ProviderReply>, ParseError>;
    fn reservation(
        &mut self,
        request: &ReservationRequest,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<SourceReservation>, ParseError>;
}

/// A callback failure retains the ordinary Await/Reserve reply for an explicit
/// host retry. If publishing that continuation stops, `reply` is Stopped and
/// retains the accepted report/source closure instead. No callback error is
/// disguised as a resource limit.
pub struct ParseHostReply {
    pub reply: ParseReply,
    pub host_error: Option<ParseError>,
}
