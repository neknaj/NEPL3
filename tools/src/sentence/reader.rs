//! An explicitly registered Reader provider around Sentence's source-only recognizer.
use nepl3_core::{
    budget::{Budget, Resource},
    diagnostic::{Diagnostic, Related, Report, Severity},
    schema::*,
    source::{SourceAdmission, SourceStore},
    value::{NdfValue, OperationRef, Record, TypedValue},
};
use nepl3_reader::{
    model::{ReadReply, ReadRequest},
    plan::{ProviderKind, ProviderSignature},
    runtime::ReaderError,
};
use nepl3_sentence_core::literal::{
    self as sentence, SentenceCode, SentenceError, SentenceOutcome,
};
use nepl3_wire::foundation::FoundationCodec;
mod descriptor;
pub fn descriptor(b: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    descriptor::descriptor(b)
}
pub fn signature(r: &SchemaRegistry, b: &mut Budget) -> Result<ProviderSignature, ReaderError> {
    b.poll()?;
    if !r.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    let s = r
        .selected("nepl3.sentence.reader", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    b.charge(Resource::Work, s.package.len() as u64 + 64)?;
    if s != &descriptor(b)?.reference(b)? {
        return Err(ReaderError::ProviderContract);
    }
    let descriptor = r.descriptor(s).ok_or(SchemaError::UnknownSchema)?;
    b.charge(Resource::Work, descriptor.operations.len() as u64 * 9)?;
    let operation = descriptor
        .operations
        .iter()
        .find(|op| op.name == "literal")
        .ok_or(ReaderError::ProviderContract)?;
    let envelope = |ty: &TypeDescriptor, name: &str| matches!(ty,TypeDescriptor::Named(t) if t.package=="nepl3.reader" && t.revision==1 && t.name==name);
    if !operation.pure
        || !envelope(&operation.input, "ReadRequest")
        || !envelope(&operation.output, "ReadReply")
    {
        return Err(ReaderError::ProviderContract);
    }
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<ProviderSignature>() as u64 + s.package.len() as u64 + 128,
    )?;
    Ok(ProviderSignature {
        operation: OperationRef {
            schema: s.clone(),
            name: "literal".into(),
        },
        kind: ProviderKind::Read,
        value_input: TypeDescriptor::Unit,
        value_output: TypeDescriptor::Named(TypeRef {
            package: "nepl3.sentence".into(),
            revision: 1,
            name: "SentenceLiteralPayload".into(),
        }),
        pure: true,
        state_type: TypeDescriptor::Unit,
        continuation_type: TypeDescriptor::Named(TypeRef {
            package: "nepl3.reader".into(),
            revision: 1,
            name: "ReaderContinuation".into(),
        }),
    })
}
fn report(b: &Budget) -> Report {
    Report {
        usage: b.usage(),
        ..Report::default()
    }
}
fn boundary(e: sentence::SentenceError) -> ReaderError {
    match e {
        SentenceError::Stopped(s) => ReaderError::Stopped(s),
        SentenceError::Source(e) => e.into(),
        SentenceError::Schema(e) => e.into(),
        SentenceError::Shape(_) | SentenceError::Syntax(_) | SentenceError::SchemaIdentity => {
            ReaderError::ProviderContract
        }
    }
}
/// The caller retains Reader's checked context, provider depth and admission.
/// The public ReaderSession boundary still validates this provider's reply.
pub fn read(
    operation: &OperationRef,
    request: ReadRequest<'_>,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<ReadReply, ReaderError> {
    let run = (|| {
        let expected = signature(registry, b)?.operation;
        b.charge(
            Resource::Work,
            (operation.schema.package.len() + operation.name.len()) as u64 + 33,
        )?;
        if operation != &expected || request.state != &NdfValue::Unit {
            return Err(ReaderError::ProviderContract);
        }
        let scan = sentence::read(
            request.snapshot,
            request.start,
            request.limit,
            request.final_input,
            registry,
            b,
            a,
        )
        .map_err(boundary)?;
        match scan.outcome {
            SentenceOutcome::NoMatch => Ok(ReadReply::NoMatch {
                expected: vec![],
                furthest: request.start,
                sources: vec![],
                source_maps: vec![],
                report: report(b),
            }),
            SentenceOutcome::NeedMore => Ok(ReadReply::NeedMore {
                expected: vec![],
                sources: vec![],
                source_maps: vec![],
                report: report(b),
            }),
            SentenceOutcome::Matched(literal) => {
                let end = literal.head.end();
                let [view] = literal.syntax.views.as_slice() else {
                    return Err(ReaderError::ProviderContract);
                };
                let view = view.view.clone_with_budget(b)?;
                let mut codec =
                    FoundationCodec::new(registry, sources, a).map_err(|_| ReaderError::Context)?;
                let value = nepl3_sentence_core::portable::literal::to_value(
                    &literal.syntax,
                    registry,
                    &mut codec,
                    b,
                )
                .map_err(|e| match e {
                    nepl3_sentence_core::portable::Error::Stopped(s) => ReaderError::Stopped(s),
                    _ => ReaderError::ProviderContract,
                })?;
                Ok(ReadReply::Matched {
                    value,
                    end,
                    new_state: NdfValue::Unit,
                    view,
                    facts: vec![],
                    sources: vec![],
                    source_maps: vec![],
                    report: report(b),
                })
            }
            SentenceOutcome::Failed(failure) => {
                let code = match failure.code {
                    SentenceCode::UnterminatedLiteral => "UnterminatedLiteral",
                    SentenceCode::UnclosedAnnotation => "UnclosedAnnotation",
                    SentenceCode::UnexpectedDelimiter => "UnexpectedDelimiter",
                    SentenceCode::SeparatorCount => "SeparatorCount",
                    SentenceCode::EmptyAnnotationPart => "EmptyAnnotationPart",
                    SentenceCode::DirectLineBreak => "DirectLineBreak",
                    SentenceCode::InvalidEscape => "InvalidEscape",
                    SentenceCode::InvalidScalar => "InvalidScalar",
                };
                let schema = &expected.schema;
                let has_related = failure.opening.is_some();
                let bytes = core::mem::size_of::<Diagnostic>() as u64
                    + if has_related {
                        core::mem::size_of::<Related>() as u64
                    } else {
                        0
                    }
                    + schema.package.len() as u64 * (if has_related { 3 } else { 2 })
                    + code.len() as u64
                    + "reader".len() as u64
                    + "SentenceDiagnosticArguments".len() as u64
                        * (if has_related { 2 } else { 1 })
                    + if has_related {
                        "AnnotationOpening".len() as u64
                    } else {
                        0
                    }
                    + failure.primary.snapshot_ref().source.0.len() as u64
                    + failure
                        .opening
                        .as_ref()
                        .map_or(0, |s| s.snapshot_ref().source.0.len() as u64);
                b.charge(Resource::Work, bytes)?;
                b.charge(Resource::AllocationUnits, bytes.saturating_mul(2))?;
                let args = || {
                    TypedValue::Record(Record {
                        schema: schema.clone(),
                        kind: "SentenceDiagnosticArguments".into(),
                        fields: vec![],
                    })
                };
                let related = match failure.opening {
                    Some(opening) => vec![Related {
                        span: Some(opening),
                        code: "AnnotationOpening".into(),
                        arguments: args(),
                    }],
                    None => vec![],
                };
                let diagnostic = Diagnostic {
                    schema: schema.clone(),
                    code: code.into(),
                    severity: Severity::Error,
                    stage: "reader".into(),
                    arguments: args(),
                    primary: Some(failure.primary),
                    related,
                    fixes: vec![],
                };
                diagnostic
                    .validate(sources, &[], registry, b)
                    .map_err(|e| match e {
                        nepl3_core::diagnostic::validation::ReportValidationError::Stopped(s) => {
                            ReaderError::Stopped(s)
                        }
                        _ => ReaderError::ProviderContract,
                    })?;
                b.charge(Resource::Diagnostics, 1)?;
                let duplicate = diagnostic.clone();
                Ok(ReadReply::Failed {
                    diagnostic,
                    recovery: None,
                    sources: vec![],
                    source_maps: vec![],
                    report: Report {
                        diagnostics: vec![duplicate],
                        ..report(b)
                    },
                })
            }
        }
    })();
    match run {
        Err(ReaderError::Stopped(reason)) => Ok(ReadReply::Stopped {
            reason,
            sources: vec![],
            source_maps: vec![],
            report: report(b),
        }),
        other => other,
    }
}
