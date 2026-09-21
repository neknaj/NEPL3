use super::*;
use nepl3_core::{
    budget::StopReason,
    diagnostic::{OperationResult, Report},
    operation::{OperationReply, Resume},
};

impl Codec for StopReason {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        variant(
            s,
            "StopReason",
            match self {
                Self::Cancelled => "Cancelled",
                Self::SourceLimit => "SourceLimit",
                Self::WorkLimit => "WorkLimit",
                Self::DepthLimit => "DepthLimit",
                Self::NodeLimit => "NodeLimit",
                Self::AllocationLimit => "AllocationLimit",
                Self::OutputLimit => "OutputLimit",
                Self::DiagnosticLimit => "DiagnosticLimit",
                Self::EventLimit => "EventLimit",
            },
            [],
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        _: &SourceStore,
        _: &mut Budget,
    ) -> Result<Self, WireError> {
        let (name, fields) = variant_parts(v, s, "StopReason")?;
        if !fields.is_empty() {
            return Err(WireError::InvalidType);
        }
        Ok(match name {
            "Cancelled" => Self::Cancelled,
            "SourceLimit" => Self::SourceLimit,
            "WorkLimit" => Self::WorkLimit,
            "DepthLimit" => Self::DepthLimit,
            "NodeLimit" => Self::NodeLimit,
            "AllocationLimit" => Self::AllocationLimit,
            "OutputLimit" => Self::OutputLimit,
            "DiagnosticLimit" => Self::DiagnosticLimit,
            "EventLimit" => Self::EventLimit,
            _ => return Err(WireError::InvalidType),
        })
    }
}

