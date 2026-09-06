use nepl3_core::{budget::*, diagnostic::*, schema::*, source::*, syntax::*, value::*, view::*};
use nepl3_reader::{model::*, plan::*, runtime::*};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 1_000_000_000,
        depth: 512,
        nodes: 1_000_000,
        allocation_units: 1_000_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
fn reader_type(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.reader".into(),
        revision: 1,
        name: name.into(),
    })
}
fn registry() -> Result<(SchemaRegistry, SchemaRef), SchemaError> {
    let mut registry = SchemaRegistry::default();
    let mut b = budget();
    let foundation = nepl3_core::schema::foundation::descriptor(&mut b)?;
    let reference = foundation.reference(&mut b)?;
    registry.register(reference, foundation, &mut b)?;
    let reader = nepl3_reader::schema::descriptor(&mut b)?;
    let reference = reader.reference(&mut b)?;
    registry.register(reference, reader, &mut b)?;
    let descriptor = SchemaDescriptor {
        package: "test".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Node".into(),
            constraints: vec![],
            shape: TypeShape::Record { fields: vec![] },
        }],
        operations: vec![
            OperationDescriptor {
                name: "read".into(),
                input: reader_type("ReadRequest"),
                output: reader_type("ReadReply"),
                pure: true,
            },
            OperationDescriptor {
                name: "transform".into(),
                input: reader_type("TransformRequest"),
                output: reader_type("TransformReply"),
                pure: true,
            },
            OperationDescriptor {
                name: "dependent".into(),
                input: reader_type("DependentRequest"),
                output: reader_type("ReadReply"),
                pure: true,
            },
        ],
    };
    let schema = descriptor.reference(&mut b)?;
    registry.register(schema.clone(), descriptor, &mut b)?;
    registry.finalize(&mut b)?;
    Ok((registry, schema))
}
fn plan(
    schema: &SchemaRef,
    expressions: Vec<ReaderExpr>,
    root: u64,
    output: TypeDescriptor,
) -> ReaderPlan {
    ReaderPlan {
        schema: schema.clone(),
        state_type: TypeDescriptor::Unit,
        expressions,
        rules: vec![ReaderRule {
            name: "entry".into(),
            root: ReaderId(root),
            output,
        }],
        providers: vec![],
    }
}
fn context(schema: &SchemaRef, registry: &SchemaRegistry) -> Result<ReaderContext, ReaderError> {
    let mut context = ReaderContext {
        schema: schema.clone(),
        category: "Token".into(),
        mode: "test".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest: Digest([0; 32]),
            value: Environment {
                bindings: vec![],
                resources: vec![],
            },
        },
        origins: vec![],
    };
    context.environment.digest = nepl3_wire::environment::environment_digest(
        &context.environment.value,
        registry
            .selected("nepl3.foundation", 1)
            .ok_or(SchemaError::UnknownSchema)?,
        registry,
        &mut budget(),
    )
    .map_err(|_| ReaderError::Context)?;
    Ok(context)
}
fn check_context<'a>(
    raw: &'a ReaderContext,
    store: &'a SourceStore,
    registry: &SchemaRegistry,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<nepl3_reader::context::CheckedReaderContext<'a>, ReaderError> {
    let mut codec = nepl3_wire::foundation::FoundationCodec::new(registry, store, admission)
        .map_err(|_| ReaderError::Context)?;
    raw.check(&mut codec, store, registry, budget)
        .map_err(|_| ReaderError::Context)
}

fn source(text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId("test".into()),
        0,
        "memory:test".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}
