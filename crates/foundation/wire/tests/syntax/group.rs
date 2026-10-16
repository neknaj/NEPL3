#[path = "group/archive.rs"]
mod archive;
use super::foreign::closed;
use super::*;
use nepl3_core::{
    budget::StopReason,
    value::{Record, TypedValue},
};
#[test]
fn grouped_foreign_capture_shares_provenance_and_preserves_selected_environments() -> TestResult {
    use nepl3_core::syntax::group::{CaptureField, ForeignClosureGroup};
    let (schema, registry, mut owner) = fixture()?;
    let mut second = owner.environments[0].clone();
    second.id = 19;
    owner.environments.push(second);
    let mut field = owner.nodes[0].fields[0].clone();
    let FieldValue::Foreign(guest) = &mut field else {
        return Err("foreign fixture".into());
    };
    guest.environment.id = 19;
    owner.nodes[0].fields.push(field);
    let checked = owner
        .validate(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let selections = [
        CaptureField {
            owner: 0,
            node: NodeRef(0),
            field: 0,
        },
        CaptureField {
            owner: 0,
            node: NodeRef(0),
            field: 1,
        },
        CaptureField {
            owner: 0,
            node: NodeRef(0),
            field: 0,
        },
    ];
    let group = ForeignClosureGroup::capture(
        &[&checked],
        &selections,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(group.provenances.len(), 1);
    assert_eq!(group.environments.len(), 2);
    assert_eq!(group.entries.len(), 3);
    assert_eq!(group.entries[0].owner, group.entries[2].owner);
    assert_ne!(group.entries[0].owner, group.entries[1].owner);
    let valid = group
        .validate(&registry, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    for (index, selection) in selections.iter().enumerate() {
        let expected = ForeignClosure::capture_at(
            &checked,
            selection.node,
            selection.field,
            &registry,
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let view = valid.entry(index).ok_or("group entry")?;
        let actual = view
            .materialize(&mut budget())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(actual, expected);
        assert_eq!(
            encode_foreign_closure(
                &actual,
                &schema,
                &registry,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .map_err(|e| format!("{e:?}"))?,
            encode_foreign_closure(
                &expected,
                &schema,
                &registry,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .map_err(|e| format!("{e:?}"))?
        );
    }
    let bytes = nepl3_wire::syntax::group::encode(
        &group,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let decoded = nepl3_wire::syntax::group::decode(
        &bytes,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(decoded, group);
    {
        use nepl3_core::value_codec::FoundationValueCodec;
        let sources = nepl3_core::source::SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            nepl3_wire::foundation::FoundationCodec::new(&registry, &sources, &mut admission)
                .map_err(|e| format!("{e:?}"))?;
        let value = codec
            .encode_foreign_group(&group, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            codec
                .decode_foreign_group(&value, &mut budget())
                .map_err(|e| format!("{e:?}"))?,
            group
        );
    }
    assert_eq!(
        nepl3_wire::syntax::group::encode(
            &decoded,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(|e| format!("{e:?}"))?,
        bytes
    );
    for selections in [vec![2, 0], vec![1], vec![1, 0, 1], vec![]] {
        let selected = valid
            .select(
                &selections,
                &registry,
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(selected.entries.len(), selections.len());
        assert_eq!(
            selected.provenances.len(),
            usize::from(!selections.is_empty())
        );
        assert_eq!(
            selected.environments.len(),
            if selections.is_empty() {
                0
            } else if selections == [1, 0, 1] {
                2
            } else {
                1
            }
        );
        if selections == [1, 0, 1] {
            assert_eq!(selected.environments, group.environments);
            assert_eq!(selected.provenances, group.provenances);
            assert_eq!(
                selected
                    .entries
                    .iter()
                    .map(|v| v.owner.0)
                    .collect::<Vec<_>>(),
                vec![1, 0, 1]
            );
        }
        let selected_proof = selected
            .validate(&registry, &mut budget(), &mut SourceAdmission::default())
            .map_err(|e| format!("{e:?}"))?;
        for (new, old) in selections.iter().enumerate() {
            let expected = valid
                .entry(*old as usize)
                .ok_or("old")?
                .materialize(&mut budget())
                .map_err(|e| format!("{e:?}"))?;
            let actual = selected_proof
                .entry(new)
                .ok_or("new")?
                .materialize(&mut budget())
                .map_err(|e| format!("{e:?}"))?;
            assert_eq!(actual, expected);
        }
    }
    assert!(matches!(
        valid.select(
            &[u64::MAX],
            &registry,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(nepl3_core::syntax::group::GroupError::Entry(u64::MAX))
    ));
    let mut stopped = budget();
    stopped.cancel();
    assert!(matches!(
        valid.select(
            &[],
            &SchemaRegistry::default(),
            &mut stopped,
            &mut SourceAdmission::default()
        ),
        Err(nepl3_core::syntax::group::GroupError::Stopped(
            StopReason::Cancelled
        ))
    ));
    assert!(matches!(
        valid.select(
            &[0],
            &SchemaRegistry::default(),
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(nepl3_core::syntax::group::GroupError::Syntax(
            SyntaxError::Schema(nepl3_core::schema::SchemaError::Unfinalized)
        ))
    ));
    let source = &group.provenances[0].sources[0];
    let conflicting = SourceSnapshot::new(
        source.identity().source.clone(),
        source.identity().revision,
        source.uri().into(),
        b"zz".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut conflict = SourceAdmission::default();
    conflict
        .admit_existing(&conflicting, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(
        valid
            .select(&[0], &registry, &mut budget(), &mut conflict)
            .is_err()
    );
    let selections = [2, 0];
    let mut measured = budget();
    valid
        .select(
            &selections,
            &registry,
            &mut measured,
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
    for (used, reason) in [
        (measured.usage().work, StopReason::WorkLimit),
        (
            measured.usage().allocation_units,
            StopReason::AllocationLimit,
        ),
        (measured.usage().nodes, StopReason::NodeLimit),
        (measured.usage().source_bytes, StopReason::SourceLimit),
        (measured.usage().depth, StopReason::DepthLimit),
    ] {
        assert!(used > 0);
        for exact in [false, true] {
            let mut limits = budget().limits();
            let n = used - u64::from(!exact);
            match reason {
                StopReason::WorkLimit => limits.work = n,
                StopReason::AllocationLimit => limits.allocation_units = n,
                StopReason::NodeLimit => limits.nodes = n,
                StopReason::SourceLimit => limits.source_bytes = n,
                StopReason::DepthLimit => limits.depth = n,
                _ => return Err("case".into()),
            }
            let mut b = Budget::new(limits);
            let result = valid.select(
                &selections,
                &registry,
                &mut b,
                &mut SourceAdmission::default(),
            );
            if exact {
                result.map_err(|e| format!("{e:?}"))?;
            } else {
                assert!(
                    matches!(result,Err(nepl3_core::syntax::group::GroupError::Stopped(actual)) if actual==reason)
                );
                assert_eq!(b.poll(), Err(reason));
            }
            assert_eq!(b.current_depth(), 0);
        }
    }
    let mut limits = budget().limits();
    limits.depth = measured.usage().depth;
    let mut tight = Budget::new(limits);
    assert!(matches!(
        tight.with_depth_at_least(3, |b| valid.select(
            &selections,
            &registry,
            b,
            &mut SourceAdmission::default()
        )),
        Err(nepl3_core::syntax::group::GroupError::Stopped(
            StopReason::DepthLimit
        ))
    ));
    assert_eq!(tight.current_depth(), 0);
    assert!(valid.entry(3).is_none());
    Ok(())
}

#[test]
fn grouped_foreign_depths_do_not_mix_unrelated_owner_costs() -> TestResult {
    use nepl3_core::syntax::group::{CaptureField, ForeignClosureGroup};
    let (_, registry, owner) = fixture()?;
    let mut deep = owner.clone();
    let leaf = deep.origins[0].clone();
    deep.origins = (1..20)
        .map(|n| Origin::Composite(vec![OriginId(n)]))
        .collect();
    deep.origins.push(leaf);
    let a = deep
        .validate(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let z = owner
        .validate(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let fields = [
        CaptureField {
            owner: 0,
            node: NodeRef(0),
            field: 0,
        },
        CaptureField {
            owner: 1,
            node: NodeRef(0),
            field: 0,
        },
    ];
    let group = ForeignClosureGroup::capture(
        &[&a, &z],
        &fields,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(group.provenances.len(), 2);
    assert_eq!(group.environments.len(), 2);
    let proof = group
        .validate(&registry, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    for selections in [vec![1], vec![1, 0]] {
        let selected = proof
            .select(
                &selections,
                &registry,
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(selected.provenances.len(), selections.len());
        if selections == [1, 0] {
            assert_eq!(selected.provenances, group.provenances);
            assert_eq!(selected.environments, group.environments);
            assert_eq!(
                selected
                    .entries
                    .iter()
                    .map(|v| v.owner.0)
                    .collect::<Vec<_>>(),
                vec![1, 0]
            );
        }
        let selected_proof = selected
            .validate(&registry, &mut budget(), &mut SourceAdmission::default())
            .map_err(|e| format!("{e:?}"))?;
        for (new, old) in selections.iter().enumerate() {
            assert_eq!(
                selected_proof
                    .entry(new)
                    .ok_or("new")?
                    .materialize(&mut budget())
                    .map_err(|e| format!("{e:?}"))?,
                proof
                    .entry(*old as usize)
                    .ok_or("old")?
                    .materialize(&mut budget())
                    .map_err(|e| format!("{e:?}"))?
            );
        }
    }
    let mut baseline = budget();
    let mut admission = SourceAdmission::default();
    for (entry, depth) in [1, 20].into_iter().enumerate() {
        let value = proof
            .entry(entry)
            .ok_or("entry")?
            .materialize(&mut budget())
            .map_err(|e| format!("{e:?}"))?;
        baseline
            .with_depth_at_least(depth, |b| {
                value.validate(&registry, b, &mut admission).map(|_| ())
            })
            .map_err(|e| format!("{e:?}"))?;
    }
    let mut measured = budget();
    group
        .validate_at_depths(
            &[1, 20],
            &registry,
            &mut measured,
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(measured.usage().depth, baseline.usage().depth);
    let mut limits = budget().limits();
    limits.depth = measured.usage().depth;
    group
        .validate_at_depths(
            &[1, 20],
            &registry,
            &mut Budget::new(limits),
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
    assert!(
        group
            .validate_at_depths(
                &[20, 20],
                &registry,
                &mut Budget::new(limits),
                &mut SourceAdmission::default()
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn grouped_capture_charges_owner_growth_once_independently_of_guest_count() -> TestResult {
    use nepl3_core::syntax::group::{CaptureField, ForeignClosureGroup};
    let mut work = [[0u64; 2]; 2];
    let mut allocation = [[0u64; 2]; 2];
    let mut wire_work = [[0u64; 2]; 2];
    let mut wire_bytes = [[0usize; 2]; 2];
    let mut wire_alloc = [[0u64; 2]; 2];
    for (n, guests) in [16, 32].into_iter().enumerate() {
        for (m, origins) in [128, 256].into_iter().enumerate() {
            let (schema, registry, mut owner) = fixture()?;
            owner.origins = vec![owner.origins[0].clone(); origins];
            let checked = owner
                .validate(&registry, &mut budget())
                .map_err(|e| format!("{e:?}"))?;
            let fields = vec![
                CaptureField {
                    owner: 0,
                    node: NodeRef(0),
                    field: 0
                };
                guests
            ];
            let mut b = budget();
            let group = ForeignClosureGroup::capture(
                &[&checked],
                &fields,
                &registry,
                &mut b,
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("{e:?}"))?;
            assert_eq!(group.provenances.len(), 1);
            assert_eq!(group.entries.len(), guests);
            work[n][m] = b.usage().work;
            allocation[n][m] = b.usage().allocation_units;
            let mut wire = budget();
            let bytes = nepl3_wire::syntax::group::encode(
                &group,
                &schema,
                &registry,
                &mut SourceAdmission::default(),
                &mut wire,
            )
            .map_err(|e| format!("{e:?}"))?;
            wire_work[n][m] = wire.usage().work;
            wire_alloc[n][m] = wire.usage().allocation_units;
            wire_bytes[n][m] = bytes.len();
        }
    }
    // Doubling guest count does not multiply the cost of a larger owner table.
    assert!(work[0][1] > work[0][0]);
    assert_eq!(work[0][1] - work[0][0], work[1][1] - work[1][0]);
    assert!(allocation[0][1] > allocation[0][0]);
    assert_eq!(
        allocation[0][1] - allocation[0][0],
        allocation[1][1] - allocation[1][0]
    );
    assert!(wire_work[0][1] > wire_work[0][0]);
    assert_eq!(
        wire_work[0][1] - wire_work[0][0],
        wire_work[1][1] - wire_work[1][0]
    );
    assert!(wire_bytes[0][1] > wire_bytes[0][0]);
    assert_eq!(
        wire_bytes[0][1] - wire_bytes[0][0],
        wire_bytes[1][1] - wire_bytes[1][0]
    );
    assert_eq!(
        wire_alloc[0][1] - wire_alloc[0][0],
        wire_alloc[1][1] - wire_alloc[1][0]
    );
    Ok(())
}

#[test]
fn grouped_capture_and_validation_reject_bad_refs_and_preserve_exact_stops() -> TestResult {
    use nepl3_core::syntax::group::*;
    let (_, registry, owner) = fixture()?;
    let checked = owner
        .validate(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let fields = [CaptureField {
        owner: 0,
        node: NodeRef(0),
        field: 0,
    }];
    let run = |b: &mut Budget| {
        ForeignClosureGroup::capture(
            &[&checked],
            &fields,
            &registry,
            b,
            &mut SourceAdmission::default(),
        )
    };
    let mut measured = budget();
    let original = run(&mut measured).map_err(|e| format!("{e:?}"))?;
    for (used, reason) in [
        (measured.usage().work, StopReason::WorkLimit),
        (
            measured.usage().allocation_units,
            StopReason::AllocationLimit,
        ),
        (measured.usage().nodes, StopReason::NodeLimit),
        (measured.usage().source_bytes, StopReason::SourceLimit),
        (measured.usage().depth, StopReason::DepthLimit),
    ] {
        assert!(used > 0);
        for exact in [false, true] {
            let mut limits = budget().limits();
            let value = used - u64::from(!exact);
            match reason {
                StopReason::WorkLimit => limits.work = value,
                StopReason::AllocationLimit => limits.allocation_units = value,
                StopReason::NodeLimit => limits.nodes = value,
                StopReason::SourceLimit => limits.source_bytes = value,
                StopReason::DepthLimit => limits.depth = value,
                _ => return Err("resource case".into()),
            }
            let mut b = Budget::new(limits);
            let result = run(&mut b);
            if exact {
                assert_eq!(result.map_err(|e| format!("{e:?}"))?, original);
            } else {
                assert!(matches!(result, Err(GroupError::Stopped(actual)) if actual == reason));
                assert_eq!(b.poll(), Err(reason));
            }
            assert_eq!(b.current_depth(), 0);
        }
    }
    for case in 0..5 {
        let mut value = original.clone();
        match case {
            0 => value.entries[0].owner = OwnerEnvironmentRef(u64::MAX),
            1 => value.environments[0].provenance = ProvenanceRef(u64::MAX),
            2 => value.environments.push(value.environments[0].clone()),
            3 => value.provenances.push(value.provenances[0].clone()),
            _ => value.entries[0].syntax.environment.digest.0[0] ^= 1,
        }
        assert!(
            value
                .validate(&registry, &mut budget(), &mut SourceAdmission::default())
                .is_err()
        );
    }
    assert!(matches!(
        ForeignClosureGroup::capture(
            &[&checked, &checked],
            &fields,
            &registry,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GroupError::DuplicateOwner { .. })
    ));
    assert!(matches!(
        original.validate_at_depths(
            &[],
            &registry,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GroupError::DepthCount)
    ));
    let mut stopped = budget();
    stopped.cancel();
    assert!(matches!(
        original.validate(
            &SchemaRegistry::default(),
            &mut stopped,
            &mut SourceAdmission::default()
        ),
        Err(GroupError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}

#[test]
fn grouped_errors_preserve_all_nested_stop_families() {
    use nepl3_core::syntax::group::GroupError;
    use nepl3_core::{
        origin::OriginError, schema::SchemaError, source::SourceError, view::ViewError,
    };
    let reason = StopReason::Cancelled;
    let direct = [
        SyntaxError::Stopped(reason),
        SyntaxError::Source(SourceError::Stopped(reason)),
        SyntaxError::Origin(OriginError::Stopped(reason)),
        SyntaxError::Origin(OriginError::Source(SourceError::Stopped(reason))),
        SyntaxError::Schema(SchemaError::Stopped(reason)),
    ];
    let views = [
        ViewError::Stopped(reason),
        ViewError::Source(SourceError::Stopped(reason)),
        ViewError::Origin(OriginError::Stopped(reason)),
        ViewError::Origin(OriginError::Source(SourceError::Stopped(reason))),
        ViewError::Schema(SchemaError::Stopped(reason)),
    ];
    for error in direct
        .into_iter()
        .chain(views.into_iter().map(SyntaxError::View))
    {
        assert_eq!(GroupError::from(error), GroupError::Stopped(reason));
    }
    assert_eq!(
        GroupError::from(SyntaxError::Reference),
        GroupError::Syntax(SyntaxError::Reference)
    );
}

#[test]
fn grouped_shared_provenance_keeps_each_environment_depth() -> TestResult {
    use nepl3_core::syntax::group::{
        ForeignClosureGroup, ForeignGroupEntry, ForeignOwnerEnvironment, ForeignProvenance,
        GroupError, OwnerEnvironmentRef, ProvenanceRef,
    };
    let (schema, registry, closure) = closed()?;
    let record = |kind: &str, fields| {
        NdfValue::Record(Record {
            schema: schema.clone(),
            kind: kind.into(),
            fields,
        })
    };
    let mut nested = record("NodeRef", vec![NdfValue::U64(0)]);
    for _ in 0..12 {
        nested = record(
            "Continuation",
            vec![
                record(
                    "OperationRef",
                    vec![
                        record(
                            "SchemaRef",
                            vec![
                                NdfValue::Text(schema.package.clone()),
                                NdfValue::U64(schema.revision),
                                NdfValue::Bytes(schema.digest.0.to_vec()),
                            ],
                        ),
                        NdfValue::Text("test".into()),
                    ],
                ),
                NdfValue::U64(0),
                NdfValue::Bytes(vec![0; 32]),
                nested,
            ],
        );
    }
    let NdfValue::Record(ref nested) = nested else {
        return Err("record".into());
    };
    let mut deep = closure.owner_environment.clone();
    deep.id = 19;
    deep.value.bindings[0].value = TypedValue::Record(nested.clone());
    deep.digest = environment_digest(&deep.value, &schema, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut syntax = closure.syntax.clone();
    syntax.environment = EnvironmentRef {
        id: deep.id,
        digest: deep.digest,
    };
    let group = ForeignClosureGroup {
        provenances: vec![ForeignProvenance {
            origins: closure.owner_origins.clone(),
            sources: closure.owner_sources.clone(),
            source_maps: closure.owner_source_maps.clone(),
        }],
        environments: vec![
            ForeignOwnerEnvironment {
                provenance: ProvenanceRef(0),
                environment: deep,
            },
            ForeignOwnerEnvironment {
                provenance: ProvenanceRef(0),
                environment: closure.owner_environment.clone(),
            },
        ],
        entries: vec![
            ForeignGroupEntry {
                owner: OwnerEnvironmentRef(0),
                syntax,
            },
            ForeignGroupEntry {
                owner: OwnerEnvironmentRef(1),
                syntax: closure.syntax.clone(),
            },
            ForeignGroupEntry {
                owner: OwnerEnvironmentRef(1),
                syntax: closure.syntax.clone(),
            },
        ],
    };
    let proof = group
        .validate(&registry, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    let depths = [1, 30, 7];
    let caller = 5;
    let mut baseline = budget();
    for (i, relative) in depths.into_iter().enumerate() {
        let item = proof
            .entry(i)
            .ok_or("entry")?
            .materialize(&mut budget())
            .map_err(|e| format!("{e:?}"))?;
        baseline
            .with_depth_at_least(caller + relative, |b| {
                item.validate(&registry, b, &mut SourceAdmission::default())
                    .map(|_| ())
            })
            .map_err(|e| format!("{e:?}"))?;
    }
    let run = |b: &mut Budget| {
        b.with_depth_at_least(caller, |b| {
            group
                .validate_at_depths(&depths, &registry, b, &mut SourceAdmission::default())
                .map(|_| ())
        })
    };
    let mut measured = budget();
    run(&mut measured).map_err(|e| format!("{e:?}"))?;
    assert_eq!(measured.usage().depth, baseline.usage().depth);
    for exact in [false, true] {
        let mut limits = budget().limits();
        limits.depth = measured.usage().depth - u64::from(!exact);
        let mut b = Budget::new(limits);
        let result = run(&mut b);
        if exact {
            result.map_err(|e| format!("{e:?}"))?;
        } else {
            assert!(matches!(
                result,
                Err(GroupError::Stopped(StopReason::DepthLimit))
            ));
        }
        assert_eq!(b.current_depth(), 0);
    }
    let mut historical = budget();
    historical
        .with_depth_at_least::<_, GroupError>(1000, |_| Ok(()))
        .map_err(|e| format!("{e:?}"))?;
    run(&mut historical).map_err(|e| format!("{e:?}"))?;
    assert_eq!(historical.current_depth(), 0);
    assert_eq!(historical.usage().depth, 1000);
    let mut overflow = budget();
    assert!(matches!(
        overflow.with_depth_at_least(caller, |b| group
            .validate_at_depths(
                &[u64::MAX, 0, 0],
                &registry,
                b,
                &mut SourceAdmission::default()
            )
            .map(|_| ())),
        Err(GroupError::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(overflow.current_depth(), 0);
    // Make provenance dominate both syntax and environment. Checking it only
    // at the first (shallow) occurrence would otherwise be masked by guests.
    let mut shared = group.clone();
    let leaf = shared.provenances[0].origins[1].clone();
    shared.provenances[0].origins = (1..60)
        .map(|n| Origin::Composite(vec![OriginId(n)]))
        .collect();
    shared.provenances[0].origins.push(leaf);
    let proof = shared
        .validate(&registry, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    let mut baseline = budget();
    for (i, relative) in depths.into_iter().enumerate() {
        let value = proof
            .entry(i)
            .ok_or("entry")?
            .materialize(&mut budget())
            .map_err(|e| format!("{e:?}"))?;
        baseline
            .with_depth_at_least(caller + relative, |b| {
                value
                    .validate(&registry, b, &mut SourceAdmission::default())
                    .map(|_| ())
            })
            .map_err(|e| format!("{e:?}"))?;
    }
    let mut measured = budget();
    measured
        .with_depth_at_least(caller, |b| {
            shared
                .validate_at_depths(&depths, &registry, b, &mut SourceAdmission::default())
                .map(|_| ())
        })
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(measured.usage().depth, baseline.usage().depth);
    assert!(measured.usage().depth > 60);
    let mut limits = budget().limits();
    limits.depth = measured.usage().depth - 1;
    let mut shallow = Budget::new(limits);
    assert!(matches!(
        shallow.with_depth_at_least(caller, |b| shared
            .validate_at_depths(&depths, &registry, b, &mut SourceAdmission::default())
            .map(|_| ())),
        Err(GroupError::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(shallow.current_depth(), 0);
    Ok(())
}

#[test]
fn grouped_wire_rejects_forged_references_digests_and_exact_stops() -> TestResult {
    use nepl3_core::syntax::group::*;
    use nepl3_core::value_codec::FoundationCodecError;
    let (schema, registry, closure) = closed()?;
    let group = ForeignClosureGroup {
        provenances: vec![ForeignProvenance {
            origins: closure.owner_origins,
            sources: closure.owner_sources,
            source_maps: closure.owner_source_maps,
        }],
        environments: vec![ForeignOwnerEnvironment {
            provenance: ProvenanceRef(0),
            environment: closure.owner_environment,
        }],
        entries: vec![ForeignGroupEntry {
            owner: OwnerEnvironmentRef(0),
            syntax: closure.syntax,
        }],
    };
    let encode = |b: &mut Budget| {
        nepl3_wire::syntax::group::encode(
            &group,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            b,
        )
    };
    let mut encoding = budget();
    let bytes = encode(&mut encoding).map_err(|e| format!("{e:?}"))?;
    let decode = |b: &mut Budget| {
        nepl3_wire::syntax::group::decode(
            &bytes,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            b,
        )
    };
    let mut decoding = budget();
    assert_eq!(decode(&mut decoding).map_err(|e| format!("{e:?}"))?, group);
    for (mode, usage) in [(false, encoding.usage()), (true, decoding.usage())] {
        for (used, reason) in [
            (usage.work, StopReason::WorkLimit),
            (usage.allocation_units, StopReason::AllocationLimit),
            (usage.depth, StopReason::DepthLimit),
            (usage.nodes, StopReason::NodeLimit),
            (usage.source_bytes, StopReason::SourceLimit),
            (usage.output_bytes, StopReason::OutputLimit),
        ] {
            if used == 0 {
                continue;
            }
            for exact in [false, true] {
                let mut limits = budget().limits();
                let value = used - u64::from(!exact);
                match reason {
                    StopReason::WorkLimit => limits.work = value,
                    StopReason::AllocationLimit => limits.allocation_units = value,
                    StopReason::DepthLimit => limits.depth = value,
                    StopReason::NodeLimit => limits.nodes = value,
                    StopReason::SourceLimit => limits.source_bytes = value,
                    StopReason::OutputLimit => limits.output_bytes = value,
                    _ => return Err("case".into()),
                }
                let mut b = Budget::new(limits);
                let result = if mode {
                    decode(&mut b).map(|_| ())
                } else {
                    encode(&mut b).map(|_| ())
                };
                if exact {
                    result.map_err(|e| format!("{e:?}"))?;
                } else {
                    assert_eq!(result.err().and_then(|e| e.stop_reason()), Some(reason));
                    assert_eq!(b.poll(), Err(reason));
                }
                assert_eq!(b.current_depth(), 0);
            }
        }
    }
    fn fields(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
        match value {
            NdfValue::Record(v) => Ok(&mut v.fields),
            _ => Err("record".into()),
        }
    }
    fn list(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
        match value {
            NdfValue::List(v) => Ok(v),
            _ => Err("list".into()),
        }
    }
    // A second provenance has its own local closure even when the first table,
    // the guest, and a host ambient store contain the identical snapshot.
    let mut shared = group.clone();
    shared.provenances.push(shared.provenances[0].clone());
    let mut env = shared.environments[0].clone();
    env.provenance = ProvenanceRef(1);
    shared.environments.push(env);
    let mut entry = shared.entries[0].clone();
    entry.owner = OwnerEnvironmentRef(1);
    shared.entries.push(entry);
    let shared_bytes = nepl3_wire::syntax::group::encode(
        &shared,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut raw = nepl3_wire::decode(&shared_bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let root = fields(&mut raw)?;
    list(&mut fields(&mut list(&mut root[0])?[1])?[1])?.clear();
    let mut ambient = nepl3_core::source::SourceStore::default();
    ambient
        .insert(shared.provenances[0].sources[0].clone())
        .map_err(|e| format!("{e:?}"))?;
    {
        use nepl3_core::value_codec::FoundationValueCodec;
        let mut admission = SourceAdmission::default();
        let mut codec =
            nepl3_wire::foundation::FoundationCodec::new(&registry, &ambient, &mut admission)
                .map_err(|e| format!("{e:?}"))?;
        assert!(codec.decode_foreign_group(&raw, &mut budget()).is_err());
    }
    let forged = nepl3_wire::encode(&raw, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(
        nepl3_wire::syntax::group::decode(
            &forged,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    shared.provenances[1].sources.clear();
    assert!(
        shared
            .validate(&registry, &mut budget(), &mut SourceAdmission::default())
            .is_err()
    );
    let mut guest_missing = group.clone();
    guest_missing.entries[0].syntax.bundle.sources.clear();
    assert!(
        guest_missing
            .validate(&registry, &mut budget(), &mut SourceAdmission::default())
            .is_err()
    );
    for case in 0..10 {
        let mut raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let root = fields(&mut raw)?;
        match case {
            0 => fields(&mut fields(&mut list(&mut root[2])?[0])?[0])?[0] = NdfValue::U64(u64::MAX),
            1 => fields(&mut fields(&mut list(&mut root[1])?[0])?[0])?[0] = NdfValue::U64(u64::MAX),
            2 => {
                fields(&mut fields(&mut list(&mut root[1])?[0])?[1])?[1] =
                    NdfValue::Bytes(vec![0; 32])
            }
            3 => {
                let items = list(&mut root[1])?;
                items.push(items[0].clone());
            }
            4 => {
                let items = list(&mut root[0])?;
                items.push(items[0].clone());
            }
            5 => list(&mut fields(&mut list(&mut root[0])?[0])?[1])?.clear(),
            6 => {
                let items = list(&mut fields(&mut list(&mut root[0])?[0])?[1])?;
                items.push(items[0].clone());
            }
            _ => {
                let NdfValue::Record(header) = &mut raw else {
                    return Err("header".into());
                };
                match case {
                    7 => header.schema.package = "wrong".into(),
                    8 => header.schema.revision += 1,
                    _ => header.schema.digest.0[0] ^= 1,
                }
            }
        }
        let forged = nepl3_wire::encode(&raw, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert!(
            nepl3_wire::syntax::group::decode(
                &forged,
                &schema,
                &registry,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err()
        );
    }
    Ok(())
}

#[path = "group/identity.rs"]
mod identity;

#[path = "group/consumers.rs"]
mod consumers;
