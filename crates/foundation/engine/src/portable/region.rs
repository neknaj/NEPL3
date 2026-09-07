//! Canonical reader sidecars are independent inputs, never inferred from view
//! names. Their digest extends the existing analysis key for region requests.
use super::{PortableError, boundary, value::*};
use crate::{
    analysis::{PreparedBindingRequest, region::*},
    parse::ReaderFactBatch,
};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaError,
    source::SourceStore,
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
mod sidecar;
mod value;
pub fn request_to_value<C: FoundationValueCodec>(
    request: &RegionRequest,
    registry: &nepl3_core::schema::SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let value = request.value(&Schemas::new(registry)?, c, b)?;
    registry.validate(&expected("RegionRequest", b)?, &value, b)?;
    Ok(value)
}
pub fn request_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    registry: &nepl3_core::schema::SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionRequest, PortableError<C::Error>> {
    registry.validate(&expected("RegionRequest", b)?, value, b)?;
    RegionRequest::read(value, &Schemas::new(registry)?, c, b)
}
/// Reply validation recomputes this structural selection against the explicit
/// prepared input. It does not authenticate how the input's reader facts arose.
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &RegionReply,
    request: &RegionRequest,
    input: &PreparedRegionInput<'_, '_, '_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let r = input.binding.profile.registry();
    let s = Schemas::new(r)?;
    let mut store = SourceStore::default();
    super::binding::check::add(&mut store, &reply.sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    validate_reply(reply, request, input, &store, &mut local, b)?;
    let value = record(
        s.engine,
        "RegionReply",
        [
            reply.key.value(&s, &mut local, b)?,
            reply.capability.value(&s, &mut local, b)?,
            value::outcome_value(&reply.outcome, r, &s, &mut local, b)?,
            local.encode_report(&reply.report, b).map_err(boundary)?,
            local.encode_sources(&reply.sources, b).map_err(boundary)?,
        ],
        b,
    )?;
    r.validate(&expected("RegionReply", b)?, &value, b)?;
    Ok(value)
}
pub fn reply_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    request: &RegionRequest,
    input: &PreparedRegionInput<'_, '_, '_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionReply, PortableError<C::Error>> {
    let r = input.binding.profile.registry();
    r.validate(&expected("RegionReply", b)?, value, b)?;
    let s = Schemas::new(r)?;
    let f = fields(value, s.engine, "RegionReply", 5)?;
    let sources = c.decode_sources(&f[4], b).map_err(boundary)?;
    let mut store = SourceStore::default();
    super::binding::check::add(&mut store, &sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    let reply = RegionReply {
        key: Value::read(&f[0], &s, &mut local, b)?,
        capability: Value::read(&f[1], &s, &mut local, b)?,
        outcome: value::outcome_read(&f[2], r, &s, &mut local, b)?,
        report: local.decode_report(&f[3], b).map_err(boundary)?,
        sources,
    };
    validate_reply(&reply, request, input, &store, &mut local, b)?;
    Ok(reply)
}
fn validate_reply<C: FoundationValueCodec>(
    reply: &RegionReply,
    request: &RegionRequest,
    input: &PreparedRegionInput<'_, '_, '_>,
    store: &SourceStore,
    c: &mut C,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    b.charge(Resource::Work, 160)?;
    if reply.key != request.key || reply.capability != input.capability() {
        return Err(PortableError::RequestMismatch);
    }
    let r = input.binding.profile.registry();
    let s = Schemas::new(r)?;
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
        RegionOutcome::Invalid(e) => {
            if e.stop_reason().is_some() || !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
        RegionOutcome::Stopped(_) => {
            if !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
        RegionOutcome::Complete { .. } => {}
    }
    let expected = regions(input, request, b, c.source_admission());
    match expected.outcome {
        RegionOutcome::Stopped(reason) => return Err(reason.into()),
        RegionOutcome::Invalid(error) => return Err(error.into()),
        _ => {}
    }
    let actual = value::outcome_value(&reply.outcome, r, &s, c, b)?;
    let expected_outcome = value::outcome_value(&expected.outcome, r, &s, c, b)?;
    if !actual.equal_with_budget(&expected_outcome, b)? {
        return Err(PortableError::RequestMismatch);
    }
    if expected.sources.len() != reply.sources.len() {
        return Err(PortableError::Shape);
    }
    for (a, d) in expected.sources.iter().zip(&reply.sources) {
        b.charge(
            Resource::Work,
            (a.identity().source.0.len()
                + d.identity().source.0.len()
                + a.uri().len()
                + d.uri().len()
                + a.text().len()
                + d.text().len()) as u64
                + 40,
        )?;
        if a != d {
            return Err(PortableError::RequestMismatch);
        }
    }
    Ok(())
}

pub fn prepare<'a, 't, 'p, C: FoundationValueCodec>(
    binding: &'a PreparedBindingRequest<'t, 'p>,
    facts: Option<&'a [ReaderFactBatch]>,
    c: &mut C,
    b: &mut Budget,
) -> Result<PreparedRegionInput<'a, 't, 'p>, PortableError<C::Error>> {
    crate::analysis::region::check::validate(binding, facts, b, c.source_admission())?;
    let value = sidecar_value(binding, facts, c, b)?;
    let digest = c
        .canonical_value_digest(b"nepl3.region.reader-facts/1\0", &value, b)
        .map_err(boundary)?;
    Ok(PreparedRegionInput {
        binding,
        facts,
        key: RegionKey {
            analysis: binding.key(),
            reader_facts_digest: digest,
        },
    })
}
pub fn sidecar_value<C: FoundationValueCodec>(
    binding: &PreparedBindingRequest<'_, '_>,
    facts: Option<&[ReaderFactBatch]>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    crate::analysis::region::check::validate(binding, facts, b, c.source_admission())?;
    let registry = binding.profile.registry();
    let s = Schemas::new(registry)?;
    let reader = registry
        .selected("nepl3.reader", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    let maps = super::tree::canonical::Mappings::new(&binding.tree.tree().bundle, b)?;
    let mut store = SourceStore::default();
    for owner in &maps.entries {
        super::binding::check::add(&mut store, &owner.bundle().sources, b, c.source_admission())?;
    }
    let mut local = c.scoped(&store);
    let value = match facts {
        None => NdfValue::None,
        Some(facts) => {
            let mut values = Vec::new();
            for batch in facts {
                let (path, owner) = maps.path_value(
                    &binding.tree.tree().bundle,
                    &batch.path,
                    &s,
                    binding.profile,
                    &mut local,
                    b,
                )?;
                let mapping = maps.owner(owner, b)?;
                let node = batch.node.map(|id| mapping.mapped(id)).transpose()?;
                push(
                    &mut values,
                    record(
                        s.engine,
                        "ReaderFactBatch",
                        [
                            path,
                            node.value(&s, &mut local, b)?,
                            batch.entry.value(&s, &mut local, b)?,
                            sidecar::facts_value(&batch.facts, reader, &s, &mut local, b)?,
                            sidecar::trivia_value(&batch.trivia, &s, &mut local, b)?,
                        ],
                        b,
                    )?,
                    b,
                )?;
            }
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            NdfValue::Some(Box::new(NdfValue::List(values)))
        }
    };
    let value = record(s.engine, "RegionSidecar", [value], b)?;
    registry.validate(&expected("RegionSidecar", b)?, &value, b)?;
    Ok(value)
}
/// The caller's prepared tree may come from initial canonical tree decoding.
/// All positions resolve only in that tree's explicit source closure.
pub fn sidecar_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    binding: &PreparedBindingRequest<'_, '_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<Option<Vec<ReaderFactBatch>>, PortableError<C::Error>> {
    let registry = binding.profile.registry();
    registry.validate(&expected("RegionSidecar", b)?, value, b)?;
    let s = Schemas::new(registry)?;
    let reader = registry
        .selected("nepl3.reader", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    let map = super::tree::canonical::Mappings::new(&binding.tree.tree().bundle, b)?;
    let mut store = SourceStore::default();
    for owner in &map.entries {
        super::binding::check::add(&mut store, &owner.bundle().sources, b, c.source_admission())?;
    }
    let mut local = c.scoped(&store);
    let f = fields(value, s.engine, "RegionSidecar", 1)?;
    let result = match &f[0] {
        NdfValue::None => None,
        NdfValue::Some(v) => {
            let mut out = Vec::new();
            for value in list(v)? {
                let f = fields(value, s.engine, "ReaderFactBatch", 5)?;
                let canonical =
                    Vec::<crate::recovery::ForeignStep>::read(&f[0], &s, &mut local, b)?;
                let (path, owner) = sidecar::path_read(
                    &canonical,
                    &binding.tree.tree().bundle,
                    &map,
                    binding.profile,
                    b,
                )?;
                let node = Option::<nepl3_core::syntax::NodeRef>::read(&f[1], &s, &mut local, b)?;
                let node = if let Some(node) = node {
                    Some(sidecar::old_node(map.owner(owner, b)?, node)?)
                } else {
                    None
                };
                push(
                    &mut out,
                    ReaderFactBatch {
                        path,
                        node,
                        entry: Value::read(&f[2], &s, &mut local, b)?,
                        facts: sidecar::facts_read(&f[3], reader, &s, &mut local, b)?,
                        trivia: sidecar::trivia_read(&f[4], &s, &mut local, b)?,
                    },
                    b,
                )?;
            }
            Some(out)
        }
        _ => return Err(PortableError::Shape),
    };
    crate::analysis::region::check::validate(
        binding,
        result.as_deref(),
        b,
        local.source_admission(),
    )?;
    Ok(result)
}
