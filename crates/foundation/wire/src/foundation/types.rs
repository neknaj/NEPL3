//! Iterative symbolic type encoding; type resolution is a separate operation.
use crate::{
    WireError,
    boundary::variant,
    source::{record, text},
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::TypeDescriptor,
    value::{NdfValue, SchemaRef},
};

pub(super) fn encode(
    mut value: &TypeDescriptor,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    let mut wrappers = Vec::new();
    loop {
        budget.charge(Resource::Work, 1)?;
        budget.observe_depth(wrappers.len() as u64 + 1)?;
        match value {
            TypeDescriptor::List(inner) | TypeDescriptor::Option(inner) => {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<bool>() as u64,
                )?;
                wrappers.push(matches!(value, TypeDescriptor::List(_)));
                value = inner;
            }
            _ => break,
        }
    }
    let mut output = match value {
        TypeDescriptor::Named(reference) => {
            budget.charge(
                Resource::Work,
                (reference.package.len() + reference.name.len()) as u64,
            )?;
            if reference.package.is_empty() || reference.name.is_empty() {
                return Err(WireError::InvalidType);
            }
            let reference = record(
                schema,
                "TypeRef",
                [
                    text(&reference.package, budget)?,
                    NdfValue::U64(reference.revision),
                    text(&reference.name, budget)?,
                ],
                budget,
            )?;
            variant(schema, "TypeDescriptor", "Named", [reference], budget)?
        }
        value => {
            let name = match value {
                TypeDescriptor::Unit => "Unit",
                TypeDescriptor::Bool => "Bool",
                TypeDescriptor::U64 => "U64",
                TypeDescriptor::Integer => "Integer",
                TypeDescriptor::Natural => "Natural",
                TypeDescriptor::Rational => "Rational",
                TypeDescriptor::Text => "Text",
                TypeDescriptor::Bytes => "Bytes",
                TypeDescriptor::Bytes32 => "Bytes32",
                TypeDescriptor::NdfValue => "NdfValue",
                TypeDescriptor::NdfScalar => "NdfScalar",
                TypeDescriptor::TypedValue => "TypedValue",
                TypeDescriptor::Named(_) | TypeDescriptor::List(_) | TypeDescriptor::Option(_) => {
                    return Err(WireError::InvalidType);
                }
            };
            variant(schema, "TypeDescriptor", name, [], budget)?
        }
    };
    while let Some(list) = wrappers.pop() {
        budget.charge(Resource::Work, 1)?;
        output = variant(
            schema,
            "TypeDescriptor",
            if list { "List" } else { "Option" },
            [output],
            budget,
        )?;
    }
    Ok(output)
}
