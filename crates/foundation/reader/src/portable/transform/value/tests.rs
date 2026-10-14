use super::*;
use alloc::{format, vec};
use nepl3_core::{
    budget::Limits,
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    view::{FallbackRole, PresentationClass},
};
use nepl3_wire::foundation::FoundationCodec;
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        source_bytes: 100,
        output_bytes: 10000,
        nodes: 10000,
        depth: 100,
        ..Limits::default()
    })
}
fn error(v: impl core::fmt::Debug) -> String {
    format!("{v:?}")
}
#[test]
fn presentation_facts_use_symbolic_codec_and_preserve_nested_stops() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut budget()),
        crate::schema::descriptor(&mut budget()),
    ] {
        let d = d.map_err(error)?;
        registry
            .register(d.reference(&mut budget()).map_err(error)?, d, &mut budget())
            .map_err(error)?;
    }
    registry.finalize(&mut budget()).map_err(error)?;
    let schema = registry.selected("nepl3.reader", 1).ok_or("reader")?;
    let source = SourceSnapshot::new(
        SourceId("source".into()),
        1,
        "memory:source".into(),
        b"x".to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let span = source.span(0, 1).map_err(error)?;
    let mut store = SourceStore::default();
    store
        .insert_with_budget(source, &mut budget())
        .map_err(error)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let facts = vec![ReaderFact::Presentation {
        class: PresentationClass {
            schema: SchemaRef {
                package: "unregistered.class".into(),
                revision: 9,
                digest: Digest::of(b"class"),
            },
            name: "".into(),
            fallback: FallbackRole::Annotation,
        },
        span,
    }];
    let value = facts_value(&facts, schema, &mut codec, &mut budget()).map_err(error)?;
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(error)?;
    let cold = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
    assert_eq!(
        facts_from(&cold, schema, &mut codec, &mut budget()).map_err(error)?,
        facts
    );
    // The outer ReaderFact allocation succeeds; the delegated class validation stops.
    let mut b = Budget::new(Limits {
        allocation_units: core::mem::size_of::<ReaderFact>() as u64,
        ..budget().limits()
    });
    assert!(matches!(
        facts_from(&cold, schema, &mut codec, &mut b),
        Err(PortableError::Stopped(StopReason::AllocationLimit))
    ));
    Ok(())
}
