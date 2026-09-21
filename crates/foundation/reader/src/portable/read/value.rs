use super::super::plan::value::{Context, Value};
use super::super::transform::value::*;
use super::*;
use crate::model::Expectation;
use alloc::boxed::Box;
use nepl3_core::{diagnostic::Report, origin::Mapping, value::TypedValue};

type ReplyMetadata<'a> = (&'a [SourceSnapshot], &'a [Mapping], &'a Report);
pub(super) fn metadata<E>(reply: &ReadReply) -> Result<ReplyMetadata<'_>, PortableError<E>> {
    match reply {
        ReadReply::Matched {
            sources,
            source_maps,
            report,
            ..
        }
        | ReadReply::NoMatch {
            sources,
            source_maps,
            report,
            ..
        }
        | ReadReply::NeedMore {
            sources,
            source_maps,
            report,
            ..
        }
        | ReadReply::Failed {
            sources,
            source_maps,
            report,
            ..
        }
        | ReadReply::Stopped {
            sources,
            source_maps,
            report,
            ..
        } => Ok((sources, source_maps, report)),
        ReadReply::Await { .. } => Err(PortableError::Shape),
    }
}
pub(super) fn source_indices<E>(
    case: &str,
    count: usize,
) -> Result<(usize, usize), PortableError<E>> {
    match (case, count) {
        ("Matched", 8) => Ok((5, 6)),
        ("NoMatch" | "Failed", 5) => Ok((3, 4)),
        ("NeedMore" | "Stopped", 4) => Ok((2, 3)),
        _ => Err(PortableError::Shape),
    }
}

impl Value for Expectation {
    fn encode<C: FoundationValueCodec>(
        &self,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let schema = s.reader;
        match self {
            Self::Literal(v) => variant(schema, "Expectation", "Literal", [v.encode(s, c, b)?], b),
            Self::ScalarClass(v) => variant(
                schema,
                "Expectation",
                "ScalarClass",
                [v.encode(s, c, b)?],
                b,
            ),
            Self::EndOfInput => variant(schema, "Expectation", "EndOfInput", [], b),
            Self::TokenBoundary => variant(schema, "Expectation", "TokenBoundary", [], b),
            Self::Provider {
                operation,
                arguments,
            } => {
                let value = match arguments.clone_with_budget(b)? {
                    TypedValue::Record(v) => NdfValue::Record(v),
                    TypedValue::Variant(v) => NdfValue::Variant(v),
                };
                variant(
                    schema,
                    "Expectation",
                    "Provider",
                    [operation.encode(s, c, b)?, value],
                    b,
                )
            }
        }
    }
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        match parts(v, s.reader, "Expectation")? {
            ("Literal", [v]) => Ok(Self::Literal(Value::decode(v, s, c, b)?)),
            ("ScalarClass", [v]) => Ok(Self::ScalarClass(Value::decode(v, s, c, b)?)),
            ("EndOfInput", []) => Ok(Self::EndOfInput),
            ("TokenBoundary", []) => Ok(Self::TokenBoundary),
            ("Provider", [operation, arguments]) => {
                arguments.charge_clone(b)?;
                let arguments = match arguments {
                    NdfValue::Record(v) => TypedValue::Record(v.clone()),
                    NdfValue::Variant(v) => TypedValue::Variant(v.clone()),
                    _ => return Err(PortableError::Shape),
                };
                Ok(Self::Provider {
                    operation: Value::decode(operation, s, c, b)?,
                    arguments,
                })
            }
            _ => Err(PortableError::Shape),
        }
    }
}

