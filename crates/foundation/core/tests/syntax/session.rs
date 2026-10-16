use super::*;
use nepl3_core::syntax::group::*;

fn group(schema: &SchemaRef, origins: usize) -> ForeignClosureGroup {
    let mut owner_origins = vec![Origin::Synthetic {
        reason: "owner".into(),
        anchor: None,
    }];
    for n in 1..origins {
        owner_origins.push(Origin::Composite(vec![OriginId((n - 1) as u64)]));
    }
    let syntax = ForeignSyntax {
        schema: schema.clone(),
        category: "Expr".into(),
        root: NodeRef(0),
        bundle: bundle(schema),
        environment: EnvironmentRef {
            id: 0,
            digest: Digest([0; 32]),
        },
    };
    ForeignClosureGroup {
        provenances: vec![ForeignProvenance {
            origins: owner_origins,
            sources: vec![],
            source_maps: vec![],
        }],
        environments: vec![ForeignOwnerEnvironment {
            provenance: ProvenanceRef(0),
            environment: environment(),
        }],
        entries: vec![
            ForeignGroupEntry {
                owner: OwnerEnvironmentRef(0),
                syntax
            };
            3
        ],
    }
}
fn session<'g, 'r>(
    g: &'g ForeignClosureGroup,
    r: &'r SchemaRegistry,
    b: &mut Budget,
) -> Result<ForeignGroupValidation<'g, 'r>, String> {
    ForeignGroupValidation::new(g, r, b).map_err(|e| format!("{e:?}"))
}

#[test]
fn shared_validation_reuses_owner_cost_but_not_guest_checks() -> Result<(), String> {
    let (r, s) = registry().map_err(|e| format!("{e:?}"))?;
    let mut g = group(&s, 40);
    g.entries[2].syntax.root = NodeRef(99);
    let mut b = budget();
    let mut a = SourceAdmission::default();
    // Exclude first-use scope allocation from the cost comparison, so the
    // allocation reduction requires actual owner-check reuse.
    a.scope_with_budget(&mut b).map_err(|e| format!("{e:?}"))?;
    let mut checks = session(&g, &r, &mut b)?;
    let before = b.usage();
    checks
        .validate_entry(0, &mut b, &mut a)
        .map_err(|e| format!("{e:?}"))?;
    let first = b.usage();
    checks
        .validate_entry(1, &mut b, &mut a)
        .map_err(|e| format!("{e:?}"))?;
    let second = b.usage();
    #[cfg(target_has_atomic = "ptr")]
    {
        assert!(second.work - first.work < first.work - before.work);
        assert!(
            second.allocation_units - first.allocation_units
                < first.allocation_units - before.allocation_units
        );
    }
    assert_eq!(
        checks.validate_entry(2, &mut b, &mut a).err(),
        Some(SyntaxError::ForeignRoot)
    );
    assert_eq!(
        checks.validate_entry(99, &mut b, &mut a).err(),
        Some(SyntaxError::Reference)
    );
    Ok(())
}

