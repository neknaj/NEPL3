use nepl3_core::{
    budget::*,
    diagnostic::{OperationResult, Report},
    operation::{
        Invoke,
        request::InputValidationError,
        validation::{ResultValidationError, ValidatedResult},
    },
    schema::*,
    source::SourceStore,
    value::*,
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        nodes: 1000,
        depth: 100,
        source_bytes: 1000,
        output_bytes: 1000,
        diagnostics: 10,
        events: 10,
    })
}
fn fixture(padding: usize) -> Result<(SchemaRegistry, Invoke), SchemaError> {
    let mut registry = SchemaRegistry::default();
    let base = SchemaDescriptor {
        package: "base".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Value".into(),
            shape: TypeShape::Record { fields: vec![] },
            constraints: vec![],
        }],
        operations: vec![],
    };
    let base_ref = base.reference(&mut budget())?;
    registry.register(base_ref.clone(), base, &mut budget())?;
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: format!("padding.{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        registry.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    let ty = TypeDescriptor::Named(TypeRef {
        package: "base".into(),
        revision: 1,
        name: "Value".into(),
    });
    let owner = SchemaDescriptor {
        package: "owner".into(),
        revision: 1,
        types: vec![],
        operations: vec![OperationDescriptor {
            name: "run".into(),
            input: ty.clone(),
            output: ty,
            pure: true,
        }],
    };
    let schema = owner.reference(&mut budget())?;
    registry.register(schema.clone(), owner, &mut budget())?;
    registry.finalize(&mut budget())?;
    let value = TypedValue::Record(Record {
        schema: base_ref,
        kind: "Value".into(),
        fields: vec![],
    });
    Ok((
        registry,
        Invoke {
            request_id: 0,
            operation: OperationRef {
                schema,
                name: "run".into(),
            },
            input: value.clone(),
            environment: value,
            sources: vec![],
            resources: vec![],
            limits: budget().limits(),
        },
    ))
}
#[test]
fn operation_admission_meters_only_owner_catalog_growth() -> Result<(), String> {
    let error = |e| format!("{e:?}");
    for case in 0..3 {
        let mut usages = vec![];
        for padding in [0, 32] {
            let (registry, request) = fixture(padding).map_err(error)?;
            let mut b = budget();
            let sources = SourceStore::default();
            let result: OperationResult<TypedValue> = OperationResult::Invalid {
                partial: None,
                report: Report::default(),
            };
            match case {
                0 => request
                    .validate_input(&request.operation, &registry, &mut b)
                    .map_err(|e| format!("{e:?}"))?,
                1 => result
                    .validate_for(&request.operation, &registry, &sources, &mut b)
                    .map_err(|e| format!("{e:?}"))?,
                _ => {
                    ValidatedResult::new(result, &request.operation, &registry, &sources, &mut b)
                        .map_err(|e| format!("{e:?}"))?;
                }
            }
            usages.push(b.usage());
        }
        let mut expected = usages[0];
        expected.work += 32 * (10 + 5 + 9);
        assert_eq!(usages[1], expected, "case {case}");
    }
    Ok(())
}

#[test]
fn operation_schema_identity_stops_and_failed_proofs_remain_explicit() -> Result<(), SchemaError> {
    let (registry, request) = fixture(32)?;
    let sources = SourceStore::default();
    for mutation in 0..4 {
        let mut request = request.clone();
        match mutation {
            0 => request.operation.schema.package = "missing".into(),
            1 => request.operation.schema.revision += 1,
            2 => request.operation.schema.digest.0[0] ^= 1,
            _ => request.operation.name = "missing".into(),
        }
        let result: OperationResult<TypedValue> = OperationResult::Invalid {
            partial: None,
            report: Report::default(),
        };
        let input_error = if mutation == 3 {
            InputValidationError::UnknownOperation
        } else {
            InputValidationError::Schema(SchemaError::UnknownSchema)
        };
        let output_error = if mutation == 3 {
            ResultValidationError::UnknownOperation
        } else {
            ResultValidationError::Schema(SchemaError::UnknownSchema)
        };
        assert_eq!(
            request.validate_input(&request.operation, &registry, &mut budget()),
            Err(input_error)
        );
        assert_eq!(
            result.validate_for(&request.operation, &registry, &sources, &mut budget()),
            Err(output_error.clone())
        );
        assert!(
            matches!(ValidatedResult::new(result, &request.operation, &registry, &sources, &mut budget()), Err(e) if e == output_error)
        );
    }
    for case in 0..3 {
        let mut limits = budget().limits();
        // Covers the request's fixed equality charge, but not the padded scan.
        limits.work = 200;
        let mut stopped = Budget::new(limits);
        let result: OperationResult<TypedValue> = OperationResult::Invalid {
            partial: None,
            report: Report::default(),
        };
        match case {
            0 => assert_eq!(
                request.validate_input(&request.operation, &registry, &mut stopped),
                Err(InputValidationError::Stopped(StopReason::WorkLimit))
            ),
            1 => assert_eq!(
                result.validate_for(&request.operation, &registry, &sources, &mut stopped),
                Err(ResultValidationError::Stopped(StopReason::WorkLimit))
            ),
            _ => assert!(matches!(
                ValidatedResult::new(
                    result,
                    &request.operation,
                    &registry,
                    &sources,
                    &mut stopped
                ),
                Err(ResultValidationError::Stopped(StopReason::WorkLimit))
            )),
        }
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    }
    Ok(())
}
