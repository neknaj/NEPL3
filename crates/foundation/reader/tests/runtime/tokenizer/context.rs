use super::*;
use nepl3_core::{diagnostic::OperationResult, operation::OperationReply};
use nepl3_reader::{
    portable::{read, transform},
    tokenizer::TokenizationSession,
};
use nepl3_wire::foundation::FoundationCodec;
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}

/// Exercise the existing terminal codecs using only the tokenizer's borrowed
/// private dispatch proof. No pending state is consumed until the caller resumes.
pub(super) fn roundtrip(
    session: &TokenizationSession<'_>,
    reply: ProviderReply,
    c: &mut FoundationCodec<'_>,
    b: &mut Budget,
) -> Result<ProviderReply, String> {
    match reply {
        ProviderReply::Read(reply) => {
            assert!(matches!(
                session.pending_transform(),
                Err(ReaderError::ProviderContract)
            ));
            let proof = session.pending_read().map_err(err)?;
            let value = read::reply_to_value(&reply, &proof, c, b).map_err(err)?;
            assert!(read::reply_from_value(&NdfValue::Unit, &proof, c, b).is_err());
            let value =
                nepl3_wire::decode(&nepl3_wire::encode(&value, b).map_err(err)?, b).map_err(err)?;
            let decoded = read::reply_from_value(&value, &proof, c, b).map_err(err)?;
            assert_eq!(*reply, decoded);
            let operation = read::operation::to_reply(&decoded, &proof, c, b).map_err(err)?;
            let mut wrong = operation.clone();
            let OperationReply::Result(result) = &mut wrong else {
                return Err("unexpected Await".into());
            };
            let report = match result {
                OperationResult::Complete { report, .. }
                | OperationResult::Invalid { report, .. }
                | OperationResult::Stopped { report, .. } => report,
            };
            report.usage.work += 1;
            assert!(read::operation::from_reply(&wrong, &proof, c, b).is_err());
            let restored = read::operation::from_reply(&operation, &proof, c, b).map_err(err)?;
            let decoded = match restored {
                OperationResult::Complete { value, .. } => value,
                OperationResult::Invalid {
                    partial: Some(value),
                    ..
                }
                | OperationResult::Stopped {
                    partial: Some(value),
                    ..
                } => value,
                _ => return Err("domain partial missing".into()),
            };
            assert_eq!(*reply, decoded);
            assert!(session.pending_read().is_ok());
            Ok(ProviderReply::Read(Box::new(decoded)))
        }
        ProviderReply::Transform(reply) => {
            assert!(matches!(
                session.pending_read(),
                Err(ReaderError::ProviderContract)
            ));
            let proof = session.pending_transform().map_err(err)?;
            let value = transform::operation::to_value(&reply, &proof, c, b).map_err(err)?;
            assert!(transform::operation::from_value(&NdfValue::Unit, &proof, c, b).is_err());
            let value =
                nepl3_wire::decode(&nepl3_wire::encode(&value, b).map_err(err)?, b).map_err(err)?;
            let mut wrong = value.clone();
            let NdfValue::Variant(v) = &mut wrong else {
                return Err("operation".into());
            };
            let NdfValue::Record(usage) = &mut v.fields[3] else {
                return Err("usage".into());
            };
            let NdfValue::U64(work) = &mut usage.fields[1] else {
                return Err("work".into());
            };
            *work += 1;
            assert!(transform::operation::from_value(&wrong, &proof, c, b).is_err());
            let transform::operation::TransformCompletion::Reply(decoded) =
                transform::operation::from_value(&value, &proof, c, b).map_err(err)?
            else {
                return Err("unexpected dispatch rejection".into());
            };
            assert_eq!(reply, decoded);
            assert!(session.pending_transform().is_ok());
            Ok(ProviderReply::Transform(decoded))
        }
    }
}

pub(super) fn rejection_retains(
    session: &TokenizationSession<'_>,
    kind: ProviderKind,
    c: &mut FoundationCodec<'_>,
    b: &mut Budget,
) -> Result<(), String> {
    let report = Report {
        usage: b.usage(),
        ..Report::default()
    };
    if kind == ProviderKind::Transform {
        use transform::operation::{DispatchFailure, TransformCompletion};
        let proof = session.pending_transform().map_err(err)?;
        for failure in [
            DispatchFailure::Invalid,
            DispatchFailure::Stopped(StopReason::Cancelled),
        ] {
            let v = transform::operation::rejection_to_value(&failure, &report, &proof, c, b)
                .map_err(err)?;
            let TransformCompletion::Rejected(rejection) =
                transform::operation::from_value(&v, &proof, c, b).map_err(err)?
            else {
                return Err("fabricated partial".into());
            };
            assert_eq!(rejection.failure, failure);
            assert_eq!(rejection.report, report);
            assert!(session.pending_transform().is_ok());
        }
    } else {
        let proof = session.pending_read().map_err(err)?;
        for stopped in [false, true] {
            let op = OperationReply::Result(if stopped {
                OperationResult::Stopped {
                    reason: StopReason::Cancelled,
                    partial: None,
                    report: report.clone(),
                }
            } else {
                OperationResult::Invalid {
                    partial: None,
                    report: report.clone(),
                }
            });
            let result = read::operation::from_reply(&op, &proof, c, b).map_err(err)?;
            if stopped {
                assert!(matches!(
                    result,
                    OperationResult::Stopped {
                        reason: StopReason::Cancelled,
                        partial: None,
                        ..
                    }
                ));
            } else {
                assert!(matches!(
                    result,
                    OperationResult::Invalid { partial: None, .. }
                ));
            }
            assert!(session.pending_read().is_ok());
        }
    }
    Ok(())
}
