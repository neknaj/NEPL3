//! Declaration metadata is recomputed on the receiver; no edit viability is inferred.
use super::{PortableError, boundary, value::*};
use crate::analysis::{PreparedBindingRequest, alternatives::*, expected::ExpectedReadRequest};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    source::SourceStore,
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
mod value;

pub use super::expected::{request_decode, request_to_value};
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &DeclaredAlternativesReply,
    request: &ExpectedReadRequest,
    input: &PreparedBindingRequest<'_, '_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let r = input.profile.registry();
    let s = Schemas::new(r)?;
    let mut store = SourceStore::default();
    super::binding::check::add(&mut store, &reply.sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    validate_reply(reply, request, input, &store, &mut local, b)?;
    let value = record(
        s.engine,
        "DeclaredAlternativesReply",
        [
            reply.key.value(&s, &mut local, b)?,
            value::outcome_value(&reply.outcome, r, &s, &mut local, b)?,
            local.encode_report(&reply.report, b).map_err(boundary)?,
            local.encode_sources(&reply.sources, b).map_err(boundary)?,
        ],
        b,
    )?;
    r.validate(&expected("DeclaredAlternativesReply", b)?, &value, b)?;
    Ok(value)
}
pub fn reply_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    request: &ExpectedReadRequest,
    input: &PreparedBindingRequest<'_, '_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<DeclaredAlternativesReply, PortableError<C::Error>> {
    let r = input.profile.registry();
    r.validate(&expected("DeclaredAlternativesReply", b)?, value, b)?;
    let s = Schemas::new(r)?;
    let f = fields(value, s.engine, "DeclaredAlternativesReply", 4)?;
    let sources = c.decode_sources(&f[3], b).map_err(boundary)?;
    let mut store = SourceStore::default();
    super::binding::check::add(&mut store, &sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    let reply = DeclaredAlternativesReply {
        key: Value::read(&f[0], &s, &mut local, b)?,
        outcome: value::outcome_read(&f[1], r, &s, &mut local, b)?,
        report: local.decode_report(&f[2], b).map_err(boundary)?,
        sources,
    };
    validate_reply(&reply, request, input, &store, &mut local, b)?;
    Ok(reply)
}
fn validate_reply<C: FoundationValueCodec>(
    reply: &DeclaredAlternativesReply,
    request: &ExpectedReadRequest,
    input: &PreparedBindingRequest<'_, '_>,
    store: &SourceStore,
    c: &mut C,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    b.charge(Resource::Work, 128)?;
    if reply.key != request.key {
        return Err(PortableError::RequestMismatch);
    }
    reply
        .report
        .validate(store, &[], input.profile.registry(), b)
        .map_err(crate::facts::FactsError::from)?;
    if !reply.report.diagnostics.is_empty()
        || !reply.report.events.is_empty()
        || reply.report.trace_overflow.is_some()
    {
        return Err(PortableError::Shape);
    }
    match &reply.outcome {
        DeclaredAlternativesOutcome::Invalid(error) => {
            if error.stop_reason().is_some() || !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
        DeclaredAlternativesOutcome::Stopped(_) => {
            if !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
        DeclaredAlternativesOutcome::Complete(_) => {}
    }
    // Recompute with exactly the prepared effective limits; the receiving
    // codec budget must match these limits and pays the full recomputation.
    let result = declared_alternatives(input, request, b, c.source_admission());
    match result.outcome {
        DeclaredAlternativesOutcome::Stopped(reason) => return Err(reason.into()),
        DeclaredAlternativesOutcome::Invalid(error) => return Err(PortableError::Expected(error)),
        _ => {}
    }
    let s = Schemas::new(input.profile.registry())?;
    let actual = value::outcome_value(&reply.outcome, input.profile.registry(), &s, c, b)?;
    let computed = value::outcome_value(&result.outcome, input.profile.registry(), &s, c, b)?;
    if !actual.equal_with_budget(&computed, b)? || result.sources.len() != reply.sources.len() {
        return Err(PortableError::RequestMismatch);
    }
    for (a, d) in result.sources.iter().zip(&reply.sources) {
        let bytes = (a.identity().source.0.len() as u64)
            .saturating_add(d.identity().source.0.len() as u64)
            .saturating_add(a.uri().len() as u64)
            .saturating_add(d.uri().len() as u64)
            .saturating_add(a.text().len() as u64)
            .saturating_add(d.text().len() as u64)
            .saturating_add(40);
        b.charge(Resource::Work, bytes)?;
        if a != d {
            return Err(PortableError::RequestMismatch);
        }
    }
    Ok(())
}
