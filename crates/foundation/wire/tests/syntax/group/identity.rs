use super::*;
use nepl3_core::{
    source::{Digest, SourceStore},
    syntax::group::*,
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use nepl3_wire::foundation::FoundationCodec;

fn grouped() -> Result<(SchemaRef, SchemaRegistry, ForeignClosureGroup), String> {
    let (schema, registry, c) = closed()?;
    Ok((
        schema,
        registry,
        ForeignClosureGroup {
            provenances: vec![ForeignProvenance {
                origins: c.owner_origins,
                sources: c.owner_sources,
                source_maps: c.owner_source_maps,
            }],
            environments: vec![ForeignOwnerEnvironment {
                provenance: ProvenanceRef(0),
                environment: c.owner_environment,
            }],
            entries: vec![ForeignGroupEntry {
                owner: OwnerEnvironmentRef(0),
                syntax: c.syntax,
            }],
        },
    ))
}
fn identities(
    group: &ForeignClosureGroup,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<Vec<Digest>, WireError> {
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    FoundationCodec::new(registry, &sources, &mut admission)?.foreign_group_entry_digests(group, b)
}
#[test]
fn component_identity_preserves_subset_and_table_permutation_without_merging_owners() -> TestResult
{
    let (_, registry, mut group) = grouped()?;
    group.provenances.push(group.provenances[0].clone());
    group.environments.push(group.environments[0].clone());
    group.environments[1].provenance = ProvenanceRef(1);
    group.entries.push(group.entries[0].clone());
    group.entries[1].owner = OwnerEnvironmentRef(1);
    let original = identities(&group, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(original[0], original[1]);
    assert_eq!(group.provenances.len(), 2); // Equal content is not owner authority.
    let origin = group.provenances[1].origins[0].clone();
    group.provenances[1].origins.push(origin);
    let changed = identities(&group, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(changed[0], original[0]);
    assert_ne!(changed[1], original[1]);
    group.provenances.swap(0, 1);
    group.environments.swap(0, 1);
    for owner in &mut group.environments {
        owner.provenance.0 = 1 - owner.provenance.0;
    }
    for entry in &mut group.entries {
        entry.owner.0 = 1 - entry.owner.0;
    }
    assert_eq!(
        identities(&group, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?,
        changed
    );
    let valid = group
        .validate(&registry, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    for requested in [vec![1, 0, 1], vec![0], vec![]] {
        let selected = valid
            .select(
                &requested,
                &registry,
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            identities(&selected, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?,
            requested
                .iter()
                .map(|i| changed[*i as usize])
                .collect::<Vec<_>>()
        );
    }
    Ok(())
}
#[test]
fn component_identity_is_sensitive_to_selected_environment_and_syntax() -> TestResult {
    let (schema, registry, group) = grouped()?;
    let original = identities(&group, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut changed = group.clone();
    changed.environments[0].environment.value.bindings[0].name = "renamed".into();
    changed.environments[0].environment.digest = environment_digest(
        &changed.environments[0].environment.value,
        &schema,
        &registry,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    changed.entries[0].syntax.environment.digest = changed.environments[0].environment.digest;
    assert_ne!(
        identities(&changed, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?,
        original
    );
    let mut changed = group.clone();
    let origin = changed.entries[0].syntax.bundle.origins[0].clone();
    changed.entries[0].syntax.bundle.origins.push(origin);
    assert_ne!(
        identities(&changed, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?,
        original
    );
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
    assert_eq!(
        identities(&decoded, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?,
        original
    );
    let mut bad = group.clone();
    bad.entries[0].owner = OwnerEnvironmentRef(999);
    assert!(identities(&bad, &registry, &mut budget()).is_err());
    let mut bad = group.clone();
    bad.environments[0].environment.digest.0[0] ^= 1;
    bad.entries[0].syntax.environment.digest = bad.environments[0].environment.digest;
    assert!(identities(&bad, &registry, &mut budget()).is_err());
    Ok(())
}
#[test]
fn component_identity_limits_are_exact_and_sticky() -> TestResult {
    let (_, registry, group) = grouped()?;
    let mut measured = budget();
    identities(&group, &registry, &mut measured).map_err(|e| format!("{e:?}"))?;
    let u = measured.usage();
    for (used, reason) in [
        (u.work, StopReason::WorkLimit),
        (u.allocation_units, StopReason::AllocationLimit),
        (u.nodes, StopReason::NodeLimit),
        (u.source_bytes, StopReason::SourceLimit),
        (u.depth, StopReason::DepthLimit),
        (u.output_bytes, StopReason::OutputLimit),
    ] {
        if used == 0 {
            continue;
        }
        for exact in [false, true] {
            let mut limits = budget().limits();
            let n = used - u64::from(!exact);
            match reason {
                StopReason::WorkLimit => limits.work = n,
                StopReason::AllocationLimit => limits.allocation_units = n,
                StopReason::NodeLimit => limits.nodes = n,
                StopReason::SourceLimit => limits.source_bytes = n,
                StopReason::DepthLimit => limits.depth = n,
                StopReason::OutputLimit => limits.output_bytes = n,
                _ => return Err("case".into()),
            }
            let mut b = Budget::new(limits);
            let result = identities(&group, &registry, &mut b);
            if exact {
                result.map_err(|e| format!("{e:?}"))?;
            } else {
                assert_eq!(result.err().and_then(|e| e.stop_reason()), Some(reason));
                assert_eq!(b.poll(), Err(reason));
            }
        }
    }
    let mut stopped = budget();
    stopped.stop(StopReason::Cancelled);
    assert_eq!(
        identities(&group, &registry, &mut stopped)
            .err()
            .and_then(|e| e.stop_reason()),
        Some(StopReason::Cancelled)
    );
    Ok(())
}
#[test]
fn component_identity_owner_growth_is_independent_of_guest_count() -> TestResult {
    let mut work = [[0; 2]; 2];
    let mut allocation = [[0; 2]; 2];
    for (n, guests) in [16, 32].into_iter().enumerate() {
        for (m, origins) in [128, 256].into_iter().enumerate() {
            let (_, registry, mut group) = grouped()?;
            let origin = group.provenances[0].origins[1].clone();
            group.provenances[0].origins = vec![origin; origins];
            group.entries = vec![group.entries[0].clone(); guests];
            let mut b = budget();
            assert_eq!(
                identities(&group, &registry, &mut b)
                    .map_err(|e| format!("{e:?}"))?
                    .len(),
                guests
            );
            work[n][m] = b.usage().work;
            allocation[n][m] = b.usage().allocation_units;
        }
    }
    assert!(work[0][1] > work[0][0]);
    assert_eq!(work[0][1] - work[0][0], work[1][1] - work[1][0]);
    assert!(allocation[0][1] > allocation[0][0]);
    assert_eq!(
        allocation[0][1] - allocation[0][0],
        allocation[1][1] - allocation[1][0]
    );
    Ok(())
}

#[test]
fn component_identity_has_explicit_framing_and_standalone_component_parity() -> TestResult {
    use sha2::{Digest as _, Sha256};
    fn record(value: &NdfValue) -> Result<&Record, String> {
        match value {
            NdfValue::Record(r) => Ok(r),
            _ => Err("record".into()),
        }
    }
    fn hash(domain: &[u8], value: &NdfValue) -> Result<Digest, String> {
        let bytes = encode(value, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let mut h = Sha256::new();
        h.update(domain);
        h.update(bytes);
        Ok(Digest(h.finalize().into()))
    }
    let (schema, registry, group) = grouped()?;
    let valid = group
        .validate(&registry, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    let standalone = valid
        .entry(0)
        .ok_or("entry")?
        .materialize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&registry, &sources, &mut admission).map_err(|e| format!("{e:?}"))?;
    let flat = codec
        .encode_foreign_closure(&standalone, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let f = &record(&flat)?.fields;
    let provenance = NdfValue::Record(Record {
        schema,
        kind: "ForeignProvenance".into(),
        fields: vec![f[2].clone(), f[3].clone(), f[4].clone()],
    });
    let p = hash(b"NEPL3.Foreign.Provenance.v1\0", &provenance)?;
    let e = hash(b"NEPL3.Foreign.Environment.v1\0", &f[1])?;
    let s = hash(b"NEPL3.Foreign.Syntax.v1\0", &f[0])?;
    let composed = NdfValue::List(vec![
        NdfValue::Bytes(p.0.to_vec()),
        NdfValue::Bytes(e.0.to_vec()),
        NdfValue::Bytes(s.0.to_vec()),
    ]);
    let expected = hash(b"NEPL3.Foreign.Entry.v1\0", &composed)?;
    assert_eq!(
        identities(&group, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?,
        vec![expected]
    );
    assert_ne!(expected, hash(b"NEPL3.Foreign.Entry.v1\0", &flat)?);
    assert_ne!(expected, hash(b"NEPL3.Foreign.Entry.v2\0", &composed)?);
    let mut malformed = group.clone();
    malformed.entries[0].syntax.schema.digest.0[0] ^= 1;
    assert!(identities(&malformed, &registry, &mut budget()).is_err());
    assert!(identities(&group, &SchemaRegistry::default(), &mut budget()).is_err());
    Ok(())
}

#[test]
fn fused_group_encoding_matches_separate_validation_and_charges_less() -> TestResult {
    let (_, registry, group) = grouped()?;
    let sources = SourceStore::default();
    let mut old_admission = SourceAdmission::default();
    let mut old_budget = budget();
    group
        .validate_at_depths(&[3], &registry, &mut old_budget, &mut old_admission)
        .map_err(|e| format!("{e:?}"))?;
    let expected = FoundationCodec::new(&registry, &sources, &mut old_admission)
        .map_err(|e| format!("{e:?}"))?
        .encode_foreign_group(&group, &mut old_budget)
        .map_err(|e| format!("{e:?}"))?;
    let mut admission = SourceAdmission::default();
    let mut b = budget();
    let actual = FoundationCodec::new(&registry, &sources, &mut admission)
        .map_err(|e| format!("{e:?}"))?
        .encode_foreign_group_at_depths(&group, &[3], &registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(actual, expected);
    // The baseline actually performs the extra pass, rather than using a
    // frozen arbitrary expected total as the performance oracle.
    assert!(b.usage().work < old_budget.usage().work);
    assert_eq!(b.usage().depth, old_budget.usage().depth);
    Ok(())
}

#[test]
fn fused_group_encoding_preserves_native_error_phase() -> TestResult {
    use nepl3_core::value_codec::GroupEncodingError;
    let (_, registry, group) = grouped()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let result = FoundationCodec::new(&registry, &sources, &mut admission)
        .map_err(|e| format!("{e:?}"))?
        .encode_foreign_group_at_depths(&group, &[], &registry, &mut budget());
    assert!(matches!(
        result,
        Err(GroupEncodingError::Structure(GroupError::DepthCount))
    ));
    Ok(())
}

#[test]
fn fused_group_encoding_checks_distinct_registry_and_forged_digest() -> TestResult {
    use nepl3_core::value_codec::GroupEncodingError;
    let (_, registry, group) = grouped()?;
    let (_, equivalent, _) = grouped()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut b = budget();
    let actual = FoundationCodec::new(&registry, &sources, &mut admission)
        .map_err(|e| format!("{e:?}"))?
        .encode_foreign_group_at_depths(&group, &[2], &equivalent, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let mut admission = SourceAdmission::default();
    let expected = FoundationCodec::new(&registry, &sources, &mut admission)
        .map_err(|e| format!("{e:?}"))?
        .encode_foreign_group(&group, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(actual, expected);
    let mut same_budget = budget();
    let mut same_admission = SourceAdmission::default();
    FoundationCodec::new(&registry, &sources, &mut same_admission)
        .map_err(|e| format!("{e:?}"))?
        .encode_foreign_group_at_depths(&group, &[2], &registry, &mut same_budget)
        .map_err(|e| format!("{e:?}"))?;
    assert!(b.usage().work > same_budget.usage().work);
    let mut changed = group;
    changed.environments[0].environment.digest.0[0] ^= 1;
    changed.entries[0].syntax.environment.digest = changed.environments[0].environment.digest;
    let mut admission = SourceAdmission::default();
    // Association still agrees, but serialization must recompute the digest.
    let result = FoundationCodec::new(&registry, &sources, &mut admission)
        .map_err(|e| format!("{e:?}"))?
        .encode_foreign_group_at_depths(&changed, &[2], &registry, &mut budget());
    assert!(matches!(result, Err(GroupEncodingError::Codec(_))));
    Ok(())
}

fn fused(
    group: &ForeignClosureGroup,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    b.charge(nepl3_core::budget::Resource::Work, 7)?;
    b.with_depth_at_least(4, |b| {
        FoundationCodec::new(registry, &sources, &mut admission)?
            .encode_foreign_group_at_depths(group, &[3], registry, b)
            .map_err(|e| match e {
                nepl3_core::value_codec::GroupEncodingError::Structure(e) => e.into(),
                nepl3_core::value_codec::GroupEncodingError::Codec(e) => e,
            })
    })
}
#[test]
fn fused_group_encoding_limits_are_exact_and_sticky() -> TestResult {
    let (_, registry, group) = grouped()?;
    let mut measured = budget();
    fused(&group, &registry, &mut measured).map_err(|e| format!("{e:?}"))?;
    let u = measured.usage();
    for (used, reason) in [
        (u.work, StopReason::WorkLimit),
        (u.allocation_units, StopReason::AllocationLimit),
        (u.nodes, StopReason::NodeLimit),
        (u.source_bytes, StopReason::SourceLimit),
        (u.depth, StopReason::DepthLimit),
        (u.output_bytes, StopReason::OutputLimit),
    ] {
        if used == 0 {
            continue;
        }
        for exact in [false, true] {
            let mut limits = budget().limits();
            let n = used - u64::from(!exact);
            match reason {
                StopReason::WorkLimit => limits.work = n,
                StopReason::AllocationLimit => limits.allocation_units = n,
                StopReason::NodeLimit => limits.nodes = n,
                StopReason::SourceLimit => limits.source_bytes = n,
                StopReason::DepthLimit => limits.depth = n,
                StopReason::OutputLimit => limits.output_bytes = n,
                _ => return Err("case".into()),
            }
            let mut b = Budget::new(limits);
            let result = fused(&group, &registry, &mut b);
            if exact {
                result.map_err(|e| format!("{e:?}"))?;
            } else {
                assert_eq!(result.err().and_then(|e| e.stop_reason()), Some(reason));
                assert_eq!(b.poll(), Err(reason));
            }
        }
    }
    let mut stopped = budget();
    stopped.stop(StopReason::Cancelled);
    assert_eq!(
        fused(&group, &registry, &mut stopped)
            .err()
            .and_then(|e| e.stop_reason()),
        Some(StopReason::Cancelled)
    );
    Ok(())
}

#[test]
fn fused_group_encoding_rejects_incompatible_codec_registry() -> TestResult {
    use nepl3_core::value_codec::GroupEncodingError;
    let (_, native, group) = grouped()?;
    let mut descriptor = foundation::descriptor(&mut budget()).map_err(|e| format!("{e:?}"))?;
    descriptor.types.push(nepl3_core::schema::NamedType {
        name: "ExtraTypeForDifferentIdentity".into(),
        shape: nepl3_core::schema::TypeShape::Record { fields: vec![] },
        constraints: vec![],
    });
    let reference = descriptor
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut incompatible = SchemaRegistry::default();
    incompatible
        .register(reference, descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    incompatible
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let result = FoundationCodec::new(&incompatible, &sources, &mut admission)
        .map_err(|e| format!("{e:?}"))?
        .encode_foreign_group_at_depths(&group, &[2], &native, &mut budget());
    assert!(matches!(result, Err(GroupEncodingError::Codec(_))));
    Ok(())
}
