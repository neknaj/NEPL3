use nepl3_core::{budget::*, origin::*, schema::*, source::*, syntax::*, value::*, view::*};
#[path = "syntax/capture.rs"]
mod capture;
#[path = "syntax/sources.rs"]
mod sources;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 100,
        nodes: 1_000_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
#[test]
fn syntax_preserves_nested_stop_causes_and_does_not_reclassify_semantic_errors() {
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
        for view in [
            ViewError::Stopped(reason),
            ViewError::Source(SourceError::Stopped(reason)),
            ViewError::Schema(SchemaError::Stopped(reason)),
            ViewError::Origin(OriginError::Stopped(reason)),
            ViewError::Origin(OriginError::Source(SourceError::Stopped(reason))),
        ] {
            assert_eq!(SyntaxError::View(view.clone()).stop_reason(), Some(reason));
            assert_eq!(SyntaxError::from(view), SyntaxError::Stopped(reason));
        }
        assert_eq!(
            SyntaxError::from(SourceError::Stopped(reason)),
            SyntaxError::Stopped(reason)
        );
        assert_eq!(
            SyntaxError::from(SchemaError::Stopped(reason)),
            SyntaxError::Stopped(reason)
        );
        assert_eq!(
            SyntaxError::from(OriginError::Source(SourceError::Stopped(reason))),
            SyntaxError::Stopped(reason)
        );
    }
    let semantic = ViewError::Schema(SchemaError::WrongType);
    assert_eq!(semantic.stop_reason(), None);
    assert_eq!(
        SyntaxError::from(semantic.clone()),
        SyntaxError::View(semantic)
    );
    assert_eq!(
        SyntaxError::from(SourceError::Bounds),
        SyntaxError::Source(SourceError::Bounds)
    );
}
fn registry() -> Result<(SchemaRegistry, SchemaRef), SchemaError> {
    let descriptor = SchemaDescriptor {
        package: "surface".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Node".into(),
            shape: TypeShape::Record { fields: vec![] },
            constraints: vec![],
        }],
        operations: vec![],
    };
    let reference = descriptor.reference(&mut budget())?;
    let mut registry = SchemaRegistry::default();
    registry.register(reference.clone(), descriptor, &mut budget())?;
    registry.finalize(&mut budget())?;
    Ok((registry, reference))
}
fn node(schema: &SchemaRef, fields: Vec<FieldValue>) -> SyntaxNode {
    SyntaxNode {
        schema: schema.clone(),
        kind: "Node".into(),
        fields,
        head: None,
        cover: None,
        origin: OriginId(0),
        token: None,
    }
}
fn bundle(schema: &SchemaRef) -> SyntaxBundle {
    SyntaxBundle {
        source_maps: vec![],
        sources: vec![],
        nodes: vec![node(schema, vec![])],
        origins: vec![Origin::Synthetic {
            reason: "generated test".into(),
            anchor: None,
        }],
        root: NodeRef(0),
        environments: vec![],
        tokens: vec![],
    }
}

