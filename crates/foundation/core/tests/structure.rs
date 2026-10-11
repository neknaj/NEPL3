use nepl3_core::{budget::*, diagnostic::*, origin::*, schema::*, source::*, value::*};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 100,
        nodes: 1_000_000,
        allocation_units: 1_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}

#[test]
fn origin_append_reuses_shared_ancestry_heights() -> Result<(), OriginError> {
    let store = SourceStore::default();
    let mut graph = OriginGraph::default();
    let mut b = budget();
    let mut parent = graph.push(
        Origin::Synthetic {
            reason: "seed".into(),
            anchor: None,
        },
        &store,
        &mut b,
    )?;
    // Only 50 nodes and 98 edges, but expanding repeated parent paths would
    // visit exponentially many occurrences. Graph height is exactly 50.
    for _ in 1..50 {
        parent = graph.push(Origin::Composite(vec![parent, parent]), &store, &mut b)?;
    }
    assert_eq!(graph.origins().len(), 50);
    assert!(b.usage().work <= 200);
    let mut limits = budget().limits();
    limits.depth = 50;
    let mut stopped = Budget::new(limits);
    assert_eq!(
        graph.push(Origin::Composite(vec![parent]), &store, &mut stopped),
        Err(OriginError::Stopped(StopReason::DepthLimit))
    );
    assert_eq!(graph.origins().len(), 50);
    assert_eq!(
        graph.push(Origin::Composite(vec![]), &store, &mut stopped),
        Err(OriginError::Stopped(StopReason::DepthLimit))
    );
    Ok(())
}

#[test]
fn imported_origin_heights_survive_forward_references_and_failed_append() -> Result<(), OriginError>
{
    let store = SourceStore::default();
    let origins = vec![
        Origin::Composite(vec![OriginId(1)]),
        Origin::Synthetic {
            reason: "base".into(),
            anchor: None,
        },
    ];
    let mut graph = OriginGraph::from_origins(origins.clone(), &store, &mut budget())?;
    assert_eq!(
        graph.push(
            Origin::Composite(vec![OriginId(u64::MAX)]),
            &store,
            &mut budget()
        ),
        Err(OriginError::Reference)
    );
    for resource in 0..3 {
        let mut limits = budget().limits();
        match resource {
            0 => limits.depth = 2,
            1 => limits.nodes = 0,
            _ => limits.allocation_units = 0,
        }
        assert!(matches!(
            graph.push(
                Origin::Composite(vec![OriginId(0)]),
                &store,
                &mut Budget::new(limits)
            ),
            Err(OriginError::Stopped(_))
        ));
        assert_eq!(graph.origins(), origins);
    }
    let mut b = budget();
    assert_eq!(
        graph.push(Origin::Composite(vec![OriginId(0)]), &store, &mut b)?,
        OriginId(2)
    );
    assert_eq!(b.usage().depth, 3);
    assert_eq!(&graph.origins()[..2], origins);
    Ok(())
}
fn descriptor() -> SchemaDescriptor {
    SchemaDescriptor {
        package: "example".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Pair".into(),
            constraints: vec!["positive".into()],
            shape: TypeShape::Record {
                fields: vec![
                    FieldDescriptor {
                        name: "first".into(),
                        ty: TypeDescriptor::Natural,
                    },
                    FieldDescriptor {
                        name: "second".into(),
                        ty: TypeDescriptor::Text,
                    },
                ],
            },
        }],
        operations: vec![],
    }
}
fn registry() -> Result<(SchemaRegistry, SchemaRef), SchemaError> {
    let mut registry = SchemaRegistry::default();
    let descriptor = descriptor();
    let reference = descriptor.reference(&mut budget())?;
    registry.register(reference.clone(), descriptor, &mut budget())?;
    registry.finalize(&mut budget())?;
    Ok((registry, reference))
}

#[test]
fn validation_reuses_frontier_storage_but_visits_every_value() -> Result<(), SchemaError> {
    let (registry, _) = registry()?;
    let ty = TypeDescriptor::List(Box::new(TypeDescriptor::List(Box::new(
        TypeDescriptor::U64,
    ))));
    let value = NdfValue::List(vec![NdfValue::List(vec![NdfValue::U64(7); 32]); 100]);
    let mut b = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    registry.validate(&ty, &value, &mut b)?;
    // Root + 100 rows + 3,200 cells. Borrowed sibling groups retain only
    // branching ancestors; this shallow matrix needs no heap frontier.
    assert_eq!(b.usage().allocation_units, 0);
    assert_eq!(b.usage().nodes, 3_301);
    assert_eq!(b.usage().work, 3_301);
    assert_eq!(b.usage().depth, 3);
    let mut invalid = value.clone();
    if let NdfValue::List(rows) = &mut invalid {
        rows[99] = NdfValue::List(vec![NdfValue::Text("not a U64".into())]);
    }
    assert!(matches!(
        registry.validate(&ty, &invalid, &mut budget()),
        Err(SchemaError::WrongType)
    ));
    Ok(())
}

#[test]
fn validation_borrows_wide_siblings_but_eagerly_admits_every_child() -> Result<(), SchemaError> {
    let (registry, _) = registry()?;
    for width in [0, 1, 8, 9, 17, 100_000] {
        let value = NdfValue::List(vec![NdfValue::Unit; width]);
        let mut limits = budget().limits();
        limits.allocation_units = 0;
        let mut b = Budget::new(limits);
        registry.validate(&TypeDescriptor::NdfValue, &value, &mut b)?;
        assert_eq!(b.usage().allocation_units, 0);
        assert_eq!(b.usage().nodes, width as u64 + 1);
        assert_eq!(b.usage().work, width as u64 + 1);
    }
    let ty = TypeDescriptor::List(Box::new(TypeDescriptor::U64));
    for width in [9, 100_000] {
        let mut values = vec![NdfValue::U64(0); width];
        values[0] = NdfValue::Unit;
        let value = NdfValue::List(values);
        for (work, nodes, depth, expected, used_work, used_nodes, used_depth) in [
            (
                width as u64,
                1_000_000,
                100,
                SchemaError::Stopped(StopReason::WorkLimit),
                width as u64,
                1,
                1,
            ),
            (
                width as u64 + 1,
                1_000_000,
                100,
                SchemaError::WrongType,
                width as u64 + 1,
                2,
                2,
            ),
            (
                1_000_000,
                1,
                100,
                SchemaError::Stopped(StopReason::NodeLimit),
                width as u64 + 1,
                1,
                1,
            ),
            (
                1_000_000,
                1_000_000,
                1,
                SchemaError::Stopped(StopReason::DepthLimit),
                width as u64 + 1,
                2,
                1,
            ),
        ] {
            let mut b = Budget::new(Limits {
                work,
                nodes,
                depth,
                allocation_units: 0,
                ..budget().limits()
            });
            assert_eq!(registry.validate(&ty, &value, &mut b).err(), Some(expected));
            assert_eq!(b.usage().work, used_work);
            assert_eq!(b.usage().nodes, used_nodes);
            assert_eq!(b.usage().depth, used_depth);
            assert_eq!(b.usage().allocation_units, 0);
        }
    }
    Ok(())
}

