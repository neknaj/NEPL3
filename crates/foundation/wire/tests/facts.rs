use nepl3_core::{budget::*, facts::*, schema::*, source::*, value::*};
use nepl3_wire::{WireError, facts::*};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 10000,
        allocation_units: 20_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
#[test]
fn typed_facts_and_delta_roundtrip_preserve_ids_and_reject_semantic_forgery() -> Result<(), String>
{
    let mut registry = SchemaRegistry::default();
    let descriptor =
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("facts".into()),
        0,
        "memory:facts".into(),
        b"x".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let span = source.span(0, 1).map_err(|e| format!("{e:?}"))?;
    let original = FactSet {
        analysis_id: "analysis".into(),
        namespaces: vec![FactNamespace {
            schema: schema.clone(),
            name: "Names".into(),
            policy: NamespacePolicy::Lexical,
            root: ScopeId(0),
        }],
        scopes: vec![Scope {
            id: ScopeId(0),
            parent: None,
            origin: None,
        }],
        entities: vec![Entity {
            id: EntityId(42),
            scope: ScopeId(0),
            namespace: NamespaceRef(0),
            name: "x".into(),
            definition: Some(span.clone()),
            selection: Some(span.clone()),
            origin: None,
        }],
        occurrences: vec![Occurrence {
            id: OccurrenceId(93),
            scope: ScopeId(0),
            namespace: NamespaceRef(0),
            name: "x".into(),
            role: OccurrenceRole::Reference,
            span: span.clone(),
            origin: None,
            resolution: ReferenceResolution::Resolved(EntityId(42)),
        }],
        relations: vec![],
        edges: vec![],
        sources: vec![source],
        origins: vec![nepl3_core::origin::Origin::Direct(span)],
        source_maps: vec![],
    };
    let mut shared = budget();
    let mut admission = SourceAdmission::default();
    let bytes = encode_set(&original, &registry, &mut admission, &mut shared)
        .map_err(|e| format!("{e:?}"))?;
    let decoded =
        decode_set(&bytes, &registry, &mut admission, &mut shared).map_err(|e| format!("{e:?}"))?;
    assert_eq!(decoded, original);
    assert_eq!(shared.usage().source_bytes, 1);
    let checked = decoded
        .validate(&registry, &mut shared, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    let range = IdRange {
        start: 100,
        end: 200,
    };
    let authority = FactAuthority {
        analysis_id: "analysis".into(),
        current_scope: ScopeId(0),
        namespaces: vec![NamespaceRef(0)],
        writable_scopes: vec![],
        import_scopes: vec![],
        resolution_updates: vec![OccurrenceId(93)],
        relation_sources: vec![],
        reservation: FactReservation {
            scopes: range,
            entities: range,
            occurrences: range,
            relations: range,
        },
    };
    let delta = FactDelta {
        analysis_id: "analysis".into(),
        origin_base: 1,
        scopes: vec![Scope {
            id: ScopeId(100),
            parent: Some(ScopeId(0)),
            origin: None,
        }],
        entities: vec![],
        occurrences: vec![],
        relations: vec![],
        edges: vec![],
        resolutions: vec![ResolutionUpdate {
            occurrence: OccurrenceId(93),
            resolution: ReferenceResolution::Unresolved("x".into()),
        }],
        sources: vec![],
        origins: vec![],
        source_maps: vec![],
    };
    let bytes = encode_delta(
        &delta,
        &checked,
        &authority,
        &registry,
        &mut admission,
        &mut shared,
    )
    .map_err(|e| format!("{e:?}"))?;
    let round = decode_delta(
        &bytes,
        &checked,
        &authority,
        &registry,
        &mut admission,
        &mut shared,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(round, delta);
    let mut generated = delta.clone();
    let aux = SourceSnapshot::new(
        SourceId("facts-aux".into()),
        0,
        "memory:facts-aux".into(),
        b"aux".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let aux_span = aux.span(0, 3).map_err(|e| format!("{e:?}"))?;
    generated.entities.push(Entity {
        id: EntityId(100),
        scope: ScopeId(100),
        namespace: NamespaceRef(0),
        name: "aux".into(),
        definition: Some(aux_span.clone()),
        selection: Some(aux_span.clone()),
        origin: Some(nepl3_core::origin::OriginId(1)),
    });
    generated
        .origins
        .push(nepl3_core::origin::Origin::Direct(aux_span));
    generated.sources.push(aux);
    let auxiliary = encode_delta(
        &generated,
        &checked,
        &authority,
        &registry,
        &mut admission,
        &mut shared,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        decode_delta(
            &auxiliary,
            &checked,
            &authority,
            &registry,
            &mut admission,
            &mut shared
        )
        .map_err(|e| format!("{e:?}"))?,
        generated
    );
    assert_eq!(shared.usage().source_bytes, 4);
    let mut missing = nepl3_wire::decode(&auxiliary, &mut shared).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(record) = &mut missing else {
        return Err("FactDelta record".into());
    };
    record.fields[8] = NdfValue::List(vec![]);
    let missing = nepl3_wire::encode(&missing, &mut shared).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        decode_delta(
            &missing,
            &checked,
            &authority,
            &registry,
            &mut admission,
            &mut shared
        ),
        Err(WireError::Source(SourceError::MissingSnapshot))
    ));
    let mut denied = authority.clone();
    denied.resolution_updates.clear();
    assert!(matches!(
        decode_delta(
            &bytes,
            &checked,
            &denied,
            &registry,
            &mut admission,
            &mut shared
        ),
        Err(WireError::Facts(FactError::Authority))
    ));
    // The intrinsic/schema shape is still valid after changing a namespace ID;
    // the production typed decoder must independently reject the dangling reference.
    let bytes = encode_set(&original, &registry, &mut admission, &mut shared)
        .map_err(|e| format!("{e:?}"))?;
    let mut value = nepl3_wire::decode(&bytes, &mut shared).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(set) = &mut value else {
        return Err("FactSet record".into());
    };
    let NdfValue::List(entities) = &mut set.fields[3] else {
        return Err("entities".into());
    };
    let NdfValue::Record(entity) = &mut entities[0] else {
        return Err("entity".into());
    };
    let NdfValue::Record(namespace) = &mut entity.fields[2] else {
        return Err("namespace".into());
    };
    namespace.fields[0] = NdfValue::U64(999);
    let forged = nepl3_wire::encode(&value, &mut shared).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        decode_set(&forged, &registry, &mut admission, &mut shared),
        Err(WireError::Facts(FactError::MissingNamespace))
    ));
    let mut limits = budget().limits();
    limits.source_bytes = 0;
    assert!(matches!(
        decode_set(
            &bytes,
            &registry,
            &mut SourceAdmission::default(),
            &mut Budget::new(limits)
        ),
        Err(WireError::Stopped(StopReason::SourceLimit))
    ));
    Ok(())
}
