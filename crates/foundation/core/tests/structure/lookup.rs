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
                1 => 4 * (1 + 3),
                _ => 4 * (1 + 3),
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
                1 => 4 * (1 + 3),
                _ => 4 * (1 + 3),
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

#[test]
fn long_utf8_lookup_has_calculated_exact_and_one_short_boundaries() -> Result<(), SchemaError> {
    let package = "所有者".repeat(128);
    let name = "型🦀".repeat(128);
    let case = "場合λ".repeat(128);
    for variant in [false, true] {
        let descriptor = SchemaDescriptor {
            package: package.clone(),
            revision: 1,
            types: vec![NamedType {
                name: name.clone(),
                constraints: vec![],
                shape: if variant {
                    TypeShape::Variant {
                        variants: vec![VariantDescriptor {
                            name: case.clone(),
                            fields: vec![],
                        }],
                    }
                } else {
                    TypeShape::Record { fields: vec![] }
                },
            }],
            operations: vec![],
        };
        let mut registry = SchemaRegistry::default();
        let schema = descriptor.reference(&mut budget())?;
        registry.register(schema.clone(), descriptor, &mut budget())?;
        registry.finalize(&mut budget())?;
        let expected = TypeDescriptor::Named(TypeRef {
            package: package.clone(),
            revision: 1,
            name: name.clone(),
        });
        let value = if variant {
            NdfValue::Variant(Variant {
                schema: schema.clone(),
                type_name: name.clone(),
                variant: case.clone(),
                fields: vec![],
            })
        } else {
            NdfValue::Record(Record {
                schema: schema.clone(),
                kind: name.clone(),
                fields: vec![],
            })
        };
        let payload = typed(value.clone())?;
        let p = package.len() as u64;
        let t = name.len() as u64;
        let v = if variant {
            3 * case.len() as u64 + 1
        } else {
            0
        };
        // One schema candidate, one type candidate, one optional variant.
        // Counts are derived from UTF-8 byte lengths, not a measured baseline.
        let selected = 1 + 2 * p + 9;
        let definition = 3 * t + 1;
        let type_work = 1 + selected + definition;
        let dynamic_work = 1 + selected + p + 41 + definition + v;
        let named_work = 1 + selected + definition + 1 + 2 * p + 2 * t + 41 + v;
        let as_work = type_work + 2 * p + 2 * t + 1 + dynamic_work;
        for (route, work) in [
            (0, dynamic_work),
            (1, named_work),
            (2, dynamic_work),
            (3, type_work),
            (4, as_work),
        ] {
            for short in [false, true] {
                let mut b = Budget::new(Limits {
                    work: work - u64::from(short),
                    ..budget().limits()
                });
                let result = match route {
                    0 => registry
                        .validate(&TypeDescriptor::TypedValue, &value, &mut b)
                        .map(|_| ()),
                    1 => registry.validate(&expected, &value, &mut b).map(|_| ()),
                    2 => registry.validate_typed(&payload, &mut b),
                    3 => registry.validate_type(&expected, &mut b),
                    _ => registry.validate_typed_as(&expected, &payload, &mut b),
                };
                if short {
                    assert_eq!(
                        result,
                        Err(SchemaError::Stopped(StopReason::WorkLimit)),
                        "route={route}"
                    );
                    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
                } else {
                    result?;
                    assert_eq!(
                        b.usage(),
                        Usage {
                            work,
                            nodes: u64::from(route != 3),
                            depth: 1,
                            ..Usage::default()
                        }
                    );
                }
                assert_eq!(b.usage().allocation_units, 0);
            }
        }
        // Actual identity comparison must be admitted before reporting WrongType.
        for change_package in [false, true] {
            let mut invalid = value.clone();
            let (actual_schema, actual_name) = match &mut invalid {
                NdfValue::Record(v) => (&mut v.schema, &mut v.kind),
                NdfValue::Variant(v) => (&mut v.schema, &mut v.type_name),
                _ => return Err(SchemaError::WrongType),
            };
            if change_package {
                actual_schema.package.push('界');
            } else {
                actual_name.push('界');
            }
            let identity =
                p + actual_schema.package.len() as u64 + t + actual_name.len() as u64 + 41;
            let before_identity = 1 + selected + definition + 1;
            let mut short = Budget::new(Limits {
                work: before_identity + identity - 1,
                ..budget().limits()
            });
            assert_eq!(
                registry.validate(&expected, &invalid, &mut short).err(),
                Some(SchemaError::Stopped(StopReason::WorkLimit))
            );
            assert_eq!(short.usage().work, before_identity);
            let mut exact = Budget::new(Limits {
                work: before_identity + identity,
                ..budget().limits()
            });
            assert_eq!(
                registry.validate(&expected, &invalid, &mut exact).err(),
                Some(SchemaError::WrongType)
            );
            assert_eq!(exact.usage().work, before_identity + identity);
        }
    }
    Ok(())
}

#[test]
fn lookup_preserves_every_prior_stop_and_unfinalized_precedence() -> Result<(), SchemaError> {
    let (registry, _, value) = fixture([0, 0, 0], true)?;
    let payload = typed(value.clone())?;
    let expected = expectations()[0].clone();
    for reason in [
        StopReason::Cancelled,
        StopReason::SourceLimit,
        StopReason::WorkLimit,
        StopReason::DepthLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
        StopReason::DiagnosticLimit,
        StopReason::EventLimit,
    ] {
        for route in 0..4 {
            let mut b = budget();
            b.charge(Resource::Work, 7)?;
            b.stop(reason);
            let before = b.usage();
            let call = |r: &SchemaRegistry, b: &mut Budget| match route {
                0 => r.validate(&expected, &value, b).map(|_| ()),
                1 => r.validate_typed(&payload, b),
                2 => r.validate_type(&expected, b),
                _ => r.validate_typed_as(&expected, &payload, b),
            };
            assert_eq!(call(&registry, &mut b), Err(SchemaError::Stopped(reason)));
            assert_eq!(b.usage(), before);
            assert_eq!(
                call(&SchemaRegistry::default(), &mut b),
                Err(if route == 3 {
                    SchemaError::Stopped(reason)
                } else {
                    SchemaError::Unfinalized
                })
            );
            assert_eq!(b.usage(), before);
        }
    }
    Ok(())
}