fn admit(
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<(), WireError> {
    b.poll()?;
    for source in sources.snapshots() {
        admission.admit_existing(source, b)?;
    }
    Ok(())
}
fn report(value: &OperationReply) -> &Report {
    match value {
        OperationReply::Result(
            OperationResult::Complete { report, .. }
            | OperationResult::Invalid { report, .. }
            | OperationResult::Stopped { report, .. },
        )
        | OperationReply::Await { report, .. } => report,
    }
}
fn validate_report(
    value: &OperationReply,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    b: &mut Budget,
) -> Result<(), WireError> {
    let report = report(value);
    if report.trace_overflow.is_some()
        && !matches!(
            value,
            OperationReply::Result(OperationResult::Stopped { .. })
        )
    {
        return Err(WireError::InvalidType);
    }
    report.validate(sources, &[], registry, b)?;
    Ok(())
}
fn reply_value(
    value: &OperationReply,
    s: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    validate_report(value, registry, sources, b)?;
    let report = report(value);
    let diagnostics = report.diagnostics.value(s, b)?;
    let events = report.events.value(s, b)?;
    let usage = report.usage.value(s, b)?;
    let overflow = report.trace_overflow.value(s, b)?;
    match value {
        OperationReply::Result(OperationResult::Complete { value, .. }) => variant(
            s,
            "OperationReply",
            "Complete",
            [value.value(s, b)?, diagnostics, events, usage, overflow],
            b,
        ),
        OperationReply::Result(OperationResult::Invalid { partial, .. }) => variant(
            s,
            "OperationReply",
            "Invalid",
            [partial.value(s, b)?, diagnostics, events, usage, overflow],
            b,
        ),
        OperationReply::Result(OperationResult::Stopped {
            reason, partial, ..
        }) => variant(
            s,
            "OperationReply",
            "Stopped",
            [
                reason.value(s, b)?,
                partial.value(s, b)?,
                diagnostics,
                events,
                usage,
                overflow,
            ],
            b,
        ),
        OperationReply::Await {
            continuation,
            calls,
            ..
        } => {
            let continuation = continuation.value(s, b)?;
            let calls = sequence(calls, b, |call, b| invoke_value(call, s, admission, b))?;
            // Await's field order places events before diagnostics.
            variant(
                s,
                "OperationReply",
                "Await",
                [continuation, calls, events, diagnostics, usage, overflow],
                b,
            )
        }
    }
}
fn report_from(
    fields: &[NdfValue],
    s: &SchemaRef,
    sources: &SourceStore,
    b: &mut Budget,
) -> Result<Report, WireError> {
    let [diagnostics, events, usage, overflow] = fields else {
        return Err(WireError::InvalidType);
    };
    Ok(Report {
        diagnostics: Codec::from(diagnostics, s, sources, b)?,
        events: Codec::from(events, s, sources, b)?,
        usage: Codec::from(usage, s, sources, b)?,
        trace_overflow: Codec::from(overflow, s, sources, b)?,
    })
}
fn reply_from(
    value: &NdfValue,
    s: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<OperationReply, WireError> {
    let (case, f) = variant_parts(value, s, "OperationReply")?;
    let result = match (case, f) {
        ("Complete", [value, tail @ ..]) => OperationReply::Result(OperationResult::Complete {
            value: Codec::from(value, s, sources, b)?,
            report: report_from(tail, s, sources, b)?,
        }),
        ("Invalid", [partial, tail @ ..]) => OperationReply::Result(OperationResult::Invalid {
            partial: Codec::from(partial, s, sources, b)?,
            report: report_from(tail, s, sources, b)?,
        }),
        ("Stopped", [reason, partial, tail @ ..]) => {
            OperationReply::Result(OperationResult::Stopped {
                reason: Codec::from(reason, s, sources, b)?,
                partial: Codec::from(partial, s, sources, b)?,
                report: report_from(tail, s, sources, b)?,
            })
        }
        ("Await", [continuation, calls, events, diagnostics, usage, overflow]) => {
            OperationReply::Await {
                continuation: Codec::from(continuation, s, sources, b)?,
                calls: collect(list(calls)?, b, |v, b| invoke_from(v, s, admission, b))?,
                report: Report {
                    events: Codec::from(events, s, sources, b)?,
                    diagnostics: Codec::from(diagnostics, s, sources, b)?,
                    usage: Codec::from(usage, s, sources, b)?,
                    trace_overflow: Codec::from(overflow, s, sources, b)?,
                },
            }
        }
        _ => return Err(WireError::InvalidType),
    };
    validate_report(&result, registry, sources, b)?;
    Ok(result)
}

/// Encode a reply against the saved request's explicit source closure. The host
/// also checks the selected operation's output contract and remote usage.
pub fn encode_reply(
    value: &OperationReply,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    admit(sources, admission, b)?;
    let value = reply_value(value, schema(registry)?, registry, sources, admission, b)?;
    crate::encode_checked(&value, &expected("OperationReply"), registry, b)
}
pub fn decode_reply(
    input: &[u8],
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<OperationReply, WireError> {
    admit(sources, admission, b)?;
    let s = schema(registry)?;
    let value = crate::decode_checked(input, &expected("OperationReply"), registry, b)?;
    reply_from(value.value(), s, registry, sources, admission, b)
}
/// Dependency replies retain the caller-supplied order. Their operation identity,
/// count, budgets and source grants must match the saved Await at dispatch.
pub fn encode_resume(
    value: &Resume,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    admit(sources, admission, b)?;
    let s = schema(registry)?;
    let value = record(
        s,
        "Resume",
        [
            NdfValue::U64(value.request_id),
            value.continuation.value(s, b)?,
            sequence(&value.dependency_results, b, |v, b| {
                reply_value(v, s, registry, sources, admission, b)
            })?,
        ],
        b,
    )?;
    crate::encode_checked(&value, &expected("Resume"), registry, b)
}
pub fn decode_resume(
    input: &[u8],
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Resume, WireError> {
    admit(sources, admission, b)?;
    let s = schema(registry)?;
    let value = crate::decode_checked(input, &expected("Resume"), registry, b)?;
    let f = fields(value.value(), s, "Resume", 3)?;
    Ok(Resume {
        request_id: as_u64(&f[0])?,
        continuation: Codec::from(&f[1], s, sources, b)?,
        dependency_results: collect(list(&f[2])?, b, |v, b| {
            reply_from(v, s, registry, sources, admission, b)
        })?,
    })
}
