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
