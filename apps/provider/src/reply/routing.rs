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
    /// Receive a routed reply and require its host lifetime to be Running with
    /// the same operation identity and snapshot. Terminal replies are committed
    /// as Finished before returning. Await remains Running until the host checks
    /// dependency grants and commits suspend; do so before receiving again.
    /// On failure the host must cancel its remaining connection lifetimes.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_active_reply<S: DiagnosticSourceResolver>(
        &mut self,
        routes: &ReplyRoutes<'_, S>,
        lifetimes: &mut RequestLifetimes,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        admission: &mut SourceAdmission,
        transport: &mut Budget,
        validation: &mut Budget,
    ) -> Result<(usize, OperationReply), RouteError> {
        let result = (|| {
            let (index, reply) = self.receive_routed_reply(
                routes, registry, sources, admission, transport, validation,
            )?;
            let saved = &routes.entries[index];
            let map = |error| match error {
                LifetimeError::Stopped(reason) => RouteError::Stopped(reason),
                other => RouteError::Lifetime(other),
            };
            lifetimes
                .check_reply(
                    saved.request.request_id,
                    &saved.request.operation,
                    saved.context,
                    validation,
                )
                .map_err(map)?;
            if matches!(reply, OperationReply::Result(_)) {
                lifetimes
                    .finish(saved.request.request_id, validation)
                    .map_err(map)?;
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
