//! Transform reply encoding is tied to a host's saved native dispatch. It does
//! not authenticate remote Usage or replace the VM's resume-time validation.
pub mod operation;
mod value;
use super::*;
use crate::{
    model::*,
    plan::ReaderPlan,
    runtime::{
        ReaderError,
        validate::{ProviderBoundary, ProviderReplyRef, check_provider},
    },
};
use nepl3_core::{source::SourceAdmission, value_codec::FoundationCodecError};

pub struct TransformReplyContext<'a> {
    pub(crate) continuation: &'a ReaderContinuation,
    pub(crate) plan: &'a ReaderPlan,
    pub(crate) registry: &'a SchemaRegistry,
}
impl TransformReplyContext<'_> {
    fn schema(&self) -> Result<&SchemaRef, ReaderError> {
        self.registry
            .selected(crate::schema::PACKAGE, crate::schema::REVISION)
            .ok_or(ReaderError::Context)
    }
    fn sources(
        &self,
        added: &[SourceSnapshot],
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<SourceStore, ReaderError> {
        let mut store = SourceStore::default();
        for source in self
            .continuation
            .request
            .sources
            .iter()
            .chain(&self.continuation.current.sources)
            .chain(added)
        {
            admission.admit_existing(source, budget)?;
            for prior in store.snapshots() {
                budget.charge(
                    Resource::Work,
                    (prior.identity().source.0.len() as u64)
                        .saturating_add(source.identity().source.0.len() as u64)
                        .saturating_add(34),
                )?;
            }
            store.insert_with_budget(crate::runtime::copy::copy(source, budget)?, budget)?;
        }
        Ok(store)
    }
    fn validate(
        &self,
        reply: &TransformReply,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), ReaderError> {
        let c = self.continuation;
        let sources = self.sources(&[], budget, admission)?;
        let snapshot = sources
            .resolve(&c.request.snapshot)
            .ok_or(SourceError::MissingSnapshot)?;
        let boundary = ProviderBoundary {
            plan: self.plan,
            registry: self.registry,
            snapshot,
            declared: &c.request.sources,
            current: &c.current,
        };
        let frame = c.frames.last().ok_or(ReaderError::Continuation)?;
        let ProviderCall::Transform { depth_base, .. } = &c.pending else {
            return Err(ReaderError::ProviderContract);
        };
        budget.with_depth_at_least(*depth_base, |budget| {
            check_provider(
                &boundary,
                frame,
                &c.pending,
                ProviderReplyRef::Transform(reply),
                c.usage,
                &sources,
                budget,
                admission,
            )
        })
    }
}
fn reader<E>(error: ReaderError) -> PortableError<E> {
    match crate::runtime::stop_reason(&error) {
        Some(reason) => PortableError::Stopped(reason),
        None => PortableError::Reader(error),
    }
}
fn boundary<E: FoundationCodecError>(error: E) -> PortableError<E> {
    match error.stop_reason() {
        Some(reason) => PortableError::Stopped(reason),
        None => PortableError::Boundary(error),
    }
}
fn checked<C: FoundationValueCodec>(
    value: &NdfValue,
    context: &TransformReplyContext<'_>,
    budget: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    budget.charge(
        Resource::AllocationUnits,
        (crate::schema::PACKAGE.len() + "TransformReply".len()) as u64,
    )?;
    context
        .registry
        .validate(
            &TypeDescriptor::Named(TypeRef {
                package: crate::schema::PACKAGE.into(),
                revision: crate::schema::REVISION,
                name: "TransformReply".into(),
            }),
            value,
            budget,
        )
        .map_err(|e| reader(ReaderError::Schema(e)))?;
    Ok(())
}
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &TransformReply,
    context: &TransformReplyContext<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    context
        .validate(reply, budget, codec.source_admission())
        .map_err(reader)?;
    let sources = context
        .sources(&reply.sources, budget, codec.source_admission())
        .map_err(reader)?;
    let mut scoped = codec.scoped(&sources);
    let value = value::encode(
        reply,
        context.schema().map_err(reader)?,
        &mut scoped,
        budget,
    )?;
    checked::<C>(&value, context, budget)?;
    Ok(value)
}
pub fn reply_from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    context: &TransformReplyContext<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<TransformReply, PortableError<C::Error>> {
    checked::<C>(value, context, budget)?;
    let schema = context.schema().map_err(reader)?;
    let f = fields(value, schema, "TransformReply", 4)?;
    let added = codec.decode_sources(&f[1], budget).map_err(boundary)?;
    let sources = context
        .sources(&added, budget, codec.source_admission())
        .map_err(reader)?;
    let reply = {
        let mut scoped = codec.scoped(&sources);
        value::decode(f, added, schema, &mut scoped, budget)?
    };
    context
        .validate(&reply, budget, codec.source_admission())
        .map_err(reader)?;
    Ok(reply)
}
