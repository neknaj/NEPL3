use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{
        FieldDescriptor, NamedType, SchemaDescriptor, SchemaError, SchemaRegistry, TypeDescriptor,
        TypeRef, TypeShape,
    },
    source::Digest,
    value::{NdfValue, Record, SchemaRef},
    value_codec::FoundationCodecError,
};
use nepl3_wire::{WireError, borrowed};

fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        nodes: 100_000,
        depth: 256,
        ..Limits::default()
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn make_registry() -> Result<(SchemaRegistry, SchemaRef), String> {
    let descriptor = SchemaDescriptor {
        package: "borrowed.test".into(),
        revision: 1,
        types: vec![
            NamedType {
                name: "Pair".into(),
                shape: TypeShape::Record {
                    fields: vec![
                        FieldDescriptor {
                            name: "name".into(),
                            ty: TypeDescriptor::Text,
                        },
                        FieldDescriptor {
                            name: "payload".into(),
                            ty: TypeDescriptor::NdfValue,
                        },
                    ],
                },
                constraints: vec![],
            },
            NamedType {
                name: "Empty".into(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            },
        ],
        operations: vec![],
    };
    let reference = descriptor.reference(&mut budget()).map_err(err)?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(reference.clone(), descriptor, &mut budget())
        .map_err(err)?;
    registry.finalize(&mut budget()).map_err(err)?;
    Ok((registry, reference))
}
fn expected() -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "borrowed.test".into(),
        revision: 1,
        name: "Pair".into(),
    })
}

#[test]
fn empty_record_keeps_header_and_root_depth() -> Result<(), String> {
    let (registry, schema) = make_registry()?;
    let owned = NdfValue::Record(Record {
        schema: schema.clone(),
        kind: "Empty".into(),
        fields: vec![],
    });
    let bytes = nepl3_wire::encode(&owned, &mut budget()).map_err(err)?;
    assert_eq!(
        borrowed::record(&schema, "Empty", &[], &registry, &mut budget()).map_err(err)?,
        bytes
    );
    assert_eq!(
        borrowed::record_digest(b"empty", &schema, "Empty", &[], &registry, &mut budget())
            .map_err(err)?,
        Digest::domain(b"empty", &bytes)
    );
    let mut limits = budget().limits();
    limits.depth = 0;
    assert_eq!(
        borrowed::record(&schema, "Empty", &[], &registry, &mut Budget::new(limits))
            .err()
            .ok_or("depth")?
            .stop_reason(),
        Some(StopReason::DepthLimit)
    );
    Ok(())
}

#[test]
fn borrowed_record_matches_owned_bytes_digest_and_independent_decode() -> Result<(), String> {
    let (registry, schema) = make_registry()?;
    let name = NdfValue::Text("共有".into());
    let payload = NdfValue::List(vec![
        NdfValue::Text("data".repeat(8192)),
        NdfValue::Some(Box::new(NdfValue::U64(7))),
    ]);
    let fields = [&name, &payload];
    let mut materialized = budget();
    let owned = NdfValue::Record(Record {
        schema: schema.clone(),
        kind: "Pair".into(),
        fields: fields
            .iter()
            .map(|v| v.clone_with_budget(&mut materialized))
            .collect::<Result<_, _>>()
            .map_err(err)?,
    });
    let expected_bytes =
        nepl3_wire::encode_checked(&owned, &expected(), &registry, &mut materialized)
            .map_err(err)?;
    let mut measured = budget();
    let actual =
        borrowed::record(&schema, "Pair", &fields, &registry, &mut measured).map_err(err)?;
    assert_eq!(actual, expected_bytes);
    assert!(measured.usage().allocation_units < materialized.usage().allocation_units);
    assert_eq!(
        borrowed::record_digest(
            b"domain",
            &schema,
            "Pair",
            &fields,
            &registry,
            &mut budget()
        )
        .map_err(err)?,
        Digest::domain(b"domain", &expected_bytes)
    );
    let (receiver, _) = make_registry()?;
    let received =
        nepl3_wire::decode_checked(&actual, &expected(), &receiver, &mut budget()).map_err(err)?;
    assert_eq!(received.value(), &owned);
    // Repeated use borrows the same immutable payload and changes no bytes.
    assert_eq!(
        borrowed::record(&schema, "Pair", &fields, &registry, &mut budget()).map_err(err)?,
        actual
    );
    Ok(())
}

