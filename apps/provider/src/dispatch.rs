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

impl<R: Read, W: Write> Connection<R, W> {
    /// Deliver a decoded Resume using the original host-saved Await and grants.
    /// All checks and the Await-to-Running transition use the suite boundary.
    /// On error the connection closes; the host cancels its active lifetimes.
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
    ) -> Result<OperationReply, DispatchError> {
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
            self.send_operation_reply(
                saved.parent.request_id,
                reply,
                registry,
                sources,
                admission,
                transport,
            )
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
    /// a send failure: the operation may already have run. Any error closes this
    /// stream and requires host cancellation of its outstanding requests.
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
    ) -> Result<OperationReply, DispatchError> {
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
            self.send_operation_reply(
                request.request_id,
                reply,
                registry,
                sources,
                admission,
                transport,
            )
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
    ) -> Result<OperationReply, DispatchError> {
        let frame = ProviderFrame::Reply { request_id, reply };
        self.send(&frame, registry, sources, admission, transport)
            .map_err(DispatchError::Transport)?;
        // The frame was constructed locally with the sole Reply variant.
        let ProviderFrame::Reply { reply, .. } = frame else {
            unreachable!()
        };
        Ok(reply)
    }
}
