use super::*;
use crate::model::{ReaderCheckpoint, ReaderFact};
use alloc::vec;
use nepl3_core::{
    budget::{Limits, Usage},
    diagnostic::Report,
    source::SourceAdmission,
    syntax::{Environment, EnvironmentEntry},
    value::Variant,
    view::ViewBundle,
};
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
fn rec(schema: &SchemaRef, name: &str, fields: Vec<NdfValue>) -> NdfValue {
    NdfValue::Record(Record {
        schema: schema.clone(),
        kind: name.into(),
        fields,
    })
}
fn var(schema: &SchemaRef, name: &str, tag: &str, fields: Vec<NdfValue>) -> NdfValue {
    NdfValue::Variant(Variant {
        schema: schema.clone(),
        type_name: name.into(),
        variant: tag.into(),
        fields,
    })
}
fn reference(schema: &SchemaRef, source: &SourceSnapshot) -> NdfValue {
    rec(
        schema,
        "SourceRef",
        vec![
            NdfValue::Text(source.identity().source.0.clone()),
            NdfValue::U64(source.identity().revision),
            NdfValue::Bytes(source.identity().digest.0.to_vec()),
        ],
    )
}
#[test]
fn tokenizer_oracle_covers_all_fields_and_canonical_source_scope() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?,
        crate::schema::descriptor(&mut budget()).map_err(err)?,
    ] {
        registry
            .register(
                descriptor.reference(&mut budget()).map_err(err)?,
                descriptor,
                &mut budget(),
            )
            .map_err(err)?;
    }
    registry.finalize(&mut budget()).map_err(err)?;
    let schema = registry
        .selected("nepl3.reader", 1)
        .ok_or("reader schema")?
        .clone();
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation schema")?
        .clone();
    let make = |name: &str| {
        SourceSnapshot::new(
            SourceId(name.into()),
            0,
            alloc::format!("memory:{name}"),
            b"ab".to_vec(),
            &mut budget(),
        )
        .map_err(err)
    };
    let input = make("input")?;
    let a = make("a-generated")?;
    let z = make("z-generated")?;
    let ambient = make("ambient-only")?;
    let mut store = SourceStore::default();
    for s in [&input, &a, &z, &ambient] {
        store.insert(s.clone()).map_err(err)?;
    }
    let mut env = EnvironmentEntry {
        id: 17,
        digest: Digest([0; 32]),
        value: Environment {
            bindings: vec![],
            resources: vec![],
        },
    };
    env.digest = nepl3_wire::environment::environment_digest(
        &env.value,
        &foundation,
        &registry,
        &mut budget(),
    )
    .map_err(err)?;
    let request = OwnedReadRequest {
        snapshot: input.reference(),
        sources: vec![input.clone()],
        start: 0,
        limit: 2,
        final_input: true,
        context: ReaderContext {
            schema: schema.clone(),
            category: "Token".into(),
            mode: "oracle".into(),
            environment: env,
            origins: vec![],
        },
        state: NdfValue::Unit,
    };
    let cp = ReaderCheckpoint {
        cursor: 1,
        state: NdfValue::Text("checkpoint".into()),
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![ReaderFact::Capture {
            name: "captured".into(),
            span: z.span(0, 1).map_err(err)?,
        }],
        diagnostics: vec![],
        events: vec![],
        sources: vec![z.clone(), a.clone(), z.clone()],
        source_maps: vec![],
        trace_overflow: None,
    };
    let usage = Usage {
        source_bytes: 6,
        work: 31,
        depth: 7,
        nodes: 11,
        allocation_units: 13,
        output_bytes: 17,
        diagnostics: 0,
        events: 0,
    };
    let report = Report {
        usage,
        ..Report::default()
    };
    let native = TokenizationContinuation {
        scope: TokenizationScope {
            operation_id: "scope-operation".into(),
            profile_digest: Digest([3; 32]),
            snapshot: input.reference(),
        },
        session_id: "oracle-session".into(),
        reader_schema: schema.clone(),
        reader_plan_digest: Digest([5; 32]),
        configuration_digest: Digest([7; 32]),
        request: request.clone(),
        mode: "oracle".into(),
        target: TokenTarget::Mode,
        phase: TokenizationPhase::Take { next: 2 },
        current: cp,
        trivia: vec![Trivia {
            span: a.span(0, 1).map_err(err)?,
            kind: TriviaKind::Whitespace,
        }],
        expected: vec![crate::model::Expectation::Literal("expected".into())],
        furthest: 2,
        pending: TokenizationWait::Reservation {
            request: ReservationRequest {
                session_id: "oracle-session".into(),
                request_id: 19,
                snapshot: input.reference(),
                start: 1,
                limit: 2,
            },
        },
        depth_base: 7,
        usage,
        report: report.clone(),
    };
    let before = native.clone();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(err)?;
    let actual = value(&native, &registry, &mut codec, &mut budget()).map_err(err)?;
    // Independent outer field order and native-derived values. Reuse only the
    // established Reader request/foundation codecs for their nested contracts.
    let encoded_report = codec.encode_report(&report, &mut budget()).map_err(err)?;
    let NdfValue::Record(ref report_record) = encoded_report else {
        return Err("report".into());
    };
    let usage_value = report_record.fields[3].clone();
    let checkpoint = rec(
        &schema,
        "ReaderCheckpoint",
        vec![
            NdfValue::U64(1),
            NdfValue::Text("checkpoint".into()),
            codec
                .encode_views(&native.current.view, &mut budget())
                .map_err(err)?,
            NdfValue::List(vec![var(
                &schema,
                "ReaderFact",
                "Capture",
                vec![
                    NdfValue::Text("captured".into()),
                    codec
                        .encode_span(&z.span(0, 1).map_err(err)?, &mut budget())
                        .map_err(err)?,
                ],
            )]),
            NdfValue::List(vec![]),
            codec
                .encode_sources(&[a.clone(), z.clone()], &mut budget())
                .map_err(err)?,
            NdfValue::List(vec![]),
            NdfValue::List(vec![]),
            NdfValue::None,
        ],
    );
    let expected = rec(
        &schema,
        "TokenizationContinuation",
        vec![
            rec(
                &schema,
                "TokenizationScope",
                vec![
                    NdfValue::Text("scope-operation".into()),
                    NdfValue::Bytes(vec![3; 32]),
                    reference(&foundation, &input),
                ],
            ),
            NdfValue::Text("oracle-session".into()),
            rec(
                &foundation,
                "SchemaRef",
                vec![
                    NdfValue::Text(schema.package.clone()),
                    NdfValue::U64(schema.revision),
                    NdfValue::Bytes(schema.digest.0.to_vec()),
                ],
            ),
            NdfValue::Bytes(vec![5; 32]),
            NdfValue::Bytes(vec![7; 32]),
            request_to_value(
                &request,
                &schema,
                &mut codec,
                &store,
                &registry,
                &mut budget(),
            )
            .map_err(err)?,
            NdfValue::Text("oracle".into()),
            var(&schema, "TokenTarget", "Mode", vec![]),
            var(&schema, "TokenizationPhase", "Take", vec![NdfValue::U64(2)]),
            checkpoint,
            NdfValue::List(vec![rec(
                &foundation,
                "Trivia",
                vec![
                    codec
                        .encode_span(&a.span(0, 1).map_err(err)?, &mut budget())
                        .map_err(err)?,
                    var(&foundation, "TriviaKind", "Whitespace", vec![]),
                ],
            )]),
            NdfValue::List(vec![var(
                &schema,
                "Expectation",
                "Literal",
                vec![NdfValue::Text("expected".into())],
            )]),
            NdfValue::U64(2),
            var(
                &schema,
                "TokenizationWait",
                "Reservation",
                vec![rec(
                    &schema,
                    "ReservationRequest",
                    vec![
                        NdfValue::Text("oracle-session".into()),
                        NdfValue::U64(19),
                        reference(&foundation, &input),
                        NdfValue::U64(1),
                        NdfValue::U64(2),
                    ],
                )],
            ),
            NdfValue::U64(7),
            usage_value,
            encoded_report,
        ],
    );
    assert_eq!(actual, expected);
    assert_eq!(native, before);
    let NdfValue::Record(ref root) = actual else {
        return Err("root".into());
    };
    let NdfValue::Record(ref checkpoint) = root.fields[9] else {
        return Err("checkpoint".into());
    };
    let declarations = codec
        .decode_sources(&checkpoint.fields[5], &mut budget())
        .map_err(err)?;
    assert_eq!(declarations, vec![a, z]);
    assert!(
        !declarations
            .iter()
            .any(|s| s.identity() == ambient.identity())
    );
    assert_eq!(native.current.sources.len(), 3);
    Ok(())
}