#[test]
fn owned_syntax_copy_precharges_payloads_and_preserves_deep_foreign_ownership() -> Result<(), String>
{
    let (_, schema) = registry().map_err(|e| format!("{e:?}"))?;
    let mut inner = bundle(&schema);
    let source = SourceSnapshot::new(
        SourceId("s".repeat(100_000)),
        0,
        "memory:copy".into(),
        b"x".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    inner.sources.push(source);
    let mut limits = budget().limits();
    limits.allocation_units = 10_000;
    #[cfg(target_has_atomic = "ptr")]
    assert_eq!(
        inner.clone_with_budget(&mut Budget::new(limits)),
        Ok(inner.clone()),
        "immutable snapshot metadata is shared along with its text"
    );
    // Origin reasons are independently owned and must still be charged before
    // copying. Sharing snapshot metadata does not grant free arbitrary payloads.
    inner.origins[0] = Origin::Synthetic {
        reason: "r".repeat(100_000),
        anchor: None,
    };
    assert_eq!(
        inner.clone_with_budget(&mut Budget::new(limits)),
        Err(StopReason::AllocationLimit)
    );
    limits = budget().limits();
    limits.work = 0;
    assert_eq!(
        inner.clone_with_budget(&mut Budget::new(limits)),
        Err(StopReason::WorkLimit)
    );
    let mut operation = budget();
    let copied = inner
        .clone_with_budget(&mut operation)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(copied, inner);
    assert_eq!(
        operation.usage().source_bytes,
        0,
        "copy is storage, not a new operation admission"
    );
    drop(copied);
    // This ownership graph deliberately exceeds the machine call stack. It need not
    // be a valid language tree: the clone API copies data and makes no validity claim.
    for _ in 0..32_000 {
        let mut outer = bundle(&schema);
        outer.nodes[0].fields.push(guest(&schema, inner));
        inner = outer;
    }
    limits = budget().limits();
    limits.depth = 100;
    limits.work = u64::MAX;
    limits.allocation_units = u64::MAX;
    assert_eq!(
        inner.clone_with_budget(&mut Budget::new(limits)),
        Err(StopReason::DepthLimit)
    );
    limits.depth = u64::MAX;
    let copied = inner
        .clone_with_budget(&mut Budget::new(limits))
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(copied, inner);
    // FieldValue's public helper uses the same iterative traversal, including the box.
    let field = &inner.nodes[0].fields[0];
    assert_eq!(
        field
            .clone_with_budget(&mut Budget::new(limits))
            .map_err(|e| format!("{e:?}"))?,
        *field
    );
    Ok(())
}
fn guest(schema: &SchemaRef, inner: SyntaxBundle) -> FieldValue {
    FieldValue::Foreign(Box::new(ForeignSyntax {
        schema: schema.clone(),
        category: "Expr".into(),
        root: inner.root,
        bundle: inner,
        environment: EnvironmentRef {
            id: 0,
            digest: Digest([0; 32]),
        },
    }))
}
fn environment() -> EnvironmentEntry {
    EnvironmentEntry {
        id: 0,
        digest: Digest([0; 32]),
        value: Environment {
            bindings: vec![],
            resources: vec![],
        },
    }
}

#[test]
fn syntax_graph_checks_all_refs_and_cycles_while_allowing_shared_children()
-> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut input = bundle(&schema);
    input.nodes[0].fields = vec![FieldValue::Child(NodeRef(1)), FieldValue::Child(NodeRef(1))];
    input.nodes.push(node(&schema, vec![]));
    input.validate(&registry, &mut budget())?;
    input.nodes[1].fields = vec![FieldValue::Child(NodeRef(0))];
    assert!(matches!(
        input.validate(&registry, &mut budget()),
        Err(SyntaxError::Cycle)
    ));
    input.nodes[1].fields = vec![FieldValue::Child(NodeRef(u64::MAX))];
    assert!(matches!(
        input.validate(&registry, &mut budget()),
        Err(SyntaxError::Reference)
    ));
    input.nodes[1].fields.clear();
    input.nodes[1].origin = OriginId(u64::MAX);
    assert!(matches!(
        input.validate(&registry, &mut budget()),
        Err(SyntaxError::Reference)
    ));
    Ok(())
}

#[test]
fn foreign_ids_are_local_but_environment_and_root_must_match() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut input = bundle(&schema);
    input.environments.push(environment());
    input.nodes[0].fields.push(guest(&schema, bundle(&schema)));
    input.validate(&registry, &mut budget())?;
    if let FieldValue::Foreign(foreign) = &mut input.nodes[0].fields[0] {
        foreign.environment.digest = Digest([1; 32]);
    }
    assert!(matches!(
        input.validate(&registry, &mut budget()),
        Err(SyntaxError::Environment)
    ));
    if let FieldValue::Foreign(foreign) = &mut input.nodes[0].fields[0] {
        foreign.environment.digest = Digest([0; 32]);
        foreign.root = NodeRef(1);
    }
    assert!(matches!(
        input.validate(&registry, &mut budget()),
        Err(SyntaxError::ForeignRoot)
    ));
    Ok(())
}

#[test]
fn foreign_nesting_adds_to_containing_node_path_depth() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut inner = bundle(&schema);
    inner.nodes[0].fields.push(FieldValue::Child(NodeRef(1)));
    inner.nodes.push(node(&schema, vec![]));
    let mut outer = bundle(&schema);
    outer.environments.push(environment());
    outer.nodes[0].fields.push(FieldValue::Child(NodeRef(1)));
    outer.nodes.push(node(&schema, vec![guest(&schema, inner)]));
    let mut limits = budget().limits();
    limits.depth = 3;
    assert!(matches!(
        outer.validate(&registry, &mut Budget::new(limits)),
        Err(SyntaxError::Stopped(StopReason::DepthLimit))
    ));
    let mut budget = budget();
    outer.validate(&registry, &mut budget)?;
    assert_eq!(budget.usage().depth, 4);
    Ok(())
}