#[test]
fn validation_preserves_left_to_right_error_order() -> Result<(), SchemaError> {
    let (registry, schema) = registry()?;
    let missing = NdfValue::Record(Record {
        schema: schema.clone(),
        kind: "Missing".into(),
        fields: vec![],
    });
    let wrong_fields = NdfValue::Record(Record {
        schema,
        kind: "Pair".into(),
        fields: vec![],
    });
    for (children, expected) in [
        (
            vec![missing.clone(), wrong_fields.clone()],
            SchemaError::UnknownType,
        ),
        (
            vec![wrong_fields.clone(), missing.clone()],
            SchemaError::FieldCount,
        ),
        (
            {
                let mut v = vec![NdfValue::Unit; 9];
                v[0] = missing.clone();
                v[8] = wrong_fields.clone();
                v
            },
            SchemaError::UnknownType,
        ),
        (
            {
                let mut v = vec![NdfValue::Unit; 9];
                v[0] = wrong_fields.clone();
                v[8] = missing.clone();
                v
            },
            SchemaError::FieldCount,
        ),
        (
            {
                let mut v = vec![NdfValue::Unit; 17];
                v[0] = missing.clone();
                v[16] = wrong_fields.clone();
                v
            },
            SchemaError::UnknownType,
        ),
        (
            {
                let mut v = vec![NdfValue::Unit; 17];
                v[0] = wrong_fields;
                v[16] = missing;
                v
            },
            SchemaError::FieldCount,
        ),
    ] {
        assert!(
            matches!(registry.validate(&TypeDescriptor::NdfValue, &NdfValue::List(children), &mut budget()), Err(found) if found == expected)
        );
    }
    Ok(())
}

#[test]
fn borrowed_fields_resume_after_nested_values_and_across_spills() -> Result<(), SchemaError> {
    fn fields() -> Vec<FieldDescriptor> {
        vec![
            FieldDescriptor {
                name: "nested".into(),
                ty: TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
            },
            FieldDescriptor {
                name: "label".into(),
                ty: TypeDescriptor::Text,
            },
            FieldDescriptor {
                name: "flag".into(),
                ty: TypeDescriptor::Bool,
            },
        ]
    }
    fn value(schema: &SchemaRef, variant: bool, fields: Vec<NdfValue>) -> NdfValue {
        if variant {
            NdfValue::Variant(Variant {
                schema: schema.clone(),
                type_name: "MixedVariant".into(),
                variant: "Case".into(),
                fields,
            })
        } else {
            NdfValue::Record(Record {
                schema: schema.clone(),
                kind: "MixedRecord".into(),
                fields,
            })
        }
    }
    let descriptor = SchemaDescriptor {
        package: "mixed".into(),
        revision: 1,
        types: vec![
            NamedType {
                name: "MixedRecord".into(),
                constraints: vec![],
                shape: TypeShape::Record { fields: fields() },
            },
            NamedType {
                name: "MixedVariant".into(),
                constraints: vec![],
                shape: TypeShape::Variant {
                    variants: vec![VariantDescriptor {
                        name: "Case".into(),
                        fields: fields(),
                    }],
                },
            },
        ],
        operations: vec![],
    };
    let schema = descriptor.reference(&mut budget())?;
    let mut r = SchemaRegistry::default();
    r.register(schema.clone(), descriptor, &mut budget())?;
    r.finalize(&mut budget())?;
    let missing = NdfValue::Record(Record {
        schema: schema.clone(),
        kind: "Missing".into(),
        fields: vec![],
    });
    for variant in [false, true] {
        let named = TypeDescriptor::Named(TypeRef {
            package: "mixed".into(),
            revision: 1,
            name: if variant {
                "MixedVariant"
            } else {
                "MixedRecord"
            }
            .into(),
        });
        for ty in [
            &named,
            &TypeDescriptor::TypedValue,
            &TypeDescriptor::NdfValue,
        ] {
            let valid = value(
                &schema,
                variant,
                vec![
                    NdfValue::List(vec![NdfValue::Unit]),
                    NdfValue::Text("ok".into()),
                    NdfValue::Bool(true),
                ],
            );
            let mut b = budget();
            r.validate(ty, &valid, &mut b)?;
            assert_eq!(
                (b.usage().work, b.usage().nodes, b.usage().depth),
                (5, 5, 3)
            );
            for (nested, expected, nodes) in [
                (NdfValue::Unit, SchemaError::WrongType, 5),
                (missing.clone(), SchemaError::UnknownType, 3),
            ] {
                let invalid = value(
                    &schema,
                    variant,
                    vec![
                        NdfValue::List(vec![nested]),
                        NdfValue::Text("ok".into()),
                        NdfValue::U64(7),
                    ],
                );
                let mut b = budget();
                assert_eq!(r.validate(ty, &invalid, &mut b).err(), Some(expected));
                assert_eq!(
                    (b.usage().work, b.usage().nodes, b.usage().depth),
                    (5, nodes, 3)
                );
            }
            let mut b = budget();
            assert_eq!(
                r.validate(ty, &value(&schema, variant, vec![]), &mut b)
                    .err(),
                Some(SchemaError::FieldCount)
            );
            assert_eq!(
                (b.usage().work, b.usage().nodes, b.usage().depth),
                (1, 1, 1)
            );
        }
        let mut inner = value(
            &schema,
            variant,
            vec![
                NdfValue::List(vec![NdfValue::Unit]),
                NdfValue::Text("ok".into()),
                NdfValue::U64(7),
            ],
        );
        for _ in 0..9 {
            inner = NdfValue::List(vec![inner, missing.clone()]);
        }
        let mut b = budget();
        assert_eq!(
            r.validate(&TypeDescriptor::NdfValue, &inner, &mut b).err(),
            Some(SchemaError::WrongType)
        );
        assert!(b.usage().allocation_units > 0);
        assert_eq!(
            (b.usage().work, b.usage().nodes, b.usage().depth),
            (23, 14, 12)
        );
    }
    Ok(())
}

