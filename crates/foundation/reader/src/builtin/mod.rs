//! Standard lexical readers. Source reservations are supplied by the host, never generated here.
mod language;
pub(crate) mod lexical;
pub mod provider;
mod text;
use crate::{
    model::*,
    plan::CharClass,
    runtime::{self, ReaderError},
    schema,
};
use alloc::{string::ToString, vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    diagnostic::{Diagnostic, Report, Severity},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor},
    source::{SourceAdmission, SourceError, SourceReservation, SourceStore},
    value::{Integer, NdfValue, Rational, decimal::DecimalError},
    view::ViewBundle,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinReader {
    Name,
    Nat,
    Number,
    Text,
    Lang,
    Trivia,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinRequest {
    pub kind: BuiltinReader,
    pub request: OwnedReadRequest,
    pub reservation: Option<SourceReservation>,
}

/// Reads one lexical value. A nonfinal input boundary does not establish maximal-token completion.
/// Text alone requires a host reservation; an unsuccessful read never commits a generated source.
#[allow(clippy::too_many_arguments)]
pub fn read(
    kind: BuiltinReader,
    request: ReadRequest<'_>,
    reservation: Option<&SourceReservation>,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<ReadReply, ReaderError> {
    let result = budget.with_depth(|budget| {
        runtime::validate::request(
            &request,
            sources,
            registry,
            &TypeDescriptor::NdfValue,
            budget,
            admission,
        )?;
        if (kind == BuiltinReader::Text) != reservation.is_some() {
            return Err(ReaderError::Context);
        }
        if let Some(reservation) = reservation {
            budget.charge(Resource::Work, reservation.source_id.0.len() as u64)?;
            reservation.validate(budget)?;
            if reservation.source_id == request.snapshot.identity().source
                && reservation.revision == request.snapshot.identity().revision
            {
                return Err(SourceError::IdentityConflict.into());
            }
        }
        if kind == BuiltinReader::Text {
            return text::read(
                request,
                reservation.ok_or(ReaderError::Context)?,
                registry,
                sources,
                budget,
                admission,
            );
        }
        let input = request.snapshot.slice_range(request.start, request.limit)?;
        let scanned = lexical::scan(kind, input, request.final_input, request.start, budget)?;
        match scanned {
            lexical::Scan::Matched(end) => {
                let raw = &input[..end];
                let value = match kind {
                    BuiltinReader::Name | BuiltinReader::Lang => {
                        budget.charge(Resource::AllocationUnits, raw.len() as u64)?;
                        NdfValue::Text(raw.to_string())
                    }
                    BuiltinReader::Nat => NdfValue::Integer(
                        Integer::from_decimal_digits(raw, budget).map_err(decimal_error)?,
                    ),
                    BuiltinReader::Number => {
                        let negative = raw.starts_with('-');
                        let raw = raw.strip_prefix('-').unwrap_or(raw);
                        let (integer, fraction) = raw.split_once('.').unwrap_or((raw, ""));
                        NdfValue::Rational(
                            Rational::from_decimal_parts(negative, integer, fraction, budget)
                                .map_err(decimal_error)?,
                        )
                    }
                    BuiltinReader::Trivia => NdfValue::Unit,
                    BuiltinReader::Text => return Err(ReaderError::Context),
                };
                matched(value, request.start + end as u64, &request, budget)
            }
            other => rejection(other, kind, &request, registry, budget),
        }
    });
    runtime::stopped(result, budget)
}
fn decimal_error(error: DecimalError) -> ReaderError {
    match error {
        DecimalError::Stopped(reason) => ReaderError::Stopped(reason),
        DecimalError::InvalidDigits => ReaderError::Context,
    }
}
fn matched(
    value: NdfValue,
    end: u64,
    request: &ReadRequest<'_>,
    budget: &mut Budget,
) -> Result<ReadReply, ReaderError> {
    let new_state = request.state.clone_with_budget(budget)?;
    Ok(ReadReply::Matched {
        value,
        end,
        new_state,
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![],
        sources: vec![],
        source_maps: vec![],
        report: report(budget),
    })
}
fn report(budget: &Budget) -> Report {
    Report {
        usage: budget.usage(),
        ..Report::default()
    }
}
fn expected(kind: BuiltinReader, budget: &mut Budget) -> Result<Vec<Expectation>, ReaderError> {
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Expectation>() as u64 + 8,
    )?;
    Ok(vec![match kind {
        BuiltinReader::Name => Expectation::ScalarClass(CharClass::IdentifierStart),
        BuiltinReader::Nat | BuiltinReader::Number => Expectation::ScalarClass(CharClass::Digit),
        BuiltinReader::Lang => Expectation::ScalarClass(CharClass::AsciiLetter),
        BuiltinReader::Text => Expectation::Literal("\"".into()),
        BuiltinReader::Trivia => Expectation::ScalarClass(CharClass::Whitespace),
    }])
}
pub(crate) fn rejection(
    scan: lexical::Scan,
    kind: BuiltinReader,
    request: &ReadRequest<'_>,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<ReadReply, ReaderError> {
    let expected = expected(kind, budget)?;
    match scan {
        lexical::Scan::NeedMore => Ok(ReadReply::NeedMore {
            sources: vec![],
            source_maps: vec![],
            expected,
            report: report(budget),
        }),
        lexical::Scan::NoMatch => Ok(ReadReply::NoMatch {
            sources: vec![],
            source_maps: vec![],
            expected,
            furthest: request.start,
            report: report(budget),
        }),
        lexical::Scan::Failed(code, offset) => {
            let schema = registry
                .selected(schema::PACKAGE, schema::REVISION)
                .ok_or(SchemaError::UnknownSchema)?;
            let foundation = registry
                .selected("nepl3.foundation", 1)
                .ok_or(SchemaError::UnknownSchema)?;
            let arguments = schema::arguments(
                schema,
                foundation,
                &expected,
                request.start + offset as u64,
                budget,
            )?;
            budget.charge(Resource::Diagnostics, 1)?;
            budget.charge(
                Resource::AllocationUnits,
                (core::mem::size_of::<Diagnostic>() + schema.package.len() + code.len() + 6) as u64,
            )?;
            let diagnostic = Diagnostic {
                schema: schema.clone(),
                code: code.into(),
                severity: Severity::Error,
                stage: "reader".into(),
                arguments,
                primary: Some(request.snapshot.span_with_budget(
                    request.start + offset as u64,
                    request.start + offset as u64,
                    budget,
                )?),
                related: vec![],
                fixes: vec![],
            };
            // The separate Failed primary is the same diagnostic, not a second emitted diagnostic.
            let duplicate_arguments = diagnostic.arguments.clone_with_budget(budget)?;
            budget.charge(
                Resource::AllocationUnits,
                (core::mem::size_of::<Diagnostic>() + schema.package.len() + code.len() + 6) as u64,
            )?;
            let duplicate = Diagnostic {
                schema: schema.clone(),
                code: code.into(),
                severity: Severity::Error,
                stage: "reader".into(),
                arguments: duplicate_arguments,
                primary: Some(request.snapshot.span_with_budget(
                    request.start + offset as u64,
                    request.start + offset as u64,
                    budget,
                )?),
                related: vec![],
                fixes: vec![],
            };
            Ok(ReadReply::Failed {
                sources: vec![],
                source_maps: vec![],
                diagnostic,
                recovery: None,
                report: Report {
                    diagnostics: vec![duplicate],
                    ..report(budget)
                },
            })
        }
        lexical::Scan::Matched(_) => Err(ReaderError::Context),
    }
}