fn run(
    plan: &ReaderPlan,
    registry: &SchemaRegistry,
    text: &str,
    final_input: bool,
) -> Result<(ReadReply, Usage), ReaderError> {
    let checked = plan.check(registry, &mut budget())?;
    let mut b = budget();
    let mut reader = ReaderSession::new("single".into(), &checked, registry, &mut b)?;
    let source = source(text)?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut admission = SourceAdmission::default();
    let raw_context = context(&plan.schema, registry)?;
    let context = check_context(&raw_context, &store, registry, &mut b, &mut admission)?;
    let reply = reader.read(
        "entry",
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: text.len() as u64,
            final_input,
            context: &context,
            state: &NdfValue::Unit,
        },
        &store,
        &mut b,
        &mut admission,
    )?;
    Ok((reply, b.usage()))
}
#[test]
fn every_utf8_prefix_split_is_need_more_and_final_truncation_is_no_match() -> Result<(), ReaderError>
{
    let (registry, schema) = registry()?;
    let text = "日本😀\r\n";
    let p = plan(
        &schema,
        vec![ReaderExpr::Literal(text.into())],
        0,
        TypeDescriptor::Unit,
    );
    for end in (0..text.len()).filter(|i| text.is_char_boundary(*i)) {
        assert!(matches!(
            run(&p, &registry, &text[..end], false)?.0,
            ReadReply::NeedMore { .. }
        ));
        assert!(matches!(
            run(&p, &registry, &text[..end], true)?.0,
            ReadReply::NoMatch { .. }
        ));
    }
    assert!(matches!(
        run(&p, &registry, text, true)?.0,
        ReadReply::Matched { end: 12, .. }
    ));
    Ok(())
}
#[test]
fn choice_rewinds_captures_views_and_retains_work() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = plan(
        &schema,
        vec![
            ReaderExpr::Literal("a".into()),
            ReaderExpr::Capture {
                name: "discarded".into(),
                body: ReaderId(0),
            },
            ReaderExpr::Literal("b".into()),
            ReaderExpr::Seq(vec![ReaderId(1), ReaderId(2)]),
            ReaderExpr::Literal("ac".into()),
            ReaderExpr::Seq(vec![ReaderId(4)]),
            ReaderExpr::Choice(vec![ReaderId(3), ReaderId(5)]),
        ],
        6,
        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    );
    let (reply, usage) = run(&p, &registry, "ac", true)?;
    assert!(
        matches!(reply,ReadReply::Matched{end:2,facts,report,..} if facts.is_empty()&&report.diagnostics.is_empty())
    );
    assert!(usage.work > 4);
    Ok(())
}
#[test]
fn commit_does_not_hide_failure_or_need_more_in_choice() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = plan(
        &schema,
        vec![
            ReaderExpr::Literal("ab".into()),
            ReaderExpr::Commit(ReaderId(0)),
            ReaderExpr::Literal("ac".into()),
            ReaderExpr::Choice(vec![ReaderId(1), ReaderId(2)]),
        ],
        3,
        TypeDescriptor::Unit,
    );
    assert!(
        matches!(run(&p,&registry,"ac",true)?.0,ReadReply::Failed{diagnostic,report,..} if diagnostic.code=="ExpectedInput"&&report.diagnostics.as_slice()==core::slice::from_ref(&diagnostic))
    );
    assert!(matches!(
        run(&p, &registry, "a", false)?.0,
        ReadReply::NeedMore { .. }
    ));
    Ok(())
}
#[test]
fn bounded_repeat_zero_never_executes_nullable_body() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = plan(
        &schema,
        vec![
            ReaderExpr::Literal("".into()),
            ReaderExpr::Repeat {
                min: 0,
                max: 0,
                body: ReaderId(0),
            },
        ],
        1,
        TypeDescriptor::List(Box::new(TypeDescriptor::Unit)),
    );
    assert!(
        matches!(run(&p,&registry,"",true)?.0,ReadReply::Matched{value:NdfValue::List(ref v),end:0,..} if v.is_empty())
    );
    Ok(())
}
#[test]
fn node_sidecar_and_look_not_optional_discard_have_independent_values() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = plan(
        &schema,
        vec![
            ReaderExpr::Scalar(CharClass::Any),
            ReaderExpr::Look(ReaderId(0)),
            ReaderExpr::Node {
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
                body: ReaderId(0),
            },
            ReaderExpr::Literal("z".into()),
            ReaderExpr::Optional(ReaderId(3)),
            ReaderExpr::Not(ReaderId(3)),
            ReaderExpr::Discard(ReaderId(2)),
            ReaderExpr::Eof,
            ReaderExpr::Seq(vec![
                ReaderId(1),
                ReaderId(6),
                ReaderId(4),
                ReaderId(5),
                ReaderId(7),
            ]),
        ],
        8,
        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    );
    match run(&p, &registry, "あ", true)?.0 {
        ReadReply::Matched {
            value, end, view, ..
        } => {
            assert_eq!(end, 3);
            assert_eq!(
                value,
                NdfValue::List(vec![
                    NdfValue::Unit,
                    NdfValue::Unit,
                    NdfValue::None,
                    NdfValue::Unit,
                    NdfValue::Unit
                ])
            );
            assert_eq!(view.elements.len(), 1);
            assert_eq!(view.elements[0].span.end(), 3);
        }
        reply => {
            return Err(match reply {
                ReadReply::Stopped { reason, .. } => reason.into(),
                _ => ReaderError::Context,
            });
        }
    }
    Ok(())
}
#[test]
fn unicode_classes_use_pinned_unicode16_and_ascii_numeric_rules() {
    assert!(CharClass::IdentifierStart.contains('\u{105c0}')); // Todhri, Unicode 16.0.
    assert!(!CharClass::IdentifierStart.contains('\u{1e6c0}')); // Tai Yo, Unicode 17.0.
    assert!(CharClass::IdentifierStart.contains('_'));
    assert!(!CharClass::Digit.contains('９'));
    assert!(!CharClass::Whitespace.contains('\u{a0}'));
}
#[test]
fn until_leaves_delimiter_and_takecount_counts_scalars() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = plan(
        &schema,
        vec![
            ReaderExpr::Until("!".into()),
            ReaderExpr::TakeCount(1),
            ReaderExpr::Seq(vec![ReaderId(0), ReaderId(1)]),
        ],
        2,
        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    );
    assert!(
        matches!(run(&p,&registry,"あ😀!",true)?.0,ReadReply::Matched{value,end:8,..} if value==NdfValue::List(vec![NdfValue::Text("あ😀".into()),NdfValue::Text("!".into())]))
    );
    Ok(())
}
fn signature(schema: &SchemaRef, kind: ProviderKind) -> ProviderSignature {
    ProviderSignature {
        operation: OperationRef {
            schema: schema.clone(),
            name: match kind {
                ProviderKind::Read => "read",
                ProviderKind::Transform => "transform",
                ProviderKind::Dependent => "dependent",
            }
            .into(),
        },
        kind,
        value_input: if kind == ProviderKind::Read {
            TypeDescriptor::Unit
        } else {
            TypeDescriptor::Text
        },
        value_output: TypeDescriptor::Text,
        pure: true,
        state_type: TypeDescriptor::Unit,
        continuation_type: reader_type("ReaderContinuation"),
    }
}
fn provider_plan(schema: &SchemaRef) -> ReaderPlan {
    let signature = signature(schema, ProviderKind::Read);
    let mut p = plan(
        schema,
        vec![ReaderExpr::Call(signature.operation.clone())],
        0,
        TypeDescriptor::Text,
    );
    p.providers.push(signature);
    p
}
fn terminal(value: &str, end: u64, budget: &mut Budget) -> Result<ProviderReply, StopReason> {
    budget.charge(Resource::Work, 1)?;
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(ProviderReply::Read(Box::new(ReadReply::Matched {
        value: NdfValue::Text(value.into()),
        end,
        new_state: NdfValue::Unit,
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![],
        sources: vec![],
        source_maps: vec![],
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
    })))
}
#[test]
fn session_binds_call_source_plan_usage_and_closes_after_consumed_resume() -> Result<(), ReaderError>
{
    let (registry, schema) = registry()?;
    let p = provider_plan(&schema);
    let checked = p.check(&registry, &mut budget())?;
    let mut b = budget();
    let mut session = ReaderSession::new("one".into(), &checked, &registry, &mut b)?;
    let source = source("a")?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut admission = SourceAdmission::default();
    let raw_context = context(&schema, &registry)?;
    let context = check_context(&raw_context, &store, &registry, &mut b, &mut admission)?;
    let result = session.read(
        "entry",
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: 1,
            final_input: true,
            context: &context,
            state: &NdfValue::Unit,
        },
        &store,
        &mut b,
        &mut admission,
    )?;
    let ReadReply::Await {
        continuation,
        report,
        ..
    } = result
    else {
        return Err(ReaderError::NoPending);
    };
    assert_eq!(continuation.report, report);
    assert_eq!(continuation.usage, report.usage);
    let mut forged = continuation.clone();
    forged.session_id = "other".into();
    let reply = terminal("a", 1, &mut b)?;
    assert!(matches!(
        session.resume(&forged, reply, &store, &mut b, &mut admission),
        Err(ReaderError::Continuation)
    ));
    let mut forged = continuation.clone();
    forged.plan_digest = Digest([0; 32]);
    let reply = terminal("a", 1, &mut b)?;
    assert!(matches!(
        session.resume(&forged, reply, &store, &mut b, &mut admission),
        Err(ReaderError::Continuation)
    ));
    let reply = terminal("a", 1, &mut b)?;
    assert!(matches!(
        session.resume(&continuation, reply, &store, &mut b, &mut admission)?,
        ReadReply::Matched { end: 1, .. }
    ));
    assert_eq!(b.usage().source_bytes, 1);
    let reply = terminal("a", 1, &mut b)?;
    assert!(matches!(
        session.resume(&continuation, reply, &store, &mut b, &mut admission),
        Err(ReaderError::NoPending)
    ));
    session.close();
    let reply = terminal("a", 1, &mut b)?;
    assert!(matches!(
        session.resume(&continuation, reply, &store, &mut b, &mut admission),
        Err(ReaderError::Closed)
    ));
    Ok(())
}
#[test]
fn provider_signature_checks_operation_envelope_and_pure_decoder() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let mut p = provider_plan(&schema);
    p.providers[0].operation.name = "missing".into();
    assert!(matches!(
        p.check(&registry, &mut budget()),
        Err(PlanError::ProviderSignature)
    ));
    let mut p = provider_plan(&schema);
    p.providers[0].pure = false;
    assert!(matches!(
        p.check(&registry, &mut budget()),
        Err(PlanError::ProviderSignature)
    ));
    Ok(())
}

