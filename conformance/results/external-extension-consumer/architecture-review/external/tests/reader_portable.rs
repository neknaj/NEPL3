use nepl3_core::{
    budget::{Budget, Limits},
    origin::Origin,
    schema::{SchemaRegistry, TypeDescriptor, TypeRef},
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{Environment, EnvironmentEntry},
    value::{NdfValue, SchemaRef},
};
use nepl3_reader::{model::*, plan::*, portable::*, runtime::ReaderSession};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
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
fn fixture() -> Result<(SchemaRef, SchemaRegistry, SourceStore, OwnedReadRequest), String> {
    let mut b = budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b).map_err(|e| format!("{e:?}"))?,
        nepl3_reader::schema::descriptor(&mut b).map_err(|e| format!("{e:?}"))?,
    ] {
        let schema = descriptor.reference(&mut b).map_err(|e| format!("{e:?}"))?;
        registry
            .register(schema, descriptor, &mut b)
            .map_err(|e| format!("{e:?}"))?;
    }
    registry.finalize(&mut b).map_err(|e| format!("{e:?}"))?;
    let schema = registry
        .selected("nepl3.reader", 1)
        .ok_or("reader schema")?
        .clone();
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation schema")?;
    let source = |id: &str, text: &str| {
        SourceSnapshot::new(
            SourceId(id.into()),
            1,
            format!("memory:{id}"),
            text.as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))
    };
    let input = source("input", "a")?;
    let auxiliary = source("aux", "context")?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&environment, foundation, &registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let context = ReaderContext {
        schema: schema.clone(),
        category: "fixture".into(),
        mode: "default".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
        origins: vec![Origin::Direct(
            auxiliary.span(0, 7).map_err(|e| format!("{e:?}"))?,
        )],
    };
    let request = OwnedReadRequest {
        snapshot: input.reference(),
        sources: vec![auxiliary.clone(), input.clone()],
        start: 0,
        limit: 1,
        final_input: true,
        context,
        state: NdfValue::Unit,
    };
    let mut sources = SourceStore::default();
    for source in [input, auxiliary, source("unrelated", "not in request")?] {
        sources.insert(source).map_err(|e| format!("{e:?}"))?;
    }
    Ok((schema, registry, sources, request))
}
fn expected() -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.reader".into(),
        revision: 1,
        name: "ReadRequest".into(),
    })
}
fn run(
    request: &OwnedReadRequest,
    sources: &SourceStore,
    registry: &SchemaRegistry,
) -> Result<(NdfValue, u64, NdfValue), String> {
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(registry, sources, &mut admission).map_err(|e| format!("{e:?}"))?;
    let context = request
        .context
        .check(&mut codec, sources, registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let plan = ReaderPlan {
        schema: request.context.schema.clone(),
        state_type: TypeDescriptor::Unit,
        expressions: vec![ReaderExpr::Literal("a".into())],
        rules: vec![ReaderRule {
            name: "entry".into(),
            root: ReaderId(0),
            output: TypeDescriptor::Unit,
        }],
        providers: vec![],
    };
    let checked = plan.check(registry, &mut b).map_err(|e| format!("{e:?}"))?;
    let mut session = ReaderSession::new("portable-fixture".into(), &checked, registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let result = session
        .read(
            "entry",
            ReadRequest {
                snapshot: sources.resolve(&request.snapshot).ok_or("snapshot")?,
                start: request.start,
                limit: request.limit,
                final_input: request.final_input,
                context: &context,
                state: &request.state,
            },
            sources,
            &mut b,
            &mut admission,
        )
        .map_err(|e| format!("{e:?}"))?;
    match result {
        ReadReply::Matched {
            value,
            end,
            new_state,
            ..
        } => Ok((value, end, new_state)),
        other => Err(format!("unexpected {other:?}")),
    }
}

#[test]
fn native_request_and_checked_ndf_loopback_execute_with_identical_meaning() -> TestResult {
    let (schema, registry, global, request) = fixture()?;
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let value = request_to_value(
        &request,
        &schema,
        &mut FoundationCodec::new(&registry, &global, &mut admission)
            .map_err(|e| format!("{e:?}"))?,
        &global,
        &registry,
        &mut b,
    )
    .map_err(|e| format!("{e:?}"))?;
    let encoded = nepl3_wire::encode_checked(&value, &expected(), &registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let value = nepl3_wire::decode_checked(&encoded, &expected(), &registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let empty = SourceStore::default();
    let snapshots = request_sources(
        value.value(),
        &schema,
        &mut FoundationCodec::new(&registry, &empty, &mut admission)
            .map_err(|e| format!("{e:?}"))?,
        &registry,
        &mut b,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut declared = SourceStore::default();
    for source in snapshots {
        declared.insert(source).map_err(|e| format!("{e:?}"))?;
    }
    let restored = request_from_value(
        value.value(),
        &schema,
        &mut FoundationCodec::new(&registry, &declared, &mut admission)
            .map_err(|e| format!("{e:?}"))?,
        &declared,
        &registry,
        &mut b,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored, request);
    assert_eq!(declared.snapshots().len(), 2);
    assert_eq!(
        run(&request, &global, &registry)?,
        run(&restored, &declared, &registry)?
    );
    Ok(())
}

#[test]
fn host_global_source_cannot_fill_a_missing_request_declaration() -> TestResult {
    let (schema, registry, global, request) = fixture()?;
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let mut value = request_to_value(
        &request,
        &schema,
        &mut FoundationCodec::new(&registry, &global, &mut admission)
            .map_err(|e| format!("{e:?}"))?,
        &global,
        &registry,
        &mut b,
    )
    .map_err(|e| format!("{e:?}"))?;
    if let NdfValue::Record(request) = &mut value
        && let NdfValue::List(sources) = &mut request.fields[1]
    {
        sources.remove(0);
    }
    // Even a mistakenly global codec/store cannot provide the undeclared auxiliary snapshot.
    assert!(matches!(
        request_from_value(
            &value,
            &schema,
            &mut FoundationCodec::new(&registry, &global, &mut admission)
                .map_err(|e| format!("{e:?}"))?,
            &global,
            &registry,
            &mut b
        ),
        Err(PortableError::UndeclaredSource)
    ));
    let mut value = request_to_value(
        &request,
        &schema,
        &mut FoundationCodec::new(&registry, &global, &mut admission)
            .map_err(|e| format!("{e:?}"))?,
        &global,
        &registry,
        &mut b,
    )
    .map_err(|e| format!("{e:?}"))?;
    if let NdfValue::Record(request) = &mut value
        && let NdfValue::Record(context) = &mut request.fields[5]
        && let NdfValue::Record(environment) = &mut context.fields[3]
    {
        environment.fields[1] = NdfValue::Bytes(vec![0; 32]);
    }
    assert!(
        request_from_value(
            &value,
            &schema,
            &mut FoundationCodec::new(&registry, &global, &mut admission)
                .map_err(|e| format!("{e:?}"))?,
            &global,
            &registry,
            &mut b
        )
        .is_err()
    );
    Ok(())
}
