//! Request-correlated failure metadata. A sender's cause, capability and usage
//! are claims, not execution proof. Nested stop causes remain receiver data.
use super::*;
use nepl3_core::diagnostic::Report;

#[derive(Debug)]
pub struct RegionCompletionFailureReply {
    pub request: RegionCompletionRequest,
    pub capability: RegionCapability,
    pub error: RegionCompletionError,
    pub report: Report,
}
pub fn to_value<C: FoundationValueCodec>(
    reply: &RegionCompletionFailureReply,
    request: &RegionCompletionRequest,
    capability: RegionCapability,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let empty = SourceStore::default();
    let mut local = c.scoped(&empty);
    validate(reply, request, capability, r, &mut local, b)?;
    let s = Schemas::new(r)?;
    let cause = match &reply.error {
        RegionCompletionError::Selection(error) => variant(
            s.engine,
            "RegionCompletionError",
            "Selection",
            [super::super::query::value::error_value(
                error, r, &s, &mut local, b,
            )?],
            b,
        )?,
        RegionCompletionError::Candidates(error) => variant(
            s.engine,
            "RegionCompletionError",
            "Candidates",
            [crate::portable::completion::error::value(error, r, b)?],
            b,
        )?,
    };
    let value = record(
        s.engine,
        "RegionCompletionFailureReply",
        [
            super::request_to_value(&reply.request, r, &mut local, b)?,
            reply.capability.value(&s, &mut local, b)?,
            cause,
            local.encode_report(&reply.report, b).map_err(boundary)?,
        ],
        b,
    )?;
    r.validate(&expected("RegionCompletionFailureReply", b)?, &value, b)?;
    Ok(value)
}
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    request: &RegionCompletionRequest,
    capability: RegionCapability,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionCompletionFailureReply, PortableError<C::Error>> {
    r.validate(&expected("RegionCompletionFailureReply", b)?, value, b)?;
    let empty = SourceStore::default();
    let mut local = c.scoped(&empty);
    let s = Schemas::new(r)?;
    let f = fields(value, s.engine, "RegionCompletionFailureReply", 4)?;
    let error = match parts(&f[2], s.engine, "RegionCompletionError")? {
        ("Selection", [error]) => RegionCompletionError::Selection(
            super::super::query::value::error_read(error, r, &s, &mut local, b)?,
        ),
        ("Candidates", [error]) => RegionCompletionError::Candidates(
            crate::portable::completion::error::read(error, r, b)?,
        ),
        _ => return Err(PortableError::Shape),
    };
    let reply = RegionCompletionFailureReply {
        request: super::request_decode(&f[0], r, &mut local, b)?,
        capability: Value::read(&f[1], &s, &mut local, b)?,
        error,
        report: local.decode_report(&f[3], b).map_err(boundary)?,
    };
    validate(&reply, request, capability, r, &mut local, b)?;
    Ok(reply)
}
fn validate<C: FoundationValueCodec>(
    reply: &RegionCompletionFailureReply,
    request: &RegionCompletionRequest,
    capability: RegionCapability,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    // Canonical request equality pays for every source identity, digest, offset
    // and prefix byte, without requiring a source snapshot for a failed request.
    let actual = super::request_to_value(&reply.request, r, c, b)?;
    let expected_request = super::request_to_value(request, r, c, b)?;
    if !actual.equal_with_budget(&expected_request, b)? || reply.capability != capability {
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
