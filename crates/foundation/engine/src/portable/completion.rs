//! Successful declaration metadata is rederived from the receiver's native
//! Binding reply. Decoded values never create execution or edit proofs.
use super::{PortableError, boundary, value::*};
use crate::analysis::{
    AnalysisKey, BoundBindingReply,
    completion::{self as native, *},
};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    facts::{NamespaceRef, OccurrenceId},
    schema::SchemaRegistry,
    source::SourceStore,
    value::NdfValue,
    value_codec::FoundationValueCodec,
};

pub mod error;
pub mod failure;

/// Owned request data. Borrowing it grants no permission or execution proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedScopeCandidateRequest {
    pub key: AnalysisKey,
    pub occurrence: OccurrenceId,
    pub prefix: String,
}
impl DecodedScopeCandidateRequest {
    pub fn as_request(&self) -> ScopeCandidateRequest<'_> {
        ScopeCandidateRequest {
            key: self.key,
            occurrence: self.occurrence,
            prefix: &self.prefix,
        }
    }
}
pub fn request_to_value<C: FoundationValueCodec>(
    request: &ScopeCandidateRequest<'_>,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let s = Schemas::new(r, b)?;
    b.charge(Resource::AllocationUnits, request.prefix.len() as u64)?;
    let value = record(
        s.engine,
        "ScopeCandidateRequest",
        [
            request.key.value(&s, c, b)?,
            NdfValue::U64(request.occurrence.0),
            NdfValue::Text(request.prefix.into()),
        ],
        b,
    )?;
    r.validate(&expected("ScopeCandidateRequest", b)?, &value, b)?;
    Ok(value)
}
pub fn request_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DecodedScopeCandidateRequest, PortableError<C::Error>> {
    r.validate(&expected("ScopeCandidateRequest", b)?, value, b)?;
    let s = Schemas::new(r, b)?;
    let f = fields(value, s.engine, "ScopeCandidateRequest", 3)?;
    Ok(DecodedScopeCandidateRequest {
        key: Value::read(&f[0], &s, c, b)?,
        occurrence: OccurrenceId(u64::read(&f[1], &s, c, b)?),
        prefix: Value::read(&f[2], &s, c, b)?,
    })
}
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &ScopeCandidates,
    request: &ScopeCandidateRequest<'_>,
    binding: &BoundBindingReply,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    // Candidate metadata has no source closure. Do not let a host's ambient
    // store enter the report codec or the operation's shared admission ledger.
    let empty = SourceStore::default();
    let mut local = c.scoped(&empty);
    let c = &mut local;
    validate_reply(reply, request, binding, r, c, b)?;
    let s = Schemas::new(r, b)?;
    let candidates = candidates_value(reply, &s, c, b)?;
    let value = record(
        s.engine,
        "ScopeCandidates",
        [
            reply.key.value(&s, c, b)?,
            NdfValue::U64(reply.occurrence.0),
            NdfValue::U64(reply.namespace.0),
            candidates,
            c.encode_report(&reply.report, b).map_err(boundary)?,
        ],
        b,
    )?;
    r.validate(&expected("ScopeCandidates", b)?, &value, b)?;
    Ok(value)
}
pub fn reply_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    request: &ScopeCandidateRequest<'_>,
    binding: &BoundBindingReply,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<ScopeCandidates, PortableError<C::Error>> {
    let empty = SourceStore::default();
    let mut local = c.scoped(&empty);
    let c = &mut local;
    r.validate(&expected("ScopeCandidates", b)?, value, b)?;
    let s = Schemas::new(r, b)?;
    let f = fields(value, s.engine, "ScopeCandidates", 5)?;
    let mut candidates = Vec::new();
    for item in list(&f[3])? {
        b.charge(Resource::Nodes, 1)?;
        let f = fields(item, s.engine, "ScopeNameCandidate", 2)?;
        let candidate = NameCandidate {
            name: Value::read(&f[0], &s, c, b)?,
            resolution: c.decode_reference_resolution(&f[1], b).map_err(boundary)?,
        };
        push(&mut candidates, candidate, b)?;
    }
    let reply = ScopeCandidates {
        key: Value::read(&f[0], &s, c, b)?,
        occurrence: OccurrenceId(u64::read(&f[1], &s, c, b)?),
        namespace: NamespaceRef(u64::read(&f[2], &s, c, b)?),
        candidates,
        report: c.decode_report(&f[4], b).map_err(boundary)?,
    };
    validate_reply(&reply, request, binding, r, c, b)?;
    Ok(reply)
}
fn candidates_value<C: FoundationValueCodec>(
    reply: &ScopeCandidates,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut values = Vec::new();
    for candidate in &reply.candidates {
        b.charge(Resource::Nodes, 1)?;
        let value = record(
            s.engine,
            "ScopeNameCandidate",
            [
                candidate.name.value(s, c, b)?,
                c.encode_reference_resolution(&candidate.resolution, b)
                    .map_err(boundary)?,
            ],
            b,
        )?;
        push(&mut values, value, b)?;
    }
    Ok(NdfValue::List(values))
}
fn validate_reply<C: FoundationValueCodec>(
    reply: &ScopeCandidates,
    request: &ScopeCandidateRequest<'_>,
    binding: &BoundBindingReply,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    b.charge(Resource::Work, 128)?;
    if reply.key != request.key || reply.occurrence != request.occurrence {
        return Err(PortableError::RequestMismatch);
    }
    if !reply.report.diagnostics.is_empty()
        || !reply.report.events.is_empty()
        || reply.report.trace_overflow.is_some()
    {
        return Err(PortableError::Shape);
    }
    reply
        .report
        .validate(&SourceStore::default(), &[], r, b)
        .map_err(crate::facts::FactsError::from)?;
    // The receiver pays recomputation from its actual native analysis. Sender
    // data and claimed Usage cannot bypass stale, incomplete, or budget gates.
    let actual = native::names(binding, request, b).map_err(|error| match error {
        CandidateError::Stopped(reason) => PortableError::Stopped(reason),
        other => PortableError::Candidate(other),
    })?;
    if actual.namespace != reply.namespace {
        return Err(PortableError::RequestMismatch);
    }
    let s = Schemas::new(r, b)?;
    let supplied = candidates_value(reply, &s, c, b)?;
    let computed = candidates_value(&actual, &s, c, b)?;
    if !supplied.equal_with_budget(&computed, b)? {
        return Err(PortableError::RequestMismatch);
    }
    Ok(())
}
