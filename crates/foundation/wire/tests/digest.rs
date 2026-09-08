use nepl3_core::{
    budget::*, schema::*, source::*, value::NdfValue, value_codec::FoundationValueCodec,
};
use nepl3_wire::{WireError, foundation::FoundationCodec};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100,
        work: 1_000_000,
        depth: 100,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn canonical_digest_uses_ndf_tagged_cbor_and_keeps_budget_stops() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?;
    registry
        .register(
            descriptor.reference(&mut budget()).map_err(err)?,
            descriptor,
            &mut budget(),
        )
        .map_err(err)?;
    registry.finalize(&mut budget()).map_err(err)?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(err)?;
    // NDF/1 Unit is the tagged CBOR array [0], not CBOR null.
    let expected = Digest::domain(b"digest-test\0", &[0x81, 0x00]);
    assert_eq!(
        codec
            .canonical_value_digest(b"digest-test\0", &NdfValue::Unit, &mut budget())
            .map_err(err)?,
        expected
    );
    assert_ne!(
        codec
            .canonical_value_digest(b"another-domain\0", &NdfValue::Unit, &mut budget())
            .map_err(err)?,
        expected
    );
    // NDF Text is [5, text]. Its canonical CBOR header for 100000 ASCII
    // bytes is 82 05 7a 00 01 86 a0, independently derived from RFC8949.
    let text = "x".repeat(100_000);
    let mut encoded = vec![0x82, 0x05, 0x7a, 0x00, 0x01, 0x86, 0xa0];
    encoded.extend_from_slice(text.as_bytes());
    let expected = Digest::domain(b"large\0", &encoded);
    let mut limits = budget().limits();
    limits.output_bytes = 32;
    limits.allocation_units = 4096;
    let mut streamed = Budget::new(limits);
    assert_eq!(
        codec
            .canonical_value_digest(b"large\0", &NdfValue::Text(text), &mut streamed)
            .map_err(err)?,
        expected
    );
    assert_eq!(streamed.usage().output_bytes, 32);
    assert!(streamed.usage().work >= 2 * encoded.len() as u64 + 6);
    // A broad list must not construct its entire frontier under only enough
    // Work for the parent's CBOR headers. Input construction is outside Budget.
    let broad = NdfValue::List(vec![NdfValue::Unit; 100_000]);
    let mut limits = budget().limits();
    limits.work = 15;
    let mut bounded = Budget::new(limits);
    assert_eq!(
        codec.canonical_value_digest(b"", &broad, &mut bounded),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    assert!(bounded.usage().allocation_units <= 128);
    // Big rational denominators need conversion Work before allocating bytes,
    // even when their numerator and encoding headers are small.
    let mut denominator = vec![0; 1024];
    denominator[0] = 0x80;
    let rational = nepl3_core::value::Rational::from_canonical(
        nepl3_core::value::Integer::from(1_i64),
        &denominator,
    )
    .map_err(err)?;
    let mut limits = budget().limits();
    limits.work = 128;
    let mut bounded = Budget::new(limits);
    assert_eq!(
        codec.canonical_value_digest(b"", &NdfValue::Rational(rational), &mut bounded),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    assert!(bounded.usage().allocation_units < denominator.len() as u64);
    for resource in 0..5 {
        let mut limits = budget().limits();
        let reason = match resource {
            0 => {
                limits.work = 0;
                StopReason::WorkLimit
            }
            1 => {
                limits.allocation_units = 0;
                StopReason::AllocationLimit
            }
            2 => {
                limits.output_bytes = 0;
                StopReason::OutputLimit
            }
            3 => {
                limits.nodes = 0;
                StopReason::NodeLimit
            }
            _ => {
                limits.depth = 0;
                StopReason::DepthLimit
            }
        };
        let mut trial = Budget::new(limits);
        assert_eq!(
            codec.canonical_value_digest(b"digest-test\0", &NdfValue::Unit, &mut trial),
            Err(WireError::Stopped(reason))
        );
        assert_eq!(trial.poll(), Err(reason));
    }
    Ok(())
}
