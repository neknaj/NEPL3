use super::*;

fn owner(schema: &SchemaRef) -> Result<SyntaxBundle, SyntaxError> {
    let source = SourceSnapshot::new(
        SourceId("owner".into()),
        3,
        "memory:owner".into(),
        "値".as_bytes().to_vec(),
        &mut budget(),
    )?;
    let mut value = bundle(schema);
    value.origins = vec![
        Origin::Composite(vec![OriginId(1)]),
        Origin::Direct(source.span(0, 3)?),
    ];
    value.sources = vec![source];
    value.environments.push(environment());
    value.nodes[0].fields = vec![guest(schema, bundle(schema)), guest(schema, bundle(schema))];
    Ok(value)
}

#[test]
fn checked_owner_reuses_graphs_and_rechecks_guest_environment_and_admission()
-> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut input = owner(&schema)?;
    for _ in 0..128 {
        input.origins.push(Origin::Synthetic {
            reason: "owner table retained in arena order".into(),
            anchor: None,
        });
    }
    let checked = input.validate(&registry, &mut budget())?;
    let mut closure = ForeignCapture::new(&checked).capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut full = budget();
    let mut admission = SourceAdmission::default();
    for _ in 0..8 {
        closure.validate(&registry, &mut full, &mut admission)?;
    }
    let mut shared = budget();
    let mut admission = SourceAdmission::default();
    let proof = closure
        .provenance
        .validate(&registry, &mut shared, &mut admission)?;
    for _ in 0..8 {
        let validated = proof.validate_closure(&closure, &mut shared, &mut admission)?;
        assert!(core::ptr::eq(validated.value(), &closure));
    }
    assert!(shared.usage().work < full.usage().work);
    assert!(shared.usage().allocation_units < full.usage().allocation_units);
    // A fresh admission pays for the owner source even when graphs are reused.
    let mut fresh = budget();
    proof.validate_closure(&closure, &mut fresh, &mut SourceAdmission::default())?;
    assert_eq!(fresh.usage().source_bytes, 3);
    let mut conflicting = SourceAdmission::default();
    conflicting.create(
        SourceId("owner".into()),
        3,
        "memory:owner".into(),
        b"bad".to_vec(),
        &mut budget(),
    )?;
    assert_eq!(
        proof
            .validate_closure(&closure, &mut budget(), &mut conflicting)
            .err(),
        Some(SyntaxError::Source(SourceError::IdentityConflict))
    );
    // Neither matching data nor a numeric OriginRef grants a different owner
    // access to this graph proof. Its nonempty tables must be checked separately.
    let mut independent = closure.clone();
    independent.provenance = OwnerProvenance::from_parts(
        closure.provenance.origins().to_vec(),
        closure.provenance.sources().to_vec(),
        closure.provenance.source_maps().to_vec(),
    );
    assert_eq!(
        proof
            .validate_closure(&independent, &mut budget(), &mut admission)
            .err(),
        Some(SyntaxError::Reference)
    );
    let saved = closure.owner_environment.digest;
    closure.owner_environment.digest.0[0] ^= 1;
    assert_eq!(
        proof
            .validate_closure(&closure, &mut budget(), &mut admission)
            .err(),
        Some(SyntaxError::Environment)
    );
    closure.owner_environment.digest = saved;
    closure.syntax.root = NodeRef(99);
    assert_eq!(
        proof
            .validate_closure(&closure, &mut budget(), &mut admission)
            .err(),
        Some(SyntaxError::ForeignRoot)
    );
    closure.syntax.root = NodeRef(0);
    closure.syntax.bundle.nodes[0].origin = OriginId(99);
    assert!(
        proof
            .validate_closure(&closure, &mut budget(), &mut admission)
            .is_err()
    );
    Ok(())
}

