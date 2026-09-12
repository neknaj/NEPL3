use nepl3_core::{budget::*, facts::*, schema::*, source::*, value::*};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 10000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}

fn scope_chain(count: u64) -> Result<(FactSet, SchemaRegistry), String> {
    let (mut set, _, _, registry) = fixture()?;
    set.namespaces.clear();
    set.entities.clear();
    set.occurrences.clear();
    set.sources.clear();
    set.relations.clear();
    set.edges.clear();
    set.scopes = (0..count)
        .map(|i| Scope {
            id: ScopeId(u64::MAX - i * 7),
            parent: (i > 0).then(|| ScopeId(u64::MAX - (i - 1) * 7)),
            origin: None,
        })
        .collect();
    Ok((set, registry))
}

#[test]
fn sparse_scope_chain_uses_subquadratic_work_and_preserves_order() -> Result<(), String> {
    let mut previous = 0;
    for count in [256, 512, 1024] {
        let (set, registry) = scope_chain(count)?;
        let before = set.clone();
        let mut limits = budget().limits();
        limits.work = 1_000_000;
        limits.depth = 2048;
        let mut b = Budget::new(limits);
        set.validate(&registry, &mut b, &mut SourceAdmission::default())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(set, before);
        assert_eq!(b.usage().depth, count - 1);
        // Doubling a chain must not quadruple work. The previous independent
        // ancestry walks with linear ID lookup exceed this fixed work cap.
        if previous > 0 {
            assert!(b.usage().work < previous * 3);
        }
        previous = b.usage().work;
    }
    Ok(())
}

#[test]
fn indexed_scopes_reject_duplicates_missing_parents_cycles_and_depth() -> Result<(), String> {
    let (set, registry) = scope_chain(12)?;
    for case in 0..3 {
        let mut invalid = set.clone();
        let expected = match case {
            0 => {
                invalid.scopes[4].id = invalid.scopes[0].id;
                FactError::DuplicateId
            }
            1 => {
                invalid.scopes[4].parent = Some(ScopeId(123));
                FactError::MissingScope
            }
            _ => {
                invalid.scopes[0].parent = Some(invalid.scopes[11].id);
                FactError::Cycle
            }
        };
        assert_eq!(
            invalid
                .validate(&registry, &mut budget(), &mut SourceAdmission::default())
                .err(),
            Some(expected)
        );
    }
    for cap in [0, 10, 11] {
        let mut limits = budget().limits();
        limits.depth = cap;
        let mut b = Budget::new(limits);
        let result = set.validate(&registry, &mut b, &mut SourceAdmission::default());
        if cap == 11 {
            assert!(result.is_ok());
        } else {
            assert_eq!(
                result.err(),
                Some(FactError::Stopped(StopReason::DepthLimit))
            );
            assert_eq!(b.poll(), Err(StopReason::DepthLimit));
        }
    }
    Ok(())
}

