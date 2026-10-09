//! Independent owned-identity reference for the removed temporary-copy path.
use crate::{
    WireError,
    foundation::FoundationCodec,
    source::{expected, record, span_value, text},
};
use alloc::{format, string::String};
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason},
    schema::{SchemaRegistry, foundation},
    source::{
        SourceAdmission, SourceError, SourceId, SourceRef, SourceSnapshot, SourceStore, Span,
    },
    value::{NdfValue, SchemaRef},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};

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
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn setup() -> Result<(SchemaRef, SchemaRegistry), String> {
    let descriptor = foundation::descriptor(&mut budget()).map_err(err)?;
    let schema = descriptor.reference(&mut budget()).map_err(err)?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(err)?;
    registry.finalize(&mut budget()).map_err(err)?;
    Ok((schema, registry))
}
fn source(id: String) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId(id),
        1,
        "memory:span".into(),
        "文x".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(err)
}
fn owned_identity_span_value(
    span: &Span,
    schema: &SchemaRef,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    let id = span.snapshot_ref();
    b.charge(Resource::AllocationUnits, id.source.0.len() as u64)?;
    let reference = SourceRef {
        source_id: id.source.clone(),
        revision: id.revision,
        digest: id.digest,
    };
    b.charge(Resource::AllocationUnits, 32)?;
    let reference = record(
        schema,
        "SourceRef",
        [
            text(&reference.source_id.0, b)?,
            NdfValue::U64(reference.revision),
            NdfValue::Bytes(reference.digest.0.to_vec()),
        ],
        b,
    )?;
    record(
        schema,
        "Span",
        [
            reference,
            NdfValue::U64(span.start()),
            NdfValue::U64(span.end()),
        ],
        b,
    )
}
fn owned_identity_encode(
    span: &Span,
    schema: &SchemaRef,
    r: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    b.charge(
        Resource::AllocationUnits,
        span.snapshot_ref().source.0.len() as u64,
    )?;
    let source = sources
        .get(span.snapshot())
        .ok_or(SourceError::MissingSnapshot)?;
    admission.admit_existing(source, b)?;
    source.slice(span)?;
    let value = owned_identity_span_value(span, schema, b)?;
    b.charge(
        Resource::AllocationUnits,
        ("nepl3.foundation".len() + "Span".len()) as u64,
    )?;
    r.validate(&expected("Span"), &value, b)?;
    Ok(value)
}
#[test]
fn borrowed_span_identity_keeps_values_bytes_digests_and_exact_allocation_saving()
-> Result<(), String> {
    let (schema, r) = setup()?;
    for id in [String::from("a"), "資料/".repeat(2048)] {
        let length = id.len() as u64;
        let source = source(id)?;
        let span = source.span(0, 3).map_err(err)?;
        let mut old = budget();
        let mut new = budget();
        let old_value = owned_identity_span_value(&span, &schema, &mut old).map_err(err)?;
        let new_value = span_value(&span, &schema, &mut new).map_err(err)?;
        assert_eq!(old_value, new_value);
        let mut expected = new.usage();
        expected.allocation_units += length;
        assert_eq!(old.usage(), expected);
        assert_eq!(
            crate::encode(&old_value, &mut budget()).map_err(err)?,
            crate::encode(&new_value, &mut budget()).map_err(err)?
        );
        let mut store = SourceStore::default();
        store.insert(source).map_err(err)?;
        let run = |legacy: bool, limit: Option<u64>| -> Result<_, String> {
            let mut limits = budget().limits();
            if let Some(limit) = limit {
                limits.allocation_units = limit;
            }
            let mut b = Budget::new(limits);
            let mut admission = SourceAdmission::default();
            admission
                .admit_existing(store.get_ref(span.snapshot_ref()).ok_or("source")?, &mut b)
                .map_err(err)?;
            let value = if legacy {
                owned_identity_encode(&span, &schema, &r, &store, &mut admission, &mut b)
            } else {
                FoundationCodec::new(&r, &store, &mut admission)
                    .map_err(err)?
                    .encode_span(&span, &mut b)
            }
            .map_err(err)?;
            Ok((value, b.usage()))
        };
        let old = run(true, None)?;
        let new = run(false, None)?;
        assert_eq!(old.0, new.0);
        let mut expected = new.1;
        expected.allocation_units += 2 * length;
        assert_eq!(old.1, expected);
        assert!(run(false, Some(new.1.allocation_units)).is_ok());
        assert!(run(false, Some(new.1.allocation_units - 1)).is_err());
        let mut limits = budget().limits();
        limits.allocation_units = new.1.allocation_units - 1;
        let mut limited = Budget::new(limits);
        let mut admitted = SourceAdmission::default();
        admitted
            .admit_existing(
                store.get_ref(span.snapshot_ref()).ok_or("source")?,
                &mut limited,
            )
            .map_err(err)?;
        let mut limited_codec = FoundationCodec::new(&r, &store, &mut admitted).map_err(err)?;
        let failure = limited_codec
            .encode_span(&span, &mut limited)
            .err()
            .ok_or("expected allocation stop")?;
        assert_eq!(failure.stop_reason(), Some(StopReason::AllocationLimit));
        let stopped = limited.usage();
        assert!(stopped.allocation_units <= limits.allocation_units);
        assert_eq!(limited.poll(), Err(StopReason::AllocationLimit));
        assert_eq!(
            limited_codec.encode_span(&span, &mut limited),
            Err(WireError::Stopped(StopReason::AllocationLimit))
        );
        assert_eq!(limited.usage(), stopped);
        assert!(run(true, Some(new.1.allocation_units)).is_err());
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        assert_eq!(
            codec
                .canonical_value_digest(b"test.span.identity", &old.0, &mut budget())
                .map_err(err)?,
            codec
                .canonical_value_digest(b"test.span.identity", &new.0, &mut budget())
                .map_err(err)?
        );
    }
    Ok(())
}
#[test]
fn borrowed_span_identity_preserves_prestopped_precedence_and_missing_source() -> Result<(), String>
{
    let (schema, r) = setup()?;
    let span = source("not-declared".into())?.span(0, 3).map_err(err)?;
    let store = SourceStore::default();
    for reason in [StopReason::Cancelled, StopReason::AllocationLimit] {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let mut b = budget();
        b.charge(Resource::Work, 7).map_err(err)?;
        b.stop(reason);
        let before = b.usage();
        assert_eq!(
            codec.encode_span(&span, &mut b),
            Err(WireError::Stopped(reason))
        );
        assert_eq!(b.usage(), before);
        assert_eq!(b.poll(), Err(reason));
        assert_eq!(
            span_value(&span, &schema, &mut b),
            Err(WireError::Stopped(reason))
        );
        assert_eq!(b.usage(), before);
    }
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    assert!(matches!(
        codec.encode_span(&span, &mut budget()),
        Err(WireError::Source(SourceError::MissingSnapshot))
    ));
    let different = SourceSnapshot::new(
        SourceId("not-declared".into()),
        1,
        "memory:span".into(),
        "字x".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let mut mismatch = SourceStore::default();
    mismatch.insert(different).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &mismatch, &mut admission).map_err(err)?;
    assert!(matches!(
        codec.encode_span(&span, &mut budget()),
        Err(WireError::Source(SourceError::MissingSnapshot))
    ));
    Ok(())
}
