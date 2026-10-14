//! Source-closed evaluation requests. Limits and usage belong to the caller's
//! operation Budget, never to caller-supplied proof or paid-work assertions.
use super::*;
use crate::model::{MathEvaluationRequest, MathEvaluationResult};

pub fn request_to_value<C: FoundationValueCodec>(
    request: &MathEvaluationRequest,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let syntax = super::to_value(&request.syntax, registry, codec, budget)?;
    // A Row/SentenceGuest syntax is valid data but cannot be an evaluation request.
    crate::check::expression(&request.syntax.value, budget).map_err(expression_error)?;
    let environment = super::environment_to_value(&request.environment, registry, codec, budget)?;
    let value = record(
        schema(registry)?,
        "MathEvaluationRequest",
        [syntax, environment],
        budget,
    )?;
    check_report(registry, &value, "MathEvaluationRequest", budget)?;
    Ok(value)
}

/// Decode without any original Rust request or ambient source authority.
/// Source closure, lexical bindings and assignment validity are all checked.
pub fn request_from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<MathEvaluationRequest, PortableError<C::Error>> {
    check_report(registry, value, "MathEvaluationRequest", budget)?;
    let fields = fields(value, schema(registry)?, "MathEvaluationRequest", 2)?;
    let syntax = super::from_value(&fields[0], registry, codec, budget)?;
    crate::check::expression(&syntax.value, budget).map_err(expression_error)?;
    let environment = super::environment_from_value(&fields[1], registry, codec, budget)?;
    Ok(MathEvaluationRequest {
        syntax,
        environment,
    })
}

/// Execute raw native data at the current receiving boundary. Semantic failure
/// is a located MathEvaluationResult; cancellation/resource stops remain Err.
/// This never evaluates foreign annotation code or rewrites author notation.
pub fn execute<C: FoundationValueCodec>(
    request: &MathEvaluationRequest,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<MathEvaluationResult, PortableError<C::Error>> {
    budget.poll()?;
    schema(registry)?;
    request
        .syntax
        .validate_structure(registry, budget, codec.source_admission())?;
    let checked =
        crate::check::expression(&request.syntax.value, budget).map_err(expression_error)?;
    let environment =
        crate::environment::check(&request.environment, budget).map_err(environment_error)?;
    let report =
        crate::evaluation::report(&checked, &environment, budget).map_err(evaluation_error)?;
    budget.poll()?;
    Ok(report.result)
}

/// Standalone NDF receiver: decode, check, evaluate and encode under one budget.
/// No caller-held checked expression or environment is required.
pub fn execute_value<C: FoundationValueCodec>(
    request: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let request = request_from_value(request, registry, codec, budget)?;
    let result = execute(&request, registry, codec, budget)?;
    let value = result.put(schema(registry)?, codec, budget)?;
    check_report(registry, &value, "MathEvaluationResult", budget)?;
    Ok(value)
}

/// A schema-valid response is not authoritative: recompute it against the
/// locally retained request, its source closure and the current shared budget.
pub fn verify_result<C: FoundationValueCodec>(
    result: &NdfValue,
    request: &MathEvaluationRequest,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<MathEvaluationResult, PortableError<C::Error>> {
    budget.poll()?;
    schema(registry)?;
    request
        .syntax
        .validate_structure(registry, budget, codec.source_admission())?;
    let checked =
        crate::check::expression(&request.syntax.value, budget).map_err(expression_error)?;
    let environment =
        crate::environment::check(&request.environment, budget).map_err(environment_error)?;
    Ok(super::evaluation_result_from_value(
        result,
        &checked,
        &environment,
        registry,
        codec,
        budget,
    )?
    .result)
}

fn expression_error<E>(error: crate::check::ShapeError) -> PortableError<E> {
    match error {
        crate::check::ShapeError::Stopped(reason) => PortableError::Stopped(reason),
        other => PortableError::Expression(other),
    }
}
