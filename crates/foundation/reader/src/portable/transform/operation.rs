//! Explicit domain/operation outcome correspondence. Report duplicates are
//! verified value-for-value and never counted as two emissions.
use super::*;
use alloc::boxed::Box;
use nepl3_core::{diagnostic::Report, origin::Mapping, value::Variant};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DispatchFailure {
    Invalid,
    Stopped(StopReason),
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TransformRejection {
    pub failure: DispatchFailure,
    pub report: Report,
    /// Exact saved dispatch closure, not a source grant supplied by the reply.
    pub sources: Vec<SourceSnapshot>,
    pub source_maps: Vec<Mapping>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TransformCompletion {
    Reply(Box<TransformReply>),
    Rejected(Box<TransformRejection>),
}

/// Encode a dispatch failure without claiming a domain partial result. Every
/// Report reference must resolve in the original saved dispatch closure.
pub fn rejection_to_value<C: FoundationValueCodec>(
    failure: &DispatchFailure,
    report: &Report,
    context: &TransformReplyContext<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    if matches!(failure, DispatchFailure::Invalid) && report.trace_overflow.is_some() {
        return Err(PortableError::Shape);
    }
    let sources = context
        .sources(&[], b, codec.source_admission())
        .map_err(reader)?;
    crate::runtime::validate::report(
        report,
        context.continuation.usage,
        context.registry,
        &sources,
        b,
    )
    .map_err(reader)?;
    let encoded = {
        let mut scoped = codec.scoped(&sources);
        scoped.encode_report(report, b).map_err(boundary)?
    };
    let [diagnostics, events, usage, overflow] =
        copied_fields(&encoded, codec.foundation_schema(), b)?;
    let count = if matches!(failure, DispatchFailure::Stopped(_)) {
        6
    } else {
        5
    };
    b.charge(
        Resource::AllocationUnits,
        (count * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    let (case, fields) = match failure {
        DispatchFailure::Invalid => (
            "Invalid",
            alloc::vec![NdfValue::None, diagnostics, events, usage, overflow],
        ),
        DispatchFailure::Stopped(reason) => {
            let name = super::value::stop_name(*reason);
            b.charge(
                Resource::AllocationUnits,
                (codec.foundation_schema().package.len() + "StopReason".len() + name.len()) as u64,
            )?;
            let reason = NdfValue::Variant(Variant {
                schema: codec.foundation_schema().clone(),
                type_name: "StopReason".into(),
                variant: name.into(),
                fields: Vec::new(),
            });
            (
                "Stopped",
                alloc::vec![reason, NdfValue::None, diagnostics, events, usage, overflow],
            )
        }
    };
    let value = variant(codec.foundation_schema(), case, fields, b)?;
    checked::<C>(&value, context, b)?;
    Ok(value)
}

fn variant<E>(
    schema: &SchemaRef,
    case: &str,
    fields: Vec<NdfValue>,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    b.charge(
        Resource::AllocationUnits,
        (schema.package.len() + "OperationReply".len() + case.len()) as u64,
    )?;
    Ok(NdfValue::Variant(Variant {
        schema: schema.clone(),
        type_name: "OperationReply".into(),
        variant: case.into(),
        fields,
    }))
}
fn checked<C: FoundationValueCodec>(
    value: &NdfValue,
    context: &TransformReplyContext<'_>,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    b.charge(
        Resource::AllocationUnits,
        ("nepl3.foundation".len() + "OperationReply".len()) as u64,
    )?;
    context
        .registry
        .validate(
            &TypeDescriptor::Named(TypeRef {
                package: "nepl3.foundation".into(),
                revision: 1,
                name: "OperationReply".into(),
            }),
            value,
            b,
        )
        .map_err(|e| reader(ReaderError::Schema(e)))?;
    Ok(())
}
fn copied_fields<E>(
    report: &NdfValue,
    foundation: &SchemaRef,
    b: &mut Budget,
) -> Result<[NdfValue; 4], PortableError<E>> {
    let f = fields(report, foundation, "Report", 4)?;
    Ok([
        f[0].clone_with_budget(b)?,
        f[1].clone_with_budget(b)?,
        f[3].clone_with_budget(b)?,
        f[2].clone_with_budget(b)?,
    ])
}
pub fn to_value<C: FoundationValueCodec>(
    reply: &TransformReply,
    context: &TransformReplyContext<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let inner = reply_to_value(reply, context, codec, b)?;
    let f = fields(
        &inner,
        context.schema().map_err(reader)?,
        "TransformReply",
        4,
    )?;
    let [diagnostics, events, usage, overflow] =
        copied_fields(&f[3], codec.foundation_schema(), b)?;
    let count = if matches!(reply.outcome, TransformOutcome::Stopped { .. }) {
        6
    } else {
        5
    };
    b.charge(
        Resource::AllocationUnits,
        (count * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    let (case, fields) = match &reply.outcome {
        TransformOutcome::Complete { .. } => (
            "Complete",
            alloc::vec![inner, diagnostics, events, usage, overflow],
        ),
        TransformOutcome::Failed { .. } => {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            (
                "Invalid",
                alloc::vec![
                    NdfValue::Some(Box::new(inner)),
                    diagnostics,
                    events,
                    usage,
                    overflow
                ],
            )
        }
        TransformOutcome::Stopped { .. } => {
            let NdfValue::Record(ref record) = inner else {
                return Err(PortableError::Shape);
            };
            let NdfValue::Variant(ref outcome) = record.fields[0] else {
                return Err(PortableError::Shape);
            };
            let reason = outcome.fields[0].clone_with_budget(b)?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            (
                "Stopped",
                alloc::vec![
                    reason,
                    NdfValue::Some(Box::new(inner)),
                    diagnostics,
                    events,
                    usage,
                    overflow
                ],
            )
        }
    };
    let value = variant(codec.foundation_schema(), case, fields, b)?;
    checked::<C>(&value, context, b)?;
    Ok(value)
}

pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    context: &TransformReplyContext<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<TransformCompletion, PortableError<C::Error>> {
    checked::<C>(value, context, b)?;
    let NdfValue::Variant(v) = value else {
        return Err(PortableError::Shape);
    };
    if &v.schema != codec.foundation_schema() || v.type_name != "OperationReply" {
        return Err(PortableError::Shape);
    }
    let (inner, offset, reason) = match (v.variant.as_str(), v.fields.as_slice()) {
        ("Complete", [inner, _, _, _, _]) => (Some(inner), 1, None),
        ("Invalid", [partial, _, _, _, _]) => (optional(partial)?, 1, None),
        ("Stopped", [reason, partial, _, _, _, _]) => (optional(partial)?, 2, Some(reason)),
        // Transforms cannot delegate an Await. Do not invent a continuation.
        _ => return Err(PortableError::Shape),
    };
    let f = &v.fields[offset..];
    let report = record(
        codec.foundation_schema(),
        "Report",
        [
            f[0].clone_with_budget(b)?,
            f[1].clone_with_budget(b)?,
            f[3].clone_with_budget(b)?,
            f[2].clone_with_budget(b)?,
        ],
        b,
    )?;
    if let Some(inner) = inner {
        let encoded = fields(
            inner,
            context.schema().map_err(reader)?,
            "TransformReply",
            4,
        )?;
        // Both values are schema checked and the outer copy was fully traversed
        // while copying above; this is an exact report echo, not new emissions.
        if report != encoded[3] {
            return Err(PortableError::Shape);
        }
        let reply = reply_from_value(inner, context, codec, b)?;
        let agrees = match (&reply.outcome, v.variant.as_str()) {
            (TransformOutcome::Complete { .. }, "Complete")
            | (TransformOutcome::Failed { .. }, "Invalid") => true,
            (TransformOutcome::Stopped { reason: actual }, "Stopped") => {
                reason.and_then(stop_from) == Some(*actual)
            }
            _ => false,
        };
        if !agrees {
            return Err(PortableError::Shape);
        }
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<TransformReply>() as u64,
        )?;
        return Ok(TransformCompletion::Reply(Box::new(reply)));
    }
    let failure = if let Some(reason) = reason {
        DispatchFailure::Stopped(stop_from(reason).ok_or(PortableError::Shape)?)
    } else {
        DispatchFailure::Invalid
    };
    let sources = context
        .sources(&[], b, codec.source_admission())
        .map_err(reader)?;
    let report = {
        let mut scoped = codec.scoped(&sources);
        scoped.decode_report(&report, b).map_err(boundary)?
    };
    if report.trace_overflow.is_some() && matches!(failure, DispatchFailure::Invalid) {
        return Err(PortableError::Shape);
    }
    crate::runtime::validate::report(
        &report,
        context.continuation.usage,
        context.registry,
        &sources,
        b,
    )
    .map_err(reader)?;
    let mut owned = Vec::new();
    for source in sources.snapshots() {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<SourceSnapshot>() as u64,
        )?;
        owned.push(crate::runtime::copy::copy(source, b)?);
    }
    let source_maps = crate::runtime::copy::copy(&context.continuation.current.source_maps, b)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<TransformRejection>() as u64,
    )?;
    Ok(TransformCompletion::Rejected(Box::new(
        TransformRejection {
            failure,
            report,
            sources: owned,
            source_maps,
        },
    )))
}
fn optional<E>(value: &NdfValue) -> Result<Option<&NdfValue>, PortableError<E>> {
    match value {
        NdfValue::None => Ok(None),
        NdfValue::Some(v) => Ok(Some(v)),
        _ => Err(PortableError::Shape),
    }
}
fn stop_from(value: &NdfValue) -> Option<StopReason> {
    let NdfValue::Variant(v) = value else {
        return None;
    };
    if v.type_name != "StopReason" || !v.fields.is_empty() {
        return None;
    }
    match v.variant.as_str() {
        "Cancelled" => Some(StopReason::Cancelled),
        "SourceLimit" => Some(StopReason::SourceLimit),
        "WorkLimit" => Some(StopReason::WorkLimit),
        "DepthLimit" => Some(StopReason::DepthLimit),
        "NodeLimit" => Some(StopReason::NodeLimit),
        "AllocationLimit" => Some(StopReason::AllocationLimit),
        "OutputLimit" => Some(StopReason::OutputLimit),
        "DiagnosticLimit" => Some(StopReason::DiagnosticLimit),
        "EventLimit" => Some(StopReason::EventLimit),
        _ => None,
    }
}