#[test]
fn syntax_validation_meters_shared_sources_without_recharging_text_copies()
-> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut allocations = Vec::new();
    for size in [1, 100_000] {
        let source = SourceSnapshot::new(
            SourceId("validation-source".into()),
            0,
            "memory:validation-source".into(),
            vec![b'x'; size],
            &mut budget(),
        )?;
        let mut input = bundle(&schema);
        input.sources.push(source);
        let mut validation = budget();
        input.validate(&registry, &mut validation)?;
        // Admission still accounts for all source bytes, even when storage is
        // shared. The bundle and its source remain unchanged after validation.
        assert_eq!(validation.usage().source_bytes, size as u64);
        assert_eq!(input.sources[0].text().len(), size);
        allocations.push(validation.usage().allocation_units);

        let mut stopped = budget();
        let mut admission = SourceAdmission::default();
        admission.admit_existing(&input.sources[0], &mut stopped)?;
        // Model earlier work in this same request. Leave enough allocation for
        // the validator's pending-bundle slot, but none for its source index.
        // Admission remains shared; no stopped request is given a fresh budget.
        stopped.charge(
            Resource::AllocationUnits,
            stopped.limits().allocation_units
                - stopped.usage().allocation_units
                - core::mem::size_of::<(&SyntaxBundle, u64)>() as u64,
        )?;
        assert!(matches!(
            input.validate_with_sources(&registry, &mut stopped, &mut admission),
            Err(SyntaxError::Stopped(StopReason::AllocationLimit))
        ));
        assert_eq!(
            stopped.usage().allocation_units,
            stopped.limits().allocation_units
        );
        input.sources.push(input.sources[0].clone());
        assert!(matches!(
            input.validate(&registry, &mut budget()),
            Err(SyntaxError::DuplicateSource)
        ));
    }
    // Pointer-atomic targets clone immutable storage handles; targets without
    // atomics actually copy text and must retain the corresponding charge.
    #[cfg(target_has_atomic = "ptr")]
    assert_eq!(allocations[0], allocations[1]);
    #[cfg(not(target_has_atomic = "ptr"))]
    assert!(allocations[1] >= allocations[0] + 99_999);
    Ok(())
}

#[test]
fn syntax_validation_borrows_origin_table_but_checks_every_entry() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut input = bundle(&schema);
    input.origins = vec![
        Origin::Synthetic {
            reason: "generated".into(),
            anchor: None
        };
        1_000
    ];
    // 100 KB allows the cycle/depth validator's state and traversal storage for
    // 1,000 disconnected origins, but not a second owned Origin table.
    let limits = Limits {
        allocation_units: 100_000,
        ..budget().limits()
    };
    input.validate(&registry, &mut Budget::new(limits))?;
    // The syntax root uses only origin 0. Unused provenance must still be checked.
    input.origins[999] = Origin::Composite(vec![OriginId(1_000)]);
    assert!(matches!(
        input.validate(&registry, &mut Budget::new(limits)),
        Err(SyntaxError::Origin(OriginError::Reference))
    ));
    Ok(())
}

#[test]
fn source_child_cover_must_follow_head_and_stay_in_parent() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let source = SourceSnapshot::new(
        SourceId("x".into()),
        0,
        "memory:x".into(),
        b"abc".to_vec(),
        &mut budget(),
    )?;
    let mut input = bundle(&schema);
    input.sources.push(source.clone());
    input.nodes[0].head = Some(source.span(0, 1)?);
    input.nodes[0].cover = Some(source.span(0, 3)?);
    input.nodes[0].fields.push(FieldValue::Child(NodeRef(1)));
    let mut child = node(&schema, vec![]);
    child.cover = Some(source.span(1, 3)?);
    input.nodes.push(child);
    input.validate(&registry, &mut budget())?;
    input.nodes[1].cover = Some(source.span(0, 1)?);
    assert!(matches!(
        input.validate(&registry, &mut budget()),
        Err(SyntaxError::ChildOrder)
    ));
    input.sources.push(source);
    assert!(matches!(
        input.validate(&registry, &mut budget()),
        Err(SyntaxError::DuplicateSource)
    ));
    Ok(())
}

