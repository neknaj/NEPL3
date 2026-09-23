use super::*;
use nepl3_core::source::{SourceId, SourceSnapshot};
use nepl3_core::{
    origin::{Origin, OriginId},
    syntax::*,
};
use nepl3_suite::environment::*;

#[test]
fn indexed_environment_rejects_duplicates_and_preserves_input_at_every_stop() -> Result<(), String>
{
    let (registry, call) = fixture()?;
    let mut source = environment(&call);
    let binding = source.bindings[0].clone();
    source.bindings = ["丙", "甲", "乙"]
        .into_iter()
        .map(|name| EnvironmentBinding {
            name: name.into(),
            ..binding.clone()
        })
        .collect();
    let resource = source.resources[0].clone();
    source.resources = ["三", "一", "二"]
        .into_iter()
        .map(|id| ResourceContent {
            id: id.into(),
            ..resource.clone()
        })
        .collect();
    let before = source.clone();
    let origins = [Origin::Synthetic {
        reason: "fixture".into(),
        anchor: None,
    }];
    let sources = SourceStore::default();
    let mut measured = budget();
    let proof = source
        .validate(&origins, &sources, &registry, &mut measured)
        .map_err(|e| format!("{e:?}"))?;
    for binding in &source.bindings {
        assert_eq!(
            proof
                .binding(&binding.namespace, &binding.name, &mut budget())
                .map_err(|e| format!("{e:?}"))?,
            Some(binding)
        );
        let mut other = binding.namespace.clone();
        other.schema.revision += 1;
        assert!(
            proof
                .binding(&other, &binding.name, &mut budget())
                .map_err(|e| format!("{e:?}"))?
                .is_none()
        );
    }
    for resource in &source.resources {
        assert_eq!(
            proof
                .resource(&resource.id, &mut budget())
                .map_err(|e| format!("{e:?}"))?,
            Some(resource)
        );
    }
    assert!(
        proof
            .resource("missing", &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .is_none()
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        proof
            .resource("missing", &mut cancelled)
            .err()
            .and_then(|e| e.stop_reason()),
        Some(StopReason::Cancelled)
    );
    assert_eq!(
        proof
            .binding(&binding.namespace, "missing", &mut cancelled)
            .err()
            .and_then(|e| e.stop_reason()),
        Some(StopReason::Cancelled)
    );
    let usage = measured.usage();
    for (axis, maximum) in [usage.work, usage.allocation_units].into_iter().enumerate() {
        for limit in 0..maximum {
            let mut limits = budget().limits();
            let reason = if axis == 0 {
                limits.work = limit;
                StopReason::WorkLimit
            } else {
                limits.allocation_units = limit;
                StopReason::AllocationLimit
            };
            let error = source
                .validate(&origins, &sources, &registry, &mut Budget::new(limits))
                .err()
                .ok_or("insufficient construction budget accepted")?;
            assert_eq!(error.stop_reason(), Some(reason));
            assert_eq!(source, before);
        }
    }
    let mut duplicate = source.clone();
    duplicate.bindings[2] = duplicate.bindings[0].clone();
    assert!(matches!(
        duplicate.validate(&origins, &sources, &registry, &mut budget()),
        Err(SyntaxError::Environment)
    ));
    let mut duplicate = source.clone();
    duplicate.resources[2] = duplicate.resources[0].clone();
    assert!(matches!(
        duplicate.validate(&origins, &sources, &registry, &mut budget()),
        Err(SyntaxError::ResourceDigest)
    ));
    assert_eq!(source, before);
    Ok(())
}

#[test]
fn indexed_environment_growth_preserves_order_and_scales_across_input_axes() -> Result<(), String> {
    fn measure(
        bindings: usize,
        selected: usize,
        resources: usize,
        width: usize,
    ) -> Result<(u64, u64), String> {
        let (registry, call) = fixture()?;
        let mut source = environment(&call);
        let binding = source.bindings[0].clone();
        let resource = source.resources[0].clone();
        // Reverse declaration order prevents a sorted-input-only fast path.
        source.bindings = (0..bindings)
            .rev()
            .map(|id| EnvironmentBinding {
                name: format!("{}-{id:04}", "名".repeat(width)),
                ..binding.clone()
            })
            .collect();
        source.resources = (0..resources)
            .rev()
            .map(|id| ResourceContent {
                id: format!("{}-{id:04}", "源".repeat(width)),
                ..resource.clone()
            })
            .collect();
        let before = source.clone();
        let origins = [Origin::Synthetic {
            reason: "fixture".into(),
            anchor: None,
        }];
        let sources = SourceStore::default();
        let limits = Limits {
            work: 100_000_000,
            allocation_units: 100_000_000,
            ..budget().limits()
        };
        let mut construction = Budget::new(limits);
        let proof = source
            .validate(&origins, &sources, &registry, &mut construction)
            .map_err(|e| format!("{e:?}"))?;
        let expected = TypeDescriptor::TypedValue;
        let rules: Vec<_> = source
            .bindings
            .iter()
            .rev()
            .take(selected)
            .map(|binding| BindingProjection {
                namespace: &binding.namespace,
                name: &binding.name,
                target_namespace: &binding.namespace,
                target_name: &binding.name,
                expected: &expected,
            })
            .collect();
        let resource_ids: Vec<_> = source
            .resources
            .iter()
            .rev()
            .map(|r| r.id.as_str())
            .collect();
        let mut selection = Budget::new(limits);
        let projected =
            project(&proof, &rules, &resource_ids, &mut selection).map_err(|e| format!("{e:?}"))?;
        assert_eq!(projected.value().bindings.len(), selected);
        for (actual, expected) in projected
            .value()
            .bindings
            .iter()
            .zip(source.bindings.iter().rev())
        {
            assert_eq!(actual, expected);
        }
        assert_eq!(
            projected.value().resources,
            source.resources.iter().rev().cloned().collect::<Vec<_>>()
        );
        assert_eq!(projected.origins(), origins);
        assert_eq!(source, before);
        // A retained index serves a second projection with identical cost.
        let mut repeated = Budget::new(limits);
        let again =
            project(&proof, &rules, &resource_ids, &mut repeated).map_err(|e| format!("{e:?}"))?;
        assert_eq!(again.value(), projected.value());
        assert_eq!(repeated.usage(), selection.usage());
        Ok((construction.usage().work, selection.usage().work))
    }
    for axis in 0..4 {
        let mut previous = None;
        for size in [64, 128, 256] {
            let (bindings, selected, resources, width) = match axis {
                0 => (size, 16, 0, 1),
                1 => (512, size, 0, 1),
                2 => (1, 1, size, 1),
                _ => (128, 64, 64, size / 8),
            };
            let usage = measure(bindings, selected, resources, width)?;
            if let Some((build, project)) = previous {
                // Doubling each independent axis allows logarithmic sorting
                // overhead while rejecting repeated all-pairs comparisons.
                assert!(
                    usage.0 < build * 3,
                    "axis {axis}: build {previous:?} -> {usage:?}"
                );
                assert!(
                    usage.1 < project * 3,
                    "axis {axis}: project {previous:?} -> {usage:?}"
                );
            }
            previous = Some(usage);
        }
    }
    Ok(())
}

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