#[test]
fn descriptor_canonical_json_has_independently_specified_field_order() -> Result<(), SchemaError> {
    let descriptor = descriptor();
    let expected = r#"{"operations":{},"package":"example","revision":1,"types":{"Pair":{"constraints":["positive"],"record":[["first","Natural"],["second","Text"]]}}}"#;
    assert_eq!(
        descriptor.canonical_json(&mut budget())?,
        expected.as_bytes()
    );
    let mut reordered = descriptor.clone();
    if let TypeShape::Record { fields } = &mut reordered.types[0].shape {
        fields.reverse();
    }
    assert_ne!(
        descriptor.reference(&mut budget())?,
        reordered.reference(&mut budget())?
    );
    Ok(())
}

#[test]
fn descriptor_map_order_is_irrelevant_but_duplicates_are_rejected() -> Result<(), SchemaError> {
    let mut a = descriptor();
    let mut other = a.types[0].clone();
    other.name = "Other".into();
    a.types.push(other);
    let mut b = a.clone();
    b.types.reverse();
    assert_eq!(a.reference(&mut budget())?, b.reference(&mut budget())?);
    a.types.push(a.types[0].clone());
    assert_eq!(a.reference(&mut budget()), Err(SchemaError::DuplicateName));
    Ok(())
}

#[test]
fn canonical_json_escapes_controls_without_unicode_normalization() -> Result<(), SchemaError> {
    let mut descriptor = descriptor();
    descriptor.package = "日\n\"\\é".into();
    let bytes = descriptor.canonical_json(&mut budget())?;
    assert!(String::from_utf8_lossy(&bytes).contains(r#""package":"日\u000a\"\\é""#));
    Ok(())
}

#[test]
fn registry_rejects_wrong_digest_and_unknown_unused_references() -> Result<(), SchemaError> {
    let mut registry = SchemaRegistry::default();
    let mut descriptor = descriptor();
    let mut wrong = descriptor.reference(&mut budget())?;
    wrong.digest = Digest([0; 32]);
    assert_eq!(
        registry.register(wrong, descriptor.clone(), &mut budget()),
        Err(SchemaError::IdentityMismatch)
    );
    descriptor.operations.push(OperationDescriptor {
        name: "unvisited".into(),
        input: TypeDescriptor::Unit,
        output: TypeDescriptor::Named(TypeRef {
            package: "missing".into(),
            revision: 1,
            name: "T".into(),
        }),
        pure: true,
    });
    registry.register(
        descriptor.reference(&mut budget())?,
        descriptor,
        &mut budget(),
    )?;
    assert!(matches!(
        registry.validate(&TypeDescriptor::Unit, &NdfValue::Unit, &mut budget()),
        Err(SchemaError::Unfinalized)
    ));
    assert_eq!(
        registry.finalize(&mut budget()),
        Err(SchemaError::UnknownSchema)
    );
    Ok(())
}

#[test]
fn registry_accepts_mutual_symbolic_references_then_seals_them() -> Result<(), SchemaError> {
    let mut registry = SchemaRegistry::default();
    for (package, target) in [("a", "b"), ("b", "a")] {
        let descriptor = SchemaDescriptor {
            package: package.into(),
            revision: 1,
            operations: vec![],
            types: vec![NamedType {
                name: "Node".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "next".into(),
                        ty: TypeDescriptor::Option(Box::new(TypeDescriptor::Named(TypeRef {
                            package: target.into(),
                            revision: 1,
                            name: "Node".into(),
                        }))),
                    }],
                },
            }],
        };
        registry.register(
            descriptor.reference(&mut budget())?,
            descriptor,
            &mut budget(),
        )?;
    }
    registry.finalize(&mut budget())?;
    Ok(())
}

#[test]
fn structural_boundary_checks_field_count_kind_intrinsic_subset_and_negative_natural()
-> Result<(), SchemaError> {
    let (registry, reference) = registry()?;
    let record = Record {
        schema: reference,
        kind: "Pair".into(),
        fields: vec![
            NdfValue::Integer(Integer::from(0i64)),
            NdfValue::Text("ok".into()),
        ],
    };
    let value = NdfValue::Record(record.clone());
    assert_eq!(
        registry
            .validate(&TypeDescriptor::TypedValue, &value, &mut budget())?
            .value(),
        &value
    );
    // Constraint ID positive is deliberately NOT a claim that a domain positivity check ran.
    assert!(matches!(
        registry.validate(&TypeDescriptor::NdfScalar, &value, &mut budget()),
        Err(SchemaError::WrongType)
    ));
    let mut wrong = record.clone();
    wrong.fields.pop();
    assert!(matches!(
        registry.validate(
            &TypeDescriptor::NdfValue,
            &NdfValue::Record(wrong),
            &mut budget()
        ),
        Err(SchemaError::FieldCount)
    ));
    let mut wrong = record;
    wrong.fields[0] = NdfValue::Integer(Integer::from(-1i64));
    assert!(matches!(
        registry.validate(
            &TypeDescriptor::NdfValue,
            &NdfValue::Record(wrong),
            &mut budget()
        ),
        Err(SchemaError::WrongType)
    ));
    assert!(matches!(
        registry.validate(
            &TypeDescriptor::Bytes32,
            &NdfValue::Bytes(vec![0; 31]),
            &mut budget()
        ),
        Err(SchemaError::WrongType)
    ));
    Ok(())
}

#[test]
fn validator_and_descriptor_depth_share_parent_budget_and_peak() -> Result<(), SchemaError> {
    let (registry, _) = registry()?;
    let mut limits = budget().limits();
    limits.depth = 2;
    let mut budget = Budget::new(limits);
    let ty = TypeDescriptor::List(Box::new(TypeDescriptor::Unit));
    let value = NdfValue::List(vec![NdfValue::Unit]);
    registry.validate(&ty, &value, &mut budget)?;
    assert_eq!(budget.usage().depth, 2);
    let result = budget.with_depth(|b| registry.validate(&ty, &value, b));
    assert!(matches!(
        result,
        Err(SchemaError::Stopped(StopReason::DepthLimit))
    ));
    let mut descriptor = descriptor();
    descriptor.operations.push(OperationDescriptor {
        name: "op".into(),
        input: ty,
        output: TypeDescriptor::Unit,
        pure: true,
    });
    assert!(matches!(
        budget.with_depth(|b| descriptor.reference(b)),
        Err(SchemaError::Stopped(StopReason::DepthLimit))
    ));
    Ok(())
}

