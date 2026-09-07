//! Raw transport is checked against explicit prepared region input and an
//! actually completed binding result. It never constructs either execution proof.
use super::*;
use crate::analysis::{BoundBindingReply, region::query::*};
use nepl3_core::schema::SchemaRegistry;
mod value;
pub fn request_to_value<C: FoundationValueCodec>(
    request: &RegionQueryRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let value = value::request_value(request, &Schemas::new(r)?, c, b)?;
    r.validate(&expected("RegionQueryRequest", b)?, &value, b)?;
    Ok(value)
}
pub fn request_decode<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionQueryRequest, PortableError<C::Error>> {
    r.validate(&expected("RegionQueryRequest", b)?, v, b)?;
    value::request_read(v, &Schemas::new(r)?, c, b)
}
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &RegionQueryReply,
    request: &RegionQueryRequest,
    input: &PreparedRegionInput<'_, '_, '_>,
    binding: &BoundBindingReply,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let r = input.binding.profile.registry();
    let s = Schemas::new(r)?;
    let mut store = SourceStore::default();
    crate::portable::binding::check::add(&mut store, &reply.sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    validate(reply, request, input, binding, &store, &mut local, b)?;
    let value = record(
        s.engine,
        "RegionQueryReply",
        [
            reply.key.value(&s, &mut local, b)?,
            reply.capability.value(&s, &mut local, b)?,
            value::outcome_value(&reply.outcome, r, &s, &mut local, b)?,
            local.encode_report(&reply.report, b).map_err(boundary)?,
            local.encode_sources(&reply.sources, b).map_err(boundary)?,
        ],
        b,
    )?;
    r.validate(&expected("RegionQueryReply", b)?, &value, b)?;
    Ok(value)
}
pub fn reply_decode<C: FoundationValueCodec>(
    v: &NdfValue,
    request: &RegionQueryRequest,
    input: &PreparedRegionInput<'_, '_, '_>,
    binding: &BoundBindingReply,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionQueryReply, PortableError<C::Error>> {
    let r = input.binding.profile.registry();
    let s = Schemas::new(r)?;
    r.validate(&expected("RegionQueryReply", b)?, v, b)?;
    let f = fields(v, s.engine, "RegionQueryReply", 5)?;
    let sources = c.decode_sources(&f[4], b).map_err(boundary)?;
    let mut store = SourceStore::default();
    crate::portable::binding::check::add(&mut store, &sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    let reply = RegionQueryReply {
        key: Value::read(&f[0], &s, &mut local, b)?,
        capability: Value::read(&f[1], &s, &mut local, b)?,
        outcome: value::outcome_read(&f[2], r, &s, &mut local, b)?,
        report: local.decode_report(&f[3], b).map_err(boundary)?,
        sources,
    };
    validate(&reply, request, input, binding, &store, &mut local, b)?;
    Ok(reply)
}
fn validate<C: FoundationValueCodec>(
    reply: &RegionQueryReply,
    request: &RegionQueryRequest,
    input: &PreparedRegionInput<'_, '_, '_>,
    binding: &BoundBindingReply,
    store: &SourceStore,
    c: &mut C,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    b.charge(Resource::Work, 160)?;
    if reply.key != request.region.key || reply.capability != input.capability() {
        return Err(PortableError::RequestMismatch);
    }
    let r = input.binding.profile.registry();
    reply
        .report
        .validate(store, &[], r, b)
        .map_err(crate::facts::FactsError::from)?;
    if !reply.report.diagnostics.is_empty()
        || !reply.report.events.is_empty()
        || reply.report.trace_overflow.is_some()
    {
        return Err(PortableError::Shape);
    }
    match &reply.outcome {
        RegionQueryOutcome::Invalid(e) => {
            if e.stop_reason().is_some() || !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
        RegionQueryOutcome::Stopped(_) => {
            if !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
        RegionQueryOutcome::Complete { .. } => {}
    }
    let actual =
        crate::analysis::region::query::query(input, binding, request, b, c.source_admission());
    if let RegionQueryOutcome::Stopped(reason) = actual.outcome {
        return Err(reason.into());
    }
    let s = Schemas::new(r)?;
    let claimed = value::outcome_value(&reply.outcome, r, &s, c, b)?;
    let expected = value::outcome_value(&actual.outcome, r, &s, c, b)?;
    if !claimed.equal_with_budget(&expected, b)? || reply.sources.len() != actual.sources.len() {
        return Err(PortableError::RequestMismatch);
    }
    for (a, e) in reply.sources.iter().zip(&actual.sources) {
        b.charge(
            Resource::Work,
            (a.identity().source.0.len()
                + e.identity().source.0.len()
                + a.uri().len()
                + e.uri().len()
                + a.text().len()
                + e.text().len()) as u64
                + 42,
        )?;
        if a != e {
            return Err(PortableError::RequestMismatch);
        }
    }
    Ok(())
}
