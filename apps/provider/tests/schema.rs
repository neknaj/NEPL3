use nepl3_core::{budget::*, schema::*, value::*};
use nepl3_provider::schema::{SchemaAdmissionError, admit};
#[path = "schema/exchange.rs"]
mod exchange;

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 128,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn error(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}
fn bootstrap() -> Result<SchemaRegistry, String> {
    let mut registry = SchemaRegistry::default();
    let descriptor = foundation::descriptor(&mut budget()).map_err(error)?;
    registry
        .register(
            descriptor.reference(&mut budget()).map_err(error)?,
            descriptor,
            &mut budget(),
        )
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    Ok(registry)
}
fn descriptor(package: &str, field: TypeDescriptor) -> SchemaDescriptor {
    SchemaDescriptor {
        package: package.into(),
        revision: 1,
        types: vec![NamedType {
            name: "Value".into(),
            shape: TypeShape::Record {
                fields: vec![FieldDescriptor {
                    name: "value".into(),
                    ty: field,
                }],
            },
            constraints: vec![],
        }],
        operations: vec![],
    }
}
fn reference(package: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: package.into(),
        revision: 1,
        name: "Value".into(),
    })
}
#[test]
fn complete_batch_resolves_forward_dependencies_and_validates_values() -> Result<(), String> {
    let bootstrap = bootstrap()?;
    let a = descriptor("test.a", reference("test.b"));
    let b = descriptor("test.b", TypeDescriptor::U64);
    let ids = [
        a.reference(&mut budget()).map_err(error)?,
        b.reference(&mut budget()).map_err(error)?,
    ];
    let a_bytes = nepl3_wire::schema::encode(&a, &bootstrap, &mut budget()).map_err(error)?;
    let b_bytes = nepl3_wire::schema::encode(&b, &bootstrap, &mut budget()).map_err(error)?;
    let registry = admit(bootstrap, &ids, &[&a_bytes, &b_bytes], &mut budget()).map_err(error)?;
    assert!(registry.is_finalized());
    let value = TypedValue::Record(Record {
        schema: ids[0].clone(),
        kind: "Value".into(),
        fields: vec![NdfValue::Record(Record {
            schema: ids[1].clone(),
            kind: "Value".into(),
            fields: vec![NdfValue::U64(42)],
        })],
    });
    registry
        .validate_typed(&value, &mut budget())
        .map_err(error)?;
    assert_eq!(registry.selected("test.a", 1), Some(&ids[0]));
    Ok(())
}

#[test]
fn missing_conflicting_duplicate_and_stopped_batches_fail() -> Result<(), String> {
    let base = bootstrap()?;
    let a = descriptor("test.a", reference("test.b"));
    let id = a.reference(&mut budget()).map_err(error)?;
    let bytes = nepl3_wire::schema::encode(&a, &base, &mut budget()).map_err(error)?;
    assert!(matches!(
        admit(bootstrap()?, core::slice::from_ref(&id), &[], &mut budget()),
        Err(SchemaAdmissionError::Count)
    ));
    // The descriptor is valid in isolation but its referenced package is absent.
    assert!(matches!(
        admit(
            bootstrap()?,
            core::slice::from_ref(&id),
            &[&bytes],
            &mut budget()
        ),
        Err(SchemaAdmissionError::Schema(_))
    ));
    assert!(matches!(
        admit(
            bootstrap()?,
            &[id.clone(), id.clone()],
            &[&bytes, &bytes],
            &mut budget()
        ),
        Err(SchemaAdmissionError::DuplicateSelection)
    ));
    let different = descriptor("test.a", TypeDescriptor::Bool)
        .reference(&mut budget())
        .map_err(error)?;
    assert!(matches!(
        admit(bootstrap()?, &[different], &[&bytes], &mut budget()),
        Err(SchemaAdmissionError::Wire(_))
    ));
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        admit(
            bootstrap()?,
            core::slice::from_ref(&id),
            &[&bytes],
            &mut stopped
        ),
        Err(SchemaAdmissionError::Stopped(StopReason::WorkLimit))
    ));
    let mut no_allocation = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    assert!(matches!(
        admit(
            bootstrap()?,
            core::slice::from_ref(&id),
            &[&bytes],
            &mut no_allocation
        ),
        Err(SchemaAdmissionError::Stopped(StopReason::AllocationLimit))
    ));
    assert!(matches!(
        admit(SchemaRegistry::default(), &[], &[], &mut budget()),
        Err(SchemaAdmissionError::Schema(SchemaError::Unfinalized))
    ));
    // A failed batch cannot mutate a separately retained active registry.
    assert!(base.is_finalized());
    assert!(base.selected("test.a", 1).is_none());
    Ok(())
}
