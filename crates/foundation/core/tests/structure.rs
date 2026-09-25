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
        allocation_units: 16_384,
        ..budget().limits()
    });
    registry.validate(&ty, &value, &mut b)?;
    // Root + 100 rows + 3,200 cells; storage is for the pending frontier, not
    // a fresh allocation for each visited node. No value checks are skipped.
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
fn validation_bounds_frontier_before_expanding_wide_input() -> Result<(), SchemaError> {
    let (registry, _) = registry()?;
    let slot = core::mem::size_of::<(&TypeDescriptor, &NdfValue, u64)>() as u64;
    for width in [2, 100_000] {
        let value = NdfValue::List(vec![NdfValue::Unit; width]);
        for (limits, reason) in [
            (
                Limits {
                    work: 1,
                    ..budget().limits()
                },
                StopReason::WorkLimit,
            ),
            (
                Limits {
                    allocation_units: slot,
                    ..budget().limits()
                },
                StopReason::AllocationLimit,
            ),
        ] {
            let mut b = Budget::new(limits);
            assert!(
                matches!(registry.validate(&TypeDescriptor::NdfValue, &value, &mut b), Err(SchemaError::Stopped(found)) if found == reason)
            );
            assert_eq!(b.usage().allocation_units, slot);
            assert_eq!(b.poll(), Err(reason));
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
        (vec![wrong_fields, missing], SchemaError::FieldCount),
    ] {
        assert!(
            matches!(registry.validate(&TypeDescriptor::NdfValue, &NdfValue::List(children), &mut budget()), Err(found) if found == expected)
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
fn sorted_record_lookup_preserves_ids_errors_and_logarithmic_work() -> Result<(), SchemaError> {
    for count in [16usize, 64, 256] {
        let mut description = descriptor();
        description.types = (0..count)
            .rev()
            .map(|i| NamedType {
                name: format!("Kind{i:04}"),
                constraints: vec![],
                shape: TypeShape::Record { fields: vec![] },
            })
            .collect();
        let reference = description.reference(&mut budget())?;
        let mut registry = SchemaRegistry::default();
        registry.register(reference.clone(), description, &mut budget())?;
        registry.finalize(&mut budget())?;
        for i in 0..count {
            let name = format!("Kind{i:04}");
            assert_eq!(registry.kind_id(&reference, &name)?, i as u64);
            let mut measured = budget();
            registry.validate_record_fields(&reference, &name, &[], &mut measured)?;
            // One schema comparison, at most log2(count)+1 type comparisons,
            // and one record charge. Fixed-width names isolate search growth.
            assert!(measured.usage().work <= 56 + 17 * (count.ilog2() as u64 + 1));
            let mut limits = budget().limits();
            limits.work = measured.usage().work;
            registry.validate_record_fields(&reference, &name, &[], &mut Budget::new(limits))?;
            limits.work -= 1;
            let mut stopped = Budget::new(limits);
            assert_eq!(
                registry.validate_record_fields(&reference, &name, &[], &mut stopped),
                Err(SchemaError::Stopped(StopReason::WorkLimit))
            );
            assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        }
        for missing in ["", "Kind0000a", "Z"] {
            assert_eq!(
                registry.kind_id(&reference, missing),
                Err(SchemaError::UnknownType)
            );
            assert_eq!(
                registry.validate_record_fields(&reference, missing, &[], &mut budget()),
                Err(SchemaError::UnknownType)
            );
        }
    }
    Ok(())
}

#[test]
fn sorted_variant_lookup_preserves_all_validation_entry_points() -> Result<(), SchemaError> {
    let mut description = descriptor();
    description.types[0].shape = TypeShape::Variant {
        variants: ["Z", "A", "M"]
            .into_iter()
            .map(|name| VariantDescriptor {
                name: name.into(),
                fields: vec![FieldDescriptor {
                    name: "value".into(),
                    ty: TypeDescriptor::U64,
                }],
            })
            .collect(),
    };
    let reference = description.reference(&mut budget())?;
    let mut registry = SchemaRegistry::default();
    registry.register(reference.clone(), description, &mut budget())?;
    registry.finalize(&mut budget())?;
    for name in ["A", "M", "Z", "", "B", "ZZ"] {
        for fields in [vec![NdfValue::U64(7)], vec![], vec![NdfValue::Unit]] {
            let expected = if !["A", "M", "Z"].contains(&name) {
                Err(SchemaError::UnknownVariant)
            } else if fields.is_empty() {
                Err(SchemaError::FieldCount)
            } else if fields[0] == NdfValue::Unit {
                Err(SchemaError::WrongType)
            } else {
                Ok(())
            };
            let variant = Variant {
                schema: reference.clone(),
                type_name: "Pair".into(),
                variant: name.into(),
                fields,
            };
            assert_eq!(
                registry.validate_typed(&TypedValue::Variant(variant.clone()), &mut budget()),
                expected
            );
            let value = NdfValue::Variant(variant);
            assert_eq!(
                registry
                    .validate(&TypeDescriptor::TypedValue, &value, &mut budget())
                    .map(|_| ()),
                expected
            );
            let named = TypeDescriptor::Named(TypeRef {
                package: reference.package.clone(),
                revision: reference.revision,
                name: "Pair".into(),
            });
            assert_eq!(
                registry.validate(&named, &value, &mut budget()).map(|_| ()),
                expected
            );
        }
    }
    Ok(())
}
