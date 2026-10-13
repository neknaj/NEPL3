//! Session-bound canonical echoes. Received state is compared, never imported.
use super::*;
use crate::portable::{PortableError, continuation};
use nepl3_core::value_codec::FoundationValueCodec;

#[derive(Clone, Copy)]
enum Echo {
    Continuation,
    Reply,
}

impl ReaderSession<'_> {
    /// Export every field of this private pending continuation using its complete
    /// canonical Reader schema. Source tables use canonical wire order and unique
    /// identities (identical repeated declarations coalesce); native
    /// rollback storage keeps its original order. This is not a general-purpose
    /// continuation decoder, an Await operation envelope, or a new-session proof.
    /// Export failure never mutates the pending slot. Transport work is metered;
    /// emitted Usage and Report always retain the saved values.
    pub fn pending_continuation_value<C: FoundationValueCodec>(
        &self,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        self.pending_value(Echo::Continuation, codec, budget)
    }

    /// Export the complete pending domain ReadReply::Await envelope. Its call
    /// and report repeat the corresponding private continuation projections.
    /// This is not a common OperationReply::Await or a general reply decoder.
    /// Export failure retains pending and never re-emits saved diagnostics/events.
    pub fn pending_reply_value<C: FoundationValueCodec>(
        &self,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        self.pending_value(Echo::Reply, codec, budget)
    }

    fn pending_value<C: FoundationValueCodec>(
        &self,
        projection: Echo,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        if self.closed {
            return Err(PortableError::Reader(ReaderError::Closed));
        }
        let saved = self
            .pending
            .as_ref()
            .ok_or(PortableError::Reader(ReaderError::NoPending))?;
        match projection {
            Echo::Continuation => {
                continuation::value(&saved.continuation, self.registry, codec, budget)
            }
            Echo::Reply => {
                continuation::await_value(&saved.continuation, self.registry, codec, budget)
            }
        }
    }

    /// Accept only the exact complete canonical projection emitted by this
    /// session, then validate the new provider reply and resume private state.
    /// No received sources, counters, frames or context are admitted as authority.
    /// This does not deserialize arbitrary continuations or resume remote work.
    /// Non-stopping rejection retains pending; shared-operation stops use the
    /// existing terminal path and retain accepted collector artifacts.
    pub fn resume_continuation_value<C: FoundationValueCodec>(
        &mut self,
        echo: &NdfValue,
        reply: ProviderReply,
        sources: &SourceStore,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<ReadReply, PortableError<C::Error>> {
        self.resume_value(Echo::Continuation, echo, reply, sources, codec, budget)
    }

    /// Compare every field of a pending domain Await echo, then resume the
    /// existing private session with the provider reply. The received envelope
    /// supplies neither state nor source/accounting authority. Malformed or
    /// foreign preflight rejection retains pending, including on stopped budgets.
    pub fn resume_reply_value<C: FoundationValueCodec>(
        &mut self,
        echo: &NdfValue,
        reply: ProviderReply,
        sources: &SourceStore,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<ReadReply, PortableError<C::Error>> {
        self.resume_value(Echo::Reply, echo, reply, sources, codec, budget)
    }

    fn resume_value<C: FoundationValueCodec>(
        &mut self,
        projection: Echo,
        echo: &NdfValue,
        reply: ProviderReply,
        sources: &SourceStore,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<ReadReply, PortableError<C::Error>> {
        if self.closed {
            return Err(PortableError::Reader(ReaderError::Closed));
        }
        let saved = self
            .pending
            .as_ref()
            .ok_or(PortableError::Reader(ReaderError::NoPending))?;
        // Match the native resume preflight: a foreign session cannot consume
        // this slot even if the supplied operation Budget has already stopped.
        // Shape labels have fixed expected sizes; identity comparison is bounded
        // by this host-owned session ID, not arbitrary received nested metadata.
        let nested = match (projection, echo) {
            (Echo::Continuation, value) => Some(value),
            (Echo::Reply, NdfValue::Variant(v))
                if v.schema == *self.reader_schema
                    && v.type_name == "ReadReply"
                    && v.variant == "Await"
                    && v.fields.len() == 3 =>
            {
                Some(&v.fields[1])
            }
            _ => None,
        };
        let session = match nested {
            Some(NdfValue::Record(r))
                if r.schema == *self.reader_schema
                    && r.kind == "ReaderContinuation"
                    && r.fields.len() == 10 =>
            {
                match &r.fields[0] {
                    NdfValue::Text(id) => Some(id),
                    _ => None,
                }
            }
            _ => None,
        };
        if saved.limits != budget.limits()
            || !usage_at_least(budget.usage(), saved.continuation.usage)
            || session != Some(&saved.continuation.session_id)
        {
            return Err(PortableError::Reader(ReaderError::Continuation));
        }
        if let Err(s) = budget.poll() {
            return self.stop_pending(s, budget).map_err(PortableError::Reader);
        }
        let checked = (|| {
            let expected = self.pending_value(projection, codec, budget)?;
            // The expected projection is structurally validated. Fully metered
            // exact equality establishes the same shape for the received value
            // without doing registry lookups on attacker-selected schema labels.
            if !echo.equal_with_budget(&expected, budget)? {
                return Err(PortableError::Reader(ReaderError::Continuation));
            }
            Ok(())
        })();
        if let Err(error) = checked {
            if let Some(s) = budget.poll().err().or_else(|| error.stop_reason()) {
                let s = budget.stop(s);
                return self.stop_pending(s, budget).map_err(PortableError::Reader);
            }
            return Err(error);
        }
        self.resume_saved(reply, sources, budget, codec.source_admission())
            .map_err(PortableError::Reader)
    }
}
