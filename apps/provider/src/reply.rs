//! Reply admission for a transport with one outstanding exchange.
use crate::*;
use nepl3_core::{
    diagnostic::validation::DiagnosticSourceResolver,
    operation::{Invoke, OperationReply},
    source::Digest,
};
use nepl3_suite::dispatch::suspending;
mod routing;
pub use routing::{ActiveReplyFailure, ReplyContext, ReplyRoutes, RouteError, UncommittedReply};

#[derive(Debug)]
pub enum ReplyError {
    Transport(TransportError),
    Closed,
    UnexpectedFrame,
    RequestId { expected: u64, received: u64 },
    Validation(suspending::Error),
}

impl<R: Read, W: Write> Connection<R, W> {
    /// Receive the next response to a previously sent Invoke or Resume. The
    /// caller retains the admitted original request and its context across all
    /// Await generations. Only one exchange may be outstanding on this path.
    ///
    /// The source store supplies codec closure; `authorized_sources` supplies
    /// the narrower diagnostic grants for this request. A protocol or validation
    /// failure closes the connection. The host then cancels remaining lifetimes
    /// and interrupts the underlying process as required by its transport policy.
    /// Await dependencies are returned for explicit host authorization/scheduling.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_reply(
        &mut self,
        request: &Invoke,
        context: Digest,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        authorized_sources: &impl DiagnosticSourceResolver,
        admission: &mut SourceAdmission,
        transport: &mut Budget,
        validation: &mut Budget,
    ) -> Result<OperationReply, ReplyError> {
        let result = (|| {
            let frame = self
                .receive(registry, sources, admission, transport)
                .map_err(ReplyError::Transport)?;
            let Some(frame) = frame else {
                return Err(ReplyError::Closed);
            };
            let ProviderFrame::Reply { request_id, reply } = frame else {
                return Err(if matches!(frame, ProviderFrame::Close) {
                    ReplyError::Closed
                } else {
                    ReplyError::UnexpectedFrame
                });
            };
            if request_id != request.request_id {
                return Err(ReplyError::RequestId {
                    expected: request.request_id,
                    received: request_id,
                });
            }
            suspending::validate_reply(
                &reply,
                request,
                context,
                registry,
                authorized_sources,
                validation,
            )
            .map_err(ReplyError::Validation)?;
            Ok(reply)
        })();
        if result.is_err() {
            self.closed = true;
        }
        result
    }
}
