use super::*;
use alloc::{format, string::String, vec};
use nepl3_core::{
    budget::{Limits, StopReason},
    schema::foundation,
};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn fixture(size: usize) -> Result<(SchemaRef, SchemaRegistry, EnvironmentEntry), String> {
    let mut b = budget();
    let descriptor = foundation::descriptor(&mut b).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor.reference(&mut b).map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    registry.finalize(&mut b).map_err(|e| format!("{e:?}"))?;
    let data = vec![42; size];
    let value = Environment {
        bindings: vec![EnvironmentBinding {
            namespace: NamespaceRef {
                schema: schema.clone(),
                name: "fixture".into(),
            },
            name: "source".into(),
            value: TypedValue::Record(nepl3_core::value::Record {
                schema: schema.clone(),
                kind: "SourceRef".into(),
                fields: vec![
                    NdfValue::Text("source".into()),
                    NdfValue::U64(1),
                    NdfValue::Bytes(vec![0; 32]),
                ],
            }),
            origin: None,
        }],
        resources: vec![ResourceContent {
            id: "fixture".into(),
            digest: Digest::of(&data),
            bytes: data,
        }],
    };
    let digest =
        environment_digest(&value, &schema, &registry, &mut b).map_err(|e| format!("{e:?}"))?;
    Ok((
        schema,
        registry,
        EnvironmentEntry {
            id: 7,
            digest,
            value,
        },
    ))
}
// Independent old composition: hashing builds a temporary Environment, then
// the boundary record builds it again. Its bytes remain the compatibility oracle.
fn duplicate(
    entry: &EnvironmentEntry,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    if environment_digest(&entry.value, schema, registry, b)? != entry.digest {
        return Err(SyntaxError::Environment.into());
    }
    record(
        schema,
        "EnvironmentEntry",
        [
            NdfValue::U64(entry.id),
            bytes(&entry.digest.0, b)?,
            environment_value(&entry.value, schema, b)?,
        ],
        b,
    )
}
#[test]
fn entry_reuses_exact_environment_value_without_changing_wire_bytes() -> Result<(), String> {
    for size in [0, 65536] {
        let (schema, registry, entry) = fixture(size)?;
        let mut before = budget();
        let mut after = budget();
        let old =
            duplicate(&entry, &schema, &registry, &mut before).map_err(|e| format!("{e:?}"))?;
        let new =
            entry_value(&entry, &schema, &registry, &mut after).map_err(|e| format!("{e:?}"))?;
        assert_eq!(old, new);
        assert!(before.usage().allocation_units > after.usage().allocation_units + size as u64);
        assert!(before.usage().work > after.usage().work);
        assert_eq!(
            crate::encode(&old, &mut budget()).map_err(|e| format!("{e:?}"))?,
            crate::encode(&new, &mut budget()).map_err(|e| format!("{e:?}"))?
        );
        let decoded =
            entry_from(&new, &schema, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert_eq!(decoded, entry);
    }
    Ok(())
}
#[test]
fn entry_reuse_still_rejects_forged_digest() -> Result<(), String> {
    let (schema, registry, mut entry) = fixture(10)?;
    entry.digest.0[0] ^= 1;
    assert!(matches!(
        entry_value(&entry, &schema, &registry, &mut budget()),
        Err(WireError::Syntax(SyntaxError::Environment))
    ));
    Ok(())
}
#[test]
fn entry_reuse_preserves_sticky_resource_stops() -> Result<(), String> {
    let (schema, registry, entry) = fixture(100)?;
    for allocation in [false, true] {
        let mut limits = budget().limits();
        if allocation {
            limits.allocation_units = 0;
        } else {
            limits.work = 0;
        }
        let mut b = Budget::new(limits);
        assert!(entry_value(&entry, &schema, &registry, &mut b).is_err());
        assert_eq!(
            b.poll(),
            Err(if allocation {
                StopReason::AllocationLimit
            } else {
                StopReason::WorkLimit
            })
        );
    }
    let mut b = budget();
    b.cancel();
    assert!(entry_value(&entry, &schema, &registry, &mut b).is_err());
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn entry_reuse_still_validates_typed_environment_payload() -> Result<(), String> {
    let (schema, registry, mut entry) = fixture(0)?;
    if let TypedValue::Record(value) = &mut entry.value.bindings[0].value {
        value.fields.clear();
    }
    assert!(matches!(
        entry_value(&entry, &schema, &registry, &mut budget()),
        Err(WireError::Schema(
            nepl3_core::schema::SchemaError::FieldCount
        ))
    ));
    Ok(())
}
