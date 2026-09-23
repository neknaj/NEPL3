//! Immutable host routing inputs; operation lifetime transitions stay with the host.
use super::*;
use nepl3_core::budget::Resource;
use nepl3_core::operation::lifetime::{LifetimeError, RequestLifetimes};

pub struct ReplyContext<'a, S> {
    pub request: &'a Invoke,
    pub context: Digest,
    pub authorized_sources: &'a S,
}

#[derive(Debug)]
pub enum RouteError {
    Stopped(StopReason),
    UnorderedOrDuplicate,
    UnknownRequest(u64),
    Reply(ReplyError),
    Lifetime(LifetimeError),
}
impl From<StopReason> for RouteError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// A structurally checked reply whose lifetime correlation did not commit.
/// The saved request and diagnostic grants stay borrowed from the original
/// route table. This value does not authorize delivery, replay or Resume.
pub struct UncommittedReply<'a, S> {
    pub route_index: usize,
    pub saved: &'a ReplyContext<'a, S>,
    pub reply: OperationReply,
}

/// Retains a received reply when only the final lifetime check failed.
/// Decode, routing and output-validation failures carry no checked reply.
/// Inline ownership avoids an allocation after either Budget has stopped.
pub struct ActiveReplyFailure<'a, S> {
    pub cause: RouteError,
    pub uncommitted: Option<UncommittedReply<'a, S>>,
}
impl<S> From<RouteError> for ActiveReplyFailure<'_, S> {
    fn from(cause: RouteError) -> Self {
        Self {
            cause,
            uncommitted: None,
        }
    }
}
impl<S> core::fmt::Debug for ActiveReplyFailure<'_, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ActiveReplyFailure")
            .field("cause", &self.cause)
            .field(
                "uncommitted_request",
                &self
                    .uncommitted
                    .as_ref()
                    .map(|r| r.saved.request.request_id),
            )
            .finish()
    }
}

/// Host-saved requests in strictly increasing request-ID order. The borrow
/// keeps IDs, contexts and grants unchanged after the single ordering check.
/// This table is an index, not execution authorization or a lifetime table.
pub struct ReplyRoutes<'a, S> {
    entries: &'a [ReplyContext<'a, S>],
}
impl<'a, S> ReplyRoutes<'a, S> {
    pub fn new(
        entries: &'a [ReplyContext<'a, S>],
        budget: &mut Budget,
    ) -> Result<Self, RouteError> {
        budget.poll()?;
        for pair in entries.windows(2) {
            budget.charge(Resource::Work, 1)?;
            if pair[0].request.request_id >= pair[1].request.request_id {
                return Err(RouteError::UnorderedOrDuplicate);
            }
        }
        Ok(Self { entries })
    }
    fn find(&self, id: u64, budget: &mut Budget) -> Result<usize, RouteError> {
        budget.poll()?;
        let (mut start, mut end) = (0, self.entries.len());
        while start < end {
            budget.charge(Resource::Work, 1)?;
            let middle = start + (end - start) / 2;
            match self.entries[middle].request.request_id.cmp(&id) {
                core::cmp::Ordering::Less => start = middle + 1,
                core::cmp::Ordering::Greater => end = middle,
                core::cmp::Ordering::Equal => return Ok(middle),
            }
        }
        Err(RouteError::UnknownRequest(id))
    }
}

impl<R: Read, W: Write> Connection<R, W> {
    /// Receive an active reply and close every remaining connection lifetime on
    /// transport, validation or correlation failure. Cleanup is allocation-free
    /// and works after either budget stops. The callback interrupts execution;
    /// it must not block or panic. Terminal requests receive no notification.
    /// The table must contain only requests owned by this connection. The host
    /// retains responsibility for process termination/reaping and Await setup.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn receive_managed_reply<'a, S: DiagnosticSourceResolver>(
        &mut self,
        routes: &'a ReplyRoutes<'a, S>,
        lifetimes: &mut RequestLifetimes,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        admission: &mut SourceAdmission,
        transport: &mut Budget,
        validation: &mut Budget,
        cancel: impl FnMut(u64),
    ) -> Result<(usize, OperationReply), ActiveReplyFailure<'a, S>> {
        let result = self.receive_active_reply(
            routes, lifetimes, registry, sources, admission, transport, validation,
        );
        if result.is_err() {
            lifetimes.close(cancel);
        }
        result
    }

    /// Receive a routed reply and require its host lifetime to be Running with
    /// the same operation identity and snapshot. Terminal replies are committed
    /// as Finished before returning. Await remains Running until the host checks
    /// dependency grants and commits suspend; do so before receiving again.
    /// On failure the host must cancel its remaining connection lifetimes.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn receive_active_reply<'a, S: DiagnosticSourceResolver>(
        &mut self,
        routes: &'a ReplyRoutes<'a, S>,
        lifetimes: &mut RequestLifetimes,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        admission: &mut SourceAdmission,
        transport: &mut Budget,
        validation: &mut Budget,
    ) -> Result<(usize, OperationReply), ActiveReplyFailure<'a, S>> {
        let result = (|| {
            let (index, reply) = self.receive_routed_reply(
                routes, registry, sources, admission, transport, validation,
            )?;
            let saved = &routes.entries[index];
            let map = |error| match error {
                LifetimeError::Stopped(reason) => RouteError::Stopped(reason),
                other => RouteError::Lifetime(other),
            };
            let accepted = if matches!(reply, OperationReply::Result(_)) {
                lifetimes.finish_reply(
                    saved.request.request_id,
                    &saved.request.operation,
                    saved.context,
                    validation,
                )
            } else {
                lifetimes.check_reply(
                    saved.request.request_id,
                    &saved.request.operation,
                    saved.context,
                    validation,
                )
            };
            if let Err(error) = accepted {
                return Err(ActiveReplyFailure {
                    cause: map(error),
                    uncommitted: Some(UncommittedReply {
                        route_index: index,
                        saved,
                        reply,
                    }),
                });
            }
            Ok((index, reply))
        })();
        if result.is_err() {
            self.closed = true;
        }
        result
    }

    /// Admit an out-of-order reply using its saved host context and narrow
    /// diagnostic grants. Returns the table index and validated reply.
    /// The host must check/update its request lifetime before scheduling or
    /// receiving another reply. Repeated replies require lifetime rejection.
    /// This method performs no callbacks and does not authorize Await children.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_routed_reply<S: DiagnosticSourceResolver>(
        &mut self,
        routes: &ReplyRoutes<'_, S>,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        admission: &mut SourceAdmission,
        transport: &mut Budget,
        validation: &mut Budget,
    ) -> Result<(usize, OperationReply), RouteError> {
        let result = (|| {
            let frame = self
                .receive(registry, sources, admission, transport)
                .map_err(|e| RouteError::Reply(ReplyError::Transport(e)))?;
            let Some(ProviderFrame::Reply { request_id, reply }) = frame else {
                return Err(RouteError::Reply(
                    if matches!(frame, None | Some(ProviderFrame::Close)) {
                        ReplyError::Closed
                    } else {
                        ReplyError::UnexpectedFrame
                    },
                ));
            };
            let index = routes.find(request_id, validation)?;
            let saved = &routes.entries[index];
            suspending::validate_reply(
                &reply,
                saved.request,
                saved.context,
                registry,
                saved.authorized_sources,
                validation,
            )
            .map_err(|e| RouteError::Reply(ReplyError::Validation(e)))?;
            Ok((index, reply))
        })();
        if result.is_err() {
            self.closed = true;
        }
        result
    }
}
