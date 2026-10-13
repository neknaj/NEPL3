use super::*;
use alloc::vec;
use nepl3_core::budget::Limits;
use nepl3_wire::foundation::FoundationCodec;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        work: 10_000_000,
        depth: 256,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    alloc::format!("{e:?}")
}
#[test]
fn every_phase_preserves_canonical_field_order_and_nonempty_payloads() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?,
        crate::schema::descriptor(&mut budget()).map_err(err)?,
    ] {
        let reference = d.reference(&mut budget()).map_err(err)?;
        registry
            .register(reference, d, &mut budget())
            .map_err(err)?;
    }
    registry.finalize(&mut budget()).map_err(err)?;
    let ctx = Context::new::<nepl3_wire::WireError>(&registry).map_err(err)?;
    let sources = SourceStore::default();
    let mut a = nepl3_core::source::SourceAdmission::default();
    let mut c = FoundationCodec::new(&registry, &sources, &mut a).map_err(err)?;
    for (native, name, expected) in [
        (FramePhase::Enter, "Enter", vec![]),
        (
            FramePhase::Seq {
                next: 3,
                values: vec![NdfValue::U64(2)],
            },
            "Seq",
            vec![NdfValue::U64(3), NdfValue::List(vec![NdfValue::U64(2)])],
        ),
        (
            FramePhase::Choice {
                next: 5,
                furthest: 7,
                expected: vec![Expectation::Literal("expected".into())],
            },
            "Choice",
            vec![NdfValue::U64(5), NdfValue::U64(7)],
        ),
        (
            FramePhase::Repeat {
                count: 11,
                iteration_start: 13,
                values: vec![NdfValue::U64(17)],
            },
            "Repeat",
            vec![
                NdfValue::U64(11),
                NdfValue::U64(13),
                NdfValue::List(vec![NdfValue::U64(17)]),
            ],
        ),
        (FramePhase::AwaitChild, "AwaitChild", vec![]),
        (
            FramePhase::Then {
                first: NdfValue::Text("first".into()),
                end: 19,
            },
            "Then",
            vec![NdfValue::Text("first".into()), NdfValue::U64(19)],
        ),
        (
            FramePhase::Provider { call_id: 23 },
            "Provider",
            vec![NdfValue::U64(23)],
        ),
    ] {
        let value = phase(&native, &ctx, &mut c, &mut budget()).map_err(err)?;
        validate_named::<nepl3_wire::WireError>(
            &value,
            ctx.reader,
            "FramePhase",
            &registry,
            &mut budget(),
        )
        .map_err(err)?;
        let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
        assert_eq!(
            nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
            value
        );
        let NdfValue::Variant(v) = &value else {
            return Err("variant".into());
        };
        assert_eq!(v.variant, name);
        if name == "Choice" {
            assert_eq!(&v.fields[..2], expected.as_slice());
            let NdfValue::List(items) = &v.fields[2] else {
                return Err("expectations".into());
            };
            let NdfValue::Variant(literal) = &items[0] else {
                return Err("literal".into());
            };
            assert_eq!(literal.variant, "Literal");
            assert_eq!(literal.fields, vec![NdfValue::Text("expected".into())]);
        } else {
            assert_eq!(v.fields, expected);
        }
    }
    Ok(())
}

#[test]
fn checkpoint_field_oracle_retains_facts_and_trace_overflow() -> Result<(), String> {
    use alloc::boxed::Box;
    use nepl3_core::{diagnostic::TraceOverflow, value::Variant, view::ViewBundle};
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?,
        crate::schema::descriptor(&mut budget()).map_err(err)?,
    ] {
        let schema = descriptor.reference(&mut budget()).map_err(err)?;
        registry
            .register(schema, descriptor, &mut budget())
            .map_err(err)?;
    }
    registry.finalize(&mut budget()).map_err(err)?;
    let ctx = Context::new::<nepl3_wire::WireError>(&registry).map_err(err)?;
    let source = SourceSnapshot::new(
        SourceId("oracle".into()),
        0,
        "memory:oracle".into(),
        b"ab".to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let span = source.span(0, 1).map_err(err)?;
    let mut store = SourceStore::default();
    store.insert(source.clone()).map_err(err)?;
    let mut admission = nepl3_core::source::SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(err)?;
    let cp = ReaderCheckpoint {
        cursor: 1,
        state: NdfValue::Text("saved state".into()),
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![ReaderFact::Capture {
            name: "captured".into(),
            span: span.clone(),
        }],
        diagnostics: vec![],
        events: vec![],
        sources: vec![],
        source_maps: vec![],
        trace_overflow: Some(TraceOverflow { dropped: 13 }),
    };
    // Construct all nine ReaderCheckpoint fields independently from its schema.
    // Only the existing foundation span/view codecs supply their own nested types.
    let expected = NdfValue::Record(Record {
        schema: ctx.reader.clone(),
        kind: "ReaderCheckpoint".into(),
        fields: vec![
            NdfValue::U64(1),
            NdfValue::Text("saved state".into()),
            codec
                .encode_views(
                    &ViewBundle {
                        elements: vec![],
                        roots: vec![],
                    },
                    &mut budget(),
                )
                .map_err(err)?,
            NdfValue::List(vec![NdfValue::Variant(Variant {
                schema: ctx.reader.clone(),
                type_name: "ReaderFact".into(),
                variant: "Capture".into(),
                fields: vec![
                    NdfValue::Text("captured".into()),
                    codec.encode_span(&span, &mut budget()).map_err(err)?,
                ],
            })]),
            NdfValue::List(vec![]),
            NdfValue::List(vec![]),
            NdfValue::List(vec![]),
            NdfValue::List(vec![]),
            NdfValue::Some(Box::new(NdfValue::Record(Record {
                schema: codec.foundation_schema().clone(),
                kind: "TraceOverflow".into(),
                fields: vec![NdfValue::U64(13)],
            }))),
        ],
    });
    let actual = checkpoint(
        &cp,
        &[source],
        Usage {
            work: 17,
            ..Usage::default()
        },
        &ctx,
        &mut codec,
        &mut budget(),
    )
    .map_err(err)?;
    assert_eq!(actual, expected);
    validate_named::<nepl3_wire::WireError>(
        &actual,
        ctx.reader,
        "ReaderCheckpoint",
        &registry,
        &mut budget(),
    )
    .map_err(err)?;
    assert_eq!(cp.trace_overflow, Some(TraceOverflow { dropped: 13 }));
    assert_eq!(cp.facts.len(), 1);
    Ok(())
}
