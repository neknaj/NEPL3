//! Registered builtin reader diagnostics. Messages are rendered outside the reader core.
use crate::{model::Expectation, plan::CharClass};
use alloc::{string::ToString, vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::*,
    value::{NdfValue, Record, SchemaRef, TypedValue, Variant},
};
pub const PACKAGE: &str = "nepl3.reader";
pub const REVISION: u64 = 1;
mod descriptor;
/// Constructs the generated, language-neutral reader descriptor.
pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, SchemaError> {
    descriptor::descriptor(budget)
}
pub(crate) fn arguments(
    schema: &SchemaRef,
    foundation: &SchemaRef,
    expected: &[Expectation],
    offset: u64,
    budget: &mut Budget,
) -> Result<TypedValue, SchemaError> {
    budget.charge(
        Resource::AllocationUnits,
        2048 + schema.package.len() as u64,
    )?;
    let values = expected
        .iter()
        .map(|e| expectation_value(schema, foundation, e, budget))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "ReaderDiagnosticArguments".into(),
        fields: vec![NdfValue::List(values), NdfValue::U64(offset)],
    }))
}
fn expectation_value(
    schema: &SchemaRef,
    foundation: &SchemaRef,
    expectation: &Expectation,
    budget: &mut Budget,
) -> Result<NdfValue, SchemaError> {
    budget.charge(
        Resource::AllocationUnits,
        2048 + schema.package.len() as u64,
    )?;
    let (name, fields) = match expectation {
        Expectation::Literal(text) => {
            budget.charge(Resource::AllocationUnits, text.len() as u64)?;
            ("Literal", vec![NdfValue::Text(text.clone())])
        }
        Expectation::ScalarClass(class) => {
            if let CharClass::Chars(text) | CharClass::Except(text) = class {
                budget.charge(Resource::AllocationUnits, text.len() as u64)?;
            }
            ("ScalarClass", vec![class_value(schema, class)])
        }
        Expectation::EndOfInput => ("EndOfInput", vec![]),
        Expectation::TokenBoundary => ("TokenBoundary", vec![]),
        Expectation::Provider {
            operation,
            arguments,
        } => {
            budget.charge(
                Resource::AllocationUnits,
                (foundation.package.len() * 2
                    + operation.schema.package.len()
                    + operation.name.len()) as u64,
            )?;
            let arguments = match arguments.clone_with_budget(budget)? {
                TypedValue::Record(v) => NdfValue::Record(v),
                TypedValue::Variant(v) => NdfValue::Variant(v),
            };
            (
                "Provider",
                vec![
                    NdfValue::Record(Record {
                        schema: foundation.clone(),
                        kind: "OperationRef".into(),
                        fields: vec![
                            NdfValue::Record(Record {
                                schema: foundation.clone(),
                                kind: "SchemaRef".into(),
                                fields: vec![
                                    NdfValue::Text(operation.schema.package.clone()),
                                    NdfValue::U64(operation.schema.revision),
                                    NdfValue::Bytes(operation.schema.digest.0.to_vec()),
                                ],
                            }),
                            NdfValue::Text(operation.name.clone()),
                        ],
                    }),
                    arguments,
                ],
            )
        }
    };
    Ok(NdfValue::Variant(Variant {
        schema: schema.clone(),
        type_name: "Expectation".into(),
        variant: name.into(),
        fields,
    }))
}
fn class_value(schema: &SchemaRef, class: &CharClass) -> NdfValue {
    let (name, fields) = match class {
        CharClass::Any => ("Any", vec![]),
        CharClass::Whitespace => ("Whitespace", vec![]),
        CharClass::IdentifierStart => ("IdentifierStart", vec![]),
        CharClass::IdentifierContinue => ("IdentifierContinue", vec![]),
        CharClass::Digit => ("Digit", vec![]),
        CharClass::AsciiLetter => ("AsciiLetter", vec![]),
        CharClass::Chars(text) => ("Chars", vec![NdfValue::Text(text.clone())]),
        CharClass::Except(text) => ("Except", vec![NdfValue::Text(text.clone())]),
        CharClass::Range { lo, hi } => (
            "Range",
            vec![
                NdfValue::Text(lo.to_string()),
                NdfValue::Text(hi.to_string()),
            ],
        ),
    };
    NdfValue::Variant(Variant {
        schema: schema.clone(),
        type_name: "CharClass".into(),
        variant: name.into(),
        fields,
    })
}
