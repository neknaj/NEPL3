//! Initial decoding and explicit preparation of keyed binding requests.
//! This verifies data and identities; transport authentication and host execution
//! permission remain separate. Received limits are never host authorization.
use super::{PortableError, boundary, value::*};
use crate::{analysis::*, profile::ResolvedParseProfile, recovery::ParseTree, tree::TreeError};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Limits, Resource},
    source::Digest,
    value::NdfValue,
    value_codec::FoundationValueCodec,
};

fn within(a: Limits, b: Limits) -> bool {
    a.source_bytes <= b.source_bytes
        && a.work <= b.work
        && a.depth <= b.depth
        && a.nodes <= b.nodes
        && a.allocation_units <= b.allocation_units
        && a.output_bytes <= b.output_bytes
        && a.diagnostics <= b.diagnostics
        && a.events <= b.events
}
fn options_value<C: FoundationValueCodec>(
    options: BindingOptions,
    s: &Schemas<'_>,
    _: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let BindingOptions = options;
    record(s.engine, "BindingOptions", [], b)
}
pub(super) fn key_value<C: FoundationValueCodec>(
    key: &AnalysisKey,
    s: &Schemas<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    record(
        s.engine,
        "AnalysisKey",
        [
            key.tree_digest.value(s, codec, b)?,
            key.profile_digest.value(s, codec, b)?,
            key.execution_digest.value(s, codec, b)?,
            key.request_digest.value(s, codec, b)?,
        ],
        b,
    )
}
pub(super) fn key_read<C: FoundationValueCodec>(
    value: &NdfValue,
    s: &Schemas<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<AnalysisKey, PortableError<C::Error>> {
    let f = fields(value, s.engine, "AnalysisKey", 4)?;
    Ok(AnalysisKey {
        tree_digest: Digest::read(&f[0], s, codec, b)?,
        profile_digest: Digest::read(&f[1], s, codec, b)?,
        execution_digest: Digest::read(&f[2], s, codec, b)?,
        request_digest: Digest::read(&f[3], s, codec, b)?,
    })
}
fn derive<C: FoundationValueCodec>(
    id: &str,
    tree_value: &NdfValue,
    options: BindingOptions,
    limits: Limits,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<AnalysisKey, PortableError<C::Error>> {
    b.charge(Resource::Work, 1)?;
    if id.is_empty() || !within(limits, profile.profile().limits) {
        return Err(PortableError::Shape);
    }
    let s = Schemas::new(profile.registry())?;
    let ordered =
        crate::package::identity::sorted(&profile.profile().languages, |v| (&v.alias, ""), b)
            .map_err(TreeError::from)?;
    let mut executions = Vec::new();
    for language in ordered {
        let digest = profile
            .execution_digest(&language.alias, b)
            .map_err(TreeError::from)?;
        let value = record(
            s.engine,
            "AnalysisExecution",
            [
                language.alias.value(&s, codec, b)?,
                digest.value(&s, codec, b)?,
            ],
            b,
        )?;
        push(&mut executions, value, b)?;
    }
    // The profile digest includes selected schemas, providers, resources and
    // default limits. The concrete alias execution table also fixes arena IDs.
    let execution_digest = codec
        .canonical_value_digest(
            b"nepl3.analysis.execution/1\0",
            &NdfValue::List(executions),
            b,
        )
        .map_err(boundary)?;
    b.charge(Resource::Work, id.len() as u64)?;
    b.charge(Resource::AllocationUnits, id.len() as u64)?;
    let id = String::from(id);
    let conditions = record(
        s.engine,
        "BindingConditions",
        [
            NdfValue::Text(id),
            options_value(options, &s, codec, b)?,
            limits.value(&s, codec, b)?,
        ],
        b,
    )?;
    profile
        .registry()
        .validate(&expected("BindingConditions", b)?, &conditions, b)?;
    let request_digest = codec
        .canonical_value_digest(b"nepl3.analysis.request/1\0", &conditions, b)
        .map_err(boundary)?;
    let tree_digest = codec
        .canonical_value_digest(b"nepl3.analysis.tree/1\0", tree_value, b)
        .map_err(boundary)?;
    Ok(AnalysisKey {
        tree_digest,
        profile_digest: profile.digest(),
        execution_digest,
        request_digest,
    })
}

/// Preparation has its own explicitly supplied Budget; execution later uses
/// exactly `limits`, which cannot exceed the selected profile's ceilings.
pub fn prepare<'a, 'p, C: FoundationValueCodec>(
    id: &'a str,
    tree: &'a ParseTree,
    options: BindingOptions,
    limits: Limits,
    profile: &'a ResolvedParseProfile<'p>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<PreparedBindingRequest<'a, 'p>, PortableError<C::Error>> {
    let value = super::tree::to_value(tree, profile, codec, b)?;
    let key = derive(id, &value, options, limits, profile, codec, b)?;
    let checked = tree.validate(profile, b, codec.source_admission())?;
    Ok(PreparedBindingRequest {
        analysis_id: id,
        tree: checked,
        profile,
        options,
        limits,
        key,
    })
}
pub fn prepare_received<'a, 'p, C: FoundationValueCodec>(
    request: &'a BindingRequest,
    profile: &'a ResolvedParseProfile<'p>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<PreparedBindingRequest<'a, 'p>, PortableError<C::Error>> {
    let prepared = prepare(
        &request.analysis_id,
        &request.tree,
        request.options,
        request.limits,
        profile,
        codec,
        b,
    )?;
    if prepared.key != request.key {
        return Err(PortableError::RequestMismatch);
    }
    Ok(prepared)
}
pub fn request_to_value<C: FoundationValueCodec>(
    request: &PreparedBindingRequest<'_, '_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let s = Schemas::new(request.profile.registry())?;
    b.charge(Resource::Work, request.analysis_id.len() as u64)?;
    b.charge(Resource::AllocationUnits, request.analysis_id.len() as u64)?;
    let id = String::from(request.analysis_id);
    let value = record(
        s.engine,
        "BindingRequest",
        [
            NdfValue::Text(id),
            super::tree::to_value(request.tree.tree(), request.profile, codec, b)?,
            options_value(request.options, &s, codec, b)?,
            request.limits.value(&s, codec, b)?,
            key_value(&request.key, &s, codec, b)?,
        ],
        b,
    )?;
    request
        .profile
        .registry()
        .validate(&expected("BindingRequest", b)?, &value, b)?;
    Ok(value)
}
/// First receiver: no original Rust request or ambient source store is required.
/// A successful return is an owned request, not permission to execute its limits.
pub fn request_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<BindingRequest, PortableError<C::Error>> {
    profile
        .registry()
        .validate(&expected("BindingRequest", b)?, value, b)?;
    let s = Schemas::new(profile.registry())?;
    let f = fields(value, s.engine, "BindingRequest", 5)?;
    let analysis_id = String::read(&f[0], &s, codec, b)?;
    let tree = super::tree::from_value(&f[1], profile, codec, b)?;
    fields(&f[2], s.engine, "BindingOptions", 0)?;
    let options = BindingOptions;
    let limits = Limits::read(&f[3], &s, codec, b)?;
    let key = key_read(&f[4], &s, codec, b)?;
    // tree::from_value checks canonical numbering and all local declarations.
    let actual = derive(&analysis_id, &f[1], options, limits, profile, codec, b)?;
    if actual != key {
        return Err(PortableError::RequestMismatch);
    }
    Ok(BindingRequest {
        analysis_id,
        tree,
        options,
        limits,
        key,
    })
}

/// Received data retains its key, but contains no BindingAnalysis proof.
pub struct DecodedBoundBindingReply {
    pub key: AnalysisKey,
    pub reply: super::binding::DecodedBindingReply,
}

pub fn access_error_to_value(
    error: BindingAccessError,
    registry: &nepl3_core::schema::SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<core::convert::Infallible>> {
    let s = Schemas::new(registry)?;
    let value = match error {
        BindingAccessError::Stopped(reason) => variant(
            s.engine,
            "BindingAccessError",
            "Stopped",
            [super::facts::stop_value(reason, &s, b)?],
            b,
        )?,
        BindingAccessError::LimitsMismatch => {
            variant(s.engine, "BindingAccessError", "LimitsMismatch", [], b)?
        }
        BindingAccessError::StaleAnalysis => {
            variant(s.engine, "BindingAccessError", "StaleAnalysis", [], b)?
        }
        BindingAccessError::MissingSource => {
            variant(s.engine, "BindingAccessError", "MissingSource", [], b)?
        }
        BindingAccessError::Incomplete => {
            variant(s.engine, "BindingAccessError", "Incomplete", [], b)?
        }
    };
    registry.validate(&expected("BindingAccessError", b)?, &value, b)?;
    Ok(value)
}
pub fn access_error_from_value(
    value: &NdfValue,
    registry: &nepl3_core::schema::SchemaRegistry,
    b: &mut Budget,
) -> Result<BindingAccessError, PortableError<core::convert::Infallible>> {
    registry.validate(&expected("BindingAccessError", b)?, value, b)?;
    let s = Schemas::new(registry)?;
    let (name, fields) = parts(value, s.engine, "BindingAccessError")?;
    Ok(match (name, fields) {
        ("Stopped", [reason]) => BindingAccessError::Stopped(super::facts::stop_from(reason, &s)?),
        ("LimitsMismatch", []) => BindingAccessError::LimitsMismatch,
        ("StaleAnalysis", []) => BindingAccessError::StaleAnalysis,
        ("MissingSource", []) => BindingAccessError::MissingSource,
        ("Incomplete", []) => BindingAccessError::Incomplete,
        _ => return Err(PortableError::Shape),
    })
}
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &BoundBindingReply,
    registry: &nepl3_core::schema::SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let s = Schemas::new(registry)?;
    let value = record(
        s.engine,
        "BoundBindingReply",
        [
            key_value(&reply.key(), &s, codec, b)?,
            super::binding::reply_to_value(reply.reply(), registry, codec, b)?,
        ],
        b,
    )?;
    registry.validate(&expected("BoundBindingReply", b)?, &value, b)?;
    Ok(value)
}
/// The caller supplies its previously prepared request. A matching self-claimed
/// key does not authenticate the sender or prove the returned resolutions.
pub fn reply_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    request: &PreparedBindingRequest<'_, '_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<DecodedBoundBindingReply, PortableError<C::Error>> {
    let registry = request.profile.registry();
    registry.validate(&expected("BoundBindingReply", b)?, value, b)?;
    let s = Schemas::new(registry)?;
    let f = fields(value, s.engine, "BoundBindingReply", 2)?;
    let key = key_read(&f[0], &s, codec, b)?;
    if key != request.key {
        return Err(PortableError::RequestMismatch);
    }
    let reply = super::binding::reply_from_value(&f[1], registry, codec, b)?;
    use super::binding::DecodedBindingOutcome;
    let facts = match &reply.outcome {
        DecodedBindingOutcome::Complete(result) => Some(&result.facts),
        DecodedBindingOutcome::Invalid { progress, .. }
        | DecodedBindingOutcome::Stopped { progress, .. } => progress.facts.as_ref(),
    };
    if let Some(facts) = facts {
        b.charge(
            Resource::Work,
            (request.analysis_id.len() as u64).saturating_add(facts.analysis_id.len() as u64),
        )?;
        if facts.analysis_id != request.analysis_id {
            return Err(PortableError::RequestMismatch);
        }
    }
    Ok(DecodedBoundBindingReply { key, reply })
}
