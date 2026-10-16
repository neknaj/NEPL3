use super::*;
use nepl3_core::{
    schema::archive as contracts, source::SourceStore, syntax::group::*,
    value_codec::FoundationValueCodec,
};
use nepl3_wire::{
    foundation::FoundationCodec,
    syntax::group::{self as legacy, archive as codec},
};
fn setup() -> Result<
    (
        SchemaRef,
        SchemaRef,
        SchemaRef,
        SchemaRegistry,
        ForeignClosureGroup,
    ),
    String,
> {
    let (f, mut r, c) = closed()?;
    let mut b = budget();
    let source = contracts::descriptor(&mut b).map_err(|e| format!("{e:?}"))?;
    let group = contracts::group::descriptor(&mut b).map_err(|e| format!("{e:?}"))?;
    let s = source.reference(&mut b).map_err(|e| format!("{e:?}"))?;
    let a = group.reference(&mut b).map_err(|e| format!("{e:?}"))?;
    r.register(s.clone(), source, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    r.register(a.clone(), group, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    r.finalize(&mut b).map_err(|e| format!("{e:?}"))?;
    let provenance = ForeignProvenance {
        origins: c.owner_origins,
        sources: c.owner_sources,
        source_maps: c.owner_source_maps,
    };
    let other = SourceSnapshot::new(
        SourceId("other-owner".into()),
        1,
        "memory:other-owner".into(),
        b"xy".to_vec(),
        &mut b,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut second = provenance.clone();
    second.sources = vec![other.clone()];
    for origin in &mut second.origins {
        if let Origin::Direct(span) = origin {
            *span = other
                .span(span.start(), span.end())
                .map_err(|e| format!("{e:?}"))?;
        }
    }
    let mut nested = c.syntax.clone();
    let environment = nested.bundle.environments[0].clone();
    let mut child = nested.clone();
    child.environment = EnvironmentRef {
        id: environment.id,
        digest: environment.digest,
    };
    nested.bundle.nodes[0]
        .fields
        .push(FieldValue::Foreign(Box::new(child)));
    Ok((
        a,
        s,
        f,
        r,
        ForeignClosureGroup {
            provenances: vec![provenance, second],
            environments: vec![
                ForeignOwnerEnvironment {
                    provenance: ProvenanceRef(0),
                    environment: c.owner_environment.clone(),
                },
                ForeignOwnerEnvironment {
                    provenance: ProvenanceRef(1),
                    environment: c.owner_environment,
                },
            ],
            entries: vec![
                ForeignGroupEntry {
                    owner: OwnerEnvironmentRef(0),
                    syntax: nested,
                },
                ForeignGroupEntry {
                    owner: OwnerEnvironmentRef(1),
                    syntax: c.syntax,
                },
            ],
        },
    ))
}
#[test]
fn grouped_archive_keeps_owners_nested_sets_and_v1_entry_digests() -> TestResult {
    let (a, s, f, r, group) = setup()?;
    let bytes = codec::encode(
        &group,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let decoded = codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(decoded, group);
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        c.foreign_group_entry_digests(&decoded, &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        c.foreign_group_entry_digests(&group, &mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    let original = group
        .validate(&r, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    let restored = decoded
        .validate(&r, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    for i in 0..2 {
        assert_eq!(
            original
                .entry(i)
                .ok_or("entry")?
                .materialize(&mut budget())
                .map_err(|e| format!("{e:?}"))?,
            restored
                .entry(i)
                .ok_or("entry")?
                .materialize(&mut budget())
                .map_err(|e| format!("{e:?}"))?
        );
    }
    assert!(
        legacy::decode(
            &bytes,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    let old = legacy::encode(
        &group,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(
        codec::decode(
            &old,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn missing_or_misplaced_provenance_cannot_use_another_component_sources() -> TestResult {
    let (a, s, f, r, group) = setup()?;
    let bytes = codec::encode(
        &group,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    for mode in 0..4 {
        let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let NdfValue::Record(root) = &mut value else {
            return Err("root".into());
        };
        let NdfValue::Record(sources) = &mut root.fields[0] else {
            return Err("sources".into());
        };
        let NdfValue::List(sets) = &mut sources.fields[1] else {
            return Err("sets".into());
        };
        match mode {
            0 => sets[0] = NdfValue::List(vec![]),
            1 => sets.swap(0, 1),
            2 => {
                sets.pop();
            }
            _ => sets.push(NdfValue::List(vec![])),
        }
        let bad = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert!(
            codec::decode(
                &bad,
                &a,
                &s,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err(),
            "mode {mode}"
        );
    }
    Ok(())
}
#[test]
fn group_skeleton_versions_and_ownership_errors_reject() -> TestResult {
    let (a, s, f, r, group) = setup()?;
    let bytes = codec::encode(
        &group,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    for mode in 0..4 {
        let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let NdfValue::Record(root) = &mut value else {
            return Err("root".into());
        };
        match mode {
            0 => root.schema.revision = 2,
            1 => root.fields[1] = NdfValue::Bytes(vec![0xff]),
            2 => {
                let NdfValue::Record(sets) = &mut root.fields[0] else {
                    return Err("sets".into());
                };
                let NdfValue::List(table) = &mut sets.fields[0] else {
                    return Err("table".into());
                };
                let NdfValue::Record(source) = &mut table[0] else {
                    return Err("source".into());
                };
                source.fields[2] = NdfValue::Text("forged".into());
            }
            _ => {
                let NdfValue::Bytes(skeleton) = &mut root.fields[1] else {
                    return Err("skeleton".into());
                };
                let mut data = decode(skeleton, &mut budget()).map_err(|e| format!("{e:?}"))?;
                let NdfValue::Record(data_root) = &mut data else {
                    return Err("group".into());
                };
                let NdfValue::List(entries) = &mut data_root.fields[2] else {
                    return Err("entries".into());
                };
                let NdfValue::Record(entry) = &mut entries[0] else {
                    return Err("entry".into());
                };
                let NdfValue::Record(owner) = &mut entry.fields[0] else {
                    return Err("owner".into());
                };
                owner.fields[0] = NdfValue::U64(99);
                *skeleton = encode(&data, &mut budget()).map_err(|e| format!("{e:?}"))?;
            }
        }
        let bad = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert!(
            codec::decode(
                &bad,
                &a,
                &s,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err(),
            "mode {mode}"
        );
    }
    Ok(())
}
#[test]
fn group_archive_exact_limits_and_empty_cancellation() -> TestResult {
    let (a, s, f, r, group) = setup()?;
    let bytes = codec::encode(
        &group,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut measured = budget();
    codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    let u = measured.usage();
    let exact = Limits {
        work: u.work,
        allocation_units: u.allocation_units,
        source_bytes: u.source_bytes,
        depth: u.depth,
        ..budget().limits()
    };
    codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut Budget::new(exact),
    )
    .map_err(|e| format!("{e:?}"))?;
    for (limits, reason) in [
        (
            Limits {
                work: u.work - 1,
                ..exact
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: u.allocation_units - 1,
                ..exact
            },
            StopReason::AllocationLimit,
        ),
        (
            Limits {
                source_bytes: u.source_bytes - 1,
                ..exact
            },
            StopReason::SourceLimit,
        ),
        (
            Limits {
                depth: u.depth - 1,
                ..exact
            },
            StopReason::DepthLimit,
        ),
    ] {
        let mut b = Budget::new(limits);
        assert!(
            codec::decode(
                &bytes,
                &a,
                &s,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut b
            )
            .is_err()
        );
        assert_eq!(b.poll(), Err(reason));
    }
    let empty = ForeignClosureGroup::default();
    let bytes = codec::encode(
        &empty,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        codec::decode(
            &bytes,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(|e| format!("{e:?}"))?,
        empty
    );
    let mut stopped = budget();
    stopped.stop(StopReason::Cancelled);
    assert_eq!(
        codec::decode(
            &bytes,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut stopped
        )
        .err(),
        Some(WireError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn opaque_payload_survives_and_nonempty_provenance_skeleton_rejects() -> TestResult {
    let (a, s, f, r, mut group) = setup()?;
    let payload = nepl3_wire::source::encode_sources(
        &group.entries[0].syntax.bundle.sources,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    group.entries[0].syntax.bundle.tokens[0].payload =
        decode(&payload, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let bytes = codec::encode(
        &group,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let restored = codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored, group);
    let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(root) = &mut value else {
        return Err("root".into());
    };
    root.fields[1] = NdfValue::Bytes(
        legacy::encode(
            &group,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?,
    );
    let bad = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        codec::decode(
            &bad,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::NonCanonical)
    ));
    Ok(())
}

#[path = "compact.rs"]
mod compact;
