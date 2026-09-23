//! Exact arithmetic registered through suite's checked terminal dispatch.
use crate::{
    arithmetic::{self, Application},
    contract::{self, Operation},
};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::{OperationResult, Report},
    operation::Invoke,
    schema::{SchemaError, SchemaRegistry},
    source::Digest,
    value::{NdfValue, OperationRef, Record, TypedValue},
};
use nepl3_suite::dispatch::Registration;

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Schema(SchemaError),
}

/// Host-selected executable identity and exact operation references.
pub struct Arithmetic {
    operations: [OperationRef; 3],
    implementation: Digest,
}

impl Arithmetic {
    /// Register the descriptor before the host finalizes the complete registry.
    /// The host supplies the executable identity used by native dispatch.
    pub fn register(
        registry: &mut SchemaRegistry,
        implementation: Digest,
        budget: &mut Budget,
    ) -> Result<Self, Error> {
        let descriptor = contract::descriptor(budget).map_err(Error::Stopped)?;
        let schema = descriptor.reference(budget).map_err(Error::Schema)?;
        budget
            .charge(Resource::AllocationUnits, 1024)
            .map_err(Error::Stopped)?;
        let operations =
            [Operation::Neg, Operation::Add, Operation::Mul].map(|operation| OperationRef {
                schema: schema.clone(),
                name: operation.name().into(),
            });
        registry
            .register(schema, descriptor, budget)
            .map_err(Error::Schema)?;
        Ok(Self {
            operations,
            implementation,
        })
    }

    pub fn operation(&self, operation: Operation) -> &OperationRef {
        &self.operations[match operation {
            Operation::Neg => 0,
            Operation::Add => 1,
            Operation::Mul => 2,
        }]
    }

    pub fn registrations(&self) -> [Registration<'_>; 3] {
        self.operations.each_ref().map(|operation| Registration {
            operation,
            implementation: self.implementation,
            invoke,
        })
    }
}

// Only exposed through registrations bound to the descriptor above. Suite
// validates the operation identity and input schema before entering this code.
fn invoke(
    call: &Invoke,
    _: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason> {
    budget.poll()?;
    let TypedValue::Record(input) = &call.input else {
        return Ok(OperationResult::Invalid {
            partial: None,
            report: Report {
                usage: budget.usage(),
                ..Report::default()
            },
        });
    };
    let application = match (call.operation.name.as_str(), input.fields.as_slice()) {
        ("neg", [NdfValue::Integer(value)]) => Application::Neg(value),
        ("add", [NdfValue::Integer(left), NdfValue::Integer(right)]) => {
            Application::Add(left, right)
        }
        ("mul", [NdfValue::Integer(left), NdfValue::Integer(right)]) => {
            Application::Mul(left, right)
        }
        _ => {
            return Ok(OperationResult::Invalid {
                partial: None,
                report: Report {
                    usage: budget.usage(),
                    ..Report::default()
                },
            });
        }
    };
    let value = arithmetic::apply(application, budget)?;
    budget.charge(
        Resource::AllocationUnits,
        (core::mem::size_of::<Record>()
            + core::mem::size_of::<NdfValue>()
            + call.operation.schema.package.len()
            + "Value".len()) as u64,
    )?;
    budget.charge(
        Resource::Work,
        call.operation.schema.package.len() as u64 + 1,
    )?;
    Ok(OperationResult::Complete {
        value: TypedValue::Record(Record {
            schema: call.operation.schema.clone(),
            kind: "Value".into(),
            fields: vec![NdfValue::Integer(value)],
        }),
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use external_hello_language::{budget, error};
    use nepl3_core::{source::SourceStore, value::Integer};
    use nepl3_suite::dispatch::{DispatchError, invoke_terminal};

    #[test]
    fn native_dispatch_checks_input_identity_output_and_execution_stop() -> Result<(), String> {
        let mut registry = SchemaRegistry::default();
        let identity = Digest::of(b"test arithmetic implementation");
        let arithmetic =
            Arithmetic::register(&mut registry, identity, &mut budget()).map_err(error)?;
        registry.finalize(&mut budget()).map_err(error)?;
        let operation = arithmetic.operation(Operation::Add);
        let record = |kind: &str, fields| {
            TypedValue::Record(Record {
                schema: operation.schema.clone(),
                kind: kind.into(),
                fields,
            })
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
        let registrations = arithmetic.registrations();
        let sources = SourceStore::default();
        let mut execution = budget();
        execution.charge(Resource::Work, 123).map_err(error)?;
        let result = invoke_terminal(
            &registrations,
            operation,
            identity,
            &call,
            &registry,
            &sources,
            &mut execution,
            &mut budget(),
        )
        .map_err(error)?;
        let OperationResult::Complete { value, report } = result else {
            return Err("expected Complete".into());
        };
        assert_eq!(report.usage, execution.usage());
        assert!(report.usage.work > 123);
        assert_eq!(
            value,
            record("Value", vec![NdfValue::Integer(Integer::from(-5_i64))])
        );
        assert!(matches!(
            invoke_terminal(
                &registrations,
                operation,
                Digest::of(b"different implementation"),
                &call,
                &registry,
                &sources,
                &mut budget(),
                &mut budget()
            ),
            Err(DispatchError::MissingImplementation)
        ));
        let mut execution = budget();
        execution.stop(StopReason::Cancelled);
        assert!(matches!(
            invoke_terminal(
                &registrations,
                operation,
                identity,
                &call,
                &registry,
                &sources,
                &mut execution,
                &mut budget()
            ),
            Err(DispatchError::Stopped(StopReason::Cancelled))
        ));
        call.input = record("Binary", vec![NdfValue::U64(7), NdfValue::U64(2)]);
        assert!(matches!(
            invoke_terminal(
                &registrations,
                operation,
                identity,
                &call,
                &registry,
                &sources,
                &mut budget(),
                &mut budget()
            ),
            Err(DispatchError::Input(_))
        ));
        Ok(())
    }
}