fn annotated_terminal(
    schema: &SchemaRef,
    span: Span,
    end: u64,
    budget: &mut Budget,
) -> Result<ProviderReply, StopReason> {
    budget.charge(Resource::Work, 1)?;
    budget.charge(Resource::Diagnostics, 1)?;
    budget.charge(Resource::Events, 1)?;
    let payload = TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "Node".into(),
        fields: vec![],
    });
    let diagnostic = Diagnostic {
        schema: schema.clone(),
        code: "Note".into(),
        severity: Severity::Information,
        stage: "provider".into(),
        arguments: payload.clone(),
        primary: Some(span.clone()),
        related: vec![],
        fixes: vec![],
    };
    let event = Event {
        schema: schema.clone(),
        kind: "Read".into(),
        operation_path: vec![],
        span: Some(span),
        payload,
    };
    Ok(ProviderReply::Read(Box::new(ReadReply::Matched {
        value: NdfValue::Text("a".into()),
        end,
        new_state: NdfValue::Unit,
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![],
        sources: vec![],
        source_maps: vec![],
        report: Report {
            diagnostics: vec![diagnostic],
            events: vec![event],
            usage: budget.usage(),
            trace_overflow: None,
        },
    })))
}
#[test]
fn direct_repeat_failure_and_streaming_shortage_rollback_all_prior_iteration_reports()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    for streaming in [false, true] {
        let mut p = provider_plan(&schema);
        p.expressions.push(if streaming {
            ReaderExpr::Many(ReaderId(0))
        } else {
            ReaderExpr::Repeat {
                min: 2,
                max: 3,
                body: ReaderId(0),
            }
        });
        p.rules[0].root = ReaderId(1);
        p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::Text));
        let checked = p.check(&registry, &mut budget())?;
        let mut b = budget();
        let mut session = ReaderSession::new("repeat".into(), &checked, &registry, &mut b)?;
        let source = source("a")?;
        let mut store = SourceStore::default();
        store.insert(source.clone())?;
        let mut admission = SourceAdmission::default();
        let raw_context = context(&schema, &registry)?;
        let context = check_context(&raw_context, &store, &registry, &mut b, &mut admission)?;
        let reply = session.read(
            "entry",
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: 1,
                final_input: !streaming,
                context: &context,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let ReadReply::Await { continuation, .. } = reply else {
            return Err(ReaderError::NoPending);
        };
        let provider = annotated_terminal(&schema, source.span(0, 1)?, 1, &mut b)?;
        let reply = session.resume(&continuation, provider, &store, &mut b, &mut admission)?;
        let ReadReply::Await { continuation, .. } = reply else {
            return Err(ReaderError::NoPending);
        };
        let report = Report {
            usage: b.usage(),
            ..Report::default()
        };
        let provider = ProviderReply::Read(Box::new(if streaming {
            ReadReply::NeedMore {
                expected: vec![Expectation::EndOfInput],
                report,
            }
        } else {
            ReadReply::NoMatch {
                expected: vec![Expectation::EndOfInput],
                furthest: 1,
                report,
            }
        }));
        let reply = session.resume(&continuation, provider, &store, &mut b, &mut admission)?;
        let report = match reply {
            ReadReply::NeedMore { report, .. } if streaming => report,
            ReadReply::NoMatch { report, .. } if !streaming => report,
            _ => return Err(ReaderError::Context),
        };
        assert!(report.diagnostics.is_empty());
        assert!(report.events.is_empty());
        assert_eq!(report.usage.diagnostics, 1);
        assert_eq!(report.usage.events, 1);
    }
    Ok(())
}
#[test]
fn programmed_repetition_requires_actual_consumption_and_accepts_consuming_calls()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    for empty in [false, true] {
        let mut p = provider_plan(&schema);
        p.expressions.push(ReaderExpr::Many(ReaderId(0)));
        p.rules[0].root = ReaderId(1);
        p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::Text));
        let checked = p.check(&registry, &mut budget())?;
        let mut b = budget();
        let mut session = ReaderSession::new("progress".into(), &checked, &registry, &mut b)?;
        let source = source("a")?;
        let mut store = SourceStore::default();
        store.insert(source.clone())?;
        let mut admission = SourceAdmission::default();
        let raw_context = context(&schema, &registry)?;
        let context = check_context(&raw_context, &store, &registry, &mut b, &mut admission)?;
        let reply = session.read(
            "entry",
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: 1,
                final_input: true,
                context: &context,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let ReadReply::Await { continuation, .. } = reply else {
            return Err(ReaderError::NoPending);
        };
        let provider = terminal(
            if empty { "" } else { "a" },
            if empty { 0 } else { 1 },
            &mut b,
        )?;
        let reply = session.resume(&continuation, provider, &store, &mut b, &mut admission)?;
        if empty {
            assert!(
                matches!(reply,ReadReply::Failed{diagnostic,..} if diagnostic.code=="NonProgress")
            );
        } else {
            let ReadReply::Await { continuation, .. } = reply else {
                return Err(ReaderError::NoPending);
            };
            let provider = ProviderReply::Read(Box::new(ReadReply::NoMatch {
                expected: vec![],
                furthest: 1,
                report: Report {
                    usage: b.usage(),
                    ..Report::default()
                },
            }));
            assert!(
                matches!(session.resume(&continuation,provider,&store,&mut b,&mut admission)?,ReadReply::Matched{value,end:1,..} if value==NdfValue::List(vec![NdfValue::Text("a".into())]))
            );
        }
    }
    Ok(())
}
#[test]
fn generated_sources_are_validated_transactionally_and_available_to_diagnostics()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    for invalid in [false, true] {
        let p = provider_plan(&schema);
        let checked = p.check(&registry, &mut budget())?;
        let mut b = budget();
        let mut session = ReaderSession::new("sources".into(), &checked, &registry, &mut b)?;
        let source = source("a")?;
        let mut store = SourceStore::default();
        store.insert(source.clone())?;
        let mut admission = SourceAdmission::default();
        let raw_context = context(&schema, &registry)?;
        let context = check_context(&raw_context, &store, &registry, &mut b, &mut admission)?;
        let reply = session.read(
            "entry",
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: 1,
                final_input: true,
                context: &context,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let ReadReply::Await { continuation, .. } = reply else {
            return Err(ReaderError::NoPending);
        };
        let generated = admission.create(
            SourceId("generated".into()),
            0,
            "memory:generated".into(),
            b"g".to_vec(),
            &mut b,
        )?;
        let mut provider = annotated_terminal(&schema, generated.span(0, 1)?, 1, &mut b)?;
        if let ProviderReply::Read(reply) = &mut provider
            && let ReadReply::Matched { sources, view, .. } = reply.as_mut()
        {
            sources.push(generated.clone());
            if invalid {
                view.roots.push(ViewRef(99));
            }
        }
        let result = session.resume(&continuation, provider, &store, &mut b, &mut admission);
        if invalid {
            assert!(matches!(
                result,
                Err(ReaderError::View(ViewError::Reference))
            ));
        } else {
            assert!(
                matches!(result?,ReadReply::Matched{sources,report,..} if sources==vec![generated.clone()]&&report.diagnostics[0].primary.as_ref().is_some_and(|span|span.snapshot_ref()==generated.identity()))
            );
        }
        assert_eq!(store.snapshots().len(), 1);
        assert_eq!(b.usage().source_bytes, 2);
    }
    Ok(())
}
#[test]
fn map_decode_and_then_use_typed_provider_envelopes() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    for kind in 0..3 {
        let signature = signature(
            &schema,
            if kind == 2 {
                ProviderKind::Dependent
            } else {
                ProviderKind::Transform
            },
        );
        let expr = match kind {
            0 => ReaderExpr::Map {
                provider: signature.operation.clone(),
                body: ReaderId(0),
            },
            1 => ReaderExpr::Decode {
                provider: signature.operation.clone(),
                body: ReaderId(0),
            },
            _ => ReaderExpr::Then {
                first: ReaderId(0),
                provider: signature.operation.clone(),
            },
        };
        let mut p = plan(
            &schema,
            vec![ReaderExpr::Scalar(CharClass::Any), expr],
            1,
            TypeDescriptor::Text,
        );
        p.providers.push(signature);
        let checked = p.check(&registry, &mut budget())?;
        let mut b = budget();
        let mut session = ReaderSession::new("transform".into(), &checked, &registry, &mut b)?;
        let source = source("ab")?;
        let mut store = SourceStore::default();
        store.insert(source.clone())?;
        let mut admission = SourceAdmission::default();
        let raw_context = context(&schema, &registry)?;
        let context = check_context(&raw_context, &store, &registry, &mut b, &mut admission)?;
        let reply = session.read(
            "entry",
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: 2,
                final_input: true,
                context: &context,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let ReadReply::Await {
            call, continuation, ..
        } = reply
        else {
            return Err(ReaderError::NoPending);
        };
        let provider = if kind == 2 {
            let ProviderCall::Dependent { request, .. } = call.as_ref() else {
                return Err(ReaderError::ProviderContract);
            };
            assert_eq!(request.first, NdfValue::Text("a".into()));
            assert_eq!(request.request.start, 1);
            terminal("b", 2, &mut b)?
        } else {
            let ProviderCall::Transform { request, .. } = call.as_ref() else {
                return Err(ReaderError::ProviderContract);
            };
            assert_eq!(request.value, NdfValue::Text("a".into()));
            ProviderReply::Transform(Box::new(TransformReply {
                value: NdfValue::Text("A".into()),
                view: ViewBundle {
                    elements: vec![],
                    roots: vec![],
                },
                facts: vec![],
                sources: vec![],
                source_maps: vec![],
                report: Report {
                    usage: b.usage(),
                    ..Report::default()
                },
            }))
        };
        assert!(
            matches!(session.resume(&continuation,provider,&store,&mut b,&mut admission)?,ReadReply::Matched{value,end,..} if value==NdfValue::Text(if kind==2{"b"}else{"A"}.into())&&end==if kind==2{2}else{1})
        );
    }
    Ok(())
}
#[test]
fn runtime_rejects_left_recursion_hidden_behind_empty_programmed_call() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let mut p = provider_plan(&schema);
    p.expressions.extend([
        ReaderExpr::Ref("entry".into()),
        ReaderExpr::Seq(vec![ReaderId(0), ReaderId(1)]),
    ]);
    p.rules[0].root = ReaderId(2);
    p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
    let checked = p.check(&registry, &mut budget())?;
    let mut b = budget();
    let mut session = ReaderSession::new("recursion".into(), &checked, &registry, &mut b)?;
    let source = source("")?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut admission = SourceAdmission::default();
    let raw_context = context(&schema, &registry)?;
    let context = check_context(&raw_context, &store, &registry, &mut b, &mut admission)?;
    let mut reply = session.read(
        "entry",
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: 0,
            final_input: true,
            context: &context,
            state: &NdfValue::Unit,
        },
        &store,
        &mut b,
        &mut admission,
    )?;
    for _ in 0..3 {
        match reply {
            ReadReply::Await { continuation, .. } => {
                let provider = terminal("", 0, &mut b)?;
                reply = session.resume(&continuation, provider, &store, &mut b, &mut admission)?;
            }
            ReadReply::Failed { diagnostic, .. } => {
                assert_eq!(diagnostic.code, "NonProgressRecursion");
                return Ok(());
            }
            _ => return Err(ReaderError::Context),
        }
    }
    Err(ReaderError::Context)
}