#[test]
fn shared_validation_replays_owner_depth_at_current_caller() -> Result<(), String> {
    let (r, s) = registry().map_err(|e| format!("{e:?}"))?;
    let g = group(&s, 25);
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let mut checks = session(&g, &r, &mut b)?;
    checks
        .validate_entry(0, &mut b, &mut a)
        .map_err(|e| format!("{e:?}"))?;
    let depth = b.usage().depth;
    assert!(depth > 1);
    let base = b.limits().depth - depth + 1;
    let result = b.with_depth_at_least(base, |b| checks.validate_entry(1, b, &mut a).map(|_| ()));
    assert_eq!(
        result.err().and_then(|e| e.stop_reason()),
        Some(StopReason::DepthLimit)
    );
    assert_eq!(b.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

#[test]
fn changed_admission_ledger_rechecks_owner_sources() -> Result<(), String> {
    let (r, s) = registry().map_err(|e| format!("{e:?}"))?;
    let mut g = group(&s, 2);
    let snapshot = |text: &[u8]| {
        SourceSnapshot::new(
            SourceId("owner".into()),
            0,
            "memory:owner".into(),
            text.to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))
    };
    g.provenances[0].sources.push(snapshot(b"a")?);
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let mut checks = session(&g, &r, &mut b)?;
    checks
        .validate_entry(0, &mut b, &mut a)
        .map_err(|e| format!("{e:?}"))?;
    let mut replacement = SourceAdmission::default();
    replacement
        .admit_existing(&snapshot(b"b")?, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        checks.validate_entry(1, &mut b, &mut replacement).err(),
        Some(SyntaxError::Source(SourceError::IdentityConflict))
    );
    Ok(())
}

#[test]
fn shared_validation_keeps_distinct_owners_and_associations() -> Result<(), String> {
    let (r, s) = registry().map_err(|e| format!("{e:?}"))?;
    let mut g = group(&s, 3);
    g.provenances.push(ForeignProvenance {
        origins: vec![Origin::Composite(vec![OriginId(99)])],
        sources: vec![],
        source_maps: vec![],
    });
    g.environments.push(ForeignOwnerEnvironment {
        provenance: ProvenanceRef(1),
        environment: environment(),
    });
    g.entries[1].owner = OwnerEnvironmentRef(1);
    g.entries[2].syntax.environment.id = 9;
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let mut checks = session(&g, &r, &mut b)?;
    checks
        .validate_entry(0, &mut b, &mut a)
        .map_err(|e| format!("{e:?}"))?;
    assert!(checks.validate_entry(1, &mut b, &mut a).is_err());
    assert_eq!(
        checks.validate_entry(2, &mut b, &mut a).err(),
        Some(SyntaxError::Environment)
    );
    b.cancel();
    assert_eq!(
        checks.validate_entry(0, &mut b, &mut a).err(),
        Some(SyntaxError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn failed_owner_check_is_not_cached_and_stop_is_sticky() -> Result<(), String> {
    let (r, s) = registry().map_err(|e| format!("{e:?}"))?;
    let mut g = group(&s, 3);
    g.provenances[0]
        .origins
        .push(Origin::Composite(vec![OriginId(99)]));
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let mut checks = session(&g, &r, &mut b)?;
    let first = checks.validate_entry(0, &mut b, &mut a).err();
    let work = b.usage().work;
    assert!(first.is_some());
    assert_eq!(checks.validate_entry(1, &mut b, &mut a).err(), first);
    assert!(b.usage().work > work);
    let ceiling = Limits {
        work: b.usage().work,
        ..b.limits()
    };
    let result = b.with_ceiling(ceiling, |b| checks.validate_entry(0, b, &mut a).map(|_| ()));
    assert_eq!(
        result.err(),
        Some(SyntaxError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(
        checks.validate_entry(0, &mut b, &mut a).err(),
        Some(SyntaxError::Stopped(StopReason::WorkLimit))
    );
    Ok(())
}

#[test]
fn environment_failure_does_not_retain_successful_provenance() -> Result<(), String> {
    let (r, s) = registry().map_err(|e| format!("{e:?}"))?;
    let mut g = group(&s, 40);
    g.environments[0]
        .environment
        .value
        .resources
        .push(ResourceContent {
            id: "broken".into(),
            digest: Digest([0; 32]),
            bytes: vec![1, 2, 3],
        });
    let mut b = budget();
    let mut a = SourceAdmission::default();
    // Establish the ledger marker outside the measured calls.
    a.scope_with_budget(&mut b).map_err(|e| format!("{e:?}"))?;
    let mut checks = session(&g, &r, &mut b)?;
    let before = b.usage();
    assert_eq!(
        checks.validate_entry(0, &mut b, &mut a).err(),
        Some(SyntaxError::ResourceDigest)
    );
    let first = b.usage();
    assert_eq!(
        checks.validate_entry(1, &mut b, &mut a).err(),
        Some(SyntaxError::ResourceDigest)
    );
    let second = b.usage();
    assert_eq!(
        second.allocation_units - first.allocation_units,
        first.allocation_units - before.allocation_units
    );
    Ok(())
}

#[test]
fn cached_owner_validation_obeys_tightened_depth_ceiling() -> Result<(), String> {
    let (r, s) = registry().map_err(|e| format!("{e:?}"))?;
    let g = group(&s, 25);
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let mut checks = session(&g, &r, &mut b)?;
    checks
        .validate_entry(0, &mut b, &mut a)
        .map_err(|e| format!("{e:?}"))?;
    let limits = Limits {
        depth: b.usage().depth,
        ..b.limits()
    };
    let mut entered = false;
    let result = b.with_ceiling(limits, |b| {
        b.with_depth(|b| {
            entered = true;
            checks.validate_entry(1, b, &mut a).map(|_| ())
        })
    });
    assert!(
        entered,
        "the tighter ceiling must admit entry before replay rejects depth"
    );
    assert_eq!(
        result.err().and_then(|e| e.stop_reason()),
        Some(StopReason::DepthLimit)
    );
    Ok(())
}

#[test]
fn environment_depth_is_replayed_independently_of_provenance() -> Result<(), String> {
    let mut ty = TypeDescriptor::U64;
    let mut value = NdfValue::U64(1);
    for _ in 0..20 {
        ty = TypeDescriptor::List(Box::new(ty));
        value = NdfValue::List(vec![value]);
    }
    let descriptor = SchemaDescriptor {
        package: "session-depth".into(),
        revision: 1,
        types: vec![
            NamedType {
                name: "Node".into(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            },
            NamedType {
                name: "Payload".into(),
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "items".into(),
                        ty,
                    }],
                },
                constraints: vec![],
            },
        ],
        operations: vec![],
    };
    let s = descriptor
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut r = SchemaRegistry::default();
    r.register(s.clone(), descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    r.finalize(&mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut g = group(&s, 1);
    g.environments[0]
        .environment
        .value
        .bindings
        .push(EnvironmentBinding {
            namespace: NamespaceRef {
                schema: s.clone(),
                name: "test".into(),
            },
            name: "deep".into(),
            value: TypedValue::Record(Record {
                schema: s,
                kind: "Payload".into(),
                fields: vec![value],
            }),
            origin: None,
        });
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let mut checks = session(&g, &r, &mut b)?;
    checks
        .validate_entry(0, &mut b, &mut a)
        .map_err(|e| format!("{e:?}"))?;
    let depth = b.usage().depth;
    assert!(depth >= 20);
    let base = b.limits().depth - depth + 1;
    let result = b.with_depth_at_least(base, |b| checks.validate_entry(1, b, &mut a).map(|_| ()));
    assert_eq!(
        result.err().and_then(|e| e.stop_reason()),
        Some(StopReason::DepthLimit)
    );
    Ok(())
}
