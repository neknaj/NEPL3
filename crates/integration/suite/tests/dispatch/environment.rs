use super::*;
use nepl3_core::source::{SourceId, SourceSnapshot};
use nepl3_core::{
    origin::{Origin, OriginId},
    syntax::*,
};
use nepl3_suite::environment::*;

#[test]
fn projection_keeps_unicode_provenance_and_rejects_missing_sources() -> Result<(), String> {
    let (registry, call) = fixture()?;
    let snapshot = SourceSnapshot::new(
        SourceId("source".into()),
        1,
        "memory:source".into(),
        "値".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let span = snapshot.span(0, 3).map_err(|e| format!("{e:?}"))?;
    let origins = [
        Origin::Direct(span.clone()),
        Origin::Composite(vec![OriginId(0)]),
    ];
    let mut source = environment(&call);
    source.bindings[0].origin = Some(OriginId(1));
    assert!(
        source
            .validate(&origins, &SourceStore::default(), &registry, &mut budget())
            .is_err()
    );
    let mut sources = SourceStore::default();
    sources
        .insert_with_budget(snapshot, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let proof = source
        .validate(&origins, &sources, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let ns = &source.bindings[0].namespace;
    let expected = TypeDescriptor::TypedValue;
    let result = project(
        &proof,
        &[BindingProjection {
            namespace: ns,
            name: "answer",
            target_namespace: ns,
            target_name: "selected",
            expected: &expected,
        }],
        &[],
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(result.value().bindings[0].origin, Some(OriginId(1)));
    assert_eq!(result.origins(), origins);
    let id = span.snapshot_ref();
    let retained = result
        .sources()
        .get_revision_with_budget(&id.source, id.revision, &mut budget())
        .map_err(|e| format!("{e:?}"))?
        .ok_or("source")?;
    assert_eq!(retained.slice(&span).map_err(|e| format!("{e:?}"))?, "値");
    Ok(())
}

fn environment(call: &Invoke) -> Environment {
    Environment {
        bindings: vec![EnvironmentBinding {
            namespace: NamespaceRef {
                schema: call.operation.schema.clone(),
                name: "host".into(),
            },
            name: "answer".into(),
            value: call.input.clone(),
            origin: Some(OriginId(0)),
        }],
        resources: vec![ResourceContent {
            id: "input".into(),
            digest: Digest::of(b"data"),
            bytes: b"data".to_vec(),
        }],
    }
}

#[test]
fn explicit_selection_preserves_values_and_origin_arena() -> Result<(), String> {
    let (registry, call) = fixture()?;
    let source = environment(&call);
    let origins = [Origin::Synthetic {
        reason: "host fixture".into(),
        anchor: None,
    }];
    let sources = SourceStore::default();
    let proof = source
        .validate(&origins, &sources, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let empty = project(&proof, &[], &[], &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(empty.value().bindings.is_empty());
    assert!(empty.value().resources.is_empty());
    let namespace = NamespaceRef {
        schema: call.operation.schema.clone(),
        name: "guest".into(),
    };
    let expected = TypeDescriptor::Named(TypeRef {
        package: "test.dispatch".into(),
        revision: 1,
        name: "Number".into(),
    });
    let rule = BindingProjection {
        namespace: &source.bindings[0].namespace,
        name: "answer",
        target_namespace: &namespace,
        target_name: "value",
        expected: &expected,
    };
    let result =
        project(&proof, &[rule], &["input"], &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(result.value().bindings.len(), 1);
    assert_eq!(result.value().bindings[0].namespace, namespace);
    assert_eq!(result.value().bindings[0].name, "value");
    assert_eq!(result.value().bindings[0].value, call.input);
    assert_eq!(result.value().bindings[0].origin, Some(OriginId(0)));
    assert!(core::ptr::eq(result.origins(), origins.as_slice()));
    assert!(core::ptr::eq(result.sources(), &sources));
    assert_eq!(result.value().resources, source.resources);
    result
        .validate(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

#[test]
fn projection_rejects_missing_wrong_typed_and_duplicate_selections() -> Result<(), String> {
    let (registry, call) = fixture()?;
    let source = environment(&call);
    let origins = [Origin::Synthetic {
        reason: "fixture".into(),
        anchor: None,
    }];
    let sources = SourceStore::default();
    let proof = source
        .validate(&origins, &sources, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let ns = &source.bindings[0].namespace;
    let any = TypeDescriptor::TypedValue;
    let wrong = TypeDescriptor::Bool;
    let rule = |name, expected| BindingProjection {
        namespace: ns,
        name,
        target_namespace: ns,
        target_name: "selected",
        expected,
    };
    assert!(matches!(
        project(&proof, &[rule("absent", &any)], &[], &mut budget()),
        Err(ProjectionError::MissingBinding)
    ));
    assert!(matches!(
        project(&proof, &[rule("answer", &wrong)], &[], &mut budget()),
        Err(ProjectionError::Schema(SchemaError::WrongType))
    ));
    assert!(matches!(
        project(
            &proof,
            &[rule("answer", &any), rule("answer", &any)],
            &[],
            &mut budget()
        ),
        Err(ProjectionError::Syntax(SyntaxError::Environment))
    ));
    assert!(matches!(
        project(&proof, &[], &["missing"], &mut budget()),
        Err(ProjectionError::MissingResource)
    ));
    assert!(project(&proof, &[], &["input", "input"], &mut budget()).is_err());
    Ok(())
}

#[test]
fn invalid_source_environment_cannot_become_projection_authority() -> Result<(), String> {
    let (registry, call) = fixture()?;
    let source = environment(&call);
    let sources = SourceStore::default();
    assert!(matches!(
        source.validate(&[], &sources, &registry, &mut budget()),
        Err(SyntaxError::Reference)
    ));
    let origins = [Origin::Synthetic {
        reason: "fixture".into(),
        anchor: None,
    }];
    let mut duplicate = source.clone();
    duplicate.bindings.push(source.bindings[0].clone());
    assert!(matches!(
        duplicate.validate(&origins, &sources, &registry, &mut budget()),
        Err(SyntaxError::Environment)
    ));
    let mut corrupt = source.clone();
    corrupt.resources[0].bytes.push(0);
    assert!(
        corrupt
            .validate(&origins, &sources, &registry, &mut budget())
            .is_err()
    );
    assert!(matches!(
        source.validate(
            &origins,
            &sources,
            &SchemaRegistry::default(),
            &mut budget()
        ),
        Err(SyntaxError::Schema(SchemaError::Unfinalized))
    ));
    Ok(())
}

#[test]
fn projection_stop_returns_no_partial_output_and_preserves_source() -> Result<(), String> {
    let (registry, call) = fixture()?;
    let source = environment(&call);
    let original = source.clone();
    let origins = [Origin::Synthetic {
        reason: "fixture".into(),
        anchor: None,
    }];
    let sources = SourceStore::default();
    let proof = source
        .validate(&origins, &sources, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let expected = TypeDescriptor::TypedValue;
    for resource in [Resource::Work, Resource::AllocationUnits] {
        let mut limits = budget().limits();
        match resource {
            Resource::Work => limits.work = 0,
            _ => limits.allocation_units = 0,
        }
        let mut stopped = Budget::new(limits);
        let rule = BindingProjection {
            namespace: &source.bindings[0].namespace,
            name: "answer",
            target_namespace: &source.bindings[0].namespace,
            target_name: "selected",
            expected: &expected,
        };
        assert!(matches!(
            project(&proof, &[rule], &["input"], &mut stopped),
            Err(ProjectionError::Stopped(_))
        ));
        assert!(stopped.poll().is_err());
        assert_eq!(source, original);
    }
    Ok(())
}

#[test]
fn projection_reuses_origin_validation_when_context_grows() -> Result<(), String> {
    let (registry, call) = fixture()?;
    let source = environment(&call);
    let sources = SourceStore::default();
    let expected = TypeDescriptor::TypedValue;
    let mut costs = Vec::new();
    for count in [8, 32, 128] {
        let origins = vec![
            Origin::Synthetic {
                reason: "fixture".into(),
                anchor: None
            };
            count
        ];
        let proof = source
            .validate(&origins, &sources, &registry, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let mut measured = budget();
        let rule = BindingProjection {
            namespace: &source.bindings[0].namespace,
            name: "answer",
            target_namespace: &source.bindings[0].namespace,
            target_name: "value",
            expected: &expected,
        };
        let result = project(&proof, &[rule], &[], &mut measured).map_err(|e| format!("{e:?}"))?;
        result
            .validate(&mut measured)
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(result.origins().len(), count);
        costs.push(measured.usage());
    }
    // Only selected bindings are processed after the immutable context check.
    assert_eq!(costs[0], costs[1]);
    assert_eq!(costs[1], costs[2]);
    Ok(())
}

#[test]
fn namespaces_and_target_identity_remain_explicit() -> Result<(), String> {
    let (registry, call) = fixture()?;
    let mut source = environment(&call);
    let mut other = source.bindings[0].clone();
    other.namespace.name = "other".into();
    let TypedValue::Record(value) = &mut other.value else {
        return Err("record".into());
    };
    value.fields = vec![NdfValue::U64(99)];
    source.bindings.push(other);
    let origins = [Origin::Synthetic {
        reason: "fixture".into(),
        anchor: None,
    }];
    let sources = SourceStore::default();
    let proof = source
        .validate(&origins, &sources, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let expected = TypeDescriptor::TypedValue;
    let namespace = &source.bindings[1].namespace;
    let make = |target, name| BindingProjection {
        namespace,
        name: "answer",
        target_namespace: target,
        target_name: name,
        expected: &expected,
    };
    let selected = project(&proof, &[make(namespace, "result")], &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(selected.value().bindings[0].value, source.bindings[1].value);
    assert!(project(&proof, &[make(namespace, "")], &[], &mut budget()).is_err());
    let mut invalid = namespace.clone();
    invalid.schema.digest = Digest::of(b"wrong schema");
    assert!(project(&proof, &[make(&invalid, "result")], &[], &mut budget()).is_err());
    let original = source.clone();
    for allocation in [1, 64, 128] {
        let mut limits = budget().limits();
        limits.allocation_units = allocation;
        assert!(matches!(
            project(
                &proof,
                &[make(namespace, "result")],
                &["input"],
                &mut Budget::new(limits)
            ),
            Err(ProjectionError::Stopped(StopReason::AllocationLimit))
        ));
        assert_eq!(source, original);
    }
    Ok(())
}
