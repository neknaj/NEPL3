//! Independently expected production execution of every Grammar ReaderExpr.
use super::*;
use nepl3_core::{
    diagnostic::Report,
    syntax::{Environment, EnvironmentEntry},
    value::*,
};
use nepl3_reader::{
    model::*,
    plan::{ProviderKind, ProviderSignature, ReaderPlan},
    runtime::{ProviderReply, ReaderSession},
};
fn error(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}
fn named(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.reader".into(),
        revision: 1,
        name: name.into(),
    })
}
fn fixture() -> Result<
    (
        nepl3_grammar_core::model::Document,
        SchemaRegistry,
        SchemaRef,
        Vec<compile::ReaderImport>,
    ),
    String,
> {
    let document = nepl3_tools::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/grammar/reader/expressions.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(error)?;
    assert_eq!(
        document.sources[0].text().as_bytes(),
        include_bytes!("../../../conformance/fixtures/grammar/reader/expressions.neplg")
    );
    let mut b = budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b),
        nepl3_reader::schema::descriptor(&mut b),
    ] {
        let descriptor = descriptor.map_err(error)?;
        let schema = descriptor.reference(&mut b).map_err(error)?;
        registry
            .register(schema, descriptor, &mut b)
            .map_err(error)?;
    }
    let descriptor = SchemaDescriptor {
        package: "test.reader-matrix".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Box".into(),
            constraints: vec![],
            shape: TypeShape::Record { fields: vec![] },
        }],
        operations: [
            ("read", "ReadRequest", "ReadReply"),
            ("transform", "TransformRequest", "TransformReply"),
            ("dependent", "DependentRequest", "ReadReply"),
        ]
        .into_iter()
        .map(|(name, input, output)| OperationDescriptor {
            name: name.into(),
            input: named(input),
            output: named(output),
            pure: true,
        })
        .collect(),
    };
    let schema = descriptor.reference(&mut b).map_err(error)?;
    registry
        .register(schema.clone(), descriptor, &mut b)
        .map_err(error)?;
    registry.finalize(&mut b).map_err(error)?;
    let imports = [
        ("read", ProviderKind::Read),
        ("transform", ProviderKind::Transform),
        ("dependent", ProviderKind::Dependent),
    ]
    .into_iter()
    .map(|(name, kind)| compile::ReaderImport {
        provider: name.into(),
        signature: ProviderSignature {
            operation: OperationRef {
                schema: schema.clone(),
                name: name.into(),
            },
            kind,
            pure: true,
            value_input: if kind == ProviderKind::Read {
                TypeDescriptor::Unit
            } else {
                TypeDescriptor::Text
            },
            value_output: TypeDescriptor::Text,
            state_type: TypeDescriptor::Unit,
            continuation_type: named("ReaderContinuation"),
        },
    })
    .collect();
    Ok((document, registry, schema, imports))
}
fn compile_matrix() -> Result<(ReaderPlan, SchemaRegistry), String> {
    let (document, registry, schema, imports) = fixture()?;
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let checked = document.validate(&mut b, &mut admission).map_err(error)?;
    let classes = [compile::NamedClass {
        name: "content".into(),
        class: PresentationClass {
            schema: schema.clone(),
            name: "content".into(),
            fallback: FallbackRole::Content,
        },
    }];
    let views = [compile::NamedView {
        name: "Box".into(),
        kind: KindRef {
            schema: schema.clone(),
            local_kind: registry.kind_id(&schema, "Box").map_err(error)?,
        },
    }];
    let plan = compile::reader::compile(
        &checked,
        &compile::ReaderContext {
            schema: &schema,
            state_type: &TypeDescriptor::Unit,
            imports: &imports,
            classes: &classes,
            views: &views,
            registry: &registry,
        },
        &mut b,
    )
    .map_err(error)?;
    assert_eq!(plan.rules.len(), 23);
    Ok((plan, registry))
}
fn execute(
    plan: &ReaderPlan,
    registry: &SchemaRegistry,
    rule: &str,
    input: &str,
    final_input: bool,
) -> Result<ReadReply, String> {
    execute_failure(plan, registry, rule, input, final_input, false)
}
fn execute_failure(
    plan: &ReaderPlan,
    registry: &SchemaRegistry,
    rule: &str,
    input: &str,
    final_input: bool,
    provider_failure: bool,
) -> Result<ReadReply, String> {
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let snapshot = SourceSnapshot::new(
        SourceId("matrix-input".into()),
        0,
        "memory:matrix-input".into(),
        input.as_bytes().to_vec(),
        &mut b,
    )
    .map_err(error)?;
    let mut sources = SourceStore::default();
    sources.insert(snapshot.clone()).map_err(error)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = nepl3_wire::environment::environment_digest(
        &environment,
        registry
            .selected("nepl3.foundation", 1)
            .ok_or("foundation")?,
        registry,
        &mut b,
    )
    .map_err(error)?;
    let raw = ReaderContext {
        schema: plan.schema.clone(),
        category: "Token".into(),
        mode: "Code".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
        origins: vec![],
    };
    let context = raw
        .check(
            &mut nepl3_wire::foundation::FoundationCodec::new(registry, &sources, &mut admission)
                .map_err(error)?,
            &sources,
            registry,
            &mut b,
        )
        .map_err(error)?;
    let checked = plan.check(registry, &mut b).map_err(error)?;
    let mut session =
        ReaderSession::new("matrix".into(), &checked, registry, &mut b).map_err(error)?;
    let mut reply = session
        .read(
            rule,
            ReadRequest {
                snapshot: &snapshot,
                start: 0,
                limit: input.len() as u64,
                final_input,
                context: &context,
                state: &NdfValue::Unit,
            },
            &sources,
            &mut b,
            &mut admission,
        )
        .map_err(error)?;
    while let ReadReply::Await {
        call, continuation, ..
    } = reply
    {
        let provider = if provider_failure {
            match call.as_ref() {
                ProviderCall::Transform { request, .. } => {
                    b.charge(Resource::Diagnostics, 1).map_err(error)?;
                    let diagnostic = nepl3_core::diagnostic::Diagnostic {
                        schema: plan.schema.clone(),
                        code: "Rejected".into(),
                        severity: nepl3_core::diagnostic::Severity::Error,
                        stage: "transform".into(),
                        arguments: TypedValue::Record(Record {
                            schema: plan.schema.clone(),
                            kind: "Box".into(),
                            fields: vec![],
                        }),
                        primary: Some(request.span.clone()),
                        related: vec![],
                        fixes: vec![],
                    };
                    ProviderReply::Transform(Box::new(TransformReply {
                        outcome: TransformOutcome::Failed {
                            diagnostic: Box::new(diagnostic.clone()),
                            recovery: None,
                        },
                        sources: vec![],
                        source_maps: vec![],
                        report: Report {
                            diagnostics: vec![diagnostic],
                            usage: b.usage(),
                            ..Report::default()
                        },
                    }))
                }
                ProviderCall::Read { request, .. } => {
                    ProviderReply::Read(Box::new(ReadReply::NoMatch {
                        expected: vec![],
                        furthest: request.start,
                        sources: vec![],
                        source_maps: vec![],
                        report: Report {
                            usage: b.usage(),
                            ..Report::default()
                        },
                    }))
                }
                ProviderCall::Dependent { request, .. } => {
                    ProviderReply::Read(Box::new(ReadReply::NoMatch {
                        expected: vec![],
                        furthest: request.end,
                        sources: vec![],
                        source_maps: vec![],
                        report: Report {
                            usage: b.usage(),
                            ..Report::default()
                        },
                    }))
                }
            }
        } else {
            match call.as_ref() {
                ProviderCall::Read {
                    operation, request, ..
                } => {
                    assert_eq!(operation.name, "read");
                    assert_eq!(request.start, 0);
                    if request.start == request.limit && !request.final_input {
                        ProviderReply::Read(Box::new(ReadReply::NeedMore {
                            expected: vec![],
                            sources: vec![],
                            source_maps: vec![],
                            report: Report {
                                usage: b.usage(),
                                ..Report::default()
                            },
                        }))
                    } else {
                        terminal("host", 1, &b)
                    }
                }
                ProviderCall::Transform {
                    operation, request, ..
                } => {
                    assert_eq!(operation.name, "transform");
                    assert_eq!(request.value, NdfValue::Text("a".into()));
                    assert_eq!((request.span.start(), request.span.end()), (0, 1));
                    ProviderReply::Transform(Box::new(TransformReply {
                        outcome: TransformOutcome::Complete {
                            value: NdfValue::Text("mapped".into()),
                            view: request.view.clone(),
                            facts: vec![],
                        },
                        sources: vec![],
                        source_maps: vec![],
                        report: Report {
                            usage: b.usage(),
                            ..Report::default()
                        },
                    }))
                }
                ProviderCall::Dependent {
                    operation, request, ..
                } => {
                    assert_eq!(operation.name, "dependent");
                    assert_eq!(request.first, NdfValue::Text("a".into()));
                    assert_eq!(request.end, 1);
                    terminal("dependent", 2, &b)
                }
            }
        };
        reply = session
            .resume(&continuation, provider, &sources, &mut b, &mut admission)
            .map_err(error)?;
    }
    Ok(reply)
}
fn terminal(value: &str, end: u64, budget: &Budget) -> ProviderReply {
    ProviderReply::Read(Box::new(ReadReply::Matched {
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
    }))
}
#[test]
fn all_reader_constructors_compile_and_execute_with_independent_results() -> Result<(), String> {
    let (plan, registry) = compile_matrix()?;
    let unit = NdfValue::Unit;
    let text = |s: &str| NdfValue::Text(s.into());
    let cases = [
        ("Literal", "x!", 1, unit.clone()),
        ("Scalar", "a!", 1, text("a")),
        (
            "Seq",
            "x7!",
            2,
            NdfValue::List(vec![unit.clone(), text("7")]),
        ),
        ("Choice", "y!", 1, unit.clone()),
        (
            "Many",
            "xx!",
            2,
            NdfValue::List(vec![unit.clone(), unit.clone()]),
        ),
        (
            "Some",
            "xx!",
            2,
            NdfValue::List(vec![unit.clone(), unit.clone()]),
        ),
        ("Optional", "7!", 1, NdfValue::Some(Box::new(text("7")))),
        (
            "Repeat",
            "123",
            2,
            NdfValue::List(vec![text("1"), text("2")]),
        ),
        ("Look", "x!", 0, unit.clone()),
        ("Not", "y!", 0, unit.clone()),
        ("Commit", "x!", 1, unit.clone()),
        ("Capture", "a!", 1, text("a")),
        ("Region", "a!", 1, text("a")),
        ("Node", "a!", 1, text("a")),
        ("Discard", "a!", 1, unit.clone()),
        ("Ref", "a!", 1, text("a")),
        ("Decode", "a!", 1, text("mapped")),
        ("Map", "a!", 1, text("mapped")),
        ("Then", "ab!", 2, text("dependent")),
        ("Call", "a!", 1, text("host")),
        ("Eof", "", 0, unit),
        ("Takecount", "\u{3042}b!", 4, text("\u{3042}b")),
        ("Until", "ab;tail", 2, text("ab")),
    ];
    assert_eq!(cases.len(), 23);
    for (rule, input, expected_end, expected_value) in cases {
        let reply = execute(&plan, &registry, rule, input, true)?;
        let ReadReply::Matched {
            value,
            end,
            facts,
            view,
            ..
        } = reply
        else {
            return Err(format!("{rule}: {reply:?}"));
        };
        assert_eq!((value, end), (expected_value, expected_end), "{rule}");
        match rule {
            "Capture" => assert!(
                matches!(facts.as_slice(), [ReaderFact::Capture { name, span }] if name == "cap" && span.start() == 0 && span.end() == 1)
            ),
            "Region" => assert!(
                matches!(facts.as_slice(), [ReaderFact::Presentation { class, span }] if class.name == "content" && span.start() == 0 && span.end() == 1)
            ),
            "Node" => {
                assert_eq!(view.roots.len(), 1);
                assert_eq!(view.elements.len(), 1);
            }
            _ => {}
        }
    }
    Ok(())
}
#[test]
fn reader_matrix_seed_matches_the_real_adapter() -> Result<(), String> {
    assert_eq!(
        load("conformance/fixtures/grammar/reader/expressions.neplg")?,
        fixture()?.0
    );
    Ok(())
}

