use super::*;
use alloc::boxed::Box;
use nepl3_core::{
    value::Variant,
    view::{FallbackRole, PresentationClass},
};
fn variant<E, const N: usize>(
    schema: &SchemaRef,
    name: &str,
    case: &str,
    values: [NdfValue; N],
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    b.charge(
        Resource::AllocationUnits,
        (schema.package.len() + name.len() + case.len() + N * core::mem::size_of::<NdfValue>())
            as u64,
    )?;
    Ok(NdfValue::Variant(Variant {
        schema: schema.clone(),
        type_name: name.into(),
        variant: case.into(),
        fields: Vec::from(values),
    }))
}
fn parts<'a, E>(
    v: &'a NdfValue,
    s: &SchemaRef,
    name: &str,
) -> Result<(&'a str, &'a [NdfValue]), PortableError<E>> {
    match v {
        NdfValue::Variant(v) if &v.schema == s && v.type_name == name => {
            Ok((&v.variant, &v.fields))
        }
        _ => Err(PortableError::Shape),
    }
}
fn sequence<E>(v: &NdfValue) -> Result<&[NdfValue], PortableError<E>> {
    match v {
        NdfValue::List(v) => Ok(v),
        _ => Err(PortableError::Shape),
    }
}
fn primary_value<C: FoundationValueCodec>(
    diagnostic: &nepl3_core::diagnostic::Diagnostic,
    report: &nepl3_core::diagnostic::Report,
    encoded: &NdfValue,
    c: &C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let f = fields(encoded, c.foundation_schema(), "Report", 4)?;
    let encoded_diagnostics = sequence(&f[0])?;
    for (index, item) in report.diagnostics.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if item == diagnostic {
            return encoded_diagnostics
                .get(index)
                .ok_or(PortableError::Shape)?
                .clone_with_budget(b)
                .map_err(Into::into);
        }
    }
    Err(PortableError::Shape)
}
pub(super) fn encode<C: FoundationValueCodec>(
    reply: &TransformReply,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let report = c.encode_report(&reply.report, b).map_err(boundary)?;
    let outcome = match &reply.outcome {
        TransformOutcome::Complete { value, view, facts } => variant(
            s,
            "TransformOutcome",
            "Complete",
            [
                value.clone_with_budget(b)?,
                c.encode_views(view, b).map_err(boundary)?,
                facts_value(facts, s, c, b)?,
            ],
            b,
        )?,
        TransformOutcome::Failed {
            diagnostic,
            recovery,
        } => {
            let diagnostic = primary_value(diagnostic, &reply.report, &report, c, b)?;
            let recovery = match recovery {
                None => NdfValue::None,
                Some(span) => {
                    b.charge(
                        Resource::AllocationUnits,
                        core::mem::size_of::<NdfValue>() as u64,
                    )?;
                    NdfValue::Some(Box::new(c.encode_span(span, b).map_err(boundary)?))
                }
            };
            variant(s, "TransformOutcome", "Failed", [diagnostic, recovery], b)?
        }
        TransformOutcome::Stopped { reason } => {
            let reason = variant(
                c.foundation_schema(),
                "StopReason",
                stop_name(*reason),
                [],
                b,
            )?;
            variant(s, "TransformOutcome", "Stopped", [reason], b)?
        }
    };
    record(
        s,
        "TransformReply",
        [
            outcome,
            c.encode_sources(&reply.sources, b).map_err(boundary)?,
            c.encode_mappings(&reply.source_maps, b).map_err(boundary)?,
            report,
        ],
        b,
    )
}
pub(super) fn decode<C: FoundationValueCodec>(
    f: &[NdfValue],
    sources: Vec<SourceSnapshot>,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<TransformReply, PortableError<C::Error>> {
    let report = c.decode_report(&f[3], b).map_err(boundary)?;
    let (case, v) = parts(&f[0], s, "TransformOutcome")?;
    let outcome = match (case, v) {
        ("Complete", [value, view, facts]) => TransformOutcome::Complete {
            value: value.clone_with_budget(b)?,
            view: c.decode_views(view, b).map_err(boundary)?,
            facts: facts_from(facts, s, c, b)?,
        },
        ("Failed", [primary, recovery]) => {
            let encoded = fields(&f[3], c.foundation_schema(), "Report", 4)?;
            let values = sequence(&encoded[0])?;
            let mut diagnostic = None;
            for (index, value) in values.iter().enumerate() {
                // Precharge the full comparison's variable data before Eq.
                primary.charge_clone(b)?;
                value.charge_clone(b)?;
                if value == primary {
                    if diagnostic.is_some() {
                        return Err(PortableError::Shape);
                    }
                    let value = report.diagnostics.get(index).ok_or(PortableError::Shape)?;
                    b.charge(
                        Resource::AllocationUnits,
                        core::mem::size_of::<nepl3_core::diagnostic::Diagnostic>() as u64,
                    )?;
                    diagnostic = Some(Box::new(crate::runtime::copy::copy(value, b)?));
                }
            }
            let recovery = match recovery {
                NdfValue::None => None,
                NdfValue::Some(v) => Some(c.decode_span(v, b).map_err(boundary)?),
                _ => return Err(PortableError::Shape),
            };
            TransformOutcome::Failed {
                diagnostic: diagnostic.ok_or(PortableError::Shape)?,
                recovery,
            }
        }
        ("Stopped", [reason]) => {
            let (name, f) = parts(reason, c.foundation_schema(), "StopReason")?;
            if !f.is_empty() {
                return Err(PortableError::Shape);
            }
            TransformOutcome::Stopped {
                reason: stop_from(name).ok_or(PortableError::Shape)?,
            }
        }
        _ => return Err(PortableError::Shape),
    };
    Ok(TransformReply {
        outcome,
        sources,
        source_maps: c.decode_mappings(&f[2], b).map_err(boundary)?,
        report,
    })
}
pub(super) fn stop_name(v: StopReason) -> &'static str {
    match v {
        StopReason::Cancelled => "Cancelled",
        StopReason::SourceLimit => "SourceLimit",
        StopReason::WorkLimit => "WorkLimit",
        StopReason::DepthLimit => "DepthLimit",
        StopReason::NodeLimit => "NodeLimit",
        StopReason::AllocationLimit => "AllocationLimit",
        StopReason::OutputLimit => "OutputLimit",
        StopReason::DiagnosticLimit => "DiagnosticLimit",
        StopReason::EventLimit => "EventLimit",
    }
}
fn stop_from(v: &str) -> Option<StopReason> {
    Some(match v {
        "Cancelled" => StopReason::Cancelled,
        "SourceLimit" => StopReason::SourceLimit,
        "WorkLimit" => StopReason::WorkLimit,
        "DepthLimit" => StopReason::DepthLimit,
        "NodeLimit" => StopReason::NodeLimit,
        "AllocationLimit" => StopReason::AllocationLimit,
        "OutputLimit" => StopReason::OutputLimit,
        "DiagnosticLimit" => StopReason::DiagnosticLimit,
        "EventLimit" => StopReason::EventLimit,
        _ => return None,
    })
}
fn fallback_name(v: FallbackRole) -> &'static str {
    match v {
        FallbackRole::Content => "Content",
        FallbackRole::Marker => "Marker",
        FallbackRole::Delimiter => "Delimiter",
        FallbackRole::Name => "Name",
        FallbackRole::Quantity => "Quantity",
        FallbackRole::Annotation => "Annotation",
    }
}
fn fallback_from(v: &str) -> Option<FallbackRole> {
    Some(match v {
        "Content" => FallbackRole::Content,
        "Marker" => FallbackRole::Marker,
        "Delimiter" => FallbackRole::Delimiter,
        "Name" => FallbackRole::Name,
        "Quantity" => FallbackRole::Quantity,
        "Annotation" => FallbackRole::Annotation,
        _ => return None,
    })
}
fn facts_value<C: FoundationValueCodec>(
    facts: &[ReaderFact],
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut values = Vec::new();
    for fact in facts {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<NdfValue>() as u64,
        )?;
        let value = match fact {
            ReaderFact::Capture { name, span } => variant(
                s,
                "ReaderFact",
                "Capture",
                [text(name, b)?, c.encode_span(span, b).map_err(boundary)?],
                b,
            )?,
            ReaderFact::Presentation { class, span } => {
                let foundation = c.foundation_schema();
                let class = record(
                    foundation,
                    "PresentationClass",
                    [
                        schema_value(&class.schema, foundation, b)?,
                        text(&class.name, b)?,
                        variant(
                            foundation,
                            "FallbackRole",
                            fallback_name(class.fallback),
                            [],
                            b,
                        )?,
                    ],
                    b,
                )?;
                variant(
                    s,
                    "ReaderFact",
                    "Presentation",
                    [class, c.encode_span(span, b).map_err(boundary)?],
                    b,
                )?
            }
            ReaderFact::Relation {
                schema,
                kind,
                from,
                to,
            } => variant(
                s,
                "ReaderFact",
                "Relation",
                [
                    schema_value(schema, c.foundation_schema(), b)?,
                    text(kind, b)?,
                    c.encode_span(from, b).map_err(boundary)?,
                    c.encode_span(to, b).map_err(boundary)?,
                ],
                b,
            )?,
        };
        values.push(value);
    }
    Ok(NdfValue::List(values))
}
fn facts_from<C: FoundationValueCodec>(
    value: &NdfValue,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<Vec<ReaderFact>, PortableError<C::Error>> {
    let mut facts = Vec::new();
    for value in sequence(value)? {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<ReaderFact>() as u64,
        )?;
        let (case, f) = parts(value, s, "ReaderFact")?;
        let value = match (case, f) {
            ("Capture", [name, span]) => ReaderFact::Capture {
                name: string(name, b)?,
                span: c.decode_span(span, b).map_err(boundary)?,
            },
            ("Presentation", [class, span]) => {
                let f = fields(class, c.foundation_schema(), "PresentationClass", 3)?;
                let (fallback, v) = parts(&f[2], c.foundation_schema(), "FallbackRole")?;
                if !v.is_empty() {
                    return Err(PortableError::Shape);
                }
                let class = PresentationClass {
                    schema: schema_from(&f[0], c.foundation_schema(), b)?,
                    name: string(&f[1], b)?,
                    fallback: fallback_from(fallback).ok_or(PortableError::Shape)?,
                };
                ReaderFact::Presentation {
                    class,
                    span: c.decode_span(span, b).map_err(boundary)?,
                }
            }
            ("Relation", [schema, kind, from, to]) => ReaderFact::Relation {
                schema: schema_from(schema, c.foundation_schema(), b)?,
                kind: string(kind, b)?,
                from: c.decode_span(from, b).map_err(boundary)?,
                to: c.decode_span(to, b).map_err(boundary)?,
            },
            _ => return Err(PortableError::Shape),
        };
        facts.push(value);
    }
    Ok(facts)
}
