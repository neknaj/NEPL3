//! Portable reader definitions retain arena indices and symbolic type identities.
pub(super) mod value;
use super::*;
use crate::plan::{CheckedPlan, PlanError, ReaderPlan};
use crate::tokenizer::ReaderMode;
use nepl3_core::value_codec::FoundationCodecError;
use value::{Context, Value};

fn mode_error<E>(error: crate::runtime::ReaderError) -> PortableError<E> {
    match error {
        crate::runtime::ReaderError::Stopped(reason) => PortableError::Stopped(reason),
        other => PortableError::Reader(other),
    }
}
fn validate_mode<E>(
    value: &NdfValue,
    plan: &CheckedPlan<'_>,
    budget: &mut Budget,
) -> Result<(), PortableError<E>> {
    budget.charge(
        Resource::AllocationUnits,
        ("nepl3.reader".len() + "ReaderMode".len()) as u64,
    )?;
    plan.registry()
        .validate(
            &TypeDescriptor::Named(TypeRef {
                package: "nepl3.reader".into(),
                revision: 1,
                name: "ReaderMode".into(),
            }),
            value,
            budget,
        )
        .map(|_| ())
        .map_err(|error| match error {
            SchemaError::Stopped(reason) => PortableError::Stopped(reason),
            other => PortableError::Schema(other),
        })
}
/// Encode one mode after checking rule and token-kind references against its plan.
pub fn mode_to_value<C: FoundationValueCodec>(
    mode: &ReaderMode,
    plan: &CheckedPlan<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    budget.poll()?;
    crate::tokenizer::validate_modes(core::slice::from_ref(mode), plan, plan.registry(), budget)
        .map_err(mode_error)?;
    let value = mode.encode(&Context::new(plan.registry())?, codec, budget)?;
    validate_mode(&value, plan, budget)?;
    Ok(value)
}
/// Decode one mode. The enclosing package checks uniqueness of the mode names.
pub fn mode_from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    plan: &CheckedPlan<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<ReaderMode, PortableError<C::Error>> {
    budget.poll()?;
    validate_mode(value, plan, budget)?;
    let mode = ReaderMode::decode(value, &Context::new(plan.registry())?, codec, budget)?;
    crate::tokenizer::validate_modes(core::slice::from_ref(&mode), plan, plan.registry(), budget)
        .map_err(mode_error)?;
    Ok(mode)
}

fn boundary<E: FoundationCodecError>(error: E) -> PortableError<E> {
    match error.stop_reason() {
        Some(reason) => PortableError::Stopped(reason),
        None => PortableError::Boundary(error),
    }
}
fn plan_error<E>(error: PlanError) -> PortableError<E> {
    match error {
        PlanError::Stopped(reason) => PortableError::Stopped(reason),
        other => PortableError::Plan(other),
    }
}
fn validate<E>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), PortableError<E>> {
    budget.charge(
        Resource::AllocationUnits,
        ("nepl3.reader".len() + "ReaderPlan".len()) as u64,
    )?;
    registry
        .validate(
            &TypeDescriptor::Named(TypeRef {
                package: "nepl3.reader".into(),
                revision: 1,
                name: "ReaderPlan".into(),
            }),
            value,
            budget,
        )
        .map(|_| ())
        .map_err(|error| match error {
            SchemaError::Stopped(reason) => PortableError::Stopped(reason),
            other => PortableError::Schema(other),
        })
}
/// Encode a checked plan; executable provider implementations remain host-owned.
pub fn to_value<C: FoundationValueCodec>(
    plan: &CheckedPlan<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    budget.poll()?;
    let context = Context::new(plan.registry())?;
    let value = plan.plan().encode(&context, codec, budget)?;
    validate(&value, plan.registry(), budget)?;
    Ok(value)
}
/// Decode and check all rules, expression references and provider signatures.
/// The returned definition can be checked again to obtain a borrowing proof.
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<ReaderPlan, PortableError<C::Error>> {
    budget.poll()?;
    validate(value, registry, budget)?;
    let plan = ReaderPlan::decode(value, &Context::new(registry)?, codec, budget)?;
    plan.check(registry, budget).map_err(plan_error)?;
    Ok(plan)
}