#[test]
fn cached_scope_suffix_keeps_depth_and_stops_before_missing_parent() -> Result<(), String> {
    let (mut set, registry) = scope_chain(12)?;
    // Parent IDs sort before children: each child reuses an already checked suffix.
    for (i, scope) in set.scopes.iter_mut().enumerate() {
        scope.id = ScopeId(i as u64 * 7);
        scope.parent = (i > 0).then(|| ScopeId((i as u64 - 1) * 7));
    }
    for cap in [10, 11] {
        let mut limits = budget().limits();
        limits.depth = cap;
        let mut b = Budget::new(limits);
        let result = set.validate(&registry, &mut b, &mut SourceAdmission::default());
        if cap == 11 {
            assert!(result.is_ok());
            assert_eq!(b.usage().depth, 11);
        } else {
            assert_eq!(
                result.err(),
                Some(FactError::Stopped(StopReason::DepthLimit))
            );
        }
    }
    set.scopes[0].parent = Some(ScopeId(999));
    let mut limits = budget().limits();
    limits.depth = 0;
    let mut b = Budget::new(limits);
    assert_eq!(
        set.validate(&registry, &mut b, &mut SourceAdmission::default())
            .err(),
        Some(FactError::Stopped(StopReason::DepthLimit))
    );
    assert_eq!(b.poll(), Err(StopReason::DepthLimit));
    Ok(())
}
fn fixture() -> Result<(FactSet, FactAuthority, FactDelta, SchemaRegistry), String> {
    let mut b = budget();
    let descriptor =
        nepl3_core::schema::foundation::descriptor(&mut b).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor.reference(&mut b).map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    registry.finalize(&mut b).map_err(|e| format!("{e:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("facts-source".into()),
        0,
        "memory:facts".into(),
        b"x x".to_vec(),
        &mut b,
    )
    .map_err(|e| format!("{e:?}"))?;
    let span = source.span(0, 1).map_err(|e| format!("{e:?}"))?;
    let entity = |id, scope, ns| Entity {
        id: EntityId(id),
        scope: ScopeId(scope),
        namespace: NamespaceRef(ns),
        name: "x".into(),
        definition: Some(span.clone()),
        selection: Some(span.clone()),
        origin: None,
    };
    let occurrence = |id, scope, ns| Occurrence {
        id: OccurrenceId(id),
        scope: ScopeId(scope),
        namespace: NamespaceRef(ns),
        name: "x".into(),
        role: OccurrenceRole::Reference,
        span: span.clone(),
        origin: None,
        resolution: ReferenceResolution::Unresolved("x".into()),
    };
    let set = FactSet {
        analysis_id: "analysis-1".into(),
        namespaces: vec![
            FactNamespace {
                schema: schema.clone(),
                name: "Names".into(),
                policy: NamespacePolicy::Lexical,
                root: ScopeId(0),
            },
            FactNamespace {
                schema: schema.clone(),
                name: "Names".into(),
                policy: NamespacePolicy::Lexical,
                root: ScopeId(10),
            },
        ],
        scopes: vec![
            Scope {
                id: ScopeId(0),
                parent: None,
                origin: None,
            },
            Scope {
                id: ScopeId(1),
                parent: Some(ScopeId(0)),
                origin: None,
            },
            Scope {
                id: ScopeId(10),
                parent: None,
                origin: None,
            },
            Scope {
                id: ScopeId(11),
                parent: Some(ScopeId(10)),
                origin: None,
            },
        ],
        entities: vec![entity(1, 1, 0), entity(2, 11, 1)],
        occurrences: vec![occurrence(1, 1, 0), occurrence(2, 11, 1)],
        relations: vec![],
        edges: vec![],
        sources: vec![source],
        origins: vec![],
        source_maps: vec![],
    };
    let range = IdRange {
        start: 100,
        end: 110,
    };
    let authority = FactAuthority {
        analysis_id: "analysis-1".into(),
        current_scope: ScopeId(1),
        namespaces: vec![NamespaceRef(0)],
        writable_scopes: vec![],
        import_scopes: vec![ScopeId(11)],
        resolution_updates: vec![OccurrenceId(1)],
        relation_sources: vec![],
        reservation: FactReservation {
            scopes: range,
            entities: range,
            occurrences: range,
            relations: range,
        },
    };
    let delta = FactDelta {
        analysis_id: "analysis-1".into(),
        origin_base: 0,
        scopes: vec![Scope {
            id: ScopeId(100),
            parent: Some(ScopeId(1)),
            origin: None,
        }],
        entities: vec![entity(100, 100, 0)],
        occurrences: vec![occurrence(100, 100, 0)],
        relations: vec![Relation {
            id: RelationId(100),
            source: FactTarget::Entity(EntityId(100)),
            target: FactTarget::Entity(EntityId(2)),
            payload: TypedValue::Record(Record {
                schema,
                kind: "ScopeId".into(),
                fields: vec![NdfValue::U64(100)],
            }),
        }],
        edges: vec![ScopeEdge::Import {
            from: ScopeId(100),
            to: ScopeId(11),
            namespace: NamespaceRef(0),
        }],
        resolutions: vec![ResolutionUpdate {
            occurrence: OccurrenceId(1),
            resolution: ReferenceResolution::Resolved(EntityId(100)),
        }],
        sources: vec![],
        origins: vec![],
        source_maps: vec![],
    };
    Ok((set, authority, delta, registry))
}
#[test]
fn delta_keeps_current_scope_namespace_reservations_and_foreign_scope_authority()
-> Result<(), String> {
    let (set, authority, delta, registry) = fixture()?;
    let checked = set
        .validate(&registry, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    delta
        .validate(
            &checked,
            &authority,
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
    let mut bad = delta.clone();
    bad.scopes[0].parent = Some(ScopeId(11));
    assert!(
        bad.validate(
            &checked,
            &authority,
            &mut budget(),
            &mut SourceAdmission::default()
        )
        .is_err()
    );
    let mut bad = delta.clone();
    bad.relations[0].source = FactTarget::Entity(EntityId(2));
    assert!(matches!(
        bad.validate(
            &checked,
            &authority,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(FactError::Authority)
    ));
    let mut bad = delta.clone();
    bad.resolutions[0].occurrence = OccurrenceId(2);
    assert!(matches!(
        bad.validate(
            &checked,
            &authority,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(FactError::Authority)
    ));
    let mut bad = delta.clone();
    bad.entities[0].id = EntityId(1);
    assert!(matches!(
        bad.validate(
            &checked,
            &authority,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(FactError::DuplicateId)
    ));
    let mut bad = delta.clone();
    bad.entities[0].id = EntityId(110);
    assert!(
        bad.validate(
            &checked,
            &authority,
            &mut budget(),
            &mut SourceAdmission::default()
        )
        .is_err()
    );
    let mut bad = delta.clone();
    bad.scopes[0].parent = Some(ScopeId(100));
    assert!(matches!(
        bad.validate(
            &checked,
            &authority,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(FactError::Cycle)
    ));
    let mut bad = authority.clone();
    bad.writable_scopes.push(ScopeId(11));
    assert!(matches!(
        delta.validate(
            &checked,
            &bad,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(FactError::Authority)
    ));
    let mut bad = delta.clone();
    bad.origin_base = 1;
    assert!(matches!(
        bad.validate(
            &checked,
            &authority,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(FactError::Analysis)
    ));
    Ok(())
}
#[test]
fn facts_reject_missing_sources_namespace_roots_and_fresh_operation_limits() -> Result<(), String> {
    let (set, _, _, registry) = fixture()?;
    let mut bad = set.clone();
    bad.sources.clear();
    assert!(matches!(
        bad.validate(&registry, &mut budget(), &mut SourceAdmission::default()),
        Err(FactError::Span)
    ));
    let mut bad = set.clone();
    bad.occurrences[0].scope = ScopeId(11);
    assert!(matches!(
        bad.validate(&registry, &mut budget(), &mut SourceAdmission::default()),
        Err(FactError::MissingNamespace)
    ));
    for resource in [
        Resource::Work,
        Resource::Nodes,
        Resource::AllocationUnits,
        Resource::SourceBytes,
    ] {
        let mut limits = budget().limits();
        match resource {
            Resource::Work => limits.work = 0,
            Resource::Nodes => limits.nodes = 0,
            Resource::AllocationUnits => limits.allocation_units = 0,
            Resource::SourceBytes => limits.source_bytes = 0,
            _ => return Err("fixture resource".into()),
        };
        assert!(matches!(
            set.validate(
                &registry,
                &mut Budget::new(limits),
                &mut SourceAdmission::default()
            ),
            Err(FactError::Stopped(_))
        ));
    }
    Ok(())
}
