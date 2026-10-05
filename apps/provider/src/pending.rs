//! Reply-only transport with explicit source admission before Report decoding.
use crate::*;
use core::convert::Infallible;
use nepl3_core::operation::Invoke;
use nepl3_wire::operation::{
    PendingReplyFrame, ReplyAdmissionError, ReplyFrameError, ReplyPayload,
};

#[derive(Debug)]
pub enum PendingReadError {
    Transport(TransportError),
    Frame(ReplyFrameError),
}
impl From<TransportError> for PendingReadError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}
impl From<ReplyFrameError> for PendingReadError {
    fn from(error: ReplyFrameError) -> Self {
        Self::Frame(error)
    }
}

#[derive(Debug)]
pub enum PendingFinishError<E> {
    Admission(ReplyAdmissionError<E>),
    Transport(TransportError),
}

/// An unfinished receipt keeps the connection closed even if forgotten.
/// Finishing checks codec context only; the host still owns domain acceptance,
/// request lifetimes, grants and child-process cleanup.
pub struct PendingReply<'a, R, W> {
    connection: &'a mut Connection<R, W>,
    frame: PendingReplyFrame<'a>,
}

impl<R: Read, W: Write> Connection<R, W> {
    /// Read one Reply in an operation phase. Schema exchange must have finished
    /// before I/O. Clean EOF is None; peer Close and other frames are typed errors.
    /// All failures leave the transport closed, without consuming a following frame.
    pub fn receive_pending_reply<'a>(
        &'a mut self,
        registry: &'a SchemaRegistry,
        admission: &'a mut SourceAdmission,
        budget: &'a mut Budget,
    ) -> Result<Option<PendingReply<'a, R, W>>, PendingReadError> {
        self.operation_phase()?;
        // Establish fail-closed state before I/O and before exposing a receipt.
        // Neither Drop nor unwinding is required to enforce this state.
        self.closed = true;
        let Some(bytes) = self.read_frame_bytes(budget)? else {
            return Ok(None);
        };
        let Some((frame, suffix)) = nepl3_wire::operation::decode_pending_reply_frame(
            &bytes, true, registry, admission, budget,
        )?
        else {
            return Err(TransportError::Truncated.into());
        };
        if !suffix.is_empty() {
            return Err(TransportError::Wire(WireError::TrailingData).into());
        }
        Ok(Some(PendingReply {
            connection: self,
            frame,
        }))
    }
}

impl<R: Read, W: Write> PendingReply<'_, R, W> {
    pub fn request_id(&self) -> u64 {
        self.frame.request_id()
    }

    pub fn finish(
        self,
        saved_request: &Invoke,
        original_sources: &SourceStore,
    ) -> Result<ProviderFrame, PendingFinishError<Infallible>> {
        let frame = self
            .frame
            .finish(saved_request, original_sources)
            .map_err(PendingFinishError::Admission)?;
        self.connection
            .advance(&frame, true)
            .map_err(PendingFinishError::Transport)?;
        self.connection.closed = false;
        Ok(frame)
    }

    /// Invoke a trusted source policy only after saved-request/output checks.
    /// Policy error, panic, sticky stop or later Report failure cannot reopen I/O.
    pub fn finish_with<E>(
        self,
        saved_request: &Invoke,
        original_sources: &SourceStore,
        policy: impl FnOnce(
            ReplyPayload<'_>,
            &SchemaRegistry,
            &SourceStore,
            &mut SourceAdmission,
            &mut Budget,
        ) -> Result<SourceStore, E>,
    ) -> Result<ProviderFrame, PendingFinishError<E>> {
        let frame = self
            .frame
            .finish_with(saved_request, original_sources, policy)
            .map_err(PendingFinishError::Admission)?;
        self.connection
            .advance(&frame, true)
            .map_err(PendingFinishError::Transport)?;
        self.connection.closed = false;
        Ok(frame)
    }
}
