//! Typed correspondence between reader outcomes and common operation outcomes.
use super::*;
use nepl3_core::{diagnostic::OperationResult, operation::OperationReply, value::TypedValue};

/// Prepare a terminal operation reply. Hosts retain the validated generated
/// source closure when encoding its report across a transport boundary.
pub fn to_reply<C: FoundationValueCodec>(
    reply: &ReadReply,
    context: &ReadReplyContext<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<OperationReply, PortableError<C::Error>> {
    let encoded = reply_to_value(reply, context, codec, b)?;
    encoded.charge_clone(b)?;
    let NdfValue::Variant(v) = &encoded else {
        return Err(PortableError::Shape);
    };
    let value = TypedValue::Variant(v.clone());
    let (_, _, report) = value::metadata(reply)?;
    let report = crate::runtime::copy::copy(report, b)?;
    Ok(OperationReply::Result(match reply {
        ReadReply::Matched { .. } | ReadReply::NoMatch { .. } | ReadReply::NeedMore { .. } => {
            OperationResult::Complete { value, report }
        }
        ReadReply::Failed { .. } => OperationResult::Invalid {
            partial: Some(value),
            report,
        },
        ReadReply::Stopped { reason, .. } => OperationResult::Stopped {
            reason: *reason,
            partial: Some(value),
            report,
        },
        ReadReply::Await { .. } => return Err(PortableError::Shape),
    }))
}

/// Receive a terminal operation result without consuming the pending call.
/// Dispatch failures with no partial use only the saved source authority.
pub fn from_reply<C: FoundationValueCodec>(
    reply: &OperationReply,
    context: &ReadReplyContext<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<OperationResult<ReadReply>, PortableError<C::Error>> {
    b.poll()?;
    let (inner, report, tag, reason) = match reply {
        OperationReply::Result(OperationResult::Complete { value, report }) => {
            (Some(value), report, 0, None)
        }
        OperationReply::Result(OperationResult::Invalid { partial, report }) => {
            (partial.as_ref(), report, 1, None)
        }
        OperationReply::Result(OperationResult::Stopped {
            reason,
            partial,
            report,
        }) => (partial.as_ref(), report, 2, Some(*reason)),
        OperationReply::Await { .. } => return Err(PortableError::Shape),
    };
    let decoded = if let Some(value) = inner {
        let value = match value.clone_with_budget(b)? {
            TypedValue::Record(v) => NdfValue::Record(v),
            TypedValue::Variant(v) => NdfValue::Variant(v),
        };
        let decoded = reply_from_value(&value, context, codec, b)?;
        let (_, _, inner_report) = value::metadata(&decoded)?;
        // Charge traversal of both reports before comparing their variable data.
        use crate::runtime::copy::CopyCost;
        report.charge(b)?;
        inner_report.charge(b)?;
        if report != inner_report {
            return Err(PortableError::Shape);
        }
        let agrees = match (&decoded, tag) {
            (
                ReadReply::Matched { .. } | ReadReply::NoMatch { .. } | ReadReply::NeedMore { .. },
                0,
            )
            | (ReadReply::Failed { .. }, 1) => true,
            (ReadReply::Stopped { reason: actual, .. }, 2) => Some(*actual) == reason,
            _ => false,
        };
        if !agrees {
            return Err(PortableError::Shape);
        }
        Some(decoded)
    } else {
        if tag != 2 && report.trace_overflow.is_some() {
            return Err(PortableError::Shape);
        }
        let sources = super::super::transform::dispatch_sources(
            context.continuation,
            &[],
            b,
            codec.source_admission(),
        )
        .map_err(reader)?;
        crate::runtime::validate::report(
            report,
            context.continuation.usage,
            context.registry,
            &sources,
            b,
        )
        .map_err(reader)?;
        None
    };
    let report = crate::runtime::copy::copy(report, b)?;
    Ok(match (tag, decoded, reason) {
        (0, Some(value), _) => OperationResult::Complete { value, report },
        (1, partial, _) => OperationResult::Invalid { partial, report },
        (2, partial, Some(reason)) => OperationResult::Stopped {
            reason,
            partial,
            report,
        },
        _ => return Err(PortableError::Shape),
    })
}
