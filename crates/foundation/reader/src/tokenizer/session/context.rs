//! Reply-codec proofs borrow the actual nested dispatch, never exported state.
use super::*;
use crate::portable::{read::ReadReplyContext, transform::TransformReplyContext};

impl TokenizationSession<'_> {
    /// Borrow the private pending Read/Dependent dispatch for terminal reply
    /// codecs. Its source closure, state, Usage and depth belong to the inner
    /// reader, which may have progressed beyond the outer tokenizer checkpoint.
    /// This does not consume pending or perform the outer resume admission.
    /// Codec failure, including a budget stop, leaves pending until resume,
    /// discard or close handles it through the native lifecycle.
    ///
    /// A context cannot remain usable across a mutable session operation.
    /// ```compile_fail
    /// use nepl3_reader::{runtime::ReaderError, tokenizer::TokenizationSession};
    /// fn invalidate(session: &mut TokenizationSession<'_>) -> Result<(), ReaderError> {
    ///     let context = session.pending_read()?;
    ///     session.close();
    ///     let _ = context.saved_depth()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn pending_read(&self) -> Result<ReadReplyContext<'_>, ReaderError> {
        self.check_pending_provider()?;
        self.reader.pending_read()
    }

    /// Borrow the private pending Transform dispatch for terminal reply codecs.
    /// The same non-consuming ownership and lifecycle rules as `pending_read`
    /// apply. A reservation wait cannot issue a provider-reply context.
    pub fn pending_transform(&self) -> Result<TransformReplyContext<'_>, ReaderError> {
        self.check_pending_provider()?;
        self.reader.pending_transform()
    }

    fn check_pending_provider(&self) -> Result<(), ReaderError> {
        if self.closed {
            return Err(ReaderError::Closed);
        }
        let saved = self.pending.as_ref().ok_or(ReaderError::NoPending)?;
        if !matches!(
            saved.continuation.pending,
            TokenizationWait::Provider { .. }
        ) {
            return Err(ReaderError::Continuation);
        }
        Ok(())
    }
}
