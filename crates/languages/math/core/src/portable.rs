//! Explicit Math NDF schema adapters. Decoding produces raw data followed by the
//! same source/category/graph checks used by native callers, not a evaluation proof.
mod value;
use crate::{check::StructureError, model::MathSyntax};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    source::SourceStore,
    value::{NdfValue, SchemaRef},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use value::{Value, fields, record};
#[derive(Debug, Eq, PartialEq)]
pub enum PortableError<E> {
    Stopped(StopReason),
    Foundation(E),
    Schema(SchemaError),
    Structure(StructureError),
    Shape,
    BindingMismatch,
    FreeSymbolsMismatch,
    Expression(crate::check::ShapeError),
}

/// Generate the report from a checked input, then encode its neutral schema.
pub fn free_symbols_to_value<C: FoundationValueCodec>(
    input: &crate::check::CheckedExpression<'_>,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let report = crate::free::symbols(input, budget).map_err(expression_error)?;
    let raw = report.put(schema(registry)?, codec, budget)?;
    check_report(registry, &raw, "MathFreeSymbols", budget)?;
    Ok(raw)
}

/// Recompute input requirements; reordered, omitted or forged occurrences fail.
pub fn free_symbols_from_value<C: FoundationValueCodec>(
    raw: &NdfValue,
    input: &crate::check::CheckedExpression<'_>,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<crate::model::MathFreeSymbols, PortableError<C::Error>> {
    check_report(registry, raw, "MathFreeSymbols", budget)?;
    let report = crate::model::MathFreeSymbols::read(raw, schema(registry)?, codec, budget)?;
    let expected = crate::free::symbols(input, budget).map_err(expression_error)?;
    for symbol in &report.symbols {
        budget.charge(
            Resource::Work,
            (symbol.name.len() as u64)
                .saturating_add(symbol.occurrences.len() as u64)
                .saturating_add(1),
        )?;
    }
    if report != expected {
        return Err(PortableError::FreeSymbolsMismatch);
    }
    Ok(report)
}

fn expression_error<E>(error: crate::check::ShapeError) -> PortableError<E> {
    match error {
        crate::check::ShapeError::Stopped(reason) => PortableError::Stopped(reason),
        other => PortableError::Expression(other),
    }
}

/// Encode a binding report only after comparing it to its declared input shape.
pub fn bindings_to_value<C: FoundationValueCodec>(
    report: &crate::model::MathBindings,
    input: &crate::check::ValidatedMathShape<'_>,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    budget.charge(
        Resource::Work,
        (report.definitions.len() as u64).saturating_add(report.uses.len() as u64),
    )?;
    if *report != crate::binding::analyze(input, budget)? {
        return Err(PortableError::BindingMismatch);
    }
    let raw = report.put(schema(registry)?, codec, budget)?;
    check_bindings(registry, &raw, budget)?;
    Ok(raw)
}

/// Untrusted output is not a proof: recompute binding for the supplied input.
pub fn bindings_from_value<C: FoundationValueCodec>(
    raw: &NdfValue,
    input: &crate::check::ValidatedMathShape<'_>,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<crate::model::MathBindings, PortableError<C::Error>> {
    let s = schema(registry)?;
    check_bindings(registry, raw, budget)?;
    let report = crate::model::MathBindings::read(raw, s, codec, budget)?;
    budget.charge(
        Resource::Work,
        (report.definitions.len() as u64).saturating_add(report.uses.len() as u64),
    )?;
    if report != crate::binding::analyze(input, budget)? {
        return Err(PortableError::BindingMismatch);
    }
    Ok(report)
}

fn check_bindings<E>(
    registry: &SchemaRegistry,
    raw: &NdfValue,
    budget: &mut Budget,
) -> Result<(), PortableError<E>> {
    check_report(registry, raw, "MathBindings", budget)
}
fn check_report<E>(
    registry: &SchemaRegistry,
    raw: &NdfValue,
    name: &str,
    budget: &mut Budget,
) -> Result<(), PortableError<E>> {
    budget.charge(Resource::AllocationUnits, 10 + name.len() as u64)?;
    registry.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.math".into(),
            revision: 1,
            name: name.into(),
        }),
        raw,
        budget,
    )?;
    Ok(())
}
impl<E> From<StopReason> for PortableError<E> {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl<E> From<SchemaError> for PortableError<E> {
    fn from(v: SchemaError) -> Self {
        match v {
            SchemaError::Stopped(s) => Self::Stopped(s),
            v => Self::Schema(v),
        }
    }
}
impl<E> From<StructureError> for PortableError<E> {
    fn from(v: StructureError) -> Self {
        match v {
            StructureError::Stopped(s) => Self::Stopped(s),
            v => Self::Structure(v),
        }
    }
}
fn boundary<E: FoundationCodecError>(e: E) -> PortableError<E> {
    match e.stop_reason() {
        Some(s) => PortableError::Stopped(s),
        None => PortableError::Foundation(e),
    }
}
fn schema<E>(registry: &SchemaRegistry) -> Result<&SchemaRef, PortableError<E>> {
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    registry
        .selected("nepl3.math", 1)
        .ok_or(SchemaError::UnknownSchema.into())
}
fn check<E>(
    registry: &SchemaRegistry,
    v: &NdfValue,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    b.charge(Resource::AllocationUnits, 20)?;
    registry.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.math".into(),
            revision: 1,
            name: "MathSyntax".into(),
        }),
        v,
        b,
    )?;
    Ok(())
}
pub fn to_value<C: FoundationValueCodec>(
    document: &MathSyntax,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    document.validate_structure(registry, b, c.source_admission())?;
    let s = schema(registry)?;
    let sources = c.encode_sources(&document.sources, b).map_err(boundary)?;
    let mut store = SourceStore::default();
    for source in &document.sources {
        store
            .insert_with_budget(source.clone_with_budget(b)?, b)
            .map_err(StructureError::from)?;
    }
    let mut scoped = c.scoped(&store);
    let value = document.value.put(s, &mut scoped, b)?;
    let origins = scoped
        .encode_origins(&document.origins, b)
        .map_err(boundary)?;
    let views = document.views.put(s, &mut scoped, b)?;
    let maps = document.source_maps.put(s, &mut scoped, b)?;
    let output = record(s, "MathSyntax", [value, sources, origins, views, maps], b)?;
    check(registry, &output, b)?;
    Ok(output)
}
pub fn from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<MathSyntax, PortableError<C::Error>> {
    check(registry, input, b)?;
    let s = schema(registry)?;
    let fields = fields(input, s, "MathSyntax", 5)?;
    let sources = c.decode_sources(&fields[1], b).map_err(boundary)?;
    let mut store = SourceStore::default();
    for source in &sources {
        store
            .insert_with_budget(source.clone_with_budget(b)?, b)
            .map_err(StructureError::from)?;
    }
    let mut scoped = c.scoped(&store);
    let value = Value::read(&fields[0], s, &mut scoped, b)?;
    let origins = scoped.decode_origins(&fields[2], b).map_err(boundary)?;
    let views = Value::read(&fields[3], s, &mut scoped, b)?;
    let source_maps = Value::read(&fields[4], s, &mut scoped, b)?;
    let output = MathSyntax {
        value,
        sources,
        origins,
        views,
        source_maps,
    };
    output.validate_structure(registry, b, scoped.source_admission())?;
    Ok(output)
}