fn sources() -> Result<(SourceStore, SourceSnapshot, SourceSnapshot), SourceError> {
    let a = SourceSnapshot::new(
        SourceId("a".into()),
        0,
        "memory:same".into(),
        b"ab".to_vec(),
        &mut budget(),
    )?;
    let b = SourceSnapshot::new(
        SourceId("b".into()),
        0,
        "memory:same".into(),
        b"ab".to_vec(),
        &mut budget(),
    )?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    store.insert(b.clone())?;
    Ok((store, a, b))
}

#[test]
fn imported_origin_graph_rejects_cycles_unknown_refs_and_missing_source() -> Result<(), OriginError>
{
    let (store, a, _) = sources()?;
    for origins in [
        vec![Origin::Composite(vec![OriginId(0)])],
        vec![
            Origin::Composite(vec![OriginId(1)]),
            Origin::Composite(vec![OriginId(0)]),
        ],
    ] {
        assert!(matches!(
            OriginGraph::from_origins(origins, &store, &mut budget()),
            Err(OriginError::Cycle)
        ));
    }
    assert!(matches!(
        OriginGraph::from_origins(
            vec![Origin::Composite(vec![OriginId(u64::MAX)])],
            &store,
            &mut budget()
        ),
        Err(OriginError::Reference)
    ));
    assert!(matches!(
        OriginGraph::from_origins(
            vec![Origin::Direct(a.span(0, 1)?)],
            &SourceStore::default(),
            &mut budget()
        ),
        Err(OriginError::Source(SourceError::MissingSnapshot))
    ));
    let graph = OriginGraph::from_origins(
        vec![
            Origin::Composite(vec![OriginId(1), OriginId(1)]),
            Origin::Direct(a.span(0, 1)?),
        ],
        &store,
        &mut budget(),
    )?;
    assert_eq!(graph.origins().len(), 2);
    Ok(())
}

#[test]
fn imported_origin_graph_honors_zero_nodes_and_depth_limits() -> Result<(), OriginError> {
    let (store, _, _) = sources()?;
    let origin = Origin::Synthetic {
        reason: "test".into(),
        anchor: None,
    };
    let mut limits = budget().limits();
    limits.nodes = 0;
    assert!(matches!(
        OriginGraph::from_origins(vec![origin.clone()], &store, &mut Budget::new(limits)),
        Err(OriginError::Stopped(StopReason::NodeLimit))
    ));
    limits.nodes = 100;
    limits.depth = 0;
    assert!(matches!(
        OriginGraph::from_origins(vec![origin], &store, &mut Budget::new(limits)),
        Err(OriginError::Stopped(StopReason::DepthLimit))
    ));
    Ok(())
}

#[test]
fn source_map_checks_composable_interval_cycles_not_snapshot_cycles() -> Result<(), OriginError> {
    let (store, a, b) = sources()?;
    let mut map = SourceMap::default();
    map.insert(
        Mapping {
            source: a.span(0, 1)?,
            target: b.span(0, 1)?,
            kind: MappingKind::Exact,
        },
        &store,
        &mut budget(),
    )?;
    map.insert(
        Mapping {
            source: b.span(1, 2)?,
            target: a.span(1, 2)?,
            kind: MappingKind::Exact,
        },
        &store,
        &mut budget(),
    )?;
    assert_eq!(
        map.insert(
            Mapping {
                source: b.span(0, 1)?,
                target: a.span(0, 1)?,
                kind: MappingKind::Exact
            },
            &store,
            &mut budget()
        ),
        Err(OriginError::Cycle)
    );
    assert_eq!(map.inverse(&b.span(0, 1)?, &store)?, a.span(0, 1)?);
    Ok(())
}

#[test]
fn inverse_refuses_transformed_and_ambiguous_mappings() -> Result<(), OriginError> {
    let (store, a, b) = sources()?;
    let mut map = SourceMap::default();
    map.insert(
        Mapping {
            source: a.span(0, 1)?,
            target: b.span(1, 2)?,
            kind: MappingKind::Transformed,
        },
        &store,
        &mut budget(),
    )?;
    assert_eq!(
        map.inverse(&b.span(1, 2)?, &store),
        Err(OriginError::Irreversible)
    );
    map.insert(
        Mapping {
            source: a.span(1, 2)?,
            target: b.span(1, 2)?,
            kind: MappingKind::Exact,
        },
        &store,
        &mut budget(),
    )?;
    assert_eq!(
        map.inverse(&b.span(1, 2)?, &store),
        Err(OriginError::Ambiguous)
    );
    Ok(())
}

fn event() -> Event {
    let schema = SchemaRef {
        package: "test".into(),
        revision: 1,
        digest: Digest([0; 32]),
    };
    Event {
        schema: schema.clone(),
        kind: "trace".into(),
        operation_path: vec![1],
        span: None,
        payload: TypedValue::Record(Record {
            schema,
            kind: "Payload".into(),
            fields: vec![],
        }),
    }
}
fn diagnostic() -> Diagnostic {
    let event = event();
    Diagnostic {
        schema: event.schema,
        code: "error".into(),
        severity: Severity::Error,
        stage: "reader".into(),
        arguments: event.payload,
        primary: None,
        related: vec![],
        fixes: vec![],
    }
}

#[test]
fn report_zero_event_capacity_keeps_one_typed_overflow_and_stop() {
    let mut limits = budget().limits();
    limits.events = 0;
    let mut budget = Budget::new(limits);
    let mut builder = ReportBuilder::default();
    assert_eq!(
        builder.event(event(), &mut budget),
        Err(ReportError::Stopped(StopReason::EventLimit))
    );
    assert_eq!(
        builder.event(event(), &mut budget),
        Err(ReportError::Stopped(StopReason::EventLimit))
    );
    let report = builder.finish(&budget);
    assert!(report.events.is_empty());
    assert_eq!(report.trace_overflow, Some(TraceOverflow { dropped: 1 }));
}

#[test]
fn report_candidate_rollback_discards_facts_without_refunding_budget() -> Result<(), ReportError> {
    let mut budget = budget();
    let mut builder = ReportBuilder::default();
    let checkpoint = builder.checkpoint();
    builder.event(event(), &mut budget)?;
    builder.diagnostic(diagnostic(), &mut budget)?;
    builder.rollback(checkpoint)?;
    let report = builder.finish(&budget);
    assert!(report.events.is_empty());
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.usage.events, 1);
    assert_eq!(report.usage.diagnostics, 1);
    Ok(())
}

#[test]
fn report_diagnostic_limit_has_no_fabricated_global_message() {
    let mut limits = budget().limits();
    limits.diagnostics = 0;
    let mut budget = Budget::new(limits);
    let mut builder = ReportBuilder::default();
    assert_eq!(
        builder.diagnostic(diagnostic(), &mut budget),
        Err(ReportError::Stopped(StopReason::DiagnosticLimit))
    );
    let report = builder.finish(&budget);
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.trace_overflow, None);
}