#[test]
fn owner_proof_checks_unused_entries_registry_and_budget_boundaries() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let input = owner(&schema)?;
    let checked = input.validate(&registry, &mut budget())?;
    let closure = ForeignCapture::new(&checked).capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut origins = closure.provenance.origins().to_vec();
    origins.push(Origin::Composite(vec![OriginId(999)]));
    let invalid =
        OwnerProvenance::from_parts(origins, closure.provenance.sources().to_vec(), vec![]);
    assert!(
        invalid
            .validate(&registry, &mut budget(), &mut SourceAdmission::default())
            .is_err()
    );
    assert_eq!(
        closure
            .provenance
            .validate(
                &SchemaRegistry::default(),
                &mut budget(),
                &mut SourceAdmission::default()
            )
            .err(),
        Some(SyntaxError::Schema(SchemaError::Unfinalized))
    );
    let mut other = SchemaRegistry::default();
    other.finalize(&mut budget())?;
    let mut admission = SourceAdmission::default();
    let proof = closure
        .provenance
        .validate(&other, &mut budget(), &mut admission)?;
    assert!(
        proof
            .validate_closure(&closure, &mut budget(), &mut admission)
            .is_err()
    );
    for prepare in [true, false] {
        let mut admission = SourceAdmission::default();
        let mut measured = budget();
        let proof = closure
            .provenance
            .validate(&registry, &mut measured, &mut admission)?;
        if !prepare {
            measured = budget();
            proof.validate_closure(&closure, &mut measured, &mut admission)?;
        }
        for (resource, usage, reason) in [
            (Resource::Work, measured.usage().work, StopReason::WorkLimit),
            (
                Resource::AllocationUnits,
                measured.usage().allocation_units,
                StopReason::AllocationLimit,
            ),
        ] {
            assert!(usage > 0);
            for shortage in [0, 1] {
                let mut limits = budget().limits();
                match resource {
                    Resource::Work => limits.work = usage - shortage,
                    _ => limits.allocation_units = usage - shortage,
                }
                let mut limited = Budget::new(limits);
                let result = if prepare {
                    closure
                        .provenance
                        .validate(&registry, &mut limited, &mut SourceAdmission::default())
                        .map(|_| ())
                } else {
                    proof
                        .validate_closure(&closure, &mut limited, &mut admission)
                        .map(|_| ())
                };
                if shortage == 0 {
                    result?;
                } else {
                    assert_eq!(result.err().and_then(|e| e.stop_reason()), Some(reason));
                    assert_eq!(limited.poll(), Err(reason));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn captures_share_storage_but_revalidate_each_admission_and_registry() -> Result<(), SyntaxError> {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<OwnerProvenance>();
    let (registry, schema) = registry()?;
    let input = owner(&schema)?;
    let checked = input.validate(&registry, &mut budget())?;
    let mut captures = ForeignCapture::new(&checked);
    let mut admission = SourceAdmission::default();
    let mut first_budget = budget();
    let first = captures.capture_at(NodeRef(0), 0, &registry, &mut first_budget, &mut admission)?;
    let mut second_budget = budget();
    let second =
        captures.capture_at(NodeRef(0), 1, &registry, &mut second_budget, &mut admission)?;
    assert_eq!(first, second);
    #[cfg(target_has_atomic = "ptr")]
    {
        assert!(core::ptr::eq(
            first.provenance.origins(),
            second.provenance.origins()
        ));
        assert!(first_budget.usage().allocation_units > second_budget.usage().allocation_units);
    }
    // Storage reuse does not import another operation's source admission.
    let mut new_budget = budget();
    captures.capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut new_budget,
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(new_budget.usage().source_bytes, 3);
    let mut conflicting = SourceAdmission::default();
    conflicting.create(
        SourceId("owner".into()),
        3,
        "memory:owner".into(),
        b"bad".to_vec(),
        &mut budget(),
    )?;
    assert!(matches!(
        captures.capture_at(NodeRef(0), 0, &registry, &mut budget(), &mut conflicting),
        Err(SyntaxError::Source(SourceError::IdentityConflict)),
    ));
    let mut other_registry = SchemaRegistry::default();
    other_registry.finalize(&mut budget())?;
    assert!(
        captures
            .capture_at(
                NodeRef(0),
                0,
                &other_registry,
                &mut budget(),
                &mut admission
            )
            .is_err()
    );
    let unfinalized = SchemaRegistry::default();
    assert_eq!(
        captures.capture_at(NodeRef(0), 0, &unfinalized, &mut budget(), &mut admission),
        Err(SyntaxError::Schema(SchemaError::Unfinalized))
    );
    // Failed registry replacement does not bless it or prevent returning to
    // the original immutable registry.
    assert_eq!(
        captures.capture_at(NodeRef(0), 0, &registry, &mut budget(), &mut admission)?,
        first
    );
    // The captured value owns its tables after both context and owner expire.
    drop(captures);
    drop(input);
    first.validate(&registry, &mut budget(), &mut SourceAdmission::default())?;
    assert_eq!(first.provenance.origins().len(), 2);
    Ok(())
}

#[test]
fn repeated_capture_reuses_owner_validation_and_keeps_exact_stop_boundaries()
-> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut input = owner(&schema)?;
    for _ in 0..128 {
        input.origins.push(Origin::Synthetic {
            reason: "unused but validated owner entry".into(),
            anchor: None,
        });
    }
    let checked = input.validate(&registry, &mut budget())?;
    let mut captures = ForeignCapture::new(&checked);
    let expected = captures.capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut full = budget();
    let mut reused = budget();
    let mut full_admission = SourceAdmission::default();
    let mut reused_admission = SourceAdmission::default();
    for _ in 0..8 {
        assert_eq!(
            ForeignClosure::capture_at(
                &checked,
                NodeRef(0),
                0,
                &registry,
                &mut full,
                &mut full_admission
            )?,
            expected
        );
        assert_eq!(
            captures.capture_at(NodeRef(0), 0, &registry, &mut reused, &mut reused_admission)?,
            expected
        );
    }
    #[cfg(target_has_atomic = "ptr")]
    {
        assert!(reused.usage().work * 2 < full.usage().work);
        assert!(reused.usage().allocation_units < full.usage().allocation_units);
    }
    // First preparation and warm capture both retain exact Work/Allocation
    // stops. A source admission is local to every measured operation.
    for warm in [false, true] {
        let mut measured_capture = ForeignCapture::new(&checked);
        if warm {
            measured_capture.capture_at(
                NodeRef(0),
                0,
                &registry,
                &mut budget(),
                &mut SourceAdmission::default(),
            )?;
        }
        let mut measured = budget();
        measured_capture.capture_at(
            NodeRef(0),
            0,
            &registry,
            &mut measured,
            &mut SourceAdmission::default(),
        )?;
        for (resource, used, reason) in [
            (Resource::Work, measured.usage().work, StopReason::WorkLimit),
            (
                Resource::AllocationUnits,
                measured.usage().allocation_units,
                StopReason::AllocationLimit,
            ),
        ] {
            for shortage in [0, 1] {
                let mut candidate = ForeignCapture::new(&checked);
                if warm {
                    candidate.capture_at(
                        NodeRef(0),
                        0,
                        &registry,
                        &mut budget(),
                        &mut SourceAdmission::default(),
                    )?;
                }
                let mut limits = budget().limits();
                match resource {
                    Resource::Work => limits.work = used - shortage,
                    _ => limits.allocation_units = used - shortage,
                }
                let mut limited = Budget::new(limits);
                let result = candidate.capture_at(
                    NodeRef(0),
                    0,
                    &registry,
                    &mut limited,
                    &mut SourceAdmission::default(),
                );
                if shortage == 0 {
                    assert_eq!(result?, expected);
                } else {
                    assert_eq!(result, Err(SyntaxError::Stopped(reason)));
                    assert_eq!(limited.poll(), Err(reason));
                    assert_eq!(
                        candidate.capture_at(
                            NodeRef(0),
                            0,
                            &registry,
                            &mut budget(),
                            &mut SourceAdmission::default()
                        )?,
                        expected
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn capture_reuse_observes_current_depth_after_high_water_preparation() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut input = owner(&schema)?;
    // The existing composite->direct chain has length 2. Eight parents extend
    // it to 10, including entries unused by the selected foreign field.
    let mut previous = OriginId(0);
    for _ in 0..8 {
        let next = OriginId(input.origins.len() as u64);
        input.origins.push(Origin::Composite(vec![previous]));
        previous = next;
    }
    let checked = input.validate(&registry, &mut budget())?;
    let mut captures = ForeignCapture::new(&checked);
    let mut preparation = budget();
    preparation.observe_depth(99)?;
    captures.capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut preparation,
        &mut SourceAdmission::default(),
    )?;
    for caller in [0, 3] {
        for shortage in [0, 1] {
            let mut limited = Budget::new(Limits {
                depth: caller + 10 - shortage,
                ..budget().limits()
            });
            let result = limited.with_depth_at_least(caller, |b| {
                captures.capture_at(NodeRef(0), 1, &registry, b, &mut SourceAdmission::default())
            });
            if shortage == 0 {
                result?;
                assert_eq!(limited.usage().depth, caller + 10);
            } else {
                assert_eq!(result, Err(SyntaxError::Stopped(StopReason::DepthLimit)));
                assert_eq!(limited.poll(), Err(StopReason::DepthLimit));
            }
            assert_eq!(limited.current_depth(), 0);
        }
    }
    Ok(())
}

#[test]
fn registry_revalidation_stop_preserves_the_previous_capture_proof() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let (other, _) = super::registry()?;
    let mut input = owner(&schema)?;
    for _ in 0..128 {
        input.origins.push(Origin::Synthetic {
            reason: "registry replacement validates this entry".into(),
            anchor: None,
        });
    }
    let checked = input.validate(&registry, &mut budget())?;
    let mut captures = ForeignCapture::new(&checked);
    let expected = captures.capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut warm = budget();
    captures.capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut warm,
        &mut SourceAdmission::default(),
    )?;
    let mut owner_cost = budget();
    expected
        .provenance
        .validate(&other, &mut owner_cost, &mut SourceAdmission::default())?;
    for (resource, used, reason) in [
        (
            Resource::Work,
            owner_cost.usage().work,
            StopReason::WorkLimit,
        ),
        (
            Resource::AllocationUnits,
            owner_cost.usage().allocation_units,
            StopReason::AllocationLimit,
        ),
    ] {
        let mut limits = budget().limits();
        match resource {
            Resource::Work => limits.work = used / 2,
            _ => limits.allocation_units = used / 2,
        }
        let mut limited = Budget::new(limits);
        assert_eq!(
            captures.capture_at(
                NodeRef(0),
                0,
                &other,
                &mut limited,
                &mut SourceAdmission::default()
            ),
            Err(SyntaxError::Stopped(reason))
        );
        assert_eq!(limited.poll(), Err(reason));
        let mut restored = budget();
        assert_eq!(
            captures.capture_at(
                NodeRef(0),
                0,
                &registry,
                &mut restored,
                &mut SourceAdmission::default()
            )?,
            expected
        );
        assert_eq!(restored.usage(), warm.usage());
    }
    // A different finalized registry can subsequently complete its own proof.
    assert_eq!(
        captures.capture_at(
            NodeRef(0),
            0,
            &other,
            &mut budget(),
            &mut SourceAdmission::default()
        )?,
        expected
    );
    Ok(())
}

#[test]
fn owner_proof_reapplies_origin_and_map_depth_in_each_caller() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let input = owner(&schema)?;
    let checked = input.validate(&registry, &mut budget())?;
    let closure = ForeignCapture::new(&checked).capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    // Three different depth computations: Origin DAG, snapshot DAG, and a
    // pointwise acyclic chain within one snapshot (coarse snapshot self-cycle).
    for path in 0..3 {
        let mut value = closure.clone();
        let mut origins = vec![Origin::Synthetic {
            reason: "root".into(),
            anchor: None,
        }];
        let mut sources = Vec::new();
        let mut maps = Vec::new();
        if path == 0 {
            for index in 0..8 {
                origins.push(Origin::Composite(vec![OriginId(index)]));
            }
        } else if path == 1 {
            for index in 0..9 {
                sources.push(SourceSnapshot::new(
                    SourceId(format!("map-{index}")),
                    0,
                    format!("memory:map-{index}"),
                    b"x".to_vec(),
                    &mut budget(),
                )?);
            }
            for pair in sources.windows(2) {
                maps.push(Mapping {
                    source: pair[0].span(0, 1)?,
                    target: pair[1].span(0, 1)?,
                    kind: MappingKind::Exact,
                });
            }
        } else {
            let source = SourceSnapshot::new(
                SourceId("map".into()),
                0,
                "memory:map".into(),
                b"xxxxxxxxx".to_vec(),
                &mut budget(),
            )?;
            for index in 0..8 {
                maps.push(Mapping {
                    source: source.span(index, index + 1)?,
                    target: source.span(index + 1, index + 2)?,
                    kind: MappingKind::Exact,
                });
            }
            sources.push(source);
        }
        value.provenance = OwnerProvenance::from_parts(origins, sources, maps);
        let mut preparation = budget();
        // Earlier high-water usage must not inflate the proof's relative depth.
        preparation.observe_depth(99)?;
        let proof = preparation.with_depth_at_least(2, |b| {
            value
                .provenance
                .validate(&registry, b, &mut SourceAdmission::default())
        })?;
        for caller in [0, 3] {
            for shortage in [0, 1] {
                for reuse in [false, true] {
                    let mut b = Budget::new(Limits {
                        depth: caller + 9 - shortage,
                        ..budget().limits()
                    });
                    let mut admission = SourceAdmission::default();
                    let result = b.with_depth_at_least(caller, |b| {
                        if reuse {
                            proof.validate_closure(&value, b, &mut admission)
                        } else {
                            value.validate(&registry, b, &mut admission)
                        }
                    });
                    if shortage == 0 {
                        result?;
                        assert_eq!(b.usage().depth, caller + 9, "path={path} reuse={reuse}");
                    } else {
                        assert_eq!(
                            result.err().and_then(|e| e.stop_reason()),
                            Some(StopReason::DepthLimit),
                            "path={path} reuse={reuse}"
                        );
                        assert_eq!(b.poll(), Err(StopReason::DepthLimit));
                    }
                    assert_eq!(b.current_depth(), 0);
                }
            }
        }
    }
    Ok(())
}

#[test]
fn capture_storage_has_budgeted_clones_and_survives_preparation_stop() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let input = owner(&schema)?;
    let checked = input.validate(&registry, &mut budget())?;
    let mut captures = ForeignCapture::new(&checked);
    for (work, allocation, reason) in [
        (0, 1_000_000, StopReason::WorkLimit),
        (1_000_000, 0, StopReason::AllocationLimit),
    ] {
        let mut limited = Budget::new(Limits {
            work,
            allocation_units: allocation,
            ..budget().limits()
        });
        assert_eq!(
            captures.capture_at(
                NodeRef(0),
                0,
                &registry,
                &mut limited,
                &mut SourceAdmission::default()
            ),
            Err(SyntaxError::Stopped(reason))
        );
    }
    let captured = captures.capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let expected = ForeignClosure::capture_at(
        &checked,
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(captured, expected);
    let mut measured = budget();
    ForeignCapture::new(&checked).capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut measured,
        &mut SourceAdmission::default(),
    )?;
    for allocation in [1, 64, 128, measured.usage().allocation_units - 1] {
        let mut partial = ForeignCapture::new(&checked);
        let mut limited = Budget::new(Limits {
            allocation_units: allocation,
            ..budget().limits()
        });
        assert_eq!(
            partial.capture_at(
                NodeRef(0),
                0,
                &registry,
                &mut limited,
                &mut SourceAdmission::default()
            ),
            Err(SyntaxError::Stopped(StopReason::AllocationLimit))
        );
        // An incomplete preparation exposes no closure. A fresh operation can
        // retry; retained storage is revalidated under that operation's ledger.
        assert_eq!(
            partial.capture_at(
                NodeRef(0),
                0,
                &registry,
                &mut budget(),
                &mut SourceAdmission::default()
            )?,
            expected
        );
    }
    let mut limited = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    assert_eq!(
        captures.capture_at(
            NodeRef(0),
            1,
            &registry,
            &mut limited,
            &mut SourceAdmission::default()
        ),
        Err(SyntaxError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(captured, expected);
    for (work, allocation, reason) in [
        (0, 1_000_000, StopReason::WorkLimit),
        (1_000_000, 0, StopReason::AllocationLimit),
    ] {
        let mut limited = Budget::new(Limits {
            work,
            allocation_units: allocation,
            ..budget().limits()
        });
        assert_eq!(
            captured.provenance.clone_with_budget(&mut limited),
            Err(reason)
        );
    }
    // Replacing one closure's owner tables cannot change another shared value.
    let mut changed = captured.clone_with_budget(&mut budget())?;
    changed.provenance = OwnerProvenance::from_parts(vec![], vec![], vec![]);
    assert_eq!(captured.provenance.origins(), input.origins);
    assert!(changed.provenance.origins().is_empty());
    let other_input = owner(&schema)?;
    let other_checked = other_input.validate(&registry, &mut budget())?;
    let other = ForeignCapture::new(&other_checked).capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(other, captured);
    #[cfg(target_has_atomic = "ptr")]
    assert!(!core::ptr::eq(
        other.provenance.origins(),
        captured.provenance.origins()
    ));
    Ok(())
}
