//! Session-bound canonical value echoes; private state remains authoritative.
use super::*;
use crate::portable::{PortableError, tokenizer};
use nepl3_core::{value::NdfValue, value_codec::FoundationValueCodec};

#[derive(Clone, Copy)]
enum Wait {
    Provider,
    Reservation,
}

#[derive(Clone, Copy)]
enum Echo {
    Continuation,
    Reply,
}

impl TokenizationSession<'_> {
    /// Export the complete canonical projection of this pending tokenizer state.
    /// Source table ordering/coalescing affects only the projection. Native
    /// checkpoints and the distinct inner/outer Usage remain untouched. This is
    /// not a general continuation decoder or an Await/Reserve operation envelope.
    pub fn pending_continuation_value<C: FoundationValueCodec>(
        &self,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        self.pending_value(Echo::Continuation, codec, b)
    }

    /// Export the pending domain TokenizationReply, with an Await or Reserve
    /// outcome and all outward checkpoint fields. This is not a terminal reply
    /// codec or a common OperationReply envelope. Failed export retains pending.
    pub fn pending_reply_value<C: FoundationValueCodec>(
        &self,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        self.pending_value(Echo::Reply, codec, b)
    }

    fn pending_value<C: FoundationValueCodec>(
        &self,
        projection: Echo,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        if self.closed {
            return Err(PortableError::Reader(ReaderError::Closed));
        }
        let saved = self
            .pending
            .as_ref()
            .ok_or(PortableError::Reader(ReaderError::NoPending))?;
        match projection {
            Echo::Continuation => tokenizer::value(&saved.continuation, self.registry, codec, b),
            Echo::Reply => tokenizer::reply_value(&saved.continuation, self.registry, codec, b),
        }
    }

    /// Resume the existing private provider wait only after complete canonical
    /// echo comparison. The provider reply retains its normal validation path.
    pub fn resume_continuation_value<C: FoundationValueCodec>(
        &mut self,
        echo: &NdfValue,
        reply: ProviderReply,
        sources: &SourceStore,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<TokenizationReply, PortableError<C::Error>> {
        self.resume_value(Echo::Continuation, echo, reply, sources, codec, b)
    }

    /// Compare the entire pending Await reply echo before native provider resume.
    /// Received reply fields never become checkpoint or accounting authority.
    pub fn resume_reply_value<C: FoundationValueCodec>(
        &mut self,
        echo: &NdfValue,
        reply: ProviderReply,
        sources: &SourceStore,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<TokenizationReply, PortableError<C::Error>> {
        self.resume_value(Echo::Reply, echo, reply, sources, codec, b)
    }

    fn resume_value<C: FoundationValueCodec>(
        &mut self,
        projection: Echo,
        echo: &NdfValue,
        reply: ProviderReply,
        sources: &SourceStore,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<TokenizationReply, PortableError<C::Error>> {
        let pending = self.take_value_echo(
            projection,
            echo,
            Wait::Provider,
            || reply.validate_outcome(),
            codec,
            b,
        );
        match pending {
            Ok(pending) => self
                .restore(
                    pending,
                    Resume::Provider(reply),
                    sources,
                    b,
                    codec.source_admission(),
                )
                .map_err(PortableError::Reader),
            Err(error) => self.echo_failure(error, b),
        }
    }

    /// Echo data does not grant source reservation authority. The caller must
    /// still supply the host's actual SourceReservation for the existing path.
    pub fn reserve_continuation_value<C: FoundationValueCodec>(
        &mut self,
        echo: &NdfValue,
        reservation: &SourceReservation,
        sources: &SourceStore,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<TokenizationReply, PortableError<C::Error>> {
        self.reserve_value(Echo::Continuation, echo, reservation, sources, codec, b)
    }

    /// Compare the entire pending Reserve reply echo and then validate the
    /// caller-supplied native reservation. Echoed request data grants no authority.
    pub fn reserve_reply_value<C: FoundationValueCodec>(
        &mut self,
        echo: &NdfValue,
        reservation: &SourceReservation,
        sources: &SourceStore,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<TokenizationReply, PortableError<C::Error>> {
        self.reserve_value(Echo::Reply, echo, reservation, sources, codec, b)
    }

    fn reserve_value<C: FoundationValueCodec>(
        &mut self,
        projection: Echo,
        echo: &NdfValue,
        reservation: &SourceReservation,
        sources: &SourceStore,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<TokenizationReply, PortableError<C::Error>> {
        let pending =
            self.take_value_echo(projection, echo, Wait::Reservation, || Ok(()), codec, b);
        match pending {
            Ok(pending) => self
                .restore(
                    pending,
                    Resume::Reservation(reservation),
                    sources,
                    b,
                    codec.source_admission(),
                )
                .map_err(PortableError::Reader),
            Err(error) => self.echo_failure(error, b),
        }
    }

    fn echo_failure<E: nepl3_core::value_codec::FoundationCodecError>(
        &mut self,
        error: PortableError<E>,
        b: &mut Budget,
    ) -> Result<TokenizationReply, PortableError<E>> {
        // Do not inspect unrelated Budget.stop here: a rejected foreign/fresh
        // preflight must retain this slot even if that Budget is cancelled.
        if let Some(s) = error.stop_reason() {
            let s = b.stop(s);
            self.stop_pending(s, b).map_err(PortableError::Reader)
        } else {
            Err(error)
        }
    }

    fn take_value_echo<C: FoundationValueCodec>(
        &mut self,
        projection: Echo,
        echo: &NdfValue,
        wait: Wait,
        reply_gate: impl FnOnce() -> Result<(), ReaderError>,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<Pending, PortableError<C::Error>> {
        if self.closed {
            return Err(PortableError::Reader(ReaderError::Closed));
        }
        let saved = self
            .pending
            .as_ref()
            .ok_or(PortableError::Reader(ReaderError::NoPending))?;
        // Either well-shaped wait kind is admitted independently of the called
        // route. Shared-budget polling must still precede native route rejection.
        let nested = match (projection, echo) {
            (Echo::Continuation, value) => Some(value),
            (Echo::Reply, NdfValue::Record(r))
                if r.schema == *self.wire_schema
                    && r.kind == "TokenizationReply"
                    && r.fields.len() == 8 =>
            {
                match &r.fields[0] {
                    NdfValue::Variant(v)
                        if v.schema == *self.wire_schema
                            && v.type_name == "TokenizationOutcome"
                            && matches!(v.variant.as_str(), "Await" | "Reserve")
                            && v.fields.len() == 2 =>
                    {
                        Some(&v.fields[1])
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        let session = match nested {
            Some(NdfValue::Record(r))
                if r.schema == *self.wire_schema
                    && r.kind == "TokenizationContinuation"
                    && r.fields.len() == 17 =>
            {
                match &r.fields[1] {
                    NdfValue::Text(id) => Some(id),
                    _ => None,
                }
            }
            _ => None,
        };
        if saved.limits != b.limits()
            || !runtime::usage_at_least(b.usage(), saved.continuation.usage)
            || session != Some(&saved.continuation.session_id)
        {
            return Err(PortableError::Reader(ReaderError::Continuation));
        }
        b.poll()?;
        if !matches!(
            (wait, &saved.continuation.pending),
            (Wait::Provider, TokenizationWait::Provider { .. })
                | (Wait::Reservation, TokenizationWait::Reservation { .. })
        ) {
            return Err(PortableError::Reader(ReaderError::Continuation));
        }
        reply_gate().map_err(PortableError::Reader)?;
        let checked = (|| {
            let expected = self.pending_value(projection, codec, b)?;
            if !echo.equal_with_budget(&expected, b)? {
                return Err(PortableError::Reader(ReaderError::Continuation));
            }
            Ok(())
        })();
        if let Err(error) = checked {
            // This stage runs only after limits/history/session admission.
            if let Some(s) = b.poll().err().or_else(|| error.stop_reason()) {
                return Err(PortableError::Stopped(b.stop(s)));
            }
            return Err(error);
        }
        self.pending
            .take()
            .ok_or(PortableError::Reader(ReaderError::NoPending))
    }
}
