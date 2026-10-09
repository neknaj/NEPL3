use super::*;
use nepl3_core::{
    schema::{NamedType, OperationDescriptor, SchemaDescriptor, TypeShape},
    value::{Record, TypedValue},
};

fn fixture(
    padding: usize,
    operations: usize,
) -> Result<(SchemaRegistry, Expectation), ReaderError> {
    let mut registry = SchemaRegistry::default();
    for i in 0..padding {
        let descriptor = SchemaDescriptor {
            package: format!("padding.{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        registry.register(
            descriptor.reference(&mut budget())?,
            descriptor,
            &mut budget(),
        )?;
    }
    let descriptor = SchemaDescriptor {
        package: "target".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Args".into(),
            shape: TypeShape::Record { fields: vec![] },
            constraints: vec![],
        }],
        operations: (0..operations)
            .map(|i| OperationDescriptor {
                name: format!("operation.{i:02}"),
                input: TypeDescriptor::Unit,
                output: TypeDescriptor::Unit,
                pure: true,
            })
            .collect(),
    };
    let schema = descriptor.reference(&mut budget())?;
    registry.register(schema.clone(), descriptor, &mut budget())?;
    registry.finalize(&mut budget())?;
    Ok((
        registry,
        Expectation::Provider {
            operation: OperationRef {
                schema: schema.clone(),
                name: "operation.zz".into(),
            },
            arguments: TypedValue::Record(Record {
                schema,
                kind: "Args".into(),
                fields: vec![],
            }),
        },
    ))
}

#[test]
fn provider_expectation_search_stops_before_unmetered_scan() -> Result<(), ReaderError> {
    let (registry, expected) = fixture(32, 32)?;
    let mut limits = budget().limits();
    limits.work = 1;
    let mut b = Budget::new(limits);
    assert_eq!(
        expectations(&[expected], &registry, &mut b),
        Err(ReaderError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    assert_eq!(b.usage().work, 1);
    assert_eq!(b.usage().allocation_units, 0);
    Ok(())
}

#[test]
fn provider_expectation_search_charges_exact_candidates_and_preserves_errors()
-> Result<(), ReaderError> {
    for (padding, operations) in [(0, 0), (0, 32), (32, 32)] {
        let (registry, expected) = fixture(padding, operations)?;
        let mut b = budget();
        assert_eq!(
            expectations(core::slice::from_ref(&expected), &registry, &mut b),
            Err(ReaderError::ProviderContract)
        );
        // One expectation, lookup entry, padding schemas, target schema, exact
        // identity, and every missing-name operation candidate. No argument
        // validation is reached for a missing operation.
        let work = 1
            + 1
            + padding as u64 * (10 + 6 + 9)
            + (6 + 6 + 9)
            + (6 + 41)
            + operations as u64 * (12 + 12 + 1);
        let mut want = Budget::new(budget().limits());
        want.charge(Resource::Work, work)?;
        assert_eq!(b.usage(), want.usage());
        let mut bad = expected;
        if let Expectation::Provider { operation, .. } = &mut bad {
            operation.schema.digest.0[0] ^= 1;
        }
        let mut b = budget();
        assert_eq!(
            expectations(&[bad], &registry, &mut b),
            Err(ReaderError::Schema(SchemaError::UnknownSchema))
        );
        assert_eq!(b.usage().work, work - operations as u64 * 25);
    }
    Ok(())
}

#[test]
fn provider_expectation_lookup_preserves_stops_success_and_identity() -> Result<(), ReaderError> {
    let (registry, mut expected) = fixture(0, 32)?;
    if let Expectation::Provider { operation, .. } = &mut expected {
        operation.name = "operation.31".into();
    }
    let mut complete = budget();
    expectations(core::slice::from_ref(&expected), &registry, &mut complete)?;
    assert_eq!(complete.usage().nodes, 1);
    assert_eq!(complete.usage().allocation_units, 0);
    // Limits at the expectation, schema entry/candidate, identity and operation
    // boundaries must stop before argument validation and keep the stop sticky.
    for work in [1, 2, 23, 70, 70 + 31 * 25] {
        let mut limits = budget().limits();
        limits.work = work;
        let mut b = Budget::new(limits);
        assert_eq!(
            expectations(core::slice::from_ref(&expected), &registry, &mut b),
            Err(ReaderError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(b.usage().work, work);
        assert_eq!(b.usage().nodes, 0);
        assert_eq!(b.usage().allocation_units, 0);
        let before = b.usage();
        assert_eq!(
            expectations(core::slice::from_ref(&expected), &registry, &mut b),
            Err(ReaderError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(b.usage(), before);
    }
    let mut cancelled = budget();
    cancelled.charge(Resource::Work, 7)?;
    cancelled.cancel();
    let before = cancelled.usage();
    assert_eq!(
        expectations(core::slice::from_ref(&expected), &registry, &mut cancelled),
        Err(ReaderError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(cancelled.usage(), before);
    for bad_revision in [false, true] {
        let mut bad = expected.clone();
        if let Expectation::Provider { operation, .. } = &mut bad {
            if bad_revision {
                operation.schema.revision += 1;
            } else {
                operation.schema.package = "missing".into();
            }
        }
        assert_eq!(
            expectations(&[bad], &registry, &mut budget()),
            Err(ReaderError::Schema(SchemaError::UnknownSchema))
        );
    }
    // Operation existence and argument structural validation remain separate;
    // an expectation is not an invocation requiring the operation input type.
    if let Expectation::Provider {
        arguments: TypedValue::Record(record),
        ..
    } = &mut expected
    {
        record.kind = "Missing".into();
    }
    assert_eq!(
        expectations(&[expected], &registry, &mut budget()),
        Err(ReaderError::Schema(SchemaError::UnknownType))
    );
    Ok(())
}
