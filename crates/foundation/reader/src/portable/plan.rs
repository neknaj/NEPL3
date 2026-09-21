//! Portable reader definitions retain arena indices and symbolic type identities.
mod value;
use super::*;
use crate::plan::{CheckedPlan, PlanError, ReaderPlan};
use nepl3_core::value_codec::FoundationCodecError;
use value::{Context, Value};

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