#[test]
fn forward_overlap_is_acyclic_and_map_budget_failure_does_not_insert() -> Result<(), OriginError> {
    let source = SourceSnapshot::new(
        SourceId("x".into()),
        0,
        "memory:x".into(),
        b"aaa".to_vec(),
        &mut budget(),
    )?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut map = SourceMap::default();
    let forward = Mapping {
        source: source.span(0, 2)?,
        target: source.span(1, 3)?,
        kind: MappingKind::Exact,
    };
    let mut limits = budget().limits();
    limits.work = 0;
    assert_eq!(
        map.insert(forward.clone(), &store, &mut Budget::new(limits)),
        Err(OriginError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(
        map.inverse(&source.span(1, 2)?, &store),
        Err(OriginError::Unmapped)
    );
    map.insert(forward, &store, &mut budget())?;
    assert_eq!(
        map.insert(
            Mapping {
                source: source.span(2, 3)?,
                target: source.span(0, 1)?,
                kind: MappingKind::Exact
            },
            &store,
            &mut budget()
        ),
        Err(OriginError::Cycle)
    );
    Ok(())
}

#[test]
fn empty_anchor_cycles_and_transformed_relations_are_checked() -> Result<(), OriginError> {
    let (store, a, b) = sources()?;
    let mut map = SourceMap::default();
    map.insert(
        Mapping {
            source: a.span(0, 0)?,
            target: b.span(0, 0)?,
            kind: MappingKind::Exact,
        },
        &store,
        &mut budget(),
    )?;
    assert_eq!(
        map.insert(
            Mapping {
                source: b.span(0, 0)?,
                target: a.span(0, 0)?,
                kind: MappingKind::Exact
            },
            &store,
            &mut budget()
        ),
        Err(OriginError::Cycle)
    );
    let mut transformed = SourceMap::default();
    transformed.insert(
        Mapping {
            source: a.span(0, 1)?,
            target: b.span(1, 2)?,
            kind: MappingKind::Transformed,
        },
        &store,
        &mut budget(),
    )?;
    assert_eq!(
        transformed.insert(
            Mapping {
                source: b.span(1, 2)?,
                target: a.span(0, 1)?,
                kind: MappingKind::Transformed
            },
            &store,
            &mut budget()
        ),
        Err(OriginError::Cycle)
    );
    Ok(())
}

#[test]
fn report_storage_obeys_allocation_budget() {
    let mut limits = budget().limits();
    limits.allocation_units = 0;
    let mut builder = ReportBuilder::default();
    let mut budget = Budget::new(limits);
    assert_eq!(
        builder.event(event(), &mut budget),
        Err(ReportError::Stopped(StopReason::AllocationLimit))
    );
    assert!(builder.finish(&budget).events.is_empty());
}

#[test]
fn deeply_nested_value_cleanup_uses_no_recursive_drop_chain() {
    let mut value = NdfValue::Unit;
    for _ in 0..50_000 {
        value = NdfValue::Some(Box::new(value));
    }
    let cloned = value.clone();
    assert_eq!(value, cloned);
    assert_eq!(format!("{value:?}"), "Some(..)");
    let different = NdfValue::Some(Box::new(NdfValue::Unit));
    assert_ne!(cloned, different);
    drop(value);
    drop(cloned);
}

#[test]
fn local_kind_ids_are_canonical_across_descriptor_input_order() -> Result<(), SchemaError> {
    let mut a = descriptor();
    let mut b = a.types[0].clone();
    b.name = "Alpha".into();
    a.types.push(b);
    let mut b = a.clone();
    b.types.reverse();
    let reference = a.reference(&mut budget())?;
    assert_eq!(reference, b.reference(&mut budget())?);
    let mut first = SchemaRegistry::default();
    first.register(reference.clone(), a, &mut budget())?;
    let mut second = SchemaRegistry::default();
    second.register(reference.clone(), b, &mut budget())?;
    for registry in [first, second] {
        assert_eq!(registry.kind_id(&reference, "Alpha")?, 0);
        assert_eq!(registry.kind_name(&reference, 1)?, "Pair");
        assert!(registry.kind_name(&reference, u64::MAX).is_err());
    }
    Ok(())
}

#[test]
fn rejected_deep_descriptor_is_safe_to_clone_compare_debug_and_drop() {
    let mut ty = TypeDescriptor::Unit;
    for _ in 0..100_000 {
        ty = TypeDescriptor::List(Box::new(ty));
    }
    let copy = ty.clone();
    assert_eq!(ty, copy);
    assert_eq!(format!("{ty:?}"), "List(..)");
    drop(copy);
    let mut descriptor = descriptor();
    descriptor.types[0].shape = TypeShape::Record {
        fields: vec![FieldDescriptor {
            name: "deep".into(),
            ty,
        }],
    };
    assert!(matches!(
        descriptor.reference(&mut budget()),
        Err(SchemaError::Stopped(StopReason::DepthLimit))
    ));
    drop(descriptor);
}

#[test]
fn inline_validation_preserves_deep_and_prestopped_limits() -> Result<(), SchemaError> {
    let (r, _) = registry()?;
    let mut value = NdfValue::Unit;
    for i in 0..40 {
        value = if i % 2 == 0 {
            NdfValue::Some(Box::new(value))
        } else {
            NdfValue::List(vec![value])
        };
    }
    let mut exact = budget().limits();
    exact.allocation_units = 0;
    exact.nodes = 41;
    exact.work = 41;
    exact.depth = 41;
    r.validate(&TypeDescriptor::NdfValue, &value, &mut Budget::new(exact))?;
    for resource in 0..3 {
        let mut limits = exact;
        let reason = match resource {
            0 => {
                limits.nodes -= 1;
                StopReason::NodeLimit
            }
            1 => {
                limits.work -= 1;
                StopReason::WorkLimit
            }
            _ => {
                limits.depth -= 1;
                StopReason::DepthLimit
            }
        };
        let mut b = Budget::new(limits);
        assert!(
            matches!(r.validate(&TypeDescriptor::NdfValue,&value,&mut b),Err(SchemaError::Stopped(found)) if found==reason)
        );
        assert_eq!(b.poll(), Err(reason));
    }
    let mut b = budget();
    b.cancel();
    let usage = b.usage();
    assert!(matches!(
        r.validate(&TypeDescriptor::NdfValue, &value, &mut b),
        Err(SchemaError::Stopped(StopReason::Cancelled))
    ));
    assert_eq!(b.usage(), usage);
    Ok(())
}

#[test]
fn inline_scalar_validation_needs_no_heap_and_preserves_stop_priority() -> Result<(), SchemaError> {
    let (registry, _) = registry()?;
    let limits = Limits {
        work: 1,
        nodes: 1,
        depth: 1,
        ..Limits::default()
    };
    let expected = Usage {
        work: 1,
        nodes: 1,
        depth: 1,
        ..Usage::default()
    };
    let mut valid = Budget::new(limits);
    registry.validate(&TypeDescriptor::Unit, &NdfValue::Unit, &mut valid)?;
    assert_eq!(valid.usage(), expected);
    let mut invalid = Budget::new(limits);
    assert!(matches!(
        registry.validate(&TypeDescriptor::Unit, &NdfValue::Bool(true), &mut invalid),
        Err(SchemaError::WrongType)
    ));
    assert_eq!(invalid.usage(), expected);
    assert_eq!(invalid.poll(), Ok(()));
    for resource in 0..3 {
        let mut short = limits;
        let (reason, usage) = match resource {
            0 => {
                short.work = 0;
                (StopReason::WorkLimit, Usage::default())
            }
            1 => {
                short.nodes = 0;
                (
                    StopReason::NodeLimit,
                    Usage {
                        work: 1,
                        ..Usage::default()
                    },
                )
            }
            _ => {
                short.depth = 0;
                (
                    StopReason::DepthLimit,
                    Usage {
                        work: 1,
                        nodes: 1,
                        ..Usage::default()
                    },
                )
            }
        };
        let mut b = Budget::new(short);
        assert!(
            matches!(registry.validate(&TypeDescriptor::Unit, &NdfValue::Bool(true), &mut b), Err(SchemaError::Stopped(found)) if found == reason)
        );
        assert_eq!(b.usage(), usage);
        assert_eq!(b.poll(), Err(reason));
        assert!(
            matches!(registry.validate(&TypeDescriptor::Unit, &NdfValue::Unit, &mut b), Err(SchemaError::Stopped(found)) if found == reason)
        );
        assert_eq!(b.usage(), usage);
    }
    Ok(())
}

#[test]
fn origin_source_lookup_stops_before_unbudgeted_long_identity_work() -> Result<(), OriginError> {
    let id = SourceId("x".repeat(100_000));
    let source = SourceSnapshot::new(id, 0, "memory:long".into(), b"a".to_vec(), &mut budget())?;
    let span = source.span(0, 1)?;
    let mut store = SourceStore::default();
    store.insert(source)?;
    let mut graph = OriginGraph::default();
    let mut limits = budget().limits();
    limits.work = 0;
    let mut stopped = Budget::new(limits);
    assert_eq!(
        graph.push(Origin::Direct(span), &store, &mut stopped),
        Err(OriginError::Stopped(StopReason::WorkLimit))
    );
    assert!(graph.origins().is_empty());
    Ok(())
}

#[test]
fn origin_span_work_is_metered_for_every_variant_and_import_path() -> Result<(), OriginError> {
    let name = "x".repeat(100_000);
    let original = SourceSnapshot::new(
        SourceId(name.clone()),
        0,
        "memory:origin".into(),
        b"a".to_vec(),
        &mut budget(),
    )?;
    // Equal identities in independent storage must not use the pointer shortcut.
    let stored = SourceSnapshot::new(
        SourceId(name),
        0,
        "memory:origin".into(),
        b"a".to_vec(),
        &mut budget(),
    )?;
    let span = original.span(0, 1)?;
    let mut store = SourceStore::default();
    store.insert(stored)?;
    let variants = [
        Origin::Direct(span.clone()),
        Origin::Generated {
            operation: OperationRef {
                schema: SchemaRef {
                    package: "test".into(),
                    revision: 0,
                    digest: Digest([0; 32]),
                },
                name: "expand".into(),
            },
            callsite: Some(span.clone()),
            inputs: vec![],
        },
        Origin::Synthetic {
            reason: "inserted".into(),
            anchor: Some(span),
        },
    ];
    // One index comparison (2L+1), full identity (1+L+40), geometry (1).
    let append_work = 300_043;
    for origin in variants {
        for limit in [0, 1, 200_000, 200_001, append_work - 1] {
            let mut limits = budget().limits();
            limits.work = limit;
            let mut b = Budget::new(limits);
            let mut graph = OriginGraph::default();
            assert_eq!(
                graph.push(origin.clone(), &store, &mut b),
                Err(OriginError::Stopped(StopReason::WorkLimit))
            );
            assert!(graph.origins().is_empty());
            assert!(b.usage().work <= limit);
        }
        let mut limits = budget().limits();
        limits.work = append_work;
        let mut b = Budget::new(limits);
        let mut graph = OriginGraph::default();
        assert_eq!(graph.push(origin.clone(), &store, &mut b)?, OriginId(0));
        assert_eq!(b.usage().work, append_work);
        // Imported validation adds entry and exit traversal work, not an
        // unmetered shortcut through the same source lookup.
        limits.work = append_work + 1;
        assert_eq!(
            OriginGraph::validate_origins(
                std::slice::from_ref(&origin),
                &store,
                &mut Budget::new(limits)
            ),
            Err(OriginError::Stopped(StopReason::WorkLimit))
        );
        limits.work += 1;
        let mut b = Budget::new(limits);
        OriginGraph::validate_origins(std::slice::from_ref(&origin), &store, &mut b)?;
        assert_eq!(b.usage().work, append_work + 2);
        let restored =
            OriginGraph::from_origins(vec![origin.clone()], &store, &mut Budget::new(limits))?;
        assert_eq!(restored.origins(), std::slice::from_ref(&origin));
        let before = graph.origins().to_vec();
        let mut cancelled = budget();
        cancelled.cancel();
        assert_eq!(
            graph.push(origin.clone(), &store, &mut cancelled),
            Err(OriginError::Stopped(StopReason::Cancelled))
        );
        assert_eq!(graph.origins(), before);
        assert_eq!(cancelled.usage().work, 0);
        assert_eq!(
            OriginGraph::validate_origins(&[origin], &store, &mut cancelled),
            Err(OriginError::Stopped(StopReason::Cancelled))
        );
    }
    Ok(())
}

#[test]
fn origin_missing_or_mismatched_identity_remains_typed_and_atomic() -> Result<(), OriginError> {
    let name = "x".repeat(100_000);
    let source = SourceSnapshot::new(
        SourceId(name.clone()),
        0,
        "memory:origin".into(),
        b"a".to_vec(),
        &mut budget(),
    )?;
    let origin = Origin::Direct(source.span(0, 1)?);
    for (id, revision, bytes) in [
        (format!("{}y", &name[..name.len() - 1]), 0, b"a".to_vec()),
        (name.clone(), 1, b"a".to_vec()),
        (name, 0, b"b".to_vec()),
    ] {
        let mut store = SourceStore::default();
        store.insert(SourceSnapshot::new(
            SourceId(id),
            revision,
            "memory:origin".into(),
            bytes,
            &mut budget(),
        )?)?;
        let mut graph = OriginGraph::default();
        graph.push(
            Origin::Synthetic {
                reason: "existing".into(),
                anchor: None,
            },
            &store,
            &mut budget(),
        )?;
        let before = graph.origins().to_vec();
        let mut limits = budget().limits();
        limits.work = 100;
        assert_eq!(
            graph.push(origin.clone(), &store, &mut Budget::new(limits)),
            Err(OriginError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(graph.origins(), before);
        assert_eq!(
            graph.push(origin.clone(), &store, &mut budget()),
            Err(OriginError::Source(SourceError::MissingSnapshot))
        );
        assert_eq!(graph.origins(), before);
        assert_eq!(
            OriginGraph::from_origins(vec![origin.clone()], &store, &mut budget()).err(),
            Some(OriginError::Source(SourceError::MissingSnapshot))
        );
    }
    Ok(())
}

#[test]
fn source_map_closure_revalidation_precharges_long_identity_lookup() -> Result<(), OriginError> {
    let a = SourceSnapshot::new(
        SourceId(format!("{}a", "x".repeat(99_999))),
        0,
        "memory:a".into(),
        b"a".to_vec(),
        &mut budget(),
    )?;
    let b = SourceSnapshot::new(
        SourceId(format!("{}b", "x".repeat(99_999))),
        0,
        "memory:b".into(),
        b"a".to_vec(),
        &mut budget(),
    )?;
    let mapping = Mapping {
        source: a.span(0, 1)?,
        target: b.span(0, 1)?,
        kind: MappingKind::Exact,
    };
    let mut store = SourceStore::default();
    store.insert(a)?;
    store.insert(b)?;
    let mappings = [mapping];
    let proof = SourceMap::validate_mappings(&mappings, &store, &mut budget())?;
    let mut limits = budget().limits();
    limits.work = 3;
    let mut stopped = Budget::new(limits);
    assert_eq!(
        proof.validate_sources(&store, &mut stopped),
        Err(OriginError::Stopped(StopReason::WorkLimit))
    );
    Ok(())
}

#[test]
fn source_map_bound_store_fast_path_and_independent_revalidation_remain_distinct()
-> Result<(), OriginError> {
    fn snapshot(suffix: char, bytes: &[u8]) -> Result<SourceSnapshot, SourceError> {
        SourceSnapshot::new(
            SourceId(format!("{}{suffix}", "x".repeat(99_999))),
            0,
            format!("memory:{suffix}"),
            bytes.to_vec(),
            &mut budget(),
        )
    }
    let a = snapshot('a', b"a")?;
    let b = snapshot('b', b"a")?;
    let mapping = Mapping {
        source: a.span(0, 1)?,
        target: b.span(0, 1)?,
        kind: MappingKind::Exact,
    };
    let mut store = SourceStore::default();
    store.insert(a)?;
    store.insert(b)?;
    let mappings = [mapping.clone()];
    let bound = SourceMap::validate_mappings(&mappings, &store, &mut budget())?
        .bind_sources(&store, &mut budget())?;
    let mut limits = budget().limits();
    limits.work = 1;
    let mut fast = Budget::new(limits);
    bound.validate_sources(&store, &mut fast)?;
    assert_eq!(fast.usage().work, 1);
    let mut independent = SourceStore::default();
    independent.insert(snapshot('a', b"a")?)?;
    independent.insert(snapshot('b', b"a")?)?;
    // Binary search visits b,a for source a and b for source b: 3*(2L+1).
    // Two independent identities cost 2*(L+41), geometry costs 2, and
    // closure entry, two endpoint visits cost 3:
    // total 800090.
    for cap in [0, 1, 3, 800_089] {
        limits.work = cap;
        let mut stopped = Budget::new(limits);
        assert_eq!(
            bound.validate_sources(&independent, &mut stopped),
            Err(OriginError::Stopped(StopReason::WorkLimit))
        );
        assert!(stopped.usage().work <= cap);
    }
    limits.work = 800_090;
    let mut exact = Budget::new(limits);
    bound.validate_sources(&independent, &mut exact)?;
    assert_eq!(exact.usage().work, limits.work);
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        bound.validate_sources(&store, &mut cancelled),
        Err(OriginError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(cancelled.usage().work, 0);
    let mut conflicting = SourceStore::default();
    conflicting.insert(snapshot('a', b"a")?)?;
    conflicting.insert(snapshot('b', b"b")?)?;
    assert_eq!(
        bound.validate_sources(&conflicting, &mut budget()),
        Err(OriginError::Source(SourceError::MissingSnapshot))
    );
    limits.work = 3;
    let mut map = SourceMap::default();
    assert_eq!(
        map.insert(mapping.clone(), &independent, &mut Budget::new(limits)),
        Err(OriginError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(
        map.inverse(&mapping.target, &independent),
        Err(OriginError::Unmapped)
    );
    assert_eq!(
        SourceMap::validate_mapping_parts(&[], &mappings, &independent, &mut Budget::new(limits))
            .err(),
        Some(OriginError::Stopped(StopReason::WorkLimit))
    );
    map.insert(mapping.clone(), &independent, &mut budget())?;
    assert_eq!(map.inverse(&mapping.target, &independent)?, mapping.source);
    Ok(())
}

#[test]
fn source_index_shared_key_shortcut_keeps_revision_and_independent_key_checks()
-> Result<(), OriginError> {
    let source = SourceSnapshot::new(
        SourceId("x".repeat(100_000)),
        0,
        "memory:source".into(),
        b"a".to_vec(),
        &mut budget(),
    )?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut limits = budget().limits();
    limits.work = 1;
    // Borrow the actual stored key, also exercising the shortcut on targets
    // without shared atomic snapshot storage.
    let key = &store.snapshots()[0].identity().source;
    {
        let mut same = Budget::new(limits);
        assert!(store.get_revision_with_budget(key, 0, &mut same)?.is_some());
        assert_eq!(same.usage().work, 1);
        assert!(
            store
                .get_revision_with_budget(key, 1, &mut Budget::new(limits))?
                .is_none()
        );
    }
    let independent = SourceId(source.identity().source.0.clone());
    assert_eq!(
        store.get_revision_with_budget(&independent, 0, &mut Budget::new(limits)),
        Err(StopReason::WorkLimit)
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        store.get_revision_with_budget(&source.identity().source, 0, &mut cancelled),
        Err(StopReason::Cancelled)
    );
    assert_eq!(cancelled.usage().work, 0);
    Ok(())
}

#[test]
#[cfg(target_has_atomic = "ptr")]
fn source_map_endpoint_cursors_reuse_only_resolved_shared_identities() -> Result<(), OriginError> {
    let a = SourceSnapshot::new(
        SourceId(format!("{}a", "x".repeat(99_999))),
        0,
        "memory:a".into(),
        b"ab".to_vec(),
        &mut budget(),
    )?;
    let b = SourceSnapshot::new(
        SourceId(format!("{}b", "x".repeat(99_999))),
        0,
        "memory:b".into(),
        b"ab".to_vec(),
        &mut budget(),
    )?;
    let mut store = SourceStore::default();
    store.insert(a.clone())?;
    store.insert(b.clone())?;
    for count in [1, 8, 64] {
        let mut mappings = Vec::new();
        for i in 0..count {
            // Fresh ranges still require geometry validation on a cursor hit.
            let start = i % 2;
            mappings.push(Mapping {
                source: a.span(start, start + 1)?,
                target: b.span(start, start + 1)?,
                kind: MappingKind::Exact,
            });
        }
        let proof = SourceMap::validate_mappings(&mappings, &store, &mut budget())?;
        let mut measured = budget();
        proof.validate_sources(&store, &mut measured)?;
        #[cfg(target_has_atomic = "ptr")]
        {
            // First a search compares b then a, first b search compares b;
            // all later endpoints pay one pointer probe and one range check.
            assert_eq!(measured.usage().work, 200_010 + 6 * (count - 1));
            let mut limits = budget().limits();
            limits.work = measured.usage().work - 1;
            assert_eq!(
                proof.validate_sources(&store, &mut Budget::new(limits)),
                Err(OriginError::Stopped(StopReason::WorkLimit))
            );
        }
    }
    // Warm both endpoint cursors in the first part, then change only ranges.
    // Reusing a previously returned slice would incorrectly accept this pair.
    let warm = [Mapping {
        source: a.span(0, 1)?,
        target: b.span(0, 1)?,
        kind: MappingKind::Exact,
    }];
    let changed = [Mapping {
        source: a.span(0, 1)?,
        target: b.span(1, 2)?,
        kind: MappingKind::Exact,
    }];
    assert_eq!(
        SourceMap::validate_mapping_parts(&warm, &changed, &store, &mut budget()).err(),
        Some(OriginError::Irreversible)
    );
    Ok(())
}

#[test]
#[cfg(target_has_atomic = "ptr")]
fn imported_origin_cursor_reuses_only_the_same_scoped_snapshot() -> Result<(), OriginError> {
    let source = SourceSnapshot::new(
        SourceId("x".repeat(100_000)),
        0,
        "memory:origin-cursor".into(),
        b"ab".to_vec(),
        &mut budget(),
    )?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut origins = Vec::new();
    for index in 0..64 {
        let start = index % 2;
        let span = source.span(start, start + 1)?;
        origins.push(match index % 3 {
            0 => Origin::Direct(span),
            1 => Origin::Generated {
                operation: OperationRef {
                    schema: SchemaRef {
                        package: "test".into(),
                        revision: 0,
                        digest: Digest([0; 32]),
                    },
                    name: "expand".into(),
                },
                callsite: Some(span),
                inputs: vec![],
            },
            _ => Origin::Synthetic {
                reason: "inserted".into(),
                anchor: Some(span),
            },
        });
    }
    // Each origin pays enter+exit. First source lookup/identity/range costs 3;
    // later origins pay a shared-identity probe and their own range check.
    let exact_work = 64 * 2 + 3 + 63 * 2;
    let mut limits = budget().limits();
    limits.work = exact_work;
    let mut exact = Budget::new(limits);
    OriginGraph::validate_origins(&origins, &store, &mut exact)?;
    assert_eq!(exact.usage().work, exact_work);
    limits.work -= 1;
    assert_eq!(
        OriginGraph::validate_origins(&origins, &store, &mut Budget::new(limits)),
        Err(OriginError::Stopped(StopReason::WorkLimit))
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        OriginGraph::validate_origins(&origins, &store, &mut cancelled),
        Err(OriginError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(cancelled.usage().work, 0);
    let conflicting = SourceSnapshot::new(
        source.identity().source.clone(),
        0,
        "memory:origin-cursor".into(),
        b"ac".to_vec(),
        &mut budget(),
    )?;
    origins.push(Origin::Direct(conflicting.span(0, 1)?));
    assert_eq!(
        OriginGraph::validate_origins(&origins, &store, &mut budget()),
        Err(OriginError::Source(SourceError::MissingSnapshot))
    );
    origins.pop();
    origins.push(Origin::Synthetic {
        reason: String::new(),
        anchor: Some(source.span(0, 1)?),
    });
    assert_eq!(
        OriginGraph::validate_origins(&origins, &store, &mut budget()),
        Err(OriginError::EmptyReason)
    );
    origins.pop();
    origins.push(Origin::Generated {
        operation: OperationRef {
            schema: SchemaRef {
                package: "test".into(),
                revision: 0,
                digest: Digest([0; 32]),
            },
            name: String::new(),
        },
        callsite: Some(source.span(0, 1)?),
        inputs: vec![],
    });
    assert_eq!(
        OriginGraph::validate_origins(&origins, &store, &mut budget()),
        Err(OriginError::EmptyReason)
    );
    // A separately decoded equal identity must take the charged fallback.
    let independent = SourceSnapshot::new(
        source.identity().source.clone(),
        0,
        "memory:origin-cursor".into(),
        b"ab".to_vec(),
        &mut budget(),
    )?;
    origins.pop();
    origins.push(Origin::Direct(independent.span(0, 1)?));
    let mut short = budget().limits();
    short.work = exact_work + 4;
    assert_eq!(
        OriginGraph::validate_origins(&origins, &store, &mut Budget::new(short)),
        Err(OriginError::Stopped(StopReason::WorkLimit))
    );
    // Equal prefixes and an already warm cursor cannot grant another store's
    // declarations: a new import performs its own source checks.
    assert_eq!(
        OriginGraph::validate_origins(&origins[..64], &SourceStore::default(), &mut budget()),
        Err(OriginError::Source(SourceError::MissingSnapshot))
    );
    Ok(())
}
