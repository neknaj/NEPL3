//! MiniExpr arithmetic operation boundary, separate from the syntax schema.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{
        FieldDescriptor, NamedType, OperationDescriptor, SchemaDescriptor, TypeDescriptor, TypeRef,
        TypeShape,
    },
};

pub const PACKAGE: &str = "org.example.miniexpr.operations";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Neg,
    Add,
    Mul,
}

impl Operation {
    pub fn name(self) -> &'static str {
        match self {
            Self::Neg => "neg",
            Self::Add => "add",
            Self::Mul => "mul",
        }
    }
}

/// Construct the fixed contract after reserving a conservative allowance for
/// its owned descriptors and strings. Integer payloads preserve signed values.
pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, StopReason> {
    budget.charge(Resource::Work, 4096)?;
    budget.charge(Resource::AllocationUnits, 16384)?;
    let ty = |name: &str| {
        TypeDescriptor::Named(TypeRef {
            package: PACKAGE.into(),
            revision: 1,
            name: name.into(),
        })
    };
    let record = |name: &str, fields: &[&str]| NamedType {
        name: name.into(),
        constraints: vec![],
        shape: TypeShape::Record {
            fields: fields
                .iter()
                .map(|field| FieldDescriptor {
                    name: (*field).into(),
                    ty: TypeDescriptor::Integer,
                })
                .collect(),
        },
    };
    Ok(SchemaDescriptor {
        package: PACKAGE.into(),
        revision: 1,
        types: vec![
            record("Unary", &["value"]),
            record("Binary", &["left", "right"]),
            record("Value", &["value"]),
            record("Environment", &[]),
        ],
        operations: [Operation::Neg, Operation::Add, Operation::Mul]
            .into_iter()
            .map(|operation| OperationDescriptor {
                name: operation.name().into(),
                input: ty(if operation == Operation::Neg {
                    "Unary"
                } else {
                    "Binary"
                }),
                output: ty("Value"),
                pure: true,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use external_hello_language::{budget, error};
    use nepl3_core::{
        operation::Invoke,
        schema::SchemaRegistry,
        value::{Integer, NdfValue, OperationRef, Record, TypedValue},
    };

    #[test]
    fn operation_schema_preserves_signed_values_and_rejects_wrong_arity() -> Result<(), String> {
        let descriptor = descriptor(&mut budget()).map_err(error)?;
        let identity = descriptor.reference(&mut budget()).map_err(error)?;
        let mut registry = SchemaRegistry::default();
        registry
            .register(identity.clone(), descriptor, &mut budget())
            .map_err(error)?;
        registry.finalize(&mut budget()).map_err(error)?;
        let record = |kind: &str, fields| {
            TypedValue::Record(Record {
                schema: identity.clone(),
                kind: kind.into(),
                fields,
            })
        };
        let operation = OperationRef {
            schema: identity.clone(),
            name: Operation::Add.name().into(),
        };
        let mut call = Invoke {
            request_id: 1,
            operation: operation.clone(),
            input: record(
                "Binary",
                vec![
                    NdfValue::Integer(Integer::from(-7_i64)),
                    NdfValue::Integer(Integer::from(2_i64)),
                ],
            ),
            environment: record("Environment", vec![]),
            sources: vec![],
            resources: vec![],
            limits: budget().limits(),
        };
        call.validate_input(&operation, &registry, &mut budget())
            .map_err(error)?;
        call.input = record("Unary", vec![NdfValue::Integer(Integer::from(7_i64))]);
        assert!(
            call.validate_input(&operation, &registry, &mut budget())
                .is_err()
        );
        call.input = record("Binary", vec![NdfValue::U64(7), NdfValue::U64(2)]);
        assert!(
            call.validate_input(&operation, &registry, &mut budget())
                .is_err()
        );
        // The numeric value 7 in a U64 field is not the declared Integer wire type.
        Ok(())
    }
}
