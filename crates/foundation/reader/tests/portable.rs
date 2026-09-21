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
#[test]
fn transform_request_requires_mapping_for_cross_source_view_children() -> TestResult {
    use nepl3_core::{
        origin::{Mapping, MappingKind},
        value::KindRef,
        view::{ViewBundle, ViewElement, ViewField, ViewRef},
    };
    use nepl3_reader::portable::transform::request as exchange;
    macro_rules! checked {
        ($v:expr) => {
            $v.map_err(|e| format!("{e:?}"))?
        };
    }
    let (schema, registry, mut store, request) = fixture()?;
    let mut b = budget();
    let source_span = checked!(store.resolve(&request.snapshot).ok_or("source")?.span(0, 1));
    let generated = checked!(SourceSnapshot::new(
        SourceId("mapped".into()),
        0,
        "memory:mapped".into(),
        b"b".to_vec(),
        &mut b
    ));
    let target_span = checked!(generated.span(0, 1));
    checked!(store.insert(generated));
    let kind = KindRef {
        schema: schema.clone(),
        local_kind: checked!(registry.kind_id(&schema, "ReadRequest")),
    };
    let view = ViewBundle {
        elements: vec![
            ViewElement {
                kind: kind.clone(),
                span: source_span.clone(),
                fields: vec![ViewField {
                    name: "child".into(),
                    children: vec![ViewRef(1)],
                }],
                roles: vec![],
                relations: vec![],
            },
            ViewElement {
                kind,
                span: target_span.clone(),
                fields: vec![],
                roles: vec![],
                relations: vec![],
            },
        ],
        roots: vec![ViewRef(0)],
    };
    let mappings = [Mapping {
        source: source_span.clone(),
        target: target_span,
        kind: MappingKind::Transformed,
    }];
    let request = TransformRequest {
        value: NdfValue::Unit,
        span: source_span,
        view,
        context: request.context,
    };
    let mut admission = SourceAdmission::default();
    let mut codec = checked!(FoundationCodec::new(&registry, &store, &mut admission));
    let encoded = checked!(exchange::to_value(
        &request, &schema, &mut codec, &store, &mappings, &registry, &mut b
    ));
    let bytes = checked!(nepl3_wire::encode(&encoded, &mut b));
    let encoded = checked!(nepl3_wire::decode(&bytes, &mut b));
    let restored = checked!(exchange::from_value(
        &encoded, &schema, &mut codec, &store, &mappings, &registry, &mut b
    ));
    assert_eq!(restored, request);
    assert!(
        exchange::to_value(
            &request,
            &schema,
            &mut codec,
            &store,
            &[],
            &registry,
            &mut b
        )
        .is_err()
    );
    assert!(
        exchange::from_value(
            &encoded,
            &schema,
            &mut codec,
            &store,
            &[],
            &registry,
            &mut b
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn transform_request_resolves_only_dispatch_sources_and_checks_views() -> TestResult {
    use nepl3_core::view::{ViewBundle, ViewRef};
    use nepl3_reader::portable::transform::request as exchange;
    macro_rules! checked {
        ($v:expr) => {
            $v.map_err(|e| format!("{e:?}"))?
        };
    }
    let (schema, registry, global, request) = fixture()?;
    let mut declared = SourceStore::default();
    for source in &request.sources {
        checked!(declared.insert(source.clone()));
    }
    let request = TransformRequest {
        value: NdfValue::Text("変換入力".into()),
        span: checked!(
            declared
                .resolve(&request.snapshot)
                .ok_or("snapshot")?
                .span(0, 1)
        ),
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        context: request.context,
    };
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let mut codec = checked!(FoundationCodec::new(&registry, &global, &mut admission));
    let value = checked!(exchange::to_value(
        &request,
        &schema,
        &mut codec,
        &declared,
        &[],
        &registry,
        &mut b
    ));
    let bytes = checked!(nepl3_wire::encode(&value, &mut b));
    let value = checked!(nepl3_wire::decode(&bytes, &mut b));
    let restored = checked!(exchange::from_value(
        &value,
        &schema,
        &mut codec,
        &declared,
        &[],
        &registry,
        &mut b
    ));
    assert_eq!(restored, request);
    let NdfValue::Record(record) = &value else {
        return Err("record".into());
    };
    assert_eq!(record.kind, "TransformRequest");
    assert_eq!(record.fields[0], NdfValue::Text("変換入力".into()));
    // Ambient host sources cannot supply either the context or the input span.
    for absent in ["aux", "input"] {
        let mut incomplete = SourceStore::default();
        for source in declared.snapshots() {
            if source.identity().source.0 != absent {
                checked!(incomplete.insert(source.clone()));
            }
        }
        assert!(
            exchange::to_value(
                &request,
                &schema,
                &mut codec,
                &incomplete,
                &[],
                &registry,
                &mut b
            )
            .is_err()
        );
        assert!(
            exchange::from_value(
                &value,
                &schema,
                &mut codec,
                &incomplete,
                &[],
                &registry,
                &mut b
            )
            .is_err()
        );
    }
    let mut invalid = request.clone();
    invalid.view.roots.push(ViewRef(0));
    assert!(
        exchange::to_value(
            &invalid,
            &schema,
            &mut codec,
            &declared,
            &[],
            &registry,
            &mut b
        )
        .is_err()
    );
    let mut invalid = request.clone();
    invalid.context.environment.digest.0[0] ^= 1;
    assert!(
        exchange::to_value(
            &invalid,
            &schema,
            &mut codec,
            &declared,
            &[],
            &registry,
            &mut b
        )
        .is_err()
    );
    for allocation in [false, true] {
        let mut limits = budget().limits();
        if allocation {
            limits.allocation_units = 0;
        } else {
            limits.work = 0;
        }
        let mut stopped = Budget::new(limits);
        assert!(
            exchange::from_value(
                &value,
                &schema,
                &mut codec,
                &declared,
                &[],
                &registry,
                &mut stopped
            )
            .is_err()
        );
        assert!(stopped.poll().is_err());
    }
    Ok(())
}

#[test]
fn dependent_request_preserves_first_and_admits_only_declared_sources() -> TestResult {
    macro_rules! checked {
        ($v:expr) => {
            $v.map_err(|e| format!("{e:?}"))?
        };
    }
    let (schema, registry, global, mut request) = fixture()?;
    request.start = 1;
    let request = DependentRequest {
        first: NdfValue::Text("先行結果".into()),
        end: 1,
        request,
    };
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let mut codec = checked!(FoundationCodec::new(&registry, &global, &mut admission));
    let value = checked!(dependent::to_value(
        &request, &schema, &mut codec, &global, &registry, &mut b
    ));
    let bytes = checked!(nepl3_wire::encode(&value, &mut b));
    let value = checked!(nepl3_wire::decode(&bytes, &mut b));
    let empty = SourceStore::default();
    let mut receiving_admission = SourceAdmission::default();
    let mut codec = checked!(FoundationCodec::new(
        &registry,
        &empty,
        &mut receiving_admission
    ));
    let snapshots = checked!(dependent::sources(
        &value, &schema, &mut codec, &registry, &mut b
    ));
    let mut declared = SourceStore::default();
    for source in snapshots {
        checked!(declared.insert(source));
    }
    assert_eq!(declared.snapshots().len(), 2);
    let mut codec = checked!(FoundationCodec::new(
        &registry,
        &declared,
        &mut receiving_admission
    ));
    let restored = checked!(dependent::from_value(
        &value, &schema, &mut codec, &declared, &registry, &mut b
    ));
    assert_eq!(restored, request);
    // Independently inspect schema field order and the cursor at EOF.
    let NdfValue::Record(record) = &value else {
        return Err("record".into());
    };
    assert_eq!(record.kind, "DependentRequest");
    assert_eq!(record.fields[0], NdfValue::Text("先行結果".into()));
    assert_eq!(record.fields[1], NdfValue::U64(1));
    let mut mismatch = value.clone();
    if let NdfValue::Record(record) = &mut mismatch {
        record.fields[1] = NdfValue::U64(0);
    }
    assert!(
        dependent::from_value(&mismatch, &schema, &mut codec, &declared, &registry, &mut b)
            .is_err()
    );
    let mut native_mismatch = request.clone();
    native_mismatch.end = 0;
    assert!(
        dependent::to_value(
            &native_mismatch,
            &schema,
            &mut codec,
            &declared,
            &registry,
            &mut b
        )
        .is_err()
    );
    let mut missing = value.clone();
    if let NdfValue::Record(record) = &mut missing
        && let NdfValue::Record(nested) = &mut record.fields[2]
    {
        nested.fields[1] = NdfValue::List(vec![]);
    }
    assert!(
        dependent::from_value(&missing, &schema, &mut codec, &declared, &registry, &mut b).is_err()
    );
    for allocation in [false, true] {
        let mut limits = budget().limits();
        if allocation {
            limits.allocation_units = 0;
        } else {
            limits.work = 0;
        }
        let mut stopped = Budget::new(limits);
        assert!(
            dependent::from_value(
                &value,
                &schema,
                &mut codec,
                &declared,
                &registry,
                &mut stopped
            )
            .is_err()
        );
        assert!(stopped.poll().is_err());
    }
    Ok(())
}
#[test]
fn dependent_cursor_must_be_a_source_boundary() -> TestResult {
    macro_rules! checked {
        ($v:expr) => {
            $v.map_err(|e| format!("{e:?}"))?
        };
    }
    let (schema, registry, mut store, mut request) = fixture()?;
    let mut b = budget();
    let input = checked!(SourceSnapshot::new(
        SourceId("unicode".into()),
        1,
        "memory:unicode".into(),
        "あb".as_bytes().to_vec(),
        &mut b,
    ));
    request.snapshot = input.reference();
    request.sources.push(input.clone());
    request.start = 3;
    request.limit = 4;
    checked!(store.insert(input));
    let request = DependentRequest {
        first: NdfValue::Unit,
        end: 3,
        request,
    };
    let mut admission = SourceAdmission::default();
    let mut codec = checked!(FoundationCodec::new(&registry, &store, &mut admission));
    let value = checked!(dependent::to_value(
        &request, &schema, &mut codec, &store, &registry, &mut b
    ));
    for cursor in [1, 2, 5] {
        let mut invalid = request.clone();
        invalid.end = cursor;
        invalid.request.start = cursor;
        assert!(
            dependent::to_value(&invalid, &schema, &mut codec, &store, &registry, &mut b).is_err()
        );
        let mut invalid = value.clone();
        let NdfValue::Record(record) = &mut invalid else {
            return Err("record".into());
        };
        record.fields[1] = NdfValue::U64(cursor);
        let NdfValue::Record(nested) = &mut record.fields[2] else {
            return Err("request".into());
        };
        nested.fields[2] = NdfValue::U64(cursor);
        assert!(
            dependent::from_value(&invalid, &schema, &mut codec, &store, &registry, &mut b)
                .is_err()
        );
    }
    let restored = checked!(dependent::from_value(
        &value, &schema, &mut codec, &store, &registry, &mut b
    ));
    assert_eq!(restored.end, 3);
    assert_eq!(restored.request.limit, 4);
    Ok(())
}

#[test]
fn portable_plan_preserves_arena_and_rejects_invalid_references() -> TestResult {
    use nepl3_reader::portable::plan as exchange;
    let (schema, registry, sources, _) = fixture()?;
    let plan = ReaderPlan {
        schema,
        state_type: TypeDescriptor::Unit,
        expressions: vec![
            ReaderExpr::Literal("a".into()),
            ReaderExpr::Scalar(CharClass::Range {
                lo: 'あ', hi: 'ん'
            }),
            ReaderExpr::Seq(vec![ReaderId(0), ReaderId(1)]),
        ],
        rules: vec![ReaderRule {
            name: "entry".into(),
            root: ReaderId(2),
            output: TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
        }],
        providers: vec![],
    };
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&registry, &sources, &mut admission).map_err(|e| format!("{e:?}"))?;
    let checked = plan
        .check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    use nepl3_reader::{
        builtin::BuiltinReader,
        tokenizer::{ReaderMode, SkipRule, TokenReader},
    };
    let mode = ReaderMode {
        name: "Code".into(),
        skip: vec![
            SkipRule {
                reader: TokenReader::Builtin(BuiltinReader::Trivia),
            },
            SkipRule {
                reader: TokenReader::Rule("entry".into()),
            },
        ],
        take: vec![],
    };
    let mode_value = exchange::mode_to_value(&mode, &checked, &mut codec, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        exchange::mode_from_value(&mode_value, &checked, &mut codec, &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        mode
    );
    let mut invalid_mode = mode_value.clone();
    let NdfValue::Record(record) = &mut invalid_mode else {
        return Err("mode".into());
    };
    let NdfValue::List(skip) = &mut record.fields[1] else {
        return Err("skip".into());
    };
    let NdfValue::Record(rule) = &mut skip[1] else {
        return Err("skip rule".into());
    };
    let NdfValue::Variant(reader) = &mut rule.fields[0] else {
        return Err("reader".into());
    };
    reader.fields[0] = NdfValue::Text("missing".into());
    assert!(exchange::mode_from_value(&invalid_mode, &checked, &mut codec, &mut budget()).is_err());
    let value =
        exchange::to_value(&checked, &mut codec, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        exchange::from_value(&received, &registry, &mut codec, &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        plan
    );
    let NdfValue::Record(record) = &value else {
        return Err("record".into());
    };
    assert_eq!(record.kind, "ReaderPlan");
    assert_eq!(record.fields.len(), 5);
    let NdfValue::List(expressions) = &record.fields[2] else {
        return Err("expressions".into());
    };
    let NdfValue::Variant(seq) = &expressions[2] else {
        return Err("sequence".into());
    };
    assert_eq!(seq.variant, "Seq");
    let NdfValue::List(ids) = &seq.fields[0] else {
        return Err("indices".into());
    };
    for (index, id) in ids.iter().enumerate() {
        let NdfValue::Record(id) = id else {
            return Err("ReaderId".into());
        };
        assert_eq!(id.fields, vec![NdfValue::U64(index as u64)]);
    }
    for invalid in [2, 99] {
        let mut value = value.clone();
        let NdfValue::Record(record) = &mut value else {
            return Err("record".into());
        };
        let NdfValue::List(expressions) = &mut record.fields[2] else {
            return Err("expressions".into());
        };
        let NdfValue::Variant(seq) = &mut expressions[2] else {
            return Err("sequence".into());
        };
        let NdfValue::List(ids) = &mut seq.fields[0] else {
            return Err("indices".into());
        };
        let NdfValue::Record(id) = &mut ids[0] else {
            return Err("ReaderId".into());
        };
        id.fields[0] = NdfValue::U64(invalid);
        assert!(exchange::from_value(&value, &registry, &mut codec, &mut budget()).is_err());
    }
    let mut stopped = budget();
    stopped.cancel();
    assert!(matches!(
        exchange::from_value(&value, &registry, &mut codec, &mut stopped),
        Err(PortableError::Stopped(_))
    ));
    Ok(())
}
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
    exchange_plan: bool,
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
    let plan = if exchange_plan {
        let proof = plan.check(registry, &mut b).map_err(|e| format!("{e:?}"))?;
        let encoded = nepl3_reader::portable::plan::to_value(&proof, &mut codec, &mut b)
            .map_err(|e| format!("{e:?}"))?;
        let bytes = nepl3_wire::encode(&encoded, &mut b).map_err(|e| format!("{e:?}"))?;
        let value = nepl3_wire::decode(&bytes, &mut b).map_err(|e| format!("{e:?}"))?;
        nepl3_reader::portable::plan::from_value(&value, registry, &mut codec, &mut b)
            .map_err(|e| format!("{e:?}"))?
    } else {
        plan
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
        run(&request, &global, &registry, false)?,
        run(&restored, &declared, &registry, false)?
    );
    let received_plan = run(&restored, &declared, &registry, true)?;
    assert_eq!(received_plan, (NdfValue::Unit, 1, NdfValue::Unit));
    assert_eq!(received_plan, run(&request, &global, &registry, false)?);
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
