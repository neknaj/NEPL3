use nepl3_core::{budget::*, schema::*, source::Digest, value::*};
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
fn fixture(
    padding: [usize; 3],
    variant: bool,
) -> Result<(SchemaRegistry, SchemaRef, NdfValue), SchemaError> {
    let mut registry = SchemaRegistry::default();
    for i in 0..padding[0] {
        let d = SchemaDescriptor {
            package: format!("padding.{i}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        registry.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    let mut types: Vec<_> = (0..padding[1])
        .map(|i| NamedType {
            name: format!("A{i:02}"),
            shape: TypeShape::Record { fields: vec![] },
            constraints: vec![],
        })
        .collect();
    let shape = if variant {
        let mut variants: Vec<_> = (0..padding[2])
            .map(|i| VariantDescriptor {
                name: format!("A{i:02}"),
                fields: vec![],
            })
            .collect();
        variants.push(VariantDescriptor {
            name: "ZCase".into(),
            fields: vec![],
        });
        variants.reverse(); // Registration must normalize caller order.
        TypeShape::Variant { variants }
    } else {
        TypeShape::Record { fields: vec![] }
    };
    types.push(NamedType {
        name: "ZTarget".into(),
        shape,
        constraints: vec![],
    });
    types.reverse(); // Exercise the registry's normalization, not pre-sorted input.
    let d = SchemaDescriptor {
        package: "target".into(),
        revision: 1,
        types,
        operations: vec![],
    };
    let schema = d.reference(&mut budget())?;
    registry.register(schema.clone(), d, &mut budget())?;
    registry.finalize(&mut budget())?;
    let value = if variant {
        NdfValue::Variant(Variant {
            schema: schema.clone(),
            type_name: "ZTarget".into(),
            variant: "ZCase".into(),
            fields: vec![],
        })
    } else {
        NdfValue::Record(Record {
            schema: schema.clone(),
            kind: "ZTarget".into(),
            fields: vec![],
        })
    };
    Ok((registry, schema, value))
}
fn expectations() -> [TypeDescriptor; 3] {
    [
        TypeDescriptor::Named(TypeRef {
            package: "target".into(),
            revision: 1,
            name: "ZTarget".into(),
        }),
        TypeDescriptor::NdfValue,
        TypeDescriptor::TypedValue,
    ]
}
#[test]
fn structural_lookup_charges_schema_type_and_variant_candidates() -> Result<(), SchemaError> {
    for variant in [false, true] {
        let (small, _, value) = fixture([0, 0, 0], variant)?;
        for axis in 0..if variant { 3 } else { 2 } {
            let mut padding = [0, 0, 0];
            padding[axis] = 32;
            let (large, _, large_value) = fixture(padding, variant)?;
            // The last entry among 33 sorted names is reached after four
            // padding comparisons and the target comparison (baseline: one).
            let expected_delta: u64 = match axis {
                0 => (0..32)
                    .map(|i| format!("padding.{i}").len() as u64 + 6 + 9)
                    .sum(),
                1 => 4 * (3 + 7 + 1),
                _ => 4 * (3 + 5 + 1),
            };
            for expected in expectations() {
                let mut baseline = budget();
                small.validate(&expected, &value, &mut baseline)?;
                let mut full = budget();
                large.validate(&expected, &large_value, &mut full)?;
                assert_eq!(
                    full.usage().work - baseline.usage().work,
                    expected_delta,
                    "variant={variant} axis={axis} expected={expected:?}"
                );
                assert_eq!(
                    full.usage(),
                    Usage {
                        work: full.usage().work,
                        nodes: 1,
                        depth: 1,
                        ..Usage::default()
                    }
                );
                let mut limited = Budget::new(Limits {
                    work: 1,
                    ..budget().limits()
                });
                assert!(matches!(
                    large.validate(&expected, &large_value, &mut limited),
                    Err(SchemaError::Stopped(StopReason::WorkLimit))
                ));
                assert_eq!(
                    limited.usage(),
                    Usage {
                        work: 1,
                        nodes: 1,
                        depth: 1,
                        ..Usage::default()
                    }
                );
                assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
                let mut limited = Budget::new(Limits {
                    work: full.usage().work - 1,
                    ..budget().limits()
                });
                assert!(matches!(
                    large.validate(&expected, &large_value, &mut limited),
                    Err(SchemaError::Stopped(StopReason::WorkLimit))
                ));
                assert_eq!(limited.usage().allocation_units, 0);
                let mut cancelled = budget();
                cancelled.charge(Resource::Work, 7)?;
                cancelled.cancel();
                let before = cancelled.usage();
                assert!(matches!(
                    large.validate(&expected, &large_value, &mut cancelled),
                    Err(SchemaError::Stopped(StopReason::Cancelled))
                ));
                assert_eq!(cancelled.usage(), before);
            }
        }
    }
    Ok(())
}
#[test]
fn structural_lookup_keeps_named_and_dynamic_identity_errors_distinct() -> Result<(), SchemaError> {
    let (registry, _, value) = fixture([0, 0, 0], true)?;
    for (index, expected) in expectations().into_iter().enumerate() {
        for case in 0..5 {
            let mut changed = value.clone();
            let NdfValue::Variant(v) = &mut changed else {
                return Err(SchemaError::WrongType);
            };
            let error = match case {
                0 => {
                    v.schema.digest = Digest::of(b"wrong");
                    if index == 0 {
                        SchemaError::WrongType
                    } else {
                        SchemaError::UnknownSchema
                    }
                }
                1 => {
                    v.type_name = "Absent".into();
                    if index == 0 {
                        SchemaError::WrongType
                    } else {
                        SchemaError::UnknownType
                    }
                }
                2 => {
                    v.variant = "Absent".into();
                    SchemaError::UnknownVariant
                }
                3 => {
                    v.fields.push(NdfValue::Unit);
                    SchemaError::FieldCount
                }
                _ => {
                    v.schema.revision = 2;
                    if index == 0 {
                        SchemaError::WrongType
                    } else {
                        SchemaError::UnknownSchema
                    }
                }
            };
            assert!(
                matches!(registry.validate(&expected,&changed,&mut budget()),Err(e) if e==error)
            );
        }
    }
    for (name, error) in [
        (
            TypeRef {
                package: "absent".into(),
                revision: 1,
                name: "ZTarget".into(),
            },
            SchemaError::UnknownSchema,
        ),
        (
            TypeRef {
                package: "target".into(),
                revision: 1,
                name: "Absent".into(),
            },
            SchemaError::UnknownType,
        ),
    ] {
        assert!(
            matches!(registry.validate(&TypeDescriptor::Named(name),&value,&mut budget()),Err(e) if e==error)
        );
    }
    Ok(())
}

// Fixture conversion happens outside the measured validation Budget.
fn typed(value: NdfValue) -> Result<TypedValue, SchemaError> {
    match &value {
        NdfValue::Record(v) => Ok(TypedValue::Record(v.clone())),
        NdfValue::Variant(v) => Ok(TypedValue::Variant(v.clone())),
        _ => Err(SchemaError::WrongType),
    }
}

#[test]
fn borrowed_lookup_meters_candidates_without_cloning_the_payload() -> Result<(), SchemaError> {
    for variant in [false, true] {
        let (small, _, value) = fixture([0, 0, 0], variant)?;
        let value = typed(value)?;
        for axis in 0..if variant { 3 } else { 2 } {
            let mut padding = [0, 0, 0];
            padding[axis] = 32;
            let (large, _, large_value) = fixture(padding, variant)?;
            let large_value = typed(large_value)?;
            let delta: u64 = match axis {
                0 => (0..32)
                    .map(|i| format!("padding.{i}").len() as u64 + 6 + 9)
                    .sum(),
                1 => 4 * (3 + 7 + 1),
                _ => 4 * (3 + 5 + 1),
            };
            let mut baseline = budget();
            small.validate_typed(&value, &mut baseline)?;
            let mut full = budget();
            large.validate_typed(&large_value, &mut full)?;
            assert_eq!(full.usage().work - baseline.usage().work, delta);
            assert_eq!(
                full.usage(),
                Usage {
                    work: full.usage().work,
                    nodes: 1,
                    depth: 1,
                    ..Usage::default()
                }
            );
            for (index, expected) in expectations().into_iter().enumerate() {
                let mut a = budget();
                small.validate_typed_as(&expected, &value, &mut a)?;
                let mut b = budget();
                large.validate_typed_as(&expected, &large_value, &mut b)?;
                // Named expected types are independently resolved before checking
                // the borrowed actual payload; only actual validation scans variants.
                let multiplier = if index == 0 && axis < 2 { 2 } else { 1 };
                assert_eq!(b.usage().work - a.usage().work, delta * multiplier);
                assert_eq!(b.usage().allocation_units, 0);
            }
            let mut low = Budget::new(Limits {
                work: 1,
                ..budget().limits()
            });
            assert_eq!(
                large.validate_typed(&large_value, &mut low),
                Err(SchemaError::Stopped(StopReason::WorkLimit))
            );
            assert_eq!(
                low.usage(),
                Usage {
                    work: 1,
                    ..Usage::default()
                }
            );
            assert_eq!(low.poll(), Err(StopReason::WorkLimit));
            let mut parent = budget();
            parent.with_depth(|b| large.validate_typed(&large_value, b))?;
            assert_eq!(parent.usage().depth, 2);
        }
    }
    Ok(())
}
#[test]
fn borrowed_lookup_preserves_error_and_stop_precedence() -> Result<(), SchemaError> {
    let (registry, _, value) = fixture([0, 0, 0], true)?;
    let value = typed(value)?;
    for case in 0..5 {
        let mut invalid = value.clone();
        let TypedValue::Variant(v) = &mut invalid else {
            return Err(SchemaError::WrongType);
        };
        let expected = match case {
            0 => {
                v.schema.digest = Digest::of(b"wrong");
                SchemaError::UnknownSchema
            }
            1 => {
                v.type_name = "Absent".into();
                SchemaError::UnknownType
            }
            2 => {
                v.variant = "Absent".into();
                SchemaError::UnknownVariant
            }
            3 => {
                v.fields.push(NdfValue::Unit);
                SchemaError::FieldCount
            }
            _ => {
                v.schema.revision = 2;
                SchemaError::UnknownSchema
            }
        };
        assert_eq!(
            registry.validate_typed(&invalid, &mut budget()),
            Err(expected)
        );
        let mut low = Budget::new(Limits {
            work: 0,
            ..budget().limits()
        });
        assert_eq!(
            registry.validate_typed(&invalid, &mut low),
            Err(SchemaError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(low.usage(), Usage::default());
        let mut cancelled = budget();
        cancelled.charge(Resource::Work, 7)?;
        cancelled.cancel();
        let before = cancelled.usage();
        assert_eq!(
            registry.validate_typed(&invalid, &mut cancelled),
            Err(SchemaError::Stopped(StopReason::Cancelled))
        );
        assert_eq!(cancelled.usage(), before);
        assert_eq!(
            SchemaRegistry::default().validate_typed(&invalid, &mut cancelled),
            Err(SchemaError::Unfinalized)
        );
    }
    let mut shallow = Budget::new(Limits {
        depth: 1,
        ..budget().limits()
    });
    assert_eq!(
        shallow.with_depth(|b| registry.validate_typed(&value, b)),
        Err(SchemaError::Stopped(StopReason::DepthLimit))
    );
    Ok(())
}

#[test]
fn normalized_lookup_finds_first_middle_last_and_absent_names() -> Result<(), SchemaError> {
    let (registry, schema, _) = fixture([32, 32, 32], true)?;
    for index in 0..32 {
        let name = format!("A{index:02}");
        let expected = TypeDescriptor::Named(TypeRef {
            package: "target".into(),
            revision: 1,
            name: name.clone(),
        });
        let record = NdfValue::Record(Record {
            schema: schema.clone(),
            kind: name.clone(),
            fields: vec![],
        });
        let payload = typed(record.clone())?;
        let mut b = Budget::new(Limits {
            work: 2500,
            ..budget().limits()
        });
        registry.validate_typed_as(&expected, &payload, &mut b)?;
        registry.validate(&expected, &record, &mut budget())?;
        let variant = NdfValue::Variant(Variant {
            schema: schema.clone(),
            type_name: "ZTarget".into(),
            variant: name,
            fields: vec![],
        });
        registry.validate(&TypeDescriptor::TypedValue, &variant, &mut budget())?;
        registry.validate_typed(&typed(variant)?, &mut budget())?;
    }
    for name in ["", "A", "A00x", "A15x", "A99", "ZZ", "あ"] {
        let value = NdfValue::Record(Record {
            schema: schema.clone(),
            kind: name.into(),
            fields: vec![],
        });
        assert!(matches!(
            registry.validate(&TypeDescriptor::TypedValue, &value, &mut budget()),
            Err(SchemaError::UnknownType)
        ));
        assert_eq!(
            registry.validate_typed(&typed(value)?, &mut budget()),
            Err(SchemaError::UnknownType)
        );
        let value = NdfValue::Variant(Variant {
            schema: schema.clone(),
            type_name: "ZTarget".into(),
            variant: name.into(),
            fields: vec![],
        });
        assert!(matches!(
            registry.validate(&TypeDescriptor::TypedValue, &value, &mut budget()),
            Err(SchemaError::UnknownVariant)
        ));
    }
    Ok(())
}
