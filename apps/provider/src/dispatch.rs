//! Execute an admitted request and publish its checked reply on the stream.
use crate::*;
use nepl3_core::operation::{Resume, lifetime::RequestLifetimes};
use nepl3_core::{
    diagnostic::validation::DiagnosticSourceResolver, operation::OperationReply, source::Digest,
};
use nepl3_suite::dispatch::{resume, suspending};
use nepl3_suite::grants::AuthorizedInvoke;

#[derive(Debug)]
pub enum DispatchError {
    Operation(suspending::Error),
    Resume(resume::ResumeError),
    Transport(TransportError),
}

/// A validated operation reply and its independent delivery outcome. A failed
/// send closes the stream; retain the reply for diagnostics/lifetime cleanup and
/// cancel the remaining requests. Never repeat the executed callback to retry
/// delivery. Owning the reply requires no additional allocation on failure.
#[derive(Debug)]
#[must_use = "inspect delivery status and retain the checked operation reply"]
pub struct ReplyDelivery {
    pub request_id: u64,
    pub reply: OperationReply,
    pub delivery: Result<(), TransportError>,
}

impl<R: Read, W: Write> Connection<R, W> {
    /// Deliver a decoded Resume using the original host-saved Await and grants.
    /// All checks and the Await-to-Running transition use the suite boundary.
    /// Dispatch errors and failed delivery outcomes close the connection; the
    /// host cancels its active lifetimes and can retain any checked reply.
    /// A callback that began execution consumes the generation even if sending
    /// its reply fails. The returned reply may request another Await generation.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_resume<S: DiagnosticSourceResolver>(
        &mut self,
        registration: &resume::Registration<'_>,
        implementation: Digest,
        saved: &resume::SavedAwait<'_, S>,
        request: &Resume,
        lifetimes: &mut RequestLifetimes,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        authorized_sources: &impl DiagnosticSourceResolver,
        admission: &mut SourceAdmission,
        execution: &mut Budget,
        validation: &mut Budget,
        transport: &mut Budget,
    ) -> Result<ReplyDelivery, DispatchError> {
        self.operation_phase().map_err(DispatchError::Transport)?;
        let result = (|| {
            let reply = resume::execute(
                registration,
                implementation,
                saved,
                request,
                lifetimes,
                registry,
                authorized_sources,
                execution,
                validation,
            )
            .map_err(DispatchError::Resume)?;
            Ok(self.send_operation_reply(
                saved.parent.request_id,
                reply,
                registry,
                sources,
                admission,
                transport,
            ))
        })();
        if result.is_err() {
            self.closed = true;
        }
        result
    }

    /// Requires the host's immutable environment/source/resource grant proof.
    /// The host registers its lifetime and selects this implementation independently.
    /// `context` is computed from those admitted inputs and host configuration.
    ///
    /// The returned reply is retained by the host for terminal lifetime closure
    /// or Await/Resume scheduling. The host must not execute this call again on
    /// a send failure: the operation has already run. Dispatch errors and failed
    /// ReplyDelivery outcomes close the stream and require host cancellation of
    /// outstanding requests. Only checked replies enter ReplyDelivery.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_invoke(
        &mut self,
        registration: &suspending::Registration<'_>,
        implementation: Digest,
        authorized: &AuthorizedInvoke<'_>,
        context: Digest,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        authorized_sources: &impl DiagnosticSourceResolver,
        admission: &mut SourceAdmission,
        execution: &mut Budget,
        validation: &mut Budget,
        transport: &mut Budget,
    ) -> Result<ReplyDelivery, DispatchError> {
        self.operation_phase().map_err(DispatchError::Transport)?;
        let request = authorized.request();
        let result = (|| {
            let reply = suspending::invoke(
                registration,
                implementation,
                request,
                context,
                registry,
                authorized_sources,
                execution,
                validation,
            )
            .map_err(DispatchError::Operation)?;
            Ok(self.send_operation_reply(
                request.request_id,
                reply,
                registry,
                sources,
                admission,
                transport,
            ))
        })();
        if result.is_err() {
            self.closed = true;
        }
        result
    }

    fn send_operation_reply(
        &mut self,
        request_id: u64,
        reply: OperationReply,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        admission: &mut SourceAdmission,
        transport: &mut Budget,
    ) -> ReplyDelivery {
        let frame = ProviderFrame::Reply { request_id, reply };
        let delivery = self.send(&frame, registry, sources, admission, transport);
        if delivery.is_err() {
            self.closed = true;
        }
        // The frame was constructed locally with the sole Reply variant.
        let ProviderFrame::Reply { reply, .. } = frame else {
            unreachable!()
        };
        ReplyDelivery {
            request_id,
            reply,
            delivery,
        }
    }
}
