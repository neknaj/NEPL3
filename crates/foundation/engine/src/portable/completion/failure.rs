//! Request-correlated failure metadata. Causes and sender usage are claims;
//! decoding does not certify execution or authorize edits.
use super::*;
use nepl3_core::diagnostic::Report;

#[derive(Debug)]
pub struct ScopeCandidateFailureReply {
    pub request: DecodedScopeCandidateRequest,
    pub error: CandidateError,
    pub report: Report,
}
pub fn to_value<C: FoundationValueCodec>(
    reply: &ScopeCandidateFailureReply,
    request: &ScopeCandidateRequest<'_>,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    validate(reply, request, r, b)?;
    let empty = SourceStore::default();
    let mut local = c.scoped(&empty);
    let s = Schemas::new(r)?;
    let value = record(
        s.engine,
        "ScopeCandidateFailureReply",
        [
            super::request_to_value(&reply.request.as_request(), r, &mut local, b)?,
            super::error::value(&reply.error, r, b)?,
            local.encode_report(&reply.report, b).map_err(boundary)?,
        ],
        b,
    )?;
    r.validate(&expected("ScopeCandidateFailureReply", b)?, &value, b)?;
    Ok(value)
}
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    request: &ScopeCandidateRequest<'_>,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<ScopeCandidateFailureReply, PortableError<C::Error>> {
    r.validate(&expected("ScopeCandidateFailureReply", b)?, value, b)?;
    let empty = SourceStore::default();
    let mut local = c.scoped(&empty);
    let s = Schemas::new(r)?;
    let f = fields(value, s.engine, "ScopeCandidateFailureReply", 3)?;
    let reply = ScopeCandidateFailureReply {
        request: super::request_decode(&f[0], r, &mut local, b)?,
        error: super::error::read(&f[1], r, b)?,
        report: local.decode_report(&f[2], b).map_err(boundary)?,
    };
    validate(&reply, request, r, b)?;
    Ok(reply)
}
fn validate<E>(
    reply: &ScopeCandidateFailureReply,
    request: &ScopeCandidateRequest<'_>,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    b.charge(
        Resource::Work,
        128u64
            .saturating_add(reply.request.prefix.len() as u64)
            .saturating_add(request.prefix.len() as u64),
    )?;
    if reply.request.key != request.key
        || reply.request.occurrence != request.occurrence
        || reply.request.prefix != request.prefix
    {
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
    Ok(())
}
