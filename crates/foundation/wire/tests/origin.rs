use nepl3_core::{
    budget::{Budget, Limits},
    origin::{Origin, OriginError, OriginId},
    schema::{SchemaRegistry, foundation},
    source::{SourceAdmission, SourceId, SourceSnapshot},
    value::{NdfValue, OperationRef},
};
use nepl3_wire::{WireError, decode, encode, origin::*};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 10_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}

#[test]
fn origin_variants_roundtrip_and_wire_cycles_are_rejected() -> TestResult {
    let mut budget = budget();
    let descriptor = foundation::descriptor(&mut budget).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor
        .reference(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut budget)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("origin".into()),
        1,
        "memory:origin".into(),
        b"abc".to_vec(),
        &mut budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    let span = source.span(0, 1).map_err(|e| format!("{e:?}"))?;
    let origins = vec![
        Origin::Direct(span.clone()),
        Origin::Composite(vec![OriginId(0)]),
        Origin::Generated {
            operation: OperationRef {
                schema: schema.clone(),
                name: "fixture-operation".into(),
            },
            callsite: Some(span),
            inputs: vec![OriginId(1)],
        },
        Origin::Synthetic {
            reason: "fixture generated node".into(),
            anchor: None,
        },
    ];
    // Constructing fixtures is separate from the operation whose admission is measured.
    let mut budget = Budget::new(budget.limits());
    let mut admission = SourceAdmission::default();
    let bytes = encode_origins(
        &origins,
        std::slice::from_ref(&source),
        &schema,
        &registry,
        &mut admission,
        &mut budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    let (snapshots, restored) =
        decode_origins(&bytes, &schema, &registry, &mut admission, &mut budget)
            .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored, origins);
    assert_eq!(snapshots[0], source);
    assert_eq!(budget.usage().source_bytes, 3);
    let mut value = decode(&bytes, &mut budget).map_err(|e| format!("{e:?}"))?;
    if let NdfValue::Record(bundle) = &mut value
        && let NdfValue::List(origins) = &mut bundle.fields[1]
        && let NdfValue::Variant(composite) = &mut origins[1]
        && let NdfValue::List(inputs) = &mut composite.fields[0]
        && let NdfValue::Record(reference) = &mut inputs[0]
    {
        reference.fields[0] = NdfValue::U64(1);
    }
    let bytes = encode(&value, &mut budget).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        decode_origins(&bytes, &schema, &registry, &mut admission, &mut budget),
        Err(WireError::Origin(OriginError::Cycle))
    ));
    Ok(())
}

#[test]
fn long_origin_identity_is_budgeted_at_the_foundation_codec_boundary() -> TestResult {
    use nepl3_core::{budget::StopReason, source::SourceStore, value_codec::FoundationValueCodec};
    use nepl3_wire::foundation::FoundationCodec;
    fn error(e: impl core::fmt::Debug) -> String {
        format!("{e:?}")
    }
    let descriptor = foundation::descriptor(&mut budget()).map_err(error)?;
    let schema = descriptor.reference(&mut budget()).map_err(error)?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema, descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    let source = SourceSnapshot::new(
        SourceId("x".repeat(100_000)),
        0,
        "memory:origin".into(),
        b"a".to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let origins = vec![Origin::Direct(source.span(0, 1).map_err(error)?)];
    let mut store = SourceStore::default();
    // Independently decoded storage cannot use the pointer-equal key shortcut.
    store
        .insert(
            SourceSnapshot::new(
                SourceId("x".repeat(100_000)),
                0,
                "memory:origin".into(),
                b"a".to_vec(),
                &mut budget(),
            )
            .map_err(error)?,
        )
        .map_err(error)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    // Two units cover the old entry/exit traversal. The fixed path must
    // stop at the 200001-unit index comparison after only the entry unit,
    // before inspecting the long key or serializing it.
    let mut limits = budget().limits();
    limits.work = 2;
    let mut stopped = Budget::new(limits);
    assert_eq!(
        codec.encode_origins(&origins, &mut stopped),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(stopped.usage().work, 1);
    let value = codec
        .encode_origins(&origins, &mut budget())
        .map_err(error)?;
    let restored = codec.decode_origins(&value, &mut budget()).map_err(error)?;
    assert_eq!(restored, origins);
    Ok(())
}
