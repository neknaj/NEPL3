use nepl3_core::{
    budget::*, operation::*, schema::*, source::*, syntax::ResourceContent, value::*,
};
use nepl3_wire::{WireError, operation::*};
#[path = "operation/reply.rs"]
mod reply;
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        work: 100_000_000,
        depth: 1024,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn setup() -> Result<(SchemaRegistry, Invoke), String> {
    let mut registry = SchemaRegistry::default();
    let descriptor = foundation::descriptor(&mut budget()).map_err(error)?;
    let schema = descriptor.reference(&mut budget()).map_err(error)?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    // Transport test data has a real registered type. No operation is advertised
    // or executed by this fixture; dispatch supplies its own input contract.
    let typed = TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "TraceOverflow".into(),
        fields: vec![NdfValue::U64(3)],
    });
    let environment = TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "TraceOverflow".into(),
        fields: vec![NdfValue::U64(9)],
    });
    let source = SourceSnapshot::new(
        SourceId("input".into()),
        4,
        "memory:input".into(),
        "世界".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    Ok((
        registry,
        Invoke {
            request_id: 17,
            operation: OperationRef {
                schema,
                name: "fixture".into(),
            },
            input: typed,
            environment,
            sources: vec![source],
            resources: vec![ResourceContent {
                id: "asset".into(),
                digest: Digest::of(b"abc"),
                bytes: b"abc".to_vec(),
            }],
            limits: Limits {
                source_bytes: 1,
                work: 2,
                depth: 3,
                nodes: 4,
                allocation_units: 5,
                output_bytes: 6,
                diagnostics: 7,
                events: 8,
            },
        },
    ))
}

#[test]
fn invoke_preserves_contract_field_order_and_uses_the_callers_budget() -> Result<(), String> {
    let (registry, request) = setup()?;
    let encoded = encode_invoke(
        &request,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    let raw = nepl3_wire::decode(&encoded, &mut budget()).map_err(error)?;
    let NdfValue::Record(root) = &raw else {
        return Err("Invoke record".into());
    };
    assert_eq!(root.kind, "Invoke");
    assert_eq!(root.fields.len(), 7);
    assert_eq!(root.fields[0], NdfValue::U64(17));
    // Distinct payloads expose a symmetric encoder/decoder field swap.
    for (index, expected) in [(2, 3), (3, 9)] {
        let NdfValue::Record(payload) = &root.fields[index] else {
            return Err("typed payload".into());
        };
        assert_eq!(payload.kind, "TraceOverflow");
        assert_eq!(payload.fields, vec![NdfValue::U64(expected)]);
    }
    let NdfValue::Record(limits) = &root.fields[6] else {
        return Err("Limits record".into());
    };
    // Explicit contract order: sourceBytes, work, depth, nodes, allocationUnits,
    // outputBytes, diagnostics, events. All distinct values expose swaps.
    assert_eq!(
        limits.fields,
        (1..=8).map(NdfValue::U64).collect::<Vec<_>>()
    );
    let decoded = decode_invoke(
        &encoded,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    assert_eq!(decoded, request);
    assert_eq!(decoded.sources[0].text(), "世界");
    Ok(())
}

#[test]
fn invoke_rejects_forged_content_and_schema() -> Result<(), String> {
    let (registry, request) = setup()?;
    let bytes = encode_invoke(
        &request,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    for case in ["source", "resource", "schema", "limits", "input"] {
        let mut raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
        let NdfValue::Record(root) = &mut raw else {
            return Err("record".into());
        };
        match case {
            "source" | "resource" => {
                let field = if case == "source" { 4 } else { 5 };
                let NdfValue::List(entries) = &mut root.fields[field] else {
                    return Err("entries".into());
                };
                let NdfValue::Record(entry) = &mut entries[0] else {
                    return Err("entry".into());
                };
                entry.fields[2] = if case == "source" {
                    NdfValue::Text("forged".into())
                } else {
                    NdfValue::Bytes(b"forged".to_vec())
                };
            }
            "schema" => root.schema.digest = Digest([0; 32]),
            "limits" => root.fields[6] = NdfValue::U64(1),
            _ => root.fields[2] = NdfValue::Text("untyped".into()),
        }
        let forged = nepl3_wire::encode(&raw, &mut budget()).map_err(error)?;
        assert!(
            decode_invoke(
                &forged,
                &registry,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err(),
            "{case}"
        );
    }
    for case in ["digest", "duplicate", "empty"] {
        let mut invalid = request.clone();
        match case {
            "digest" => invalid.resources[0].digest = Digest([0; 32]),
            "duplicate" => invalid.resources.push(invalid.resources[0].clone()),
            _ => invalid.resources[0].id.clear(),
        }
        assert!(
            encode_invoke(
                &invalid,
                &registry,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn continuation_roundtrip_retains_provider_parent_and_snapshot_binding() -> Result<(), String> {
    let (registry, request) = setup()?;
    let continuation = Continuation {
        provider: request.operation,
        parent_request: 17,
        snapshot_digest: Digest::of(b"snapshot"),
        state: request.input,
    };
    let bytes = encode_continuation(&continuation, &registry, &mut budget()).map_err(error)?;
    let decoded = decode_continuation(&bytes, &registry, &mut budget()).map_err(error)?;
    assert_eq!(decoded, continuation);
    decoded
        .check_binding(
            &continuation.provider,
            17,
            continuation.snapshot_digest,
            &mut budget(),
        )
        .map_err(error)?;
    for field in ["package", "revision", "digest", "name"] {
        let mut provider = continuation.provider.clone();
        match field {
            "package" => provider.schema.package.push('x'),
            "revision" => provider.schema.revision += 1,
            "digest" => provider.schema.digest = Digest([0; 32]),
            _ => provider.name.push('x'),
        }
        assert_eq!(
            decoded.check_binding(&provider, 17, continuation.snapshot_digest, &mut budget()),
            Err(ContinuationError::Provider)
        );
    }
    assert_eq!(
        decoded.check_binding(
            &continuation.provider,
            18,
            continuation.snapshot_digest,
            &mut budget()
        ),
        Err(ContinuationError::ParentRequest)
    );
    assert_eq!(
        decoded.check_binding(&continuation.provider, 17, Digest([0; 32]), &mut budget()),
        Err(ContinuationError::Snapshot)
    );
    let mut stopped = budget();
    stopped.cancel();
    assert_eq!(
        decoded.check_binding(
            &continuation.provider,
            17,
            continuation.snapshot_digest,
            &mut stopped
        ),
        Err(ContinuationError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(
        decode_continuation(&bytes, &registry, &mut stopped),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(
        encode_continuation(&continuation, &registry, &mut stopped),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn invoke_conversion_preserves_sticky_stops() -> Result<(), String> {
    let (registry, request) = setup()?;
    let bytes = encode_invoke(
        &request,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    for encode in [false, true] {
        let mut limits = budget().limits();
        limits.work = 0;
        let mut stopped = Budget::new(limits);
        if encode {
            assert_eq!(
                encode_invoke(
                    &request,
                    &registry,
                    &mut SourceAdmission::default(),
                    &mut stopped
                ),
                Err(WireError::Stopped(StopReason::WorkLimit))
            );
        } else {
            assert_eq!(
                decode_invoke(
                    &bytes,
                    &registry,
                    &mut SourceAdmission::default(),
                    &mut stopped
                ),
                Err(WireError::Stopped(StopReason::WorkLimit))
            );
        }
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    }
    Ok(())
}