#[test]
fn borrowed_record_rejects_identity_kind_count_and_nested_type_errors() -> Result<(), String> {
    let (registry, schema) = make_registry()?;
    let text = NdfValue::Text("x".into());
    let unit = NdfValue::Unit;
    let fields = [&text, &unit];
    let mut wrong = schema.clone();
    wrong.digest.0[0] ^= 1;
    for (reference, kind, values, expected) in [
        (
            &wrong,
            "Pair",
            fields.as_slice(),
            SchemaError::UnknownSchema,
        ),
        (
            &schema,
            "Absent",
            fields.as_slice(),
            SchemaError::UnknownType,
        ),
        (&schema, "Pair", &fields[..1], SchemaError::FieldCount),
        (
            &schema,
            "Pair",
            [&unit, &text].as_slice(),
            SchemaError::WrongType,
        ),
    ] {
        assert_eq!(
            borrowed::record(reference, kind, values, &registry, &mut budget()),
            Err(WireError::Schema(expected.clone()))
        );
        assert_eq!(
            borrowed::record_digest(b"d", reference, kind, values, &registry, &mut budget()),
            Err(WireError::Schema(expected))
        );
    }
    let bad = NdfValue::Record(Record {
        schema: wrong,
        kind: "Pair".into(),
        fields: vec![text.clone(), NdfValue::Unit],
    });
    assert!(borrowed::record(&schema, "Pair", &[&text, &bad], &registry, &mut budget()).is_err());
    assert_eq!(
        borrowed::record(
            &schema,
            "Pair",
            &fields,
            &SchemaRegistry::default(),
            &mut budget()
        ),
        Err(WireError::Schema(SchemaError::Unfinalized))
    );
    Ok(())
}

#[test]
fn borrowed_record_counts_parent_depth_and_preserves_sticky_limits() -> Result<(), String> {
    let (registry, schema) = make_registry()?;
    let name = NdfValue::Text("n".into());
    let payload = NdfValue::Some(Box::new(NdfValue::Some(Box::new(NdfValue::Unit))));
    let fields = [&name, &payload];
    for hash in [false, true] {
        let run = |b: &mut Budget| {
            if hash {
                borrowed::record_digest(b"domain", &schema, "Pair", &fields, &registry, b)
                    .map(|d| d.0.to_vec())
            } else {
                borrowed::record(&schema, "Pair", &fields, &registry, b)
            }
        };
        for caller_depth in 0..=1 {
            for depth in 0..=5 {
                let mut limits = budget().limits();
                limits.depth = depth;
                let mut b = Budget::new(limits);
                let result = if caller_depth == 0 {
                    run(&mut b)
                } else {
                    b.with_depth(run)
                };
                if depth < 4 + caller_depth {
                    assert_eq!(
                        result.err().ok_or("expected depth stop")?.stop_reason(),
                        Some(StopReason::DepthLimit)
                    );
                } else {
                    result.map_err(err)?;
                }
            }
        }
        let mut measured = budget();
        let expected = run(&mut measured).map_err(err)?;
        for resource in 0..4 {
            for short in [false, true] {
                let mut limits = budget().limits();
                let (cap, reason) = match resource {
                    0 => (&mut limits.work, StopReason::WorkLimit),
                    1 => (&mut limits.allocation_units, StopReason::AllocationLimit),
                    2 => (&mut limits.nodes, StopReason::NodeLimit),
                    _ => (&mut limits.output_bytes, StopReason::OutputLimit),
                };
                *cap = match resource {
                    0 => measured.usage().work,
                    1 => measured.usage().allocation_units,
                    2 => measured.usage().nodes,
                    _ => measured.usage().output_bytes,
                } - u64::from(short);
                let mut b = Budget::new(limits);
                let result = run(&mut b);
                if short {
                    assert_eq!(
                        result.err().ok_or("expected stop")?.stop_reason(),
                        Some(reason)
                    );
                    assert_eq!(
                        run(&mut b).err().ok_or("sticky stop")?.stop_reason(),
                        Some(reason)
                    );
                } else {
                    assert_eq!(result.map_err(err)?, expected);
                }
            }
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert_eq!(
            run(&mut cancelled).err().ok_or("cancelled")?.stop_reason(),
            Some(StopReason::Cancelled)
        );
    }
    Ok(())
}
