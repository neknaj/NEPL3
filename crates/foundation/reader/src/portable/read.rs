//! Read/Dependent replies are checked against the host's saved dispatch.
//! A borrowed context retains the request lifetime without consuming its slot.
use super::*;
use crate::{
    model::{ProviderCall, ReadReply, ReaderContinuation},
    plan::ProviderSignature,
    runtime::{
        ReaderError,
        validate::{ProviderBoundary, ProviderReplyRef, check_provider},
    },
};
use nepl3_core::source::SourceAdmission;

pub struct ReadReplyContext<'a> {
    pub(crate) continuation: &'a ReaderContinuation,
    pub(crate) signature: &'a ProviderSignature,
    pub(crate) registry: &'a SchemaRegistry,
}

impl ReadReplyContext<'_> {
    /// Apply the native provider checks before accepting a received reply.
    /// Await is handled by operation dispatch; this boundary accepts terminal
    /// replies to the saved Read or Dependent call, just like ReaderSession.
    pub fn validate(
        &self,
        reply: &ReadReply,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), ReaderError> {
        let c = self.continuation;
        let sources = super::transform::dispatch_sources(c, &[], budget, admission)?;
        let snapshot = sources
            .resolve(&c.request.snapshot)
            .ok_or(SourceError::MissingSnapshot)?;
        let boundary = ProviderBoundary {
            signature: self.signature,
            registry: self.registry,
            snapshot,
            declared: &c.request.sources,
            current: &c.current,
        };
        let frame = c.frames.last().ok_or(ReaderError::Continuation)?;
        let depth_base = match &c.pending {
            ProviderCall::Read { depth_base, .. } | ProviderCall::Dependent { depth_base, .. } => {
                *depth_base
            }
            ProviderCall::Transform { .. } => return Err(ReaderError::ProviderContract),
        };
        budget.with_depth_at_least(depth_base, |budget| {
            check_provider(
                &boundary,
                frame.checkpoint.view.elements.len(),
                &c.pending,
                ProviderReplyRef::Read(reply),
                c.usage,
                &sources,
                budget,
                admission,
            )
        })
    }
}
