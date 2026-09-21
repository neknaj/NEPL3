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
mod value;
use super::transform::{boundary, reader};

pub struct ReadReplyContext<'a> {
    pub(crate) continuation: &'a ReaderContinuation,
    pub(crate) signature: &'a ProviderSignature,
    pub(crate) registry: &'a SchemaRegistry,
}

impl ReadReplyContext<'_> {
    fn schema(&self) -> Result<&SchemaRef, ReaderError> {
        self.registry
            .selected(crate::schema::PACKAGE, crate::schema::REVISION)
            .ok_or(ReaderError::Context)
    }
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

fn checked<C: FoundationValueCodec>(
    value: &NdfValue,
    context: &ReadReplyContext<'_>,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    b.charge(
        Resource::AllocationUnits,
        (crate::schema::PACKAGE.len() + "ReadReply".len()) as u64,
    )?;
    context
        .registry
        .validate(
            &TypeDescriptor::Named(TypeRef {
                package: crate::schema::PACKAGE.into(),
                revision: crate::schema::REVISION,
                name: "ReadReply".into(),
            }),
            value,
            b,
        )
        .map_err(|e| reader(ReaderError::Schema(e)))?;
    Ok(())
}

/// Encode a terminal reply after the native saved-dispatch checks.
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &ReadReply,
    context: &ReadReplyContext<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    context
        .validate(reply, b, codec.source_admission())
        .map_err(reader)?;
    let (added, maps, _) = value::metadata(reply)?;
    let sources = super::transform::dispatch_sources(
        context.continuation,
        added,
        b,
        codec.source_admission(),
    )
    .map_err(reader)?;
    let result = {
        let mut scoped = codec.scoped_with_mappings(&sources, maps);
        value::encode(
            reply,
            context.schema().map_err(reader)?,
            context.registry,
            &mut scoped,
            b,
        )?
    };
    checked::<C>(&result, context, b)?;
    Ok(result)
}

/// Decode source declarations before source-bearing fields, then enforce the
/// same saved-dispatch checks as native replies. The pending slot is retained.
pub fn reply_from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    context: &ReadReplyContext<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<ReadReply, PortableError<C::Error>> {
    checked::<C>(value, context, b)?;
    let schema = context.schema().map_err(reader)?;
    let (case, fields) = super::transform::value::parts(value, schema, "ReadReply")?;
    let (source_index, map_index) = value::source_indices(case, fields.len())?;
    let added = codec
        .decode_sources(&fields[source_index], b)
        .map_err(boundary)?;
    let sources = super::transform::dispatch_sources(
        context.continuation,
        &added,
        b,
        codec.source_admission(),
    )
    .map_err(reader)?;
    let maps = codec
        .scoped(&sources)
        .decode_mappings(&fields[map_index], b)
        .map_err(boundary)?;
    let reply = {
        let mut scoped = codec.scoped_with_mappings(&sources, &maps);
        value::decode(
            case,
            fields,
            added,
            &maps,
            schema,
            context.registry,
            &mut scoped,
            b,
        )?
    };
    context
        .validate(&reply, b, codec.source_admission())
        .map_err(reader)?;
    Ok(reply)
}
