//! Engine-owned structured recovery arguments, validated by the shared collector boundary.
use super::{build, error::ParseError};
use crate::package::EntryContext;
use alloc::{vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    diagnostic::{Diagnostic, Severity},
    schema::SchemaRegistry,
    source::Span,
    value::{NdfValue, Record, SchemaRef, TypedValue},
};
fn record(
    schema: &SchemaRef,
    name: &str,
    fields: Vec<NdfValue>,
    b: &mut Budget,
) -> Result<Record, ParseError> {
    build::slot::<Record>(b)?;
    b.charge(Resource::AllocationUnits, schema.package.len() as u64)?;
    Ok(Record {
        schema: schema.clone(),
        kind: build::text(name, b)?,
        fields,
    })
}
fn slots(count: u64, b: &mut Budget) -> Result<(), ParseError> {
    b.charge(
        Resource::AllocationUnits,
        count * core::mem::size_of::<NdfValue>() as u64,
    )?;
    Ok(())
}
fn bytes(value: &[u8], b: &mut Budget) -> Result<NdfValue, ParseError> {
    b.charge(Resource::AllocationUnits, value.len() as u64)?;
    b.charge(Resource::Work, value.len() as u64)?;
    Ok(NdfValue::Bytes(value.to_vec()))
}
pub(super) fn recovery(
    entry: &EntryContext,
    span: &Span,
    missing: bool,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<Diagnostic, ParseError> {
    let engine = registry
        .selected("nepl3.engine", 1)
        .ok_or(nepl3_core::schema::SchemaError::UnknownSchema)?;
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or(nepl3_core::schema::SchemaError::UnknownSchema)?;
    slots(3, b)?;
    let schema = record(
        foundation,
        "SchemaRef",
        vec![
            NdfValue::Text(build::text(&entry.package.schema.package, b)?),
            NdfValue::U64(entry.package.schema.revision),
            bytes(&entry.package.schema.digest.0, b)?,
        ],
        b,
    )?;
    slots(2, b)?;
    let package = record(
        engine,
        "PackageIdentity",
        vec![
            NdfValue::Record(schema),
            bytes(&entry.package.semantic_digest.0, b)?,
        ],
        b,
    )?;
    slots(4, b)?;
    let expected = record(
        engine,
        "EntryContext",
        vec![
            NdfValue::Record(package),
            NdfValue::Text(build::text(&entry.alias, b)?),
            NdfValue::Text(build::text(&entry.category, b)?),
            NdfValue::Text(build::text(&entry.mode, b)?),
        ],
        b,
    )?;
    slots(2, b)?;
    let arguments = TypedValue::Record(record(
        engine,
        "EngineDiagnosticArguments",
        vec![NdfValue::Record(expected), NdfValue::U64(span.start())],
        b,
    )?);
    build::slot::<Diagnostic>(b)?;
    b.charge(Resource::AllocationUnits, engine.package.len() as u64)?;
    Ok(Diagnostic {
        schema: engine.clone(),
        code: build::text(
            if missing {
                "MissingChild"
            } else {
                "UnparsedInput"
            },
            b,
        )?,
        severity: Severity::Error,
        stage: build::text("parse", b)?,
        arguments,
        primary: Some(build::span(span, b)?),
        related: vec![],
        fixes: vec![],
    })
}
