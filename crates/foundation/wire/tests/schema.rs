use nepl3_core::{budget::*, schema::*, source::Digest, value::*};
use nepl3_wire::{WireError, schema};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 256,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 10,
        events: 10,
    })
}
fn error(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let descriptor = foundation::descriptor(&mut budget()).map_err(error)?;
    let mut registry = SchemaRegistry::default();
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
fn sample() -> SchemaDescriptor {
    SchemaDescriptor {
        package: "example.世界".into(),
        revision: 7,
        types: vec![
            NamedType {
                name: "Payload".into(),
                shape: TypeShape::Record {
                    fields: vec![
                        FieldDescriptor {
                            name: "second".into(),
                            ty: TypeDescriptor::Text,
                        },
                        FieldDescriptor {
                            name: "first".into(),
                            ty: TypeDescriptor::List(Box::new(TypeDescriptor::Option(Box::new(
                                TypeDescriptor::U64,
                            )))),
                        },
                    ],
                },
                constraints: vec!["alloc::Marker".into(), "crate::budget::".into()],
            },
            NamedType {
                name: "Choice".into(),
                shape: TypeShape::Variant {
                    variants: vec![
                        VariantDescriptor {
                            name: "Empty".into(),
                            fields: vec![],
                        },
                        VariantDescriptor {
                            name: "Some".into(),
                            fields: vec![FieldDescriptor {
                                name: "payload".into(),
                                ty: TypeDescriptor::Named(TypeRef {
                                    package: "example.世界".into(),
                                    revision: 7,
                                    name: "Payload".into(),
                                }),
                            }],
                        },
                    ],
                },
                constraints: vec![],
            },
        ],
        operations: vec![OperationDescriptor {
            name: "observe".into(),
            input: TypeDescriptor::Text,
            output: TypeDescriptor::U64,
            pure: true,
        }],
    }
}

#[test]
fn descriptors_preserve_fields_constraints_operations_and_identity() -> Result<(), String> {
    let registry = registry()?;
    for descriptor in [
        sample(),
        foundation::descriptor(&mut budget()).map_err(error)?,
    ] {
        let identity = descriptor.reference(&mut budget()).map_err(error)?;
        let bytes = schema::encode(&descriptor, &registry, &mut budget()).map_err(error)?;
        let decoded = schema::decode(&bytes, &identity, &registry, &mut budget()).map_err(error)?;
        assert_eq!(decoded, descriptor);
        assert_eq!(decoded.reference(&mut budget()).map_err(error)?, identity);
        let raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
        let NdfValue::Record(record) = &raw else {
            return Err("record".into());
        };
        assert_eq!(record.kind, "SchemaDescriptor");
        assert_eq!(record.fields.len(), 4);
        assert_eq!(record.fields[0], NdfValue::Text(descriptor.package.clone()));
        assert_eq!(record.fields[1], NdfValue::U64(descriptor.revision));
        // Input and output differ so a symmetric codec swap is observable.
        if descriptor.package == "example.世界" {
            let NdfValue::List(ops) = &record.fields[3] else {
                return Err("operations".into());
            };
            let NdfValue::Record(op) = &ops[0] else {
                return Err("operation".into());
            };
            for (index, name) in [(1, "Text"), (2, "U64")] {
                let NdfValue::Variant(ty) = &op.fields[index] else {
                    return Err("type".into());
                };
                assert_eq!(ty.variant, name);
            }
            assert_eq!(op.fields[3], NdfValue::Bool(true));
        }
    }
    Ok(())
}

#[test]
fn descriptors_reject_identity_mismatch_duplicates_and_stops() -> Result<(), String> {
    let registry = registry()?;
    let descriptor = sample();
    let identity = descriptor.reference(&mut budget()).map_err(error)?;
    let bytes = schema::encode(&descriptor, &registry, &mut budget()).map_err(error)?;
    for bad in [
        SchemaRef {
            package: "other".into(),
            ..identity.clone()
        },
        SchemaRef {
            revision: 8,
            ..identity.clone()
        },
        SchemaRef {
            digest: Digest::of(b"wrong"),
            ..identity.clone()
        },
    ] {
        assert!(matches!(
            schema::decode(&bytes, &bad, &registry, &mut budget()),
            Err(WireError::Schema(SchemaError::IdentityMismatch))
        ));
    }
    let mut duplicate = descriptor.clone();
    duplicate.types.push(duplicate.types[0].clone());
    assert!(matches!(
        schema::encode(&duplicate, &registry, &mut budget()),
        Err(WireError::Schema(SchemaError::DuplicateName))
    ));
    let mut raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
    let NdfValue::Record(record) = &mut raw else {
        return Err("record".into());
    };
    let NdfValue::List(types) = &mut record.fields[2] else {
        return Err("types".into());
    };
    types.push(types[0].clone());
    let forged = nepl3_wire::encode(&raw, &mut budget()).map_err(error)?;
    assert!(matches!(
        schema::decode(&forged, &identity, &registry, &mut budget()),
        Err(WireError::Schema(SchemaError::DuplicateName))
    ));
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(schema::decode(&bytes, &identity, &registry, &mut stopped).is_err());
    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    assert!(schema::encode(&descriptor, &SchemaRegistry::default(), &mut budget()).is_err());
    Ok(())
}

#[test]
fn descriptor_receipt_requires_dependencies_before_use() -> Result<(), String> {
    let registry = registry()?;
    let mut descriptor = sample();
    descriptor.operations[0].input = TypeDescriptor::Named(TypeRef {
        package: "missing".into(),
        revision: 1,
        name: "Input".into(),
    });
    let identity = descriptor.reference(&mut budget()).map_err(error)?;
    let bytes = schema::encode(&descriptor, &registry, &mut budget()).map_err(error)?;
    let decoded = schema::decode(&bytes, &identity, &registry, &mut budget()).map_err(error)?;
    let mut destination = SchemaRegistry::default();
    destination
        .register(identity, decoded, &mut budget())
        .map_err(error)?;
    assert!(destination.finalize(&mut budget()).is_err());
    Ok(())
}

#[test]
fn descriptor_boundaries_reject_excess_depth_schema_forgery_and_resource_stops()
-> Result<(), String> {
    let registry = registry()?;
    let descriptor = sample();
    let identity = descriptor.reference(&mut budget()).map_err(error)?;
    let bytes = schema::encode(&descriptor, &registry, &mut budget()).map_err(error)?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::DepthLimit => limits.depth = 0,
            _ => return Err("unexpected resource".into()),
        }
        for encode in [false, true] {
            let mut stopped = Budget::new(limits);
            if encode {
                assert!(schema::encode(&descriptor, &registry, &mut stopped).is_err());
            } else {
                assert!(schema::decode(&bytes, &identity, &registry, &mut stopped).is_err());
            }
            assert_eq!(stopped.poll(), Err(reason));
        }
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        schema::decode(&bytes, &identity, &registry, &mut cancelled),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    let mut raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
    let NdfValue::Record(record) = &mut raw else {
        return Err("record".into());
    };
    record.schema.digest = Digest::of(b"forged foundation");
    let forged = nepl3_wire::encode(&raw, &mut budget()).map_err(error)?;
    assert!(schema::decode(&forged, &identity, &registry, &mut budget()).is_err());

    // Spec13 canonical descriptor identity accepts depth 128, rejects 129.
    // Increase only this test's NDF depth allowance: symbolic wrappers occupy
    // multiple NDF nodes each. The production descriptor limit remains 128.
    let mut deep = sample();
    let mut ty = TypeDescriptor::U64;
    for _ in 0..128 {
        ty = TypeDescriptor::List(Box::new(ty));
    }
    deep.operations[0].input = ty;
    let large = || {
        Budget::new(Limits {
            depth: 1024,
            ..budget().limits()
        })
    };
    let identity = deep.reference(&mut large()).map_err(error)?;
    let bytes = schema::encode(&deep, &registry, &mut large()).map_err(error)?;
    assert_eq!(
        schema::decode(&bytes, &identity, &registry, &mut large()).map_err(error)?,
        deep
    );
    deep.operations[0].input = TypeDescriptor::List(Box::new(deep.operations[0].input.clone()));
    assert_eq!(
        schema::encode(&deep, &registry, &mut large()),
        Err(WireError::Schema(SchemaError::DescriptorDepth))
    );
    Ok(())
}
