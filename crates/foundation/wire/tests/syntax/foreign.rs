use super::*;
use nepl3_core::{
    budget::StopReason,
    source::SourceStore,
    value::{Record, TypedValue},
    value_codec::FoundationValueCodec,
};
use nepl3_wire::foundation::FoundationCodec;
fn closed() -> Result<(SchemaRef, SchemaRegistry, ForeignClosure), String> {
    let (schema, registry, mut owner) = fixture()?;
    // The owner uses a forward origin edge. The guest's Origin0 has a different
    // span, and its Env9 has different data. Neither local ID may be rebound.
    owner.origins = vec![
        Origin::Composite(vec![OriginId(1)]),
        owner.origins[0].clone(),
    ];
    owner.environments[0].id = 9;
    owner.environments[0]
        .value
        .bindings
        .push(EnvironmentBinding {
            namespace: NamespaceRef {
                schema: schema.clone(),
                name: "Owner".into(),
            },
            name: "owner".into(),
            value: TypedValue::Record(Record {
                schema: schema.clone(),
                kind: "NodeRef".into(),
                fields: vec![NdfValue::U64(7)],
            }),
            origin: Some(OriginId(0)),
        });
    owner.environments[0].digest = environment_digest(
        &owner.environments[0].value,
        &schema,
        &registry,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let FieldValue::Foreign(guest) = &mut owner.nodes[0].fields[0] else {
        return Err("guest".into());
    };
    guest.environment = EnvironmentRef {
        id: 9,
        digest: owner.environments[0].digest,
    };
    let mut local = owner.environments[0].clone();
    local.value.bindings[0].name = "guest".into();
    local.digest = environment_digest(&local.value, &schema, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    guest.bundle.environments.push(local);
    let checked = owner
        .validate(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let FieldValue::Foreign(guest) = &owner.nodes[0].fields[0] else {
        return Err("guest".into());
    };
    let value = ForeignClosure::capture(
        guest,
        &checked,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("{e:?}"))?;
    Ok((schema, registry, value))
}
#[test]
fn standalone_foreign_keeps_selected_owner_environment_and_origin_arena() -> TestResult {
    let (schema, registry, value) = closed()?;
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let bytes = encode_foreign_closure(&value, &schema, &registry, &mut a, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let received = decode_foreign_closure(&bytes, &schema, &registry, &mut a, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(received, value);
    assert_eq!(b.usage().source_bytes, 2);
    assert_ne!(
        received.owner_environment,
        received.syntax.bundle.environments[0]
    );
    assert_ne!(received.owner_origins[0], received.syntax.bundle.origins[0]);
    assert_eq!(
        received.owner_environment.value.bindings[0].origin,
        Some(OriginId(0))
    );
    assert_eq!(
        encode_foreign_closure(&received, &schema, &registry, &mut a, &mut b)
            .map_err(|e| format!("{e:?}"))?,
        bytes
    );
    // Trait boundary works with no sender object or ambient source store.
    let raw = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let empty = SourceStore::default();
    let mut fresh = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&registry, &empty, &mut fresh).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        codec
            .decode_foreign_closure(&raw, &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        value
    );
    Ok(())
}
#[test]
fn standalone_foreign_rejects_missing_owner_closure_hash_and_cross_arena_claims() -> TestResult {
    let (schema, registry, value) = closed()?;
    let mut variants = Vec::new();
    let mut bad = value.clone();
    bad.owner_sources.clear();
    variants.push(bad);
    let mut bad = value.clone();
    bad.syntax.bundle.sources.clear();
    variants.push(bad);
    let mut bad = value.clone();
    bad.owner_origins.clear();
    variants.push(bad);
    let mut bad = value.clone();
    bad.owner_environment.id = 10;
    variants.push(bad);
    let mut bad = value.clone();
    bad.owner_environment.value.bindings[0].name = "forged".into();
    variants.push(bad);
    let mut bad = value.clone();
    bad.owner_sources.push(bad.owner_sources[0].clone());
    variants.push(bad);
    for bad in variants {
        assert!(
            encode_foreign_closure(
                &bad,
                &schema,
                &registry,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err()
        );
    }
    let mut ambient = SourceStore::default();
    ambient
        .insert(value.owner_sources[0].clone())
        .map_err(|e| format!("{e:?}"))?;
    let bytes = encode_foreign_closure(
        &value,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    for index in [2, 3] {
        let mut raw = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let NdfValue::Record(root) = &mut raw else {
            return Err("root".into());
        };
        root.fields[index] = NdfValue::List(vec![]);
        let mut a = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&registry, &ambient, &mut a).map_err(|e| format!("{e:?}"))?;
        assert!(codec.decode_foreign_closure(&raw, &mut budget()).is_err());
    }
    let mut descriptor = registry.descriptor(&schema).ok_or("descriptor")?.clone();
    descriptor.package = "other".into();
    let other = descriptor
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut both = SchemaRegistry::default();
    both.register(
        schema.clone(),
        registry.descriptor(&schema).ok_or("descriptor")?.clone(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    both.register(other.clone(), descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    both.finalize(&mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut bad = value;
    bad.syntax.schema = other;
    assert!(matches!(
        bad.validate(&both, &mut budget(), &mut SourceAdmission::default()),
        Err(SyntaxError::ForeignRoot)
    ));
    for which in 0..5 {
        let mut limits = budget().limits();
        let reason = match which {
            0 => {
                limits.source_bytes = 0;
                StopReason::SourceLimit
            }
            1 => {
                limits.work = 0;
                StopReason::WorkLimit
            }
            2 => {
                limits.allocation_units = 0;
                StopReason::AllocationLimit
            }
            3 => {
                limits.depth = 0;
                StopReason::DepthLimit
            }
            _ => {
                limits.nodes = 0;
                StopReason::NodeLimit
            }
        };
        let mut b = Budget::new(limits);
        let Err(error) = decode_foreign_closure(
            &bytes,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut b,
        ) else {
            return Err("expected stopped".into());
        };
        assert_eq!(
            nepl3_core::value_codec::FoundationCodecError::stop_reason(&error),
            Some(reason)
        );
    }
    Ok(())
}
