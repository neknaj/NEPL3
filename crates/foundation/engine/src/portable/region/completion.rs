//! Successful structural-region candidates are checked against native selection
//! and binding. Decoding does not construct execution or edit authority.
use super::*;
pub mod failure;
use crate::analysis::{
    BoundBindingReply,
    completion::ScopeCandidateRequest,
    region::completion::{self as native, *},
};
use nepl3_core::{facts::OccurrenceId, schema::SchemaRegistry};

pub fn request_to_value<C: FoundationValueCodec>(
    request: &RegionCompletionRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let s = Schemas::new(r)?;
    let value = record(
        s.engine,
        "RegionCompletionRequest",
        [
            request.region.value(&s, c, b)?,
            request.prefix.value(&s, c, b)?,
        ],
        b,
    )?;
    r.validate(&expected("RegionCompletionRequest", b)?, &value, b)?;
    Ok(value)
}
pub fn request_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionCompletionRequest, PortableError<C::Error>> {
    r.validate(&expected("RegionCompletionRequest", b)?, value, b)?;
    let s = Schemas::new(r)?;
    let f = fields(value, s.engine, "RegionCompletionRequest", 2)?;
    Ok(RegionCompletionRequest {
        region: Value::read(&f[0], &s, c, b)?,
        prefix: Value::read(&f[1], &s, c, b)?,
    })
}
/// Complete metadata is checked against native results. Failure cause metadata
/// uses the separate request-correlated `failure` envelope.
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &RegionCompletionReply,
    request: &RegionCompletionRequest,
    input: &PreparedRegionInput<'_, '_, '_>,
    binding: &BoundBindingReply,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let RegionCompletionOutcome::Complete { region, groups } = &reply.outcome else {
        return match &reply.outcome {
            RegionCompletionOutcome::Stopped(reason) => Err((*reason).into()),
            _ => Err(PortableError::Shape),
        };
    };
    let r = input.binding.profile.registry();
    let s = Schemas::new(r)?;
    let mut store = SourceStore::default();
    crate::portable::binding::check::add(&mut store, &reply.sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    validate(reply, request, input, binding, &store, &mut local, b)?;
    let mut values = Vec::new();
    for group in groups {
        let request = ScopeCandidateRequest {
            key: request.region.key.analysis,
            occurrence: group.occurrence,
            prefix: &request.prefix,
        };
        let value = crate::portable::completion::reply_to_value(
            group, &request, binding, r, &mut local, b,
        )?;
        push(&mut values, value, b)?;
    }
    let value = record(
        s.engine,
        "RegionCandidates",
        [
            reply.key.value(&s, &mut local, b)?,
            reply.capability.value(&s, &mut local, b)?,
            region.value(&s, &mut local, b)?,
            NdfValue::List(values),
            local.encode_report(&reply.report, b).map_err(boundary)?,
            local.encode_sources(&reply.sources, b).map_err(boundary)?,
        ],
        b,
    )?;
    r.validate(&expected("RegionCandidates", b)?, &value, b)?;
    Ok(value)
}
pub fn reply_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    request: &RegionCompletionRequest,
    input: &PreparedRegionInput<'_, '_, '_>,
    binding: &BoundBindingReply,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionCompletionReply, PortableError<C::Error>> {
    let r = input.binding.profile.registry();
    r.validate(&expected("RegionCandidates", b)?, value, b)?;
    let s = Schemas::new(r)?;
    let f = fields(value, s.engine, "RegionCandidates", 6)?;
    let sources = c.decode_sources(&f[5], b).map_err(boundary)?;
    let mut store = SourceStore::default();
    crate::portable::binding::check::add(&mut store, &sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    let mut groups = Vec::new();
    for value in list(&f[3])? {
        b.charge(Resource::Nodes, 1)?;
        let f = fields(value, s.engine, "ScopeCandidates", 5)?;
        let occurrence = OccurrenceId(u64::read(&f[1], &s, &mut local, b)?);
        let request = ScopeCandidateRequest {
            key: request.region.key.analysis,
            occurrence,
            prefix: &request.prefix,
        };
        let group =
            crate::portable::completion::reply_decode(value, &request, binding, r, &mut local, b)?;
        push(&mut groups, group, b)?;
    }
    let reply = RegionCompletionReply {
        key: Value::read(&f[0], &s, &mut local, b)?,
        capability: Value::read(&f[1], &s, &mut local, b)?,
        outcome: RegionCompletionOutcome::Complete {
            region: Value::read(&f[2], &s, &mut local, b)?,
            groups,
        },
        report: local.decode_report(&f[4], b).map_err(boundary)?,
        sources,
    };
    validate(&reply, request, input, binding, &store, &mut local, b)?;
    Ok(reply)
}
fn validate<C: FoundationValueCodec>(
    reply: &RegionCompletionReply,
    request: &RegionCompletionRequest,
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
    let RegionCompletionOutcome::Complete { region, groups } = &reply.outcome else {
        return Err(PortableError::Shape);
    };
    // Every receiver pays for native structural selection, source admission,
    // captured scope lookup and comparison. Sender Usage is metadata only.
    let actual = native::names(input, binding, request, b, c.source_admission());
    let (actual_region, actual_groups) = match &actual.outcome {
        RegionCompletionOutcome::Complete { region, groups } => (region, groups),
        RegionCompletionOutcome::Stopped(reason) => return Err((*reason).into()),
        RegionCompletionOutcome::Invalid(_) => return Err(PortableError::RequestMismatch),
    };
    let s = Schemas::new(r)?;
    let region_value = region.value(&s, c, b)?;
    let actual_region_value = actual_region.value(&s, c, b)?;
    if !region_value.equal_with_budget(&actual_region_value, b)?
        || groups.len() != actual_groups.len()
        || reply.sources.len() != actual.sources.len()
    {
        return Err(PortableError::RequestMismatch);
    }
    // Individual groups are checked by the scope codec. This comparison binds
    // their identities and order to the structural operation, excluding claims
    // about sender resource use from semantic equality.
    for (supplied, expected) in groups.iter().zip(actual_groups) {
        b.charge(Resource::Work, 1)?;
        if supplied.occurrence != expected.occurrence {
            return Err(PortableError::RequestMismatch);
        }
    }
    for (supplied, expected) in reply.sources.iter().zip(&actual.sources) {
        b.charge(
            Resource::Work,
            [
                supplied.identity().source.0.len(),
                expected.identity().source.0.len(),
                supplied.uri().len(),
                expected.uri().len(),
                supplied.text().len(),
                expected.text().len(),
            ]
            .into_iter()
            .fold(42u64, |sum, len| sum.saturating_add(len as u64)),
        )?;
        if supplied != expected {
            return Err(PortableError::RequestMismatch);
        }
    }
    Ok(())
}
