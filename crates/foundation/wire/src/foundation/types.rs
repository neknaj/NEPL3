//! Iterative symbolic type encoding; type resolution is a separate operation.
use crate::{
    WireError,
    boundary::variant,
    source::{record, text},
};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{TypeDescriptor, TypeRef},
    value::{NdfValue, SchemaRef},
};

pub(super) fn decode(
    mut value: &NdfValue,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<TypeDescriptor, WireError> {
    let mut wrappers = Vec::new();
    let mut output = loop {
        budget.charge(Resource::Work, 1)?;
        budget.observe_depth(wrappers.len() as u64 + 1)?;
        let NdfValue::Variant(v) = value else {
            return Err(WireError::InvalidType);
        };
        if &v.schema != schema || v.type_name != "TypeDescriptor" {
            return Err(WireError::InvalidType);
        }
        if v.variant == "List" || v.variant == "Option" {
            let [inner] = v.fields.as_slice() else {
                return Err(WireError::InvalidType);
            };
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<bool>() as u64,
            )?;
            wrappers.push(v.variant == "List");
            value = inner;
            continue;
        }
        if v.variant == "Named" {
            let [reference] = v.fields.as_slice() else {
                return Err(WireError::InvalidType);
            };
            let fields = crate::source::fields(reference, schema, "TypeRef", 3)?;
            let [
                NdfValue::Text(package),
                NdfValue::U64(revision),
                NdfValue::Text(name),
            ] = fields
            else {
                return Err(WireError::InvalidType);
            };
            budget.charge(Resource::Work, (package.len() + name.len()) as u64)?;
            if package.is_empty() || name.is_empty() {
                return Err(WireError::InvalidType);
            }
            budget.charge(
                Resource::AllocationUnits,
                (package.len() + name.len()) as u64,
            )?;
            break TypeDescriptor::Named(TypeRef {
                package: package.clone(),
                revision: *revision,
                name: name.clone(),
            });
        }
        if !v.fields.is_empty() {
            return Err(WireError::InvalidType);
        }
        break match v.variant.as_str() {
            "Unit" => TypeDescriptor::Unit,
            "Bool" => TypeDescriptor::Bool,
            "U64" => TypeDescriptor::U64,
            "Integer" => TypeDescriptor::Integer,
            "Natural" => TypeDescriptor::Natural,
            "Rational" => TypeDescriptor::Rational,
            "Text" => TypeDescriptor::Text,
            "Bytes" => TypeDescriptor::Bytes,
            "Bytes32" => TypeDescriptor::Bytes32,
            "NdfValue" => TypeDescriptor::NdfValue,
            "NdfScalar" => TypeDescriptor::NdfScalar,
            "TypedValue" => TypeDescriptor::TypedValue,
            _ => return Err(WireError::InvalidType),
        };
    };
    while let Some(list) = wrappers.pop() {
        budget.charge(Resource::Work, 1)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<TypeDescriptor>() as u64,
        )?;
        output = if list {
            TypeDescriptor::List(Box::new(output))
        } else {
            TypeDescriptor::Option(Box::new(output))
        };
    }
    Ok(output)
}

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
