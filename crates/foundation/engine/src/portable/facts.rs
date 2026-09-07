//! Exact host-issued requests and request-relative reply source/authority closure.
use super::{PortableError, boundary, tree, value::*};
use crate::facts::{CheckedFactsRequest, FactsReply, FactsRequest};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
pub(super) mod compare;

pub fn request_to_value<C: FoundationValueCodec>(
    request: &CheckedFactsRequest<'_, '_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    request_view_to_value(&request.view(), codec, b)
}
/// Serialize the same issued request from borrowed native data. The native
/// provider path need not first clone a complete tree and existing FactSet.
pub fn request_view_to_value<C: FoundationValueCodec>(
    request: &crate::facts::CheckedFactsView<'_, '_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let profile = request.profile();
    let request = request.request();
    request.issue(profile, b, codec.source_admission())?;
    let s = Schemas::new(profile.registry())?;
    let mappings = tree::canonical::Mappings::new(&request.tree.bundle, b)?;
    let (path, owner) =
        mappings.path_value(&request.tree.bundle, request.path, &s, profile, codec, b)?;
    let node = mappings.owner(owner, b)?.mapped(request.node)?;
    let store =
        crate::facts::check::closure_view(request, None, &[], &[], b, codec.source_admission())?;
    let mut local = codec.scoped(&store);
    let value = record(
        s.engine,
        "FactsRequest",
        [
            tree::to_value(request.tree, profile, &mut local, b)?,
            path,
            node.value(&s, &mut local, b)?,
            local
                .encode_fact_set(request.existing, b)
                .map_err(boundary)?,
            local
                .encode_fact_authority(request.authority, b)
                .map_err(boundary)?,
        ],
        b,
    )?;
    profile
        .registry()
        .validate(&expected("FactsRequest", b)?, &value, b)?;
    Ok(value)
}
/// `expected` comes from the trusted dispatch slot, never from this wire value.
/// Equality includes the entire existing analysis and every authority field.
/// This is an echo/loopback check. First-time receivers use `request_decode`
/// before their host's separate transport authentication and authorization.
pub fn request_from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    expected_request: &CheckedFactsRequest<'_, '_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<FactsRequest, PortableError<C::Error>> {
    let profile = expected_request.profile();
    profile
        .registry()
        .validate(&expected("FactsRequest", b)?, value, b)?;
    let canonical = request_to_value(expected_request, codec, b)?;
    if !compare::equal(value, &canonical, b)? {
        return Err(PortableError::RequestMismatch);
    }
    request_decode(value, profile, codec, b)
}
/// Restores an untrusted request without possessing the sender's Rust request.
/// It checks type/source/tree/grant-reference invariants and returns raw data,
/// not an execution proof. Only the host's authenticated and authorized dispatch
/// path may subsequently call `FactsRequest::issue`.
pub fn request_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    profile: &crate::profile::ResolvedParseProfile<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<FactsRequest, PortableError<C::Error>> {
    profile
        .registry()
        .validate(&expected("FactsRequest", b)?, value, b)?;
    let s = Schemas::new(profile.registry())?;
    let f = fields(value, s.engine, "FactsRequest", 5)?;
    let tree = tree::from_value(&f[0], profile, codec, b)?;
    let existing = codec.decode_fact_set(&f[3], b).map_err(boundary)?;
    let store = crate::facts::check::closure_for(
        &tree,
        &existing,
        None,
        &[],
        &[],
        b,
        codec.source_admission(),
    )?;
    let mut local = codec.scoped(&store);
    let result = FactsRequest {
        tree,
        path: Value::read(&f[1], &s, &mut local, b)?,
        node: Value::read(&f[2], &s, &mut local, b)?,
        existing,
        authority: local.decode_fact_authority(&f[4], b).map_err(boundary)?,
    };
    result.validate(profile, b, local.source_admission())?;
    Ok(result)
}
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &FactsReply,
    request: &CheckedFactsRequest<'_, '_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    reply.validate(request, b, codec.source_admission())?;
    let (delta, report, sources, maps) = reply.parts();
    let profile = request.profile();
    let s = Schemas::new(profile.registry())?;
    let base = request
        .request()
        .existing
        .validate(profile.registry(), b, codec.source_admission())
        .map_err(crate::facts::FactsError::from)?;
    let store = crate::facts::check::closure(
        request.request(),
        delta,
        sources,
        maps,
        b,
        codec.source_admission(),
    )?;
    let mut local = codec.scoped(&store);
    let delta = match delta {
        Some(d) => Some(
            local
                .encode_fact_delta(d, &base, &request.request().authority, b)
                .map_err(boundary)?,
        ),
        None => None,
    };
    let report = local.encode_report(report, b).map_err(boundary)?;
    let sources = local.encode_sources(sources, b).map_err(boundary)?;
    let maps = local.encode_mappings(maps, b).map_err(boundary)?;
    let value = match reply {
        FactsReply::Complete { .. } => variant(
            s.engine,
            "FactsReply",
            "Complete",
            [delta.ok_or(PortableError::Shape)?, report, sources, maps],
            b,
        )?,
        FactsReply::Invalid { .. } => variant(
            s.engine,
            "FactsReply",
            "Invalid",
            [optional(delta, b)?, report, sources, maps],
            b,
        )?,
        FactsReply::Stopped { reason, .. } => variant(
            s.engine,
            "FactsReply",
            "Stopped",
            [
                stop_value(*reason, &s, b)?,
                optional(delta, b)?,
                report,
                sources,
                maps,
            ],
            b,
        )?,
    };
    profile
        .registry()
        .validate(&expected("FactsReply", b)?, &value, b)?;
    Ok(value)
}
pub fn reply_from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    request: &CheckedFactsRequest<'_, '_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<FactsReply, PortableError<C::Error>> {
    let profile = request.profile();
    profile
        .registry()
        .validate(&expected("FactsReply", b)?, value, b)?;
    let s = Schemas::new(profile.registry())?;
    let (case, f) = parts(value, s.engine, "FactsReply")?;
    let (reason, f) = if case == "Stopped" {
        if f.len() != 5 {
            return Err(PortableError::Shape);
        }
        (Some(stop_from(&f[0], &s)?), &f[1..])
    } else {
        (None, f)
    };
    if f.len() != 4 {
        return Err(PortableError::Shape);
    }
    let base = request
        .request()
        .existing
        .validate(profile.registry(), b, codec.source_admission())
        .map_err(crate::facts::FactsError::from)?;
    let delta_value = if case == "Complete" {
        Some(&f[0])
    } else {
        match &f[0] {
            NdfValue::None => None,
            NdfValue::Some(v) => Some(v.as_ref()),
            _ => return Err(PortableError::Shape),
        }
    };
    let delta = match delta_value {
        Some(v) => Some(
            codec
                .decode_fact_delta(v, &base, &request.request().authority, b)
                .map_err(boundary)?,
        ),
        None => None,
    };
    let sources = codec.decode_sources(&f[2], b).map_err(boundary)?;
    let store = crate::facts::check::closure(
        request.request(),
        delta.as_ref(),
        &sources,
        &[],
        b,
        codec.source_admission(),
    )?;
    let mut local = codec.scoped(&store);
    let source_maps = local.decode_mappings(&f[3], b).map_err(boundary)?;
    let report = local.decode_report(&f[1], b).map_err(boundary)?;
    let result = match case {
        "Complete" => FactsReply::Complete {
            delta: delta.ok_or(PortableError::Shape)?,
            report,
            sources,
            source_maps,
        },
        "Invalid" => FactsReply::Invalid {
            partial: delta,
            report,
            sources,
            source_maps,
        },
        "Stopped" => FactsReply::Stopped {
            reason: reason.ok_or(PortableError::Shape)?,
            partial: delta,
            report,
            sources,
            source_maps,
        },
        _ => return Err(PortableError::Shape),
    };
    result.validate(request, b, local.source_admission())?;
    Ok(result)
}
pub(super) fn optional<E>(
    v: Option<NdfValue>,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    Ok(match v {
        Some(v) => {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            NdfValue::Some(Box::new(v))
        }
        None => NdfValue::None,
    })
}
pub(super) fn stop_value<E>(
    v: StopReason,
    s: &Schemas<'_>,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    let name = match v {
        StopReason::Cancelled => "Cancelled",
        StopReason::SourceLimit => "SourceLimit",
        StopReason::WorkLimit => "WorkLimit",
        StopReason::DepthLimit => "DepthLimit",
        StopReason::NodeLimit => "NodeLimit",
        StopReason::AllocationLimit => "AllocationLimit",
        StopReason::OutputLimit => "OutputLimit",
        StopReason::DiagnosticLimit => "DiagnosticLimit",
        StopReason::EventLimit => "EventLimit",
    };
    variant(s.foundation, "StopReason", name, [], b)
}
pub(super) fn stop_from<E>(v: &NdfValue, s: &Schemas<'_>) -> Result<StopReason, PortableError<E>> {
    let (name, f) = parts(v, s.foundation, "StopReason")?;
    if !f.is_empty() {
        return Err(PortableError::Shape);
    }
    Ok(match name {
        "Cancelled" => StopReason::Cancelled,
        "SourceLimit" => StopReason::SourceLimit,
        "WorkLimit" => StopReason::WorkLimit,
        "DepthLimit" => StopReason::DepthLimit,
        "NodeLimit" => StopReason::NodeLimit,
        "AllocationLimit" => StopReason::AllocationLimit,
        "OutputLimit" => StopReason::OutputLimit,
        "DiagnosticLimit" => StopReason::DiagnosticLimit,
        "EventLimit" => StopReason::EventLimit,
        _ => return Err(PortableError::Shape),
    })
}