#[test]
fn view_child_edges_are_acyclic_but_semantic_relation_cycles_are_allowed() -> Result<(), ViewError>
{
    let (registry, schema) = registry()?;
    let source = SourceSnapshot::new(
        SourceId("x".into()),
        0,
        "memory:x".into(),
        b"abc".to_vec(),
        &mut budget(),
    )?;
    let mut sources = SourceStore::default();
    sources.insert(source.clone())?;
    let element = ViewElement {
        kind: KindRef {
            schema: schema.clone(),
            local_kind: 0,
        },
        span: source.span(0, 3)?,
        fields: vec![],
        roles: vec![],
        relations: vec![ViewRelation {
            schema,
            kind: "corresponds".into(),
            target: ViewRef(0),
        }],
    };
    let mut views = ViewBundle {
        elements: vec![element],
        roots: vec![ViewRef(0)],
    };
    views.validate(&sources, &registry, &mut budget())?;
    views.elements[0].fields.push(ViewField {
        name: "body".into(),
        children: vec![ViewRef(0)],
    });
    assert_eq!(
        views.validate(&sources, &registry, &mut budget()),
        Err(ViewError::Cycle)
    );
    Ok(())
}

#[test]
fn token_internal_view_is_sidecar_and_trivia_retains_bom_bytes() -> Result<(), ViewError> {
    let (registry, schema) = registry()?;
    let source = SourceSnapshot::new(
        SourceId("x".into()),
        0,
        "memory:x".into(),
        "\u{feff}abc".as_bytes().to_vec(),
        &mut budget(),
    )?;
    let mut sources = SourceStore::default();
    sources.insert(source.clone())?;
    let kind = KindRef {
        schema,
        local_kind: 0,
    };
    let views = ViewBundle {
        elements: vec![ViewElement {
            kind: kind.clone(),
            span: source.span(3, 6)?,
            fields: vec![],
            roles: vec![],
            relations: vec![],
        }],
        roots: vec![ViewRef(0)],
    };
    let trivia = vec![Trivia {
        span: source.span(0, 3)?,
        kind: TriviaKind::Bom,
    }];
    let token = Token {
        kind,
        head: source.span(3, 6)?,
        payload: NdfValue::Unit,
        views: views.clone(),
        leading_trivia: trivia.clone(),
    };
    views.validate(&sources, &registry, &mut budget())?;
    token.validate(&sources, &registry, &mut budget())?;
    assert_eq!(source.text().len(), 6);
    Ok(())
}

#[test]
fn deeply_nested_foreign_bundle_cleanup_is_iterative() -> Result<(), SchemaError> {
    let (_, schema) = registry()?;
    let mut input = bundle(&schema);
    for _ in 0..10_000 {
        let mut outer = bundle(&schema);
        outer.nodes[0].fields.push(guest(&schema, input));
        input = outer;
    }
    let cloned = input.clone();
    assert_eq!(input, cloned);
    assert!(format!("{input:?}").starts_with("SyntaxBundle"));
    drop(input);
    drop(cloned);
    Ok(())
}

#[test]
fn token_owns_even_unreachable_view_elements() -> Result<(), ViewError> {
    let (registry, schema) = registry()?;
    let source = SourceSnapshot::new(
        SourceId("x".into()),
        0,
        "memory:x".into(),
        b"ab".to_vec(),
        &mut budget(),
    )?;
    let element = |start, end| -> Result<ViewElement, SourceError> {
        Ok(ViewElement {
            kind: KindRef {
                schema: schema.clone(),
                local_kind: 0,
            },
            span: source.span(start, end)?,
            fields: vec![],
            roles: vec![],
            relations: vec![],
        })
    };
    let token = Token {
        kind: KindRef {
            schema: schema.clone(),
            local_kind: 0,
        },
        head: source.span(0, 1)?,
        payload: NdfValue::Unit,
        views: ViewBundle {
            elements: vec![element(0, 1)?, element(1, 2)?],
            roots: vec![ViewRef(0)],
        },
        leading_trivia: vec![],
    };
    let mut sources = SourceStore::default();
    sources.insert(source)?;
    assert_eq!(
        token.validate(&sources, &registry, &mut budget()),
        Err(ViewError::Cover)
    );
    Ok(())
}
