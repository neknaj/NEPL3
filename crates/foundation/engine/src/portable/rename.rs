//! Raw rename messages do not mint a native accepted transaction. A host must
//! authenticate writable grants and use the actual reparse/analysis workflow.
use super::{PortableError, boundary, value::*};
use crate::analysis::rename::*;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::{Digest, SourceRef, SourceStore, TextEdit},
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
mod value;

pub fn request_to_value<C: FoundationValueCodec>(
    request: &RenameRequest,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    check_request(request, b)?;
    let value = request.value(&Schemas::new(registry)?, c, b)?;
    registry.validate(&expected("RenameRequest", b)?, &value, b)?;
    Ok(value)
}
pub fn request_decode<C: FoundationValueCodec>(
    v: &NdfValue,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RenameRequest, PortableError<C::Error>> {
    registry.validate(&expected("RenameRequest", b)?, v, b)?;
    let request = RenameRequest::read(v, &Schemas::new(registry)?, c, b)?;
    check_request(&request, b)?;
    Ok(request)
}
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &RenameReply,
    request: &RenameRequest,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut store = SourceStore::default();
    super::binding::check::add(&mut store, &reply.sources, b, c.source_admission())?;
    validate(reply, request, registry, &store, b)?;
    let s = Schemas::new(registry)?;
    let mut local = c.scoped(&store);
    let v = record(
        s.engine,
        "RenameReply",
        [
            super::analysis::key_value(&reply.key, &s, &mut local, b)?,
            value::outcome_value(&reply.outcome, registry, &s, &mut local, b)?,
            local.encode_report(&reply.report, b).map_err(boundary)?,
            local.encode_sources(&reply.sources, b).map_err(boundary)?,
        ],
        b,
    )?;
    registry.validate(&expected("RenameReply", b)?, &v, b)?;
    Ok(v)
}
pub fn reply_decode<C: FoundationValueCodec>(
    v: &NdfValue,
    request: &RenameRequest,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RenameReply, PortableError<C::Error>> {
    registry.validate(&expected("RenameReply", b)?, v, b)?;
    let s = Schemas::new(registry)?;
    let f = fields(v, s.engine, "RenameReply", 4)?;
    let sources = c.decode_sources(&f[3], b).map_err(boundary)?;
    let mut store = SourceStore::default();
    super::binding::check::add(&mut store, &sources, b, c.source_admission())?;
    let mut local = c.scoped(&store);
    let reply = RenameReply {
        key: super::analysis::key_read(&f[0], &s, &mut local, b)?,
        outcome: value::outcome_read(&f[1], registry, &s, &mut local, b)?,
        report: local.decode_report(&f[2], b).map_err(boundary)?,
        sources,
    };
    validate(&reply, request, registry, &store, b)?;
    Ok(reply)
}
fn check_request<E>(request: &RenameRequest, b: &mut Budget) -> Result<(), PortableError<E>> {
    request.validate_identity(b)?;
    Ok(())
}
fn validate<E>(
    reply: &RenameReply,
    request: &RenameRequest,
    registry: &SchemaRegistry,
    store: &SourceStore,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    check_request(request, b)?;
    b.charge(Resource::Work, 128)?;
    if reply.key != request.key {
        return Err(PortableError::RequestMismatch);
    }
    reply
        .report
        .validate(store, &[], registry, b)
        .map_err(crate::facts::FactsError::from)?;
    if !reply.report.diagnostics.is_empty()
        || !reply.report.events.is_empty()
        || reply.report.trace_overflow.is_some()
    {
        return Err(PortableError::Shape);
    }
    let (new_key, edits) = match &reply.outcome {
        RenameOutcome::Complete { new_key, edits } => (new_key, edits),
        RenameOutcome::Invalid(e) => {
            if e.stop_reason().is_some() || !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
        RenameOutcome::Stopped(_) => {
            if !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
    };
    if request.new_name.is_empty()
        || edits.is_empty()
        || new_key.profile_digest != request.key.profile_digest
        || new_key.execution_digest != request.key.execution_digest
        || new_key.request_digest != request.key.request_digest
    {
        return Err(PortableError::RequestMismatch);
    }
    nepl3_core::diagnostic::validation::validate_edits(edits, store, b)
        .map_err(crate::facts::FactsError::from)?;
    for (i, edit) in edits.iter().enumerate() {
        b.charge(
            Resource::Work,
            (edit.replacement.len() + request.new_name.len()) as u64,
        )?;
        if edit.replacement != request.new_name {
            return Err(PortableError::RequestMismatch);
        }
        let mut writable = false;
        for source in &request.writable {
            b.charge(
                Resource::Work,
                (edit.span.snapshot_ref().source.0.len() + source.source_id.0.len()) as u64 + 42,
            )?;
            writable |= edit.span.snapshot_ref().source == source.source_id
                && edit.span.snapshot_ref().revision == source.revision
                && edit.span.snapshot_ref().digest == source.digest;
        }
        if !writable {
            return Err(PortableError::RequestMismatch);
        }
        if let Some(prior) = i.checked_sub(1).map(|i| &edits[i]) {
            b.charge(
                Resource::Work,
                (edit.span.snapshot_ref().source.0.len() + prior.span.snapshot_ref().source.0.len())
                    as u64
                    + 42,
            )?;
            if (
                &prior.span.snapshot_ref().source.0,
                prior.span.start(),
                prior.span.end(),
            ) >= (
                &edit.span.snapshot_ref().source.0,
                edit.span.start(),
                edit.span.end(),
            ) {
                return Err(PortableError::NonCanonical);
            }
        }
    }
    for source in &reply.sources {
        let mut used = false;
        for edit in edits {
            b.charge(
                Resource::Work,
                (source.identity().source.0.len() + edit.span.snapshot_ref().source.0.len()) as u64
                    + 42,
            )?;
            used |= source.identity() == edit.span.snapshot_ref();
        }
        if !used {
            return Err(PortableError::Shape);
        }
    }
    Ok(())
}