pub(super) fn encode<C: FoundationValueCodec>(
    reply: &ReadReply,
    s: &SchemaRef,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let context = Context::new(registry)?;
    let (sources, maps, report) = metadata(reply)?;
    let report_value = c.encode_report(report, b).map_err(boundary)?;
    let source_value = c.encode_sources(sources, b).map_err(boundary)?;
    let map_value = c.encode_mappings(maps, b).map_err(boundary)?;
    match reply {
        ReadReply::Matched {
            value,
            end,
            new_state,
            view,
            facts,
            ..
        } => variant(
            s,
            "ReadReply",
            "Matched",
            [
                value.clone_with_budget(b)?,
                NdfValue::U64(*end),
                new_state.clone_with_budget(b)?,
                c.encode_views(view, b).map_err(boundary)?,
                facts_value(facts, s, c, b)?,
                source_value,
                map_value,
                report_value,
            ],
            b,
        ),
        ReadReply::NoMatch {
            expected, furthest, ..
        } => variant(
            s,
            "ReadReply",
            "NoMatch",
            [
                expected.encode(&context, c, b)?,
                NdfValue::U64(*furthest),
                report_value,
                source_value,
                map_value,
            ],
            b,
        ),
        ReadReply::NeedMore { expected, .. } => variant(
            s,
            "ReadReply",
            "NeedMore",
            [
                expected.encode(&context, c, b)?,
                report_value,
                source_value,
                map_value,
            ],
            b,
        ),
        ReadReply::Failed {
            diagnostic,
            recovery,
            ..
        } => {
            let primary = primary_value(diagnostic, report, &report_value, c, b)?;
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
            variant(
                s,
                "ReadReply",
                "Failed",
                [primary, recovery, report_value, source_value, map_value],
                b,
            )
        }
        ReadReply::Stopped { reason, .. } => {
            let reason = variant(
                c.foundation_schema(),
                "StopReason",
                stop_name(*reason),
                [],
                b,
            )?;
            variant(
                s,
                "ReadReply",
                "Stopped",
                [reason, report_value, source_value, map_value],
                b,
            )
        }
        ReadReply::Await { .. } => Err(PortableError::Shape),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn decode<C: FoundationValueCodec>(
    case: &str,
    f: &[NdfValue],
    sources: Vec<SourceSnapshot>,
    maps: &[Mapping],
    s: &SchemaRef,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<ReadReply, PortableError<C::Error>> {
    let context = Context::new(registry)?;
    let report_index = match case {
        "Matched" => 7,
        "Failed" | "NoMatch" => 2,
        _ => 1,
    };
    let report = c.decode_report(&f[report_index], b).map_err(boundary)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of_val(maps) as u64,
    )?;
    let mut source_maps = Vec::with_capacity(maps.len());
    for mapping in maps {
        source_maps.push(crate::runtime::copy::copy(mapping, b)?);
    }
    Ok(match case {
        "Matched" => ReadReply::Matched {
            value: f[0].clone_with_budget(b)?,
            end: number(&f[1])?,
            new_state: f[2].clone_with_budget(b)?,
            view: c.decode_views(&f[3], b).map_err(boundary)?,
            facts: facts_from(&f[4], s, c, b)?,
            sources,
            source_maps,
            report,
        },
        "NoMatch" => ReadReply::NoMatch {
            expected: Value::decode(&f[0], &context, c, b)?,
            furthest: number(&f[1])?,
            sources,
            source_maps,
            report,
        },
        "NeedMore" => ReadReply::NeedMore {
            expected: Value::decode(&f[0], &context, c, b)?,
            sources,
            source_maps,
            report,
        },
        "Stopped" => {
            let (reason, empty) = parts(&f[0], c.foundation_schema(), "StopReason")?;
            if !empty.is_empty() {
                return Err(PortableError::Shape);
            }
            ReadReply::Stopped {
                reason: stop_from(reason).ok_or(PortableError::Shape)?,
                sources,
                source_maps,
                report,
            }
        }
        "Failed" => {
            let encoded = fields(&f[2], c.foundation_schema(), "Report", 4)?;
            let mut primary = None;
            for (index, value) in sequence(&encoded[0])?.iter().enumerate() {
                f[0].charge_clone(b)?;
                value.charge_clone(b)?;
                if &f[0] == value {
                    if primary.is_some() {
                        return Err(PortableError::Shape);
                    }
                    primary = Some(crate::runtime::copy::copy(
                        report.diagnostics.get(index).ok_or(PortableError::Shape)?,
                        b,
                    )?);
                }
            }
            let recovery = match &f[1] {
                NdfValue::None => None,
                NdfValue::Some(v) => Some(c.decode_span(v, b).map_err(boundary)?),
                _ => return Err(PortableError::Shape),
            };
            ReadReply::Failed {
                diagnostic: primary.ok_or(PortableError::Shape)?,
                recovery,
                sources,
                source_maps,
                report,
            }
        }
        _ => return Err(PortableError::Shape),
    })
}
