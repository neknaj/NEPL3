use super::*;
use alloc::{vec, vec::Vec};
use nepl3_core::value_codec::FoundationCodecError;
use nepl3_core::{
    budget::{Limits, Resource, StopReason},
    schema::foundation,
    value::Record,
};

type Result = core::result::Result<(), WireError>;
fn limits() -> Limits {
    Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    }
}
fn setup() -> core::result::Result<(SchemaRef, SchemaRegistry), WireError> {
    let mut b = Budget::new(limits());
    let descriptor = foundation::descriptor(&mut b)?;
    let schema = descriptor.reference(&mut b)?;
    let mut registry = SchemaRegistry::default();
    registry.register(schema.clone(), descriptor, &mut b)?;
    registry.finalize(&mut b)?;
    Ok((schema, registry))
}
fn fixture(schema: &SchemaRef, size: usize) -> Environment {
    let bytes = vec![0x61; size];
    Environment {
        bindings: vec![EnvironmentBinding {
            namespace: NamespaceRef {
                schema: schema.clone(),
                name: "fixture".into(),
            },
            name: "source".into(),
            origin: None,
            value: TypedValue::Record(Record {
                schema: schema.clone(),
                kind: "SourceRef".into(),
                fields: vec![
                    NdfValue::Text("fixture".into()),
                    NdfValue::U64(1),
                    NdfValue::Bytes(vec![0; 32]),
                ],
            }),
        }],
        resources: vec![ResourceContent {
            id: "payload".into(),
            digest: Digest::of(&bytes),
            bytes,
        }],
    }
}
// Independent prior composition: checked canonical byte vector, then domain hash.
fn old(
    env: &Environment,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> core::result::Result<(Digest, Vec<u8>), WireError> {
    let value = environment_value(env, schema, b)?;
    let bytes = crate::encode_checked(&value, &expected("Environment"), registry, b)?;
    b.charge(Resource::Work, bytes.len() as u64)?;
    Ok((Digest::domain(b"NEPL3-ENVIRONMENT-1\0", &bytes), bytes))
}
#[test]
fn streaming_environment_digest_matches_checked_bytes_and_reduces_temporary_allocation() -> Result {
    let (schema, registry) = setup()?;
    for env in [
        Environment {
            bindings: vec![],
            resources: vec![],
        },
        fixture(&schema, 32),
        fixture(&schema, 65536),
    ] {
        let mut prior = Budget::new(limits());
        let (expected_digest, bytes) = old(&env, &schema, &registry, &mut prior)?;
        let mut current = Budget::new(limits());
        let actual = environment_digest(&env, &schema, &registry, &mut current)?;
        assert_eq!(actual, expected_digest);
        assert_eq!(current.usage().output_bytes, 32);
        assert_eq!(
            prior.usage().allocation_units - current.usage().allocation_units,
            bytes.len() as u64
        );
        let entry = EnvironmentEntry {
            id: 7,
            digest: actual,
            value: env,
        };
        let value = entry_value(&entry, &schema, &registry, &mut Budget::new(limits()))?;
        let expected_value = record(
            &schema,
            "EnvironmentEntry",
            [
                NdfValue::U64(7),
                NdfValue::Bytes(actual.0.to_vec()),
                crate::decode(&bytes, &mut Budget::new(limits()))?,
            ],
            &mut Budget::new(limits()),
        )?;
        assert_eq!(value, expected_value);
        let restored = entry_from(&value, &schema, &registry, &mut Budget::new(limits()))?;
        assert_eq!(restored, entry);
    }
    Ok(())
}
#[test]
fn streaming_environment_digest_preserves_schema_and_identity_rejection() -> Result {
    let (schema, registry) = setup()?;
    let env = fixture(&schema, 128);
    let entry = EnvironmentEntry {
        id: 1,
        digest: Digest([0; 32]),
        value: env,
    };
    assert!(matches!(
        entry_value(&entry, &schema, &registry, &mut Budget::new(limits())),
        Err(WireError::Syntax(SyntaxError::Environment))
    ));
    assert!(matches!(
        environment_digest(
            &entry.value,
            &schema,
            &SchemaRegistry::default(),
            &mut Budget::new(limits())
        ),
        Err(WireError::Schema(_))
    ));
    let mut malformed = fixture(&schema, 0);
    let TypedValue::Record(value) = &mut malformed.bindings[0].value else {
        return Err(WireError::InvalidType);
    };
    value.fields.clear();
    assert!(matches!(
        environment_digest(&malformed, &schema, &registry, &mut Budget::new(limits())),
        Err(WireError::Schema(_))
    ));
    Ok(())
}
#[test]
fn streaming_environment_digest_obeys_exact_and_short_budgets_and_sticky_stops() -> Result {
    let (schema, registry) = setup()?;
    let env = fixture(&schema, 2048);
    let mut b = Budget::new(limits());
    let expected = environment_digest(&env, &schema, &registry, &mut b)?;
    let usage = b.usage();
    let exact = Limits {
        work: usage.work,
        depth: usage.depth,
        nodes: usage.nodes,
        allocation_units: usage.allocation_units,
        output_bytes: usage.output_bytes,
        ..limits()
    };
    assert_eq!(
        environment_digest(&env, &schema, &registry, &mut Budget::new(exact))?,
        expected
    );
    for (selected, reason) in [
        (
            Limits {
                work: exact.work - 1,
                ..exact
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                depth: exact.depth - 1,
                ..exact
            },
            StopReason::DepthLimit,
        ),
        (
            Limits {
                nodes: exact.nodes - 1,
                ..exact
            },
            StopReason::NodeLimit,
        ),
        (
            Limits {
                allocation_units: exact.allocation_units - 1,
                ..exact
            },
            StopReason::AllocationLimit,
        ),
        (
            Limits {
                output_bytes: 31,
                ..exact
            },
            StopReason::OutputLimit,
        ),
    ] {
        let mut b = Budget::new(selected);
        let first = environment_digest(&env, &schema, &registry, &mut b);
        assert_eq!(
            first.as_ref().err().and_then(|e| e.stop_reason()),
            Some(reason)
        );
        let retry = environment_digest(&env, &schema, &registry, &mut b);
        assert_eq!(
            retry.as_ref().err().and_then(|e| e.stop_reason()),
            Some(reason)
        );
    }
    let mut b = Budget::new(limits());
    b.cancel();
    assert_eq!(
        environment_digest(&env, &schema, &registry, &mut b),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn streaming_environment_digest_preserves_nested_values_at_nonzero_caller_depth() -> Result {
    let (schema, registry) = setup()?;
    let mut env = fixture(&schema, 256);
    for _ in 0..3 {
        let nested = environment_value(&env, &schema, &mut Budget::new(limits()))?;
        let NdfValue::Record(record) = &nested else {
            return Err(WireError::InvalidType);
        };
        env.bindings[0].value = TypedValue::Record(record.clone());
    }
    let (expected, _) = old(&env, &schema, &registry, &mut Budget::new(limits()))?;
    let mut b = Budget::new(limits());
    let actual = b.with_depth_at_least::<_, WireError>(7, |b| {
        environment_digest(&env, &schema, &registry, b)
    })?;
    assert_eq!(actual, expected);
    let peak = b.usage().depth;
    assert!(peak > 7);
    let mut b = Budget::new(Limits {
        depth: peak - 1,
        ..limits()
    });
    let result = b.with_depth_at_least::<_, WireError>(7, |b| {
        environment_digest(&env, &schema, &registry, b)
    });
    assert_eq!(
        result.as_ref().err().and_then(|e| e.stop_reason()),
        Some(StopReason::DepthLimit)
    );
    assert_eq!(b.current_depth(), 0);
    Ok(())
}