#[test]
fn stopped_reader_retains_committed_provider_reports() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let mut p = provider_plan(&schema);
    p.expressions.extend([
        ReaderExpr::Literal("q".repeat(100_000)),
        ReaderExpr::Seq(vec![ReaderId(0), ReaderId(1)]),
    ]);
    p.rules[0].root = ReaderId(2);
    p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
    let checked = p.check(&registry, &mut budget())?;
    let source = source("a")?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let raw = context(&schema, &registry)?;
    let checked_context = check_context(
        &raw,
        &store,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut measured = budget();
    let _ = ReaderSession::new("stop".into(), &checked, &registry, &mut measured)?;
    let mut limits = budget().limits();
    limits.allocation_units = measured.usage().allocation_units + 90_000;
    let mut b = Budget::new(limits);
    let mut session = ReaderSession::new("stop".into(), &checked, &registry, &mut b)?;
    let mut admission = SourceAdmission::default();
    let reply = session.read(
        "entry",
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: 1,
            final_input: true,
            context: &checked_context,
            state: &NdfValue::Unit,
        },
        &store,
        &mut b,
        &mut admission,
    )?;
    let ReadReply::Await { continuation, .. } = reply else {
        return Err(ReaderError::NoPending);
    };
    let reply = annotated_terminal(&schema, source.span(0, 1)?, 1, &mut b)?;
    match session.resume(&continuation, reply, &store, &mut b, &mut admission)? {
        ReadReply::Stopped { reason, report } => {
            assert_eq!(reason, StopReason::AllocationLimit);
            assert_eq!(report.diagnostics.len(), 1);
            assert_eq!(report.events.len(), 1);
            assert_eq!(report.usage.diagnostics, 1);
            assert_eq!(report.usage.events, 1);
        }
        _ => return Err(ReaderError::Context),
    }
    Ok(())
}
#[test]
fn long_source_identity_is_not_copied_after_allocation_budget_is_exhausted()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = plan(
        &schema,
        vec![ReaderExpr::Literal("a".into())],
        0,
        TypeDescriptor::Unit,
    );
    let checked = p.check(&registry, &mut budget())?;
    let source = SourceSnapshot::new(
        SourceId("x".repeat(100_000)),
        0,
        "memory:long".into(),
        b"a".to_vec(),
        &mut budget(),
    )?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let raw = context(&schema, &registry)?;
    let checked_context = check_context(
        &raw,
        &store,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut measured = budget();
    let _ = ReaderSession::new("long".into(), &checked, &registry, &mut measured)?;
    let mut limits = budget().limits();
    limits.allocation_units = measured.usage().allocation_units;
    let mut b = Budget::new(limits);
    let mut session = ReaderSession::new("long".into(), &checked, &registry, &mut b)?;
    assert!(matches!(
        session.read(
            "entry",
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: 1,
                final_input: true,
                context: &checked_context,
                state: &NdfValue::Unit
            },
            &store,
            &mut b,
            &mut SourceAdmission::default()
        )?,
        ReadReply::Stopped {
            reason: StopReason::AllocationLimit,
            ..
        }
    ));
    assert_eq!(b.usage().allocation_units, limits.allocation_units);
    Ok(())
}
#[test]
fn checked_context_reuse_admits_exact_source_closure_in_each_operation() -> Result<(), ReaderError>
{
    let (registry, schema) = registry()?;
    let p = provider_plan(&schema);
    let checked = p.check(&registry, &mut budget())?;
    let source = source("a")?;
    let extra = SourceSnapshot::new(
        SourceId("extra".into()),
        0,
        "memory:extra".into(),
        b"b".to_vec(),
        &mut budget(),
    )?;
    let unrelated = SourceSnapshot::new(
        SourceId("unused".into()),
        0,
        "memory:unused".into(),
        vec![b'x'; 10_000],
        &mut budget(),
    )?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    store.insert(extra.clone())?;
    store.insert(unrelated)?;
    let mut raw = context(&schema, &registry)?;
    raw.origins
        .push(nepl3_core::origin::Origin::Direct(extra.span(0, 1)?));
    let checked_context = check_context(
        &raw,
        &store,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    for cap in [1, 2] {
        let mut limits = budget().limits();
        limits.source_bytes = cap;
        let mut b = Budget::new(limits);
        let mut session = ReaderSession::new("closure".into(), &checked, &registry, &mut b)?;
        let reply = session.read(
            "entry",
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: 1,
                final_input: true,
                context: &checked_context,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut SourceAdmission::default(),
        )?;
        if cap == 1 {
            assert!(matches!(
                reply,
                ReadReply::Stopped {
                    reason: StopReason::SourceLimit,
                    ..
                }
            ));
        } else {
            let ReadReply::Await { continuation, .. } = reply else {
                return Err(ReaderError::NoPending);
            };
            assert_eq!(continuation.request.sources.len(), 2);
            assert!(
                continuation
                    .request
                    .sources
                    .iter()
                    .all(|s| s.identity().source.0 != "unused")
            );
            assert_eq!(b.usage().source_bytes, 2);
        }
    }
    Ok(())
}
#[test]
fn fresh_budget_and_forged_call_or_source_cannot_resume_a_saved_slot() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = provider_plan(&schema);
    let checked = p.check(&registry, &mut budget())?;
    let mut b = budget();
    let mut session = ReaderSession::new("identity".into(), &checked, &registry, &mut b)?;
    let source = source("a")?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut admission = SourceAdmission::default();
    let raw = context(&schema, &registry)?;
    let checked_context = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
    let reply = session.read(
        "entry",
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: 1,
            final_input: true,
            context: &checked_context,
            state: &NdfValue::Unit,
        },
        &store,
        &mut b,
        &mut admission,
    )?;
    let ReadReply::Await { continuation, .. } = reply else {
        return Err(ReaderError::NoPending);
    };
    let mut fresh = Budget::new(b.limits());
    let reply = terminal("a", 1, &mut fresh)?;
    assert!(matches!(
        session.resume(
            &continuation,
            reply,
            &store,
            &mut fresh,
            &mut SourceAdmission::default()
        ),
        Err(ReaderError::Continuation)
    ));
    for change in 0..4 {
        let mut forged = continuation.clone();
        match change {
            0 => {
                if let ProviderCall::Read { call_id, .. } = &mut forged.pending {
                    *call_id += 1;
                }
            }
            1 => forged.request.snapshot.revision += 1,
            2 => forged.usage.work = 0,
            _ => forged.depth_base += 1,
        };
        let reply = terminal("a", 1, &mut b)?;
        assert!(matches!(
            session.resume(&forged, reply, &store, &mut b, &mut admission),
            Err(ReaderError::Continuation)
        ));
    }
    let mut absent = ReaderSession::new("other".into(), &checked, &registry, &mut b)?;
    let reply = terminal("a", 1, &mut b)?;
    assert!(matches!(
        absent.resume(&continuation, reply, &store, &mut b, &mut admission),
        Err(ReaderError::NoPending)
    ));
    let (_, base) = match &continuation.pending {
        ProviderCall::Read {
            call_id,
            depth_base,
            ..
        } => (*call_id, *depth_base),
        _ => return Err(ReaderError::ProviderContract),
    };
    b.with_depth_at_least(base, |b| b.observe_depth(7))?;
    let reply = terminal("a", 1, &mut b)?;
    assert!(
        matches!(session.resume(&continuation,reply,&store,&mut b,&mut admission)?,ReadReply::Matched{report,..} if report.usage.depth>=base+7)
    );
    Ok(())
}
#[test]
fn cancellation_during_provider_wait_stops_and_consumes_the_pending_slot() -> Result<(), ReaderError>
{
    let (registry, schema) = registry()?;
    let p = provider_plan(&schema);
    let checked = p.check(&registry, &mut budget())?;
    let mut b = budget();
    let mut session = ReaderSession::new("cancel".into(), &checked, &registry, &mut b)?;
    let source = source("a")?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut admission = SourceAdmission::default();
    let raw = context(&schema, &registry)?;
    let checked_context = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
    let reply = session.read(
        "entry",
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: 1,
            final_input: true,
            context: &checked_context,
            state: &NdfValue::Unit,
        },
        &store,
        &mut b,
        &mut admission,
    )?;
    let ReadReply::Await { continuation, .. } = reply else {
        return Err(ReaderError::NoPending);
    };
    let reply = terminal("a", 1, &mut b)?;
    b.cancel();
    assert!(matches!(
        session.resume(&continuation, reply, &store, &mut b, &mut admission)?,
        ReadReply::Stopped {
            reason: StopReason::Cancelled,
            ..
        }
    ));
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn checked_context_reuse_rejects_same_id_revision_with_changed_content() -> Result<(), ReaderError>
{
    let (registry, schema) = registry()?;
    let p = plan(
        &schema,
        vec![ReaderExpr::Literal("a".into())],
        0,
        TypeDescriptor::Unit,
    );
    let checked = p.check(&registry, &mut budget())?;
    let referenced = SourceSnapshot::new(
        SourceId("context".into()),
        0,
        "memory:context".into(),
        b"a".to_vec(),
        &mut budget(),
    )?;
    let changed = SourceSnapshot::new(
        SourceId("context".into()),
        0,
        "memory:context".into(),
        b"b".to_vec(),
        &mut budget(),
    )?;
    let mut original = SourceStore::default();
    original.insert(referenced.clone())?;
    let mut raw = context(&schema, &registry)?;
    raw.origins
        .push(nepl3_core::origin::Origin::Direct(referenced.span(0, 1)?));
    let proof = check_context(
        &raw,
        &original,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let input = source("a")?;
    let mut current = SourceStore::default();
    current.insert(input.clone())?;
    current.insert(changed)?;
    let mut b = budget();
    let mut session = ReaderSession::new("conflict".into(), &checked, &registry, &mut b)?;
    assert!(matches!(
        session.read(
            "entry",
            ReadRequest {
                snapshot: &input,
                start: 0,
                limit: 1,
                final_input: true,
                context: &proof,
                state: &NdfValue::Unit
            },
            &current,
            &mut b,
            &mut SourceAdmission::default()
        ),
        Err(ReaderError::Source(SourceError::IdentityConflict))
    ));
    Ok(())
}

#[test]
fn failed_choice_restores_programmed_state_before_next_provider_call() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let mut p = provider_plan(&schema);
    p.state_type = TypeDescriptor::U64;
    p.providers[0].state_type = TypeDescriptor::U64;
    p.expressions.extend([
        ReaderExpr::Literal("b".into()),
        ReaderExpr::Seq(vec![ReaderId(0), ReaderId(1)]),
        ReaderExpr::Seq(vec![ReaderId(0)]),
        ReaderExpr::Choice(vec![ReaderId(2), ReaderId(3)]),
    ]);
    p.rules[0].root = ReaderId(4);
    p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
    let checked = p.check(&registry, &mut budget())?;
    let mut b = budget();
    let mut session = ReaderSession::new("state".into(), &checked, &registry, &mut b)?;
    let source = source("aX")?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut admission = SourceAdmission::default();
    let raw = context(&schema, &registry)?;
    let checked_context = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
    let reply = session.read(
        "entry",
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: 2,
            final_input: true,
            context: &checked_context,
            state: &NdfValue::U64(0),
        },
        &store,
        &mut b,
        &mut admission,
    )?;
    let ReadReply::Await { continuation, .. } = reply else {
        return Err(ReaderError::NoPending);
    };
    let mut reply = terminal("a", 1, &mut b)?;
    if let ProviderReply::Read(reply) = &mut reply
        && let ReadReply::Matched { new_state, .. } = reply.as_mut()
    {
        *new_state = NdfValue::U64(1);
    }
    let reply = session.resume(&continuation, reply, &store, &mut b, &mut admission)?;
    let ReadReply::Await {
        call, continuation, ..
    } = reply
    else {
        return Err(ReaderError::NoPending);
    };
    let ProviderCall::Read { request, .. } = call.as_ref() else {
        return Err(ReaderError::ProviderContract);
    };
    assert_eq!(request.start, 0);
    assert_eq!(request.state, NdfValue::U64(0));
    let mut reply = terminal("a", 1, &mut b)?;
    if let ProviderReply::Read(reply) = &mut reply
        && let ReadReply::Matched { new_state, .. } = reply.as_mut()
    {
        *new_state = NdfValue::U64(2);
    }
    assert!(matches!(
        session.resume(&continuation, reply, &store, &mut b, &mut admission)?,
        ReadReply::Matched {
            new_state: NdfValue::U64(2),
            end: 1,
            ..
        }
    ));
    Ok(())
}
#[test]
fn static_plan_rejects_unknown_references_types_and_unproductive_recursion()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let unknown = plan(
        &schema,
        vec![ReaderExpr::Seq(vec![ReaderId(u64::MAX)])],
        0,
        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    );
    assert!(matches!(
        unknown.check(&registry, &mut budget()),
        Err(PlanError::Reference)
    ));
    let recursive = plan(
        &schema,
        vec![ReaderExpr::Ref("entry".into())],
        0,
        TypeDescriptor::Unit,
    );
    assert!(matches!(
        recursive.check(&registry, &mut budget()),
        Err(PlanError::LeftRecursion)
    ));
    let empty = plan(
        &schema,
        vec![
            ReaderExpr::Literal("".into()),
            ReaderExpr::Many(ReaderId(0)),
        ],
        1,
        TypeDescriptor::List(Box::new(TypeDescriptor::Unit)),
    );
    assert!(matches!(
        empty.check(&registry, &mut budget()),
        Err(PlanError::NonProgress)
    ));
    let mismatched = plan(
        &schema,
        vec![
            ReaderExpr::Literal("a".into()),
            ReaderExpr::Scalar(CharClass::Any),
            ReaderExpr::Choice(vec![ReaderId(0), ReaderId(1)]),
        ],
        2,
        TypeDescriptor::Unit,
    );
    assert!(matches!(
        mismatched.check(&registry, &mut budget()),
        Err(PlanError::OutputType)
    ));
    let mut unknown = plan(
        &schema,
        vec![ReaderExpr::Literal("a".into())],
        0,
        TypeDescriptor::Unit,
    );
    unknown.state_type = reader_type("DoesNotExist");
    assert!(matches!(
        unknown.check(&registry, &mut budget()),
        Err(PlanError::Schema(SchemaError::UnknownType))
    ));
    let mut duplicate = plan(
        &schema,
        vec![ReaderExpr::Literal("a".into())],
        0,
        TypeDescriptor::Unit,
    );
    duplicate.rules.push(duplicate.rules[0].clone());
    assert!(matches!(
        duplicate.check(&registry, &mut budget()),
        Err(PlanError::DuplicateRule)
    ));
    Ok(())
}
#[test]
fn plan_identity_is_deterministic_but_distinguishes_arena_representation() -> Result<(), ReaderError>
{
    let (registry, schema) = registry()?;
    let p = provider_plan(&schema);
    let digest = p.digest(&mut budget())?;
    assert_eq!(digest, p.clone().digest(&mut budget())?);
    let mut alternate = p.clone();
    alternate
        .expressions
        .push(ReaderExpr::Literal("unused".into()));
    assert_ne!(digest, alternate.digest(&mut budget())?);
    let checked = p.check(&registry, &mut budget())?;
    let (other_registry, _) = self::registry()?;
    assert!(matches!(
        ReaderSession::new("other".into(), &checked, &other_registry, &mut budget()),
        Err(ReaderError::Context)
    ));
    Ok(())
}

