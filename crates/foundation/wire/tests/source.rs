use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{SchemaRegistry, foundation},
    source::{SourceAdmission, SourceError, SourceId, SourceSnapshot, SourceStore},
    value::{NdfValue, SchemaRef},
};
use nepl3_wire::{WireError, decode, encode, source::*};

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 1024,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn setup() -> Result<(SchemaRef, SchemaRegistry), String> {
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
    Ok((schema, registry))
}
fn snapshot(id: &str, uri: &str, text: &str) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId(id.into()),
        1,
        uri.into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))
}
fn wire(bytes: &[u8], change: impl FnOnce(&mut Vec<NdfValue>)) -> Result<Vec<u8>, String> {
    let mut value = decode(bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(root) = &mut value else {
        return Err("bundle record".into());
    };
    let Some(NdfValue::List(entries)) = root.fields.first_mut() else {
        return Err("source list".into());
    };
    change(entries);
    encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))
}

#[test]
fn source_and_span_roundtrip_preserve_identity_bytes_and_shared_admission() -> TestResult {
    let (schema, registry) = setup()?;
    // Equal locators and contents must never coalesce distinct document identities.
    let sources = vec![
        snapshot("b", "memory:test", "a\r\n文😀")?,
        snapshot("a", "memory:test", "a\r\n文😀")?,
    ];
    let bytes = encode_sources(
        &sources,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut admission = SourceAdmission::default();
    let mut usage = budget();
    let restored = decode_sources(&bytes, &schema, &registry, &mut admission, &mut usage)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored[0].reference().source_id.0, "a");
    assert_eq!(restored[0].text().as_bytes(), sources[0].text().as_bytes());
    assert_ne!(restored[0].id(), restored[1].id());
    let charged = usage.usage().source_bytes;
    decode_sources(&bytes, &schema, &registry, &mut admission, &mut usage)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(usage.usage().source_bytes, charged);
    let span = restored[0].span(3, 6).map_err(|e| format!("{e:?}"))?;
    let encoded =
        encode_span(&span, &schema, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut store = SourceStore::default();
    for source in restored {
        store.insert(source).map_err(|e| format!("{e:?}"))?;
    }
    assert_eq!(
        decode_span(&encoded, &schema, &registry, &store, &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        span
    );
    assert!(matches!(
        decode_span(
            &encoded,
            &schema,
            &registry,
            &SourceStore::default(),
            &mut budget()
        ),
        Err(WireError::Source(SourceError::MissingSnapshot))
    ));
    Ok(())
}

#[test]
fn malformed_source_declarations_reject_without_committing_and_retry_does_not_refund() -> TestResult
{
    let (schema, registry) = setup()?;
    let sources = vec![
        snapshot("a", "memory:a", "first")?,
        snapshot("b", "memory:b", "second")?,
    ];
    let bytes = encode_sources(
        &sources,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let forged = wire(&bytes, |entries| {
        if let NdfValue::Record(content) = &mut entries[1]
            && let NdfValue::Record(reference) = &mut content.fields[0]
        {
            reference.fields[2] = NdfValue::Bytes(vec![0; 32]);
        }
    })?;
    let mut admission = SourceAdmission::default();
    let mut usage = budget();
    assert!(matches!(
        decode_sources(&forged, &schema, &registry, &mut admission, &mut usage),
        Err(WireError::Source(SourceError::ExpectedDigest))
    ));
    assert_eq!(usage.usage().source_bytes, 5);
    // No SourceStore is returned/modified by the failed bundle; only resource ledger survives.
    decode_sources(&bytes, &schema, &registry, &mut admission, &mut usage)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(usage.usage().source_bytes, 11);
    let changed_uri = wire(&bytes, |entries| {
        if let NdfValue::Record(content) = &mut entries[0] {
            content.fields[1] = NdfValue::Text("memory:other".into());
        }
    })?;
    assert!(matches!(
        decode_sources(&changed_uri, &schema, &registry, &mut admission, &mut usage),
        Err(WireError::Source(SourceError::IdentityConflict))
    ));
    for bad in [
        wire(&bytes, |entries| entries.reverse())?,
        wire(&bytes, |entries| {
            entries[1] = entries[0].clone();
        })?,
    ] {
        assert!(matches!(
            decode_sources(
                &bad,
                &schema,
                &registry,
                &mut SourceAdmission::default(),
                &mut budget()
            ),
            Err(WireError::Source(SourceError::IdentityConflict))
        ));
    }
    Ok(())
}

#[test]
fn native_and_wire_admission_reject_zero_source_limit_and_span_scalar_interior() -> TestResult {
    let (schema, registry) = setup()?;
    let source = snapshot("a", "memory:a", "文")?;
    let bytes = encode_sources(
        std::slice::from_ref(&source),
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let zero = || {
        let mut limits = budget().limits();
        limits.source_bytes = 0;
        Budget::new(limits)
    };
    assert!(matches!(
        encode_sources(
            std::slice::from_ref(&source),
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut zero()
        ),
        Err(WireError::Stopped(StopReason::SourceLimit))
    ));
    assert!(matches!(
        decode_sources(
            &bytes,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut zero()
        ),
        Err(WireError::Stopped(StopReason::SourceLimit))
    ));
    // Wire framing/UTF-8 is validated before embedded source admission is possible.
    let mut malformed = bytes.clone();
    let position = malformed
        .windows(3)
        .position(|bytes| bytes == "文".as_bytes())
        .ok_or("missing UTF-8 fixture")?;
    malformed[position] = 0xff;
    assert!(matches!(
        decode_sources(
            &malformed,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut zero()
        ),
        Err(WireError::InvalidUtf8)
    ));
    // A native raw-byte snapshot constructor already knows the input byte length.
    assert!(matches!(
        SourceSnapshot::new(
            SourceId("bad".into()),
            1,
            "memory:bad".into(),
            vec![0xff],
            &mut zero()
        ),
        Err(SourceError::Stopped(StopReason::SourceLimit))
    ));
    let span = source.span(0, 3).map_err(|e| format!("{e:?}"))?;
    let bytes =
        encode_span(&span, &schema, &registry, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    if let NdfValue::Record(record) = &mut value {
        record.fields[1] = NdfValue::U64(1);
    }
    let bytes = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut store = SourceStore::default();
    store.insert(source).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        decode_span(&bytes, &schema, &registry, &store, &mut budget()),
        Err(WireError::Source(SourceError::ScalarBoundary))
    ));
    Ok(())
}