#[test]
fn all_reader_constructors_preserve_failure_optional_and_commit_semantics() -> Result<(), String> {
    let (plan, registry) = compile_matrix()?;
    let cases = [
        ("Literal", "!"),
        ("Scalar", "!"),
        ("Seq", "x!"),
        ("Choice", "!"),
        ("Many", "!"),
        ("Some", "!"),
        ("Optional", "!"),
        ("Repeat", ""),
        ("Look", "!"),
        ("Not", "x"),
        ("Commit", "!"),
        ("Capture", "!"),
        ("Region", "!"),
        ("Node", "!"),
        ("Discard", "!"),
        ("Ref", "!"),
        ("Decode", "a"),
        ("Map", "a"),
        ("Then", "ab"),
        ("Call", "a"),
        ("Eof", "x"),
        ("Takecount", "a"),
        ("Until", "abc"),
    ];
    assert_eq!(cases.len(), 23);
    for (rule, input) in cases {
        let reply = execute_failure(&plan, &registry, rule, input, true, true)?;
        match (rule, reply) {
            ("Many", ReadReply::Matched { value, end: 0, .. }) => {
                assert_eq!(value, NdfValue::List(vec![]))
            }
            ("Optional", ReadReply::Matched { value, end: 0, .. }) => {
                assert_eq!(value, NdfValue::None)
            }
            ("Commit", ReadReply::Failed { .. }) => {}
            (
                "Map" | "Decode",
                ReadReply::Failed {
                    diagnostic, report, ..
                },
            ) => {
                assert_eq!(diagnostic.code, "Rejected");
                assert_eq!(report.diagnostics.len(), 1);
                assert_eq!(report.usage.diagnostics, 1);
            }
            ("Many" | "Optional" | "Commit" | "Map" | "Decode", other) => {
                return Err(format!("{rule}: {other:?}"));
            }
            (_, ReadReply::NoMatch { report, .. }) => {
                assert!(report.diagnostics.is_empty());
                assert!(report.events.is_empty());
            }
            (_, other) => return Err(format!("{rule}: {other:?}")),
        }
    }
    Ok(())
}

#[test]
fn compiled_reader_streaming_boundaries_and_rollback_keep_raw_positions() -> Result<(), String> {
    let (plan, registry) = compile_matrix()?;
    let cases = [
        ("Literal", ""),
        ("Scalar", ""),
        ("Map", ""),
        ("Decode", ""),
        ("Then", ""),
        ("Call", ""),
        ("Seq", "x"),
        ("Choice", ""),
        ("Many", "x"),
        ("Some", "x"),
        ("Optional", ""),
        ("Repeat", "1"),
        ("Look", ""),
        ("Not", ""),
        ("Commit", ""),
        ("Capture", ""),
        ("Region", ""),
        ("Node", ""),
        ("Discard", ""),
        ("Ref", ""),
        ("Eof", ""),
        ("Takecount", "\u{3042}"),
        ("Until", "abc"),
    ];
    assert_eq!(cases.len(), 23);
    for (rule, input) in cases {
        let reply = execute(&plan, &registry, rule, input, false)?;
        assert!(
            matches!(reply, ReadReply::NeedMore { .. }),
            "{rule}: {reply:?}"
        );
    }
    Ok(())
}