#[test]
fn allocation_faults_across_later_resumes_preserve_already_accepted_reports()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let mut p = provider_plan(&schema);
    p.expressions
        .push(ReaderExpr::Seq(vec![ReaderId(0), ReaderId(0), ReaderId(0)]));
    p.rules[0].root = ReaderId(1);
    p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
    let checked = p.check(&registry, &mut budget())?;
    let source = source("aaa")?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let raw = context(&schema, &registry)?;
    let proof = check_context(
        &raw,
        &store,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut stops = 0;
    let mut completions = 0;
    // Sweep public allocation caps through echo validation, resume preparation,
    // provider boundary, VM execution and construction of the following Await.
    for cap in (60_000..160_000).step_by(512) {
        let mut limits = budget().limits();
        limits.allocation_units = cap;
        let mut b = Budget::new(limits);
        let mut session = ReaderSession::new("faults".into(), &checked, &registry, &mut b)?;
        let mut admission = SourceAdmission::default();
        let reply = session.read(
            "entry",
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: 3,
                final_input: true,
                context: &proof,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let ReadReply::Await { continuation, .. } = reply else {
            continue;
        };
        let first = annotated_terminal(&schema, source.span(0, 1)?, 1, &mut b)?;
        let reply = session.resume(&continuation, first, &store, &mut b, &mut admission)?;
        let ReadReply::Await {
            mut continuation, ..
        } = reply
        else {
            continue;
        };
        for end in [2, 3] {
            let reply = terminal("a", end, &mut b)?;
            match session.resume(&continuation, reply, &store, &mut b, &mut admission)? {
                ReadReply::Stopped { reason, report } => {
                    assert_eq!(reason, StopReason::AllocationLimit);
                    assert_eq!(report.diagnostics.len(), 1, "cap={cap}, end={end}");
                    assert_eq!(report.events.len(), 1, "cap={cap}, end={end}");
                    stops += 1;
                    break;
                }
                ReadReply::Await {
                    continuation: next,
                    report,
                    ..
                } => {
                    assert_eq!(report.diagnostics.len(), 1);
                    assert_eq!(report.events.len(), 1);
                    continuation = next;
                }
                ReadReply::Matched { report, .. } => {
                    assert_eq!(report.diagnostics.len(), 1);
                    assert_eq!(report.events.len(), 1);
                    completions += 1;
                    break;
                }
                _ => return Err(ReaderError::Context),
            }
        }
    }
    assert!(stops > 0);
    assert!(completions > 0);
    Ok(())
}