#[test]
fn empty_catalogs_and_wrong_shapes_keep_lookup_error_order() -> Result<(), SchemaError> {
    let (populated, schema, original) = fixture([0, 0, 0], true)?;
    let expected = expectations()[0].clone();
    let mut empty = SchemaRegistry::default();
    empty.finalize(&mut budget())?;
    assert_eq!(
        empty.validate_type(&expected, &mut budget()),
        Err(SchemaError::UnknownSchema)
    );
    assert_eq!(
        empty.validate_typed(&typed(original.clone())?, &mut budget()),
        Err(SchemaError::UnknownSchema)
    );
    for empty_types in [false, true] {
        let d = SchemaDescriptor {
            package: "target".into(),
            revision: 1,
            types: if empty_types {
                vec![]
            } else {
                vec![NamedType {
                    name: "ZTarget".into(),
                    constraints: vec![],
                    shape: TypeShape::Variant { variants: vec![] },
                }]
            },
            operations: vec![],
        };
        let mut r = SchemaRegistry::default();
        let sr = d.reference(&mut budget())?;
        r.register(sr.clone(), d, &mut budget())?;
        r.finalize(&mut budget())?;
        let value = NdfValue::Variant(Variant {
            schema: sr.clone(),
            type_name: "ZTarget".into(),
            variant: "Absent".into(),
            fields: vec![NdfValue::Unit],
        });
        let error = if empty_types {
            SchemaError::UnknownType
        } else {
            SchemaError::UnknownVariant
        };
        for expected in expectations() {
            assert_eq!(
                r.validate(&expected, &value, &mut budget()).err(),
                Some(error.clone())
            );
        }
        assert_eq!(r.validate_typed(&typed(value)?, &mut budget()), Err(error));
        let record = NdfValue::Record(Record {
            schema: sr,
            kind: "ZTarget".into(),
            fields: vec![],
        });
        assert_eq!(
            r.validate(&expected, &record, &mut budget()).err(),
            Some(if empty_types {
                SchemaError::UnknownType
            } else {
                SchemaError::WrongType
            })
        );
    }
    // Actual identity wins over absent variants and field count for Named.
    let invalid = NdfValue::Variant(Variant {
        schema: SchemaRef {
            digest: Digest::of(b"wrong"),
            ..schema
        },
        type_name: "Absent".into(),
        variant: "Absent".into(),
        fields: vec![NdfValue::Unit],
    });
    assert_eq!(
        populated.validate(&expected, &invalid, &mut budget()).err(),
        Some(SchemaError::WrongType)
    );
    assert_eq!(
        populated
            .validate(&TypeDescriptor::NdfValue, &invalid, &mut budget())
            .err(),
        Some(SchemaError::UnknownSchema)
    );
    assert_eq!(
        populated.validate_typed(&typed(invalid)?, &mut budget()),
        Err(SchemaError::UnknownSchema)
    );
    Ok(())
}

#[test]
fn repeated_expected_resolution_never_caches_actual_validation() -> Result<(), SchemaError> {
    let (r, _, good) = fixture([32, 32, 32], true)?;
    let expected = TypeDescriptor::List(Box::new(expectations()[0].clone()));
    let mut one = budget();
    r.validate(&expected, &NdfValue::List(vec![good.clone()]), &mut one)?;
    let mut two = budget();
    r.validate(
        &expected,
        &NdfValue::List(vec![good.clone(), good.clone()]),
        &mut two,
    )?;
    // A repeated expected ref costs one cache comparison; actual identity and
    // variant lookup still run for the second value. Four first-byte misses
    // and the exact five-byte ZCase match comprise the variant lookup.
    let second = 1 + 1 + (6 + 6 + 7 + 7 + 41) + (4 * 4 + 1 + 3 * 5);
    assert_eq!(two.usage().work - one.usage().work, second);
    assert_eq!(two.usage().nodes - one.usage().nodes, 1);
    assert_eq!(two.usage().allocation_units, 0);
    for case in 0..4 {
        let mut bad = good.clone();
        let NdfValue::Variant(v) = &mut bad else {
            return Err(SchemaError::WrongType);
        };
        let error = match case {
            0 => {
                v.schema.digest = Digest::of(b"different");
                SchemaError::WrongType
            }
            1 => {
                v.type_name = "Different".into();
                SchemaError::WrongType
            }
            2 => {
                v.variant = "Absent".into();
                SchemaError::UnknownVariant
            }
            _ => {
                v.fields.push(NdfValue::Unit);
                SchemaError::FieldCount
            }
        };
        assert_eq!(
            r.validate(
                &expected,
                &NdfValue::List(vec![good.clone(), bad]),
                &mut budget()
            )
            .err(),
            Some(error)
        );
    }
    let mut short = Budget::new(Limits {
        work: two.usage().work - 1,
        ..budget().limits()
    });
    assert_eq!(
        r.validate(
            &expected,
            &NdfValue::List(vec![good.clone(), good]),
            &mut short
        )
        .err(),
        Some(SchemaError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(short.poll(), Err(StopReason::WorkLimit));
    Ok(())
}
