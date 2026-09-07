//! Query transport preserves typed results and source declarations. Raw reply
//! validation is not execution authentication or a completed BindingAnalysis.
use super::{PortableError, boundary, value::*};
use crate::analysis::{BindingAccessError, query::*};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    facts::{EntityId, OccurrenceRole, ReferenceResolution},
    schema::SchemaRegistry,
    source::{SourceRef, SourceStore, Span},
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
mod check;
pub(super) mod value;

pub fn request_to_value<C: FoundationValueCodec>(
    request: &QueryRequest,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let value = request.value(&Schemas::new(registry)?, c, b)?;
    registry.validate(&expected("QueryRequest", b)?, &value, b)?;
    Ok(value)
}
/// A source reference is a requested identity, not authority to read it. The
/// native query resolves it only in its keyed, completed analysis.
pub fn request_decode<C: FoundationValueCodec>(
    v: &NdfValue,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<QueryRequest, PortableError<C::Error>> {
    registry.validate(&expected("QueryRequest", b)?, v, b)?;
    QueryRequest::read(v, &Schemas::new(registry)?, c, b)
}
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &QueryReply,
    request: &QueryRequest,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut store = SourceStore::default();
    super::binding::check::add(&mut store, &reply.sources, b, c.source_admission())?;
    check::validate(reply, request, registry, &store, b)?;
    let s = Schemas::new(registry)?;
    let mut local = c.scoped(&store);
    let outcome = value::outcome_value(&reply.outcome, registry, &s, &mut local, b)?;
    let value = record(
        s.engine,
        "QueryReply",
        [
            super::analysis::key_value(&reply.key, &s, &mut local, b)?,
            outcome,
            local.encode_report(&reply.report, b).map_err(boundary)?,
            local.encode_sources(&reply.sources, b).map_err(boundary)?,
        ],
        b,
    )?;
    registry.validate(&expected("QueryReply", b)?, &value, b)?;
    Ok(value)
}
/// The receiver uses only sources declared by this reply; ambient sources cannot
/// repair a missing range. The caller supplies the request being answered.
pub fn reply_decode<C: FoundationValueCodec>(
    v: &NdfValue,
    request: &QueryRequest,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<QueryReply, PortableError<C::Error>> {
    registry.validate(&expected("QueryReply", b)?, v, b)?;
    let s = Schemas::new(registry)?;
    let f = fields(v, s.engine, "QueryReply", 4)?;
    let sources = c.decode_sources(&f[3], b).map_err(boundary)?;
    let mut store = SourceStore::default();
    super::binding::check::add(&mut store, &sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    let reply = QueryReply {
        key: super::analysis::key_read(&f[0], &s, &mut local, b)?,
        outcome: value::outcome_read(&f[1], registry, &s, &mut local, b)?,
        report: local.decode_report(&f[2], b).map_err(boundary)?,
        sources,
    };
    check::validate(&reply, request, registry, &store, b)?;
    Ok(reply)
}
