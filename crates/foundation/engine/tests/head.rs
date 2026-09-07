#[path = "head/portable.rs"]
mod portable;
#[path = "parse/support.rs"]
mod support;
use nepl3_core::{
    schema::TypeDescriptor,
    source::{Digest, SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{Environment, EnvironmentEntry, FieldValue},
    value::{KindRef, NdfValue, OperationRef},
};
use nepl3_engine::{head::*, package::*, parse::*, profile::*, selection::*};
use nepl3_reader::{
    model::ReaderContext,
    plan::{ReaderExpr, ReaderId, ReaderRule},
    tokenizer::{ReaderMode, SkipRule, TakeRule, TokenReader},
};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
use support::*;

#[test]
fn dynamic_head_uses_compound_completed_child_and_restores_normal_child_context() -> TestResult {
    let result =
        run("choose alt @let z x tail", true).map_err(|e| format!("dynamic parse: {e}"))?;
    let ParseOutcome::Complete { tree, cursor, .. } = result.outcome else {
        return Err(format!("expected Complete: {:?}", result.outcome).into());
    };
    // choose has exactly selector/body. Its body starts in Alt (consumes @),
    // while let's ordinary children use their own Code reads and finish at x.
    assert_eq!(cursor, 19);
    let root = &tree.bundle.nodes[tree.bundle.root.0 as usize];
    assert_eq!(root.fields.len(), 2);
    let FieldValue::Child(selector) = root.fields[0] else {
        return Err("selector child".into());
    };
    let token = &tree.bundle.tokens[tree.bundle.nodes[selector.0 as usize]
        .token
        .ok_or("selector token")?
        .0 as usize];
    assert_eq!(
        token.payload,
        NdfValue::List(vec![NdfValue::Unit, NdfValue::Unit])
    );
    let root_selection = tree.contexts[0]
        .nodes
        .iter()
        .find(|v| v.node == tree.bundle.root)
        .ok_or("root selection")?;
    let ShapeSelection::Dynamic {
        shape,
        child_contexts,
        ..
    } = &root_selection.shape
    else {
        return Err("dynamic selection".into());
    };
    assert_eq!(shape.fields.len(), 2);
    assert_eq!(child_contexts[1].mode, "Alt");
    let FieldValue::Child(body) = root.fields[1] else {
        return Err("body child".into());
    };
    let body_node = &tree.bundle.nodes[body.0 as usize];
    let FieldValue::Child(inner) = body_node.fields[1] else {
        return Err("ordinary nested child".into());
    };
    let selected = tree.contexts[0]
        .nodes
        .iter()
        .find(|v| v.node == inner)
        .ok_or("inner selection")?;
    assert_eq!(selected.entry.mode, "Code");
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Owned,
    Sealed,
    Portable,
    Native,
    NativeFallback,
    NativeError,
    MixedOwned,
    MixedNative,
    Foreign,
    Cancel,
    Fail,
    ShapeFail,
    LongFailure,
    DeepFailure,
    Sources,
    Auxiliary,
    Overflow,
    OverflowStop,
}
fn run(input: &str, exercise_rejections: bool) -> Result<ParseReply, String> {
    run_case(input, Case::Owned, exercise_rejections)
}
fn run_case(input: &str, case: Case, exercise_rejections: bool) -> Result<ParseReply, String> {
    let (mut package, registry) = fixture()?;
    let mut setup = budget();
    let kind = |name| -> Result<KindRef, String> {
        Ok(KindRef {
            schema: package.schema.clone(),
            local_kind: registry
                .kind_id(&package.schema, name)
                .map_err(|v| format!("{v:?}"))?,
        })
    };
    let compound_kind = kind("Token:Compound")?;
    let compound_leaf = kind("Leaf:Compound")?;
    package.reader.expressions = vec![
        ReaderExpr::Literal("a".into()),
        ReaderExpr::Literal("lt".into()),
        ReaderExpr::Seq(vec![ReaderId(0), ReaderId(1)]),
        ReaderExpr::Literal("@".into()),
    ];
    package.reader.rules = vec![
        ReaderRule {
            name: "selector".into(),
            root: ReaderId(2),
            output: TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
        },
        ReaderRule {
            name: "at".into(),
            root: ReaderId(3),
            output: TypeDescriptor::Unit,
        },
    ];
    package.modes.push(ReaderMode {
        name: "Selector".into(),
        skip: package.modes[0].skip.clone(),
        take: vec![TakeRule {
            reader: TokenReader::Rule("selector".into()),
            kind: compound_kind.clone(),
        }],
    });
    let mut alt = package.modes[0].clone();
    alt.name = "Alt".into();
    alt.skip.push(SkipRule {
        reader: TokenReader::Rule("at".into()),
    });
    package.modes.push(alt);
    package.categories.push(Category {
        name: "Selector".into(),
        mode: "Selector".into(),
    });
    package.categories.push(Category {
        name: "Body".into(),
        mode: "Alt".into(),
    });
    package.bindings.push(Binding::Group(vec![]));
    package.bindings.push(Binding::Visit("name".into()));
    package
        .bindings
        .push(Binding::Group(vec![BindingId(5), BindingId(1)]));
    package.reads.push(ReadSpec::Local {
        category: "Selector".into(),
    });
    package.leaves.push(Leaf {
        category: "Selector".into(),
        kind: compound_leaf,
        token_kind: compound_kind,
        payload: TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
        binding: BindingId(4),
        styles: vec![],
    });
    let mut body_leaf = package.leaves[0].clone();
    body_leaf.category = "Body".into();
    package.leaves.push(body_leaf);
    let mut body_form = package.forms[0].clone();
    body_form.category = "Body".into();
    package.forms.push(body_form);
    let mut shape = HeadShape {
        kind: package.forms[0].kind.clone(),
        fields: vec![
            FieldSpec {
                name: "name".into(),
                read: ReadSpecId(2),
            },
            FieldSpec {
                name: "body".into(),
                read: ReadSpecId(1),
            },
        ],
        binding: BindingId(6),
        styles: vec![],
    };
    if case == Case::Foreign {
        package.reads.push(ReadSpec::Foreign {
            alias: "Guest".into(),
            category: "Selector".into(),
        });
        package.bindings.push(Binding::Visit("first".into()));
        package.bindings.push(Binding::Visit("second".into()));
        package
            .bindings
            .push(Binding::Group(vec![BindingId(7), BindingId(8)]));
        shape.kind.local_kind = registry
            .kind_id(&package.schema, "Form:Pair")
            .map_err(|v| format!("{v:?}"))?;
        shape.fields = vec![
            FieldSpec {
                name: "first".into(),
                read: ReadSpecId(3),
            },
            FieldSpec {
                name: "second".into(),
                read: ReadSpecId(1),
            },
        ];
        shape.binding = BindingId(9);
    }
    if matches!(case, Case::MixedOwned | Case::MixedNative) {
        use nepl3_reader::plan::{ProviderKind, ProviderSignature};
        let operation = OperationRef {
            schema: package.schema.clone(),
            name: "read".into(),
        };
        package
            .reader
            .expressions
            .push(ReaderExpr::Call(operation.clone()));
        package.reader.rules[0].root = ReaderId(4);
        package.reader.providers.push(ProviderSignature {
            operation,
            kind: ProviderKind::Read,
            value_input: TypeDescriptor::Unit,
            value_output: TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
            state_type: TypeDescriptor::Unit,
            continuation_type: TypeDescriptor::Named(nepl3_core::schema::TypeRef {
                package: "nepl3.reader".into(),
                revision: 1,
                name: "ReaderContinuation".into(),
            }),
            pure: true,
        });
    }
    let identity = package
        .check(&registry, &mut setup)
        .and_then(|v| v.semantic_identity(&mut setup))
        .map_err(|v| format!("{v:?}"))?;
    let engine = registry
        .selected("nepl3.engine", 1)
        .ok_or("engine")?
        .clone();
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let head_provider = HeadProviderRef {
        shape: OperationRef {
            schema: engine.clone(),
            name: "headShape".into(),
        },
        child_context: OperationRef {
            schema: engine.clone(),
            name: "headChildContext".into(),
        },
    };
    let mut operations = vec![
        head_provider.shape.clone(),
        head_provider.child_context.clone(),
    ];
    if matches!(case, Case::MixedOwned | Case::MixedNative) {
        operations.push(OperationRef {
            schema: package.schema.clone(),
            name: "read".into(),
        });
    }
    let implementation_digest = Digest::of(b"test choose head provider implementation");
    let providers = vec![ProviderImplementation {
        provider: "choose-provider".into(),
        revision: 1,
        implementation_digest,
        operations: operations.clone(),
    }];
    let mut profile = ParseProfile {
        id: "dynamic-profile".into(),
        languages: vec![LanguageRegistration {
            alias: "Host".into(),
            package: identity,
            default_category: "Expr".into(),
        }],
        schemas: vec![
            package.schema.clone(),
            engine,
            foundation.clone(),
            registry
                .selected("nepl3.reader", 1)
                .ok_or("reader")?
                .clone(),
        ],
        category_modes: vec![],
        head_providers: vec![HeadRegistration {
            alias: "Host".into(),
            category: "Expr".into(),
            provider: head_provider,
        }],
        providers: operations
            .iter()
            .map(|operation| ProviderRequirement {
                provider: "choose-provider".into(),
                revision: 1,
                implementation_digest,
                operation: operation.clone(),
            })
            .collect(),
        allowlist: operations,
        resources: vec![],
        limits: budget().limits(),
    };
    if case == Case::Foreign {
        profile.languages.push(LanguageRegistration {
            alias: "Guest".into(),
            package: profile.languages[0].package.clone(),
            default_category: "Expr".into(),
        });
    }
    let packages = [&package];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &providers,
                resources: &[],
            },
            &registry,
            &mut setup,
        )
        .map_err(|v| format!("{v:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("head-input".into()),
        0,
        "memory:head-input".into(),
        input.as_bytes().to_vec(),
        &mut setup,
    )
    .map_err(|v| format!("{v:?}"))?;
    let mut sources = SourceStore::default();
    sources
        .insert(source.clone())
        .map_err(|v| format!("{v:?}"))?;
    let mut origins = vec![];
    if case == Case::Auxiliary {
        let auxiliary = SourceSnapshot::new(
            SourceId("head-auxiliary".into()),
            0,
            "memory:head-auxiliary".into(),
            b"auxiliary".to_vec(),
            &mut setup,
        )
        .map_err(|v| format!("{v:?}"))?;
        origins.push(nepl3_core::origin::Origin::Direct(
            auxiliary.span(0, 9).map_err(|v| format!("{v:?}"))?,
        ));
        sources.insert(auxiliary).map_err(|v| format!("{v:?}"))?;
    }
    let mut primary_only = SourceStore::default();
    primary_only
        .insert(source.clone())
        .map_err(|v| format!("{v:?}"))?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&environment, &foundation, &registry, &mut setup)
        .map_err(|v| format!("{v:?}"))?;
    let raw = ReaderContext {
        schema: package.schema.clone(),
        category: "Expr".into(),
        mode: "Code".into(),
        origins,
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
    };
    let mut setup_admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut setup_admission)
        .map_err(|v| format!("{v:?}"))?;
    let context = raw
        .check(&mut codec, &sources, &registry, &mut setup)
        .map_err(|v| format!("{v:?}"))?;
    let mut env_inputs = vec![EnvironmentInput {
        alias: "Host",
        context: &context,
    }];
    if case == Case::Foreign {
        env_inputs.push(EnvironmentInput {
            alias: "Guest",
            context: &context,
        });
    }
    let environments =
        ParseEnvironmentSet::prepare(&resolved, &env_inputs, &sources, &mut codec, &mut setup)
            .map_err(|v| format!("{v:?}"))?;
    let entry = resolved
        .entry("Host", None, &mut setup)
        .map_err(|v| format!("{v:?}"))?;
    let mut limits = budget().limits();
    if case == Case::LongFailure {
        limits.work = 100_000;
    }
    if case == Case::DeepFailure {
        limits.depth = 32;
    }
    let mut b = nepl3_core::budget::Budget::new(limits);
    let mut a = SourceAdmission::default();
    let mut session = ParseSession::new("dynamic-session".into(), &resolved, &environments, &mut b)
        .map_err(|v| format!("{v:?}"))?;
    let mut states = vec![LanguageReaderState {
        alias: "Host".into(),
        state: NdfValue::Unit,
    }];
    if case == Case::Foreign {
        states.push(LanguageReaderState {
            alias: "Guest".into(),
            state: NdfValue::Unit,
        });
    }
    let request = ParseRequest {
        snapshot: &source,
        start: 0,
        limit: input.len() as u64,
        final_input: true,
        entry: &entry,
        states: &states,
    };
    let mut host = Host {
        profile: &resolved,
        shape: &shape,
        case,
        calls: 0,
        reader_calls: 0,
    };
    let mut reply = if matches!(
        case,
        Case::Native | Case::NativeFallback | Case::NativeError | Case::MixedNative
    ) {
        let result = session
            .read_with_host(request, &sources, &mut b, &mut a, &mut host)
            .map_err(|v| format!("native {v:?}"))?;
        assert_eq!(result.host_error.is_some(), case == Case::NativeError);
        if case == Case::MixedNative {
            assert_eq!(host.reader_calls, 1);
        }
        result.reply
    } else if case == Case::Sealed {
        session
            .read_completed(request, &sources, &mut b, &mut a)
            .map(unseal)
            .map_err(|v| format!("sealed read {v:?}"))?
    } else {
        session
            .read(request, &sources, &mut b, &mut a)
            .map_err(|v| format!("{v:?}"))?
    };
    let mut calls = 0;
    loop {
        if let ParseOutcome::Await { call, continuation } = &reply.outcome {
            let nepl3_reader::model::ProviderCall::Read { depth_base, .. } = call.as_ref() else {
                return Err("reader call".into());
            };
            let value = b
                .with_depth_at_least(*depth_base, |b| provide_selector(call, b, &mut a))
                .map_err(|v| format!("selector provider {v:?}"))?;
            let nepl3_reader::runtime::ProviderReply::Read(good) = &value else {
                return Err("read reply".into());
            };
            let mut invalid = good.as_ref().clone();
            let nepl3_reader::model::ReadReply::Matched { report, .. } = &mut invalid else {
                return Err("matched reply".into());
            };
            report.trace_overflow = Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 });
            assert_eq!(
                session.resume(
                    continuation,
                    nepl3_reader::runtime::ProviderReply::Read(Box::new(invalid)),
                    &sources,
                    &mut b,
                    &mut a
                ),
                Err(ParseError::Reader(
                    nepl3_reader::runtime::ReaderError::ProviderContract
                ))
            );
            // The selected provider promises a compound payload. A malformed
            // value must preserve all three pending layers for the same retry.
            let mut invalid = good.as_ref().clone();
            let nepl3_reader::model::ReadReply::Matched { value: payload, .. } = &mut invalid
            else {
                return Err("matched reply".into());
            };
            *payload = nepl3_core::value::NdfValue::Unit;
            assert!(matches!(
                session.resume(
                    continuation,
                    nepl3_reader::runtime::ProviderReply::Read(Box::new(invalid)),
                    &sources,
                    &mut b,
                    &mut a
                ),
                Err(ParseError::Reader(
                    nepl3_reader::runtime::ReaderError::Schema(_)
                ))
            ));
            reply = session
                .resume(continuation, value, &sources, &mut b, &mut a)
                .map_err(|v| format!("selector resume {v:?}"))?;
            continue;
        }
        let ParseOutcome::AwaitHead { call, continuation } = &reply.outcome else {
            break;
        };
        calls += 1;
        let mut provider_reply = if case == Case::Portable {
            portable::answer(call, &resolved, &shape, &sources, &mut b, &mut a)?
        } else {
            answer(call, &resolved, &shape, &mut b).map_err(|v| format!("answer {v:?}"))?
        };
        if matches!(
            case,
            Case::Cancel | Case::Fail | Case::LongFailure | Case::DeepFailure
        ) {
            if calls == 1 {
                let note = diagnostic(call, "head-note");
                b.charge(nepl3_core::budget::Resource::Diagnostics, 1)
                    .map_err(|v| format!("{v:?}"))?;
                b.charge(nepl3_core::budget::Resource::Events, 1)
                    .map_err(|v| format!("{v:?}"))?;
                provider_reply.report.diagnostics.push(note.clone());
                provider_reply.report.events.push(ProjectedEvent {
                    schema: note.schema.clone(),
                    kind: "head-event".into(),
                    operation_path: vec![],
                    span: note.primary.clone(),
                    payload: note.arguments.clone(),
                });
            }
            if matches!(call.request, HeadRequest::ChildContext { index: 1, .. }) {
                if case == Case::Cancel {
                    let mut fresh = nepl3_core::budget::Budget::new(b.limits());
                    fresh.cancel();
                    assert_eq!(
                        session.resume_head(
                            continuation,
                            provider_reply.clone(),
                            &sources,
                            &mut fresh,
                            &mut a
                        ),
                        Err(ParseError::Continuation)
                    );
                    b.cancel();
                    let result = session
                        .resume_head(
                            continuation,
                            provider_reply.clone(),
                            &sources,
                            &mut b,
                            &mut a,
                        )
                        .map_err(|v| format!("cancel {v:?}"))?;
                    assert_eq!(
                        session.resume_head(continuation, provider_reply, &sources, &mut b, &mut a),
                        Err(ParseError::NoPending)
                    );
                    let mut fresh = budget();
                    let mut admission = SourceAdmission::default();
                    let next = session
                        .read(
                            ParseRequest {
                                snapshot: &source,
                                start: 0,
                                limit: input.len() as u64,
                                final_input: true,
                                entry: &entry,
                                states: &states,
                            },
                            &sources,
                            &mut fresh,
                            &mut admission,
                        )
                        .map_err(|v| format!("reuse {v:?}"))?;
                    assert!(matches!(next.outcome, ParseOutcome::AwaitHead { .. }));
                    return Ok(result);
                }
                let mut diagnostic = diagnostic(call, "head-failure");
                if case == Case::LongFailure {
                    diagnostic.code = "long".repeat(100_000);
                }
                if case == Case::DeepFailure {
                    let mut value = NdfValue::Unit;
                    for _ in 0..100 {
                        value = NdfValue::Some(Box::new(value));
                    }
                    diagnostic.arguments =
                        nepl3_core::value::TypedValue::Record(nepl3_core::value::Record {
                            schema: package.schema.clone(),
                            kind: "Token:Compound".into(),
                            fields: vec![NdfValue::List(vec![value])],
                        });
                }
                b.charge(nepl3_core::budget::Resource::Diagnostics, 1)
                    .map_err(|v| format!("{v:?}"))?;
                provider_reply.outcome = HeadOutcome::Failed {
                    diagnostic: Box::new(diagnostic.clone()),
                };
                provider_reply.report.diagnostics.push(diagnostic);
            }
            provider_reply.report.usage = b.usage();
        }
        if case == Case::ShapeFail && calls == 1 {
            let diagnostic = diagnostic(call, "shape-failure");
            b.charge(nepl3_core::budget::Resource::Diagnostics, 1)
                .map_err(|v| format!("{v:?}"))?;
            provider_reply.outcome = HeadOutcome::Failed {
                diagnostic: Box::new(diagnostic.clone()),
            };
            provider_reply.report.diagnostics.push(diagnostic);
            provider_reply.report.usage = b.usage();
        }
        if matches!(case, Case::Sources | Case::Auxiliary) {
            if calls == 1 {
                b.charge(nepl3_core::budget::Resource::Diagnostics, 1)
                    .map_err(|v| format!("{v:?}"))?;
                provider_reply
                    .report
                    .diagnostics
                    .push(diagnostic(call, "accepted-after-retry"));
                provider_reply.report.usage = b.usage();
                let changed = SourceSnapshot::new(
                    SourceId("head-input".into()),
                    0,
                    "memory:other-locator".into(),
                    input.as_bytes().to_vec(),
                    &mut setup,
                )
                .map_err(|v| format!("{v:?}"))?;
                let mut changed_store = SourceStore::default();
                changed_store
                    .insert(changed)
                    .map_err(|v| format!("{v:?}"))?;
                assert_eq!(
                    session.resume_head(
                        continuation,
                        provider_reply.clone(),
                        &changed_store,
                        &mut b,
                        &mut a
                    ),
                    Err(ParseError::Source(
                        nepl3_core::source::SourceError::IdentityConflict
                    ))
                );
            }
            // Test both a nonempty first Report and the next empty Report: source
            // prerequisites cannot depend on whether this callback emitted a note.
            if calls <= 2 {
                assert_eq!(
                    session.resume_head(
                        continuation,
                        provider_reply.clone(),
                        &SourceStore::default(),
                        &mut b,
                        &mut a
                    ),
                    Err(ParseError::Source(
                        nepl3_core::source::SourceError::MissingSnapshot
                    ))
                );
            }
        }
        if matches!(case, Case::Overflow | Case::OverflowStop) {
            let mut wrong = provider_reply.clone();
            wrong.report.trace_overflow =
                Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 });
            assert_eq!(
                session.resume_head(continuation, wrong.clone(), &sources, &mut b, &mut a),
                Err(ParseError::Head(HeadError::Report))
            );
            if calls == 1 {
                let note = diagnostic(call, "overflow-failure");
                b.charge(nepl3_core::budget::Resource::Diagnostics, 1)
                    .map_err(|v| format!("{v:?}"))?;
                wrong.outcome = HeadOutcome::Failed {
                    diagnostic: Box::new(note.clone()),
                };
                wrong.report.diagnostics.push(note);
                wrong.report.usage = b.usage();
                assert_eq!(
                    session.resume_head(continuation, wrong.clone(), &sources, &mut b, &mut a),
                    Err(ParseError::Head(HeadError::Report))
                );
                if case == Case::OverflowStop {
                    // A stop can still report an unsuccessful terminal event attempt.
                    wrong.outcome = HeadOutcome::Stopped {
                        reason: nepl3_core::budget::StopReason::Cancelled,
                    };
                    return session
                        .resume_head(continuation, wrong, &sources, &mut b, &mut a)
                        .map_err(|v| format!("overflow stop {v:?}"));
                }
            }
        }
        if exercise_rejections {
            let mut wrong = continuation.as_ref().clone();
            wrong.progress.frames[0].arity += 1;
            assert_eq!(
                session.resume_head(&wrong, provider_reply.clone(), &sources, &mut b, &mut a),
                Err(ParseError::Continuation)
            );
            if let HeadOutcome::Shape { shape: Some(shape) } = &provider_reply.outcome {
                let mut bad = shape.as_ref().clone();
                bad.fields.push(bad.fields[0].clone());
                let mut wrong = provider_reply.clone();
                wrong.outcome = HeadOutcome::Shape {
                    shape: Some(Box::new(bad)),
                };
                assert_eq!(
                    session.resume_head(continuation, wrong, &sources, &mut b, &mut a),
                    Err(ParseError::Package(
                        nepl3_engine::package::PackageError::KindShape
                    ))
                );
            }
            if calls == 1 {
                let mut wrong = provider_reply.clone();
                let mut note = diagnostic(call, "unread-location");
                let mut span = call.head.window.span.clone();
                span.start = span.end + 1;
                span.end = input.len() as u64;
                note.primary = Some(span);
                b.charge(nepl3_core::budget::Resource::Diagnostics, 1)
                    .map_err(|v| format!("{v:?}"))?;
                wrong.report.diagnostics.push(note);
                wrong.report.usage = b.usage();
                assert_eq!(
                    session.resume_head(continuation, wrong, &sources, &mut b, &mut a),
                    Err(ParseError::Head(HeadError::Projection))
                );
            }
            let mut wrong = continuation.as_ref().clone();
            wrong.call.identity.call_id += 1;
            assert_eq!(
                session.resume_head(&wrong, provider_reply.clone(), &sources, &mut b, &mut a),
                Err(ParseError::Continuation)
            );
            let mut wrong = provider_reply.clone();
            wrong.identity.profile_digest = Digest::of(b"other-profile");
            assert_eq!(
                session.resume_head(continuation, wrong, &sources, &mut b, &mut a),
                Err(ParseError::Head(HeadError::Identity))
            );
            if matches!(call.request, HeadRequest::ChildContext { .. }) {
                let mut wrong = provider_reply.clone();
                wrong.outcome = HeadOutcome::Shape {
                    shape: Some(Box::new(shape.clone())),
                };
                assert_eq!(
                    session.resume_head(continuation, wrong, &sources, &mut b, &mut a),
                    Err(ParseError::Head(HeadError::ReplyKind))
                );
            }
        }
        if case == Case::Auxiliary && calls == 2 {
            let changed = SourceSnapshot::new(
                SourceId("head-auxiliary".into()),
                0,
                "memory:changed-auxiliary".into(),
                b"auxiliary".to_vec(),
                &mut setup,
            )
            .map_err(|v| format!("{v:?}"))?;
            let mut changed_store = SourceStore::default();
            changed_store
                .insert(source.clone())
                .map_err(|v| format!("{v:?}"))?;
            changed_store
                .insert(changed)
                .map_err(|v| format!("{v:?}"))?;
            assert_eq!(
                session.resume_head(
                    continuation,
                    provider_reply.clone(),
                    &changed_store,
                    &mut b,
                    &mut a
                ),
                Err(ParseError::Source(
                    nepl3_core::source::SourceError::IdentityConflict
                ))
            );
        }
        let caller_sources = if case == Case::Auxiliary {
            &primary_only
        } else {
            &sources
        };
        reply = if case == Case::Sealed {
            session
                .resume_head_completed(continuation, provider_reply, caller_sources, &mut b, &mut a)
                .map(unseal)
        } else {
            session.resume_head(continuation, provider_reply, caller_sources, &mut b, &mut a)
        }
        .map_err(|v| format!("resume call {calls}: {v:?}"))?;
    }
    if matches!(case, Case::LongFailure | Case::DeepFailure) {
        return Ok(reply);
    }
    let tree = match &reply.outcome {
        ParseOutcome::Complete { tree, .. } | ParseOutcome::Recovered { tree, .. } => tree,
        _ => return Err(format!("terminal {:?}", reply.outcome)),
    };
    tree.validate(&resolved, &mut b, &mut a)
        .map_err(|v| format!("tree: {v:?}"))?;
    if case == Case::Portable {
        portable::tree_roundtrip(tree, &resolved)?;
    }
    if case == Case::Owned {
        assert_eq!(calls, 4);
    } // choose shape, two contexts, and ordinary x's explicit None.
    Ok(reply)
}

fn answer(
    call: &HeadCall,
    profile: &ResolvedParseProfile<'_>,
    chosen: &HeadShape,
    b: &mut nepl3_core::budget::Budget,
) -> Result<HeadReply, ParseError> {
    let outcome = match &call.request {
        HeadRequest::Shape => HeadOutcome::Shape {
            shape: if call.head.window.bytes == b"choose" {
                Some(Box::new(chosen.clone()))
            } else {
                None
            },
        },
        HeadRequest::ChildContext {
            index,
            completed,
            shape,
        } => {
            assert_eq!(completed.roots.len() as u64, *index);
            let field = shape
                .fields
                .get(*index as usize)
                .ok_or(ParseError::Reference)?;
            let mut entry = profile.read_entry(&call.entry, field.read, b)?.entry;
            if *index == 1 {
                let node = &completed.nodes[completed.roots[0].0 as usize];
                let value = &node.token.as_ref().ok_or(ParseError::Reference)?.payload;
                if *value != NdfValue::List(vec![NdfValue::Unit, NdfValue::Unit]) {
                    return Err(HeadError::Context.into());
                }
                let last_read = completed
                    .windows
                    .iter()
                    .map(|v| v.span.end)
                    .max()
                    .ok_or(ParseError::Reference)?;
                assert_eq!(last_read, call.head.window.span.end + 4);
                entry.category = "Body".into();
                entry.mode = "Alt".into();
            }
            HeadOutcome::ChildContext { context: entry }
        }
    };
    Ok(HeadReply {
        identity: call.identity.clone(),
        outcome,
        report: ProjectedReport {
            usage: b.usage(),
            ..ProjectedReport::default()
        },
    })
}
fn diagnostic(call: &HeadCall, code: &str) -> ProjectedDiagnostic {
    ProjectedDiagnostic {
        schema: call.entry.package.schema.clone(),
        code: code.into(),
        severity: nepl3_core::diagnostic::Severity::Error,
        stage: "head".into(),
        arguments: nepl3_core::value::TypedValue::Record(nepl3_core::value::Record {
            schema: call.entry.package.schema.clone(),
            kind: "Leaf:Name".into(),
            fields: vec![],
        }),
        primary: Some(call.head.window.span.clone()),
        related: vec![],
        fixes: vec![],
    }
}
struct Host<'a, 'p> {
    profile: &'a ResolvedParseProfile<'p>,
    shape: &'a HeadShape,
    case: Case,
    calls: usize,
    reader_calls: usize,
}
impl ParseHost for Host<'_, '_> {
    fn head(
        &mut self,
        call: &HeadCall,
        requirement: &ProviderRequirement,
        b: &mut nepl3_core::budget::Budget,
        _: &mut SourceAdmission,
    ) -> Result<Option<HeadReply>, ParseError> {
        self.calls += 1;
        assert_eq!(requirement.provider, "choose-provider");
        assert_eq!(
            requirement.implementation_digest,
            Digest::of(b"test choose head provider implementation")
        );
        assert_eq!(requirement.operation, call.identity.operation);
        assert_eq!(b.current_depth(), call.depth_base);
        if self.calls == 2 {
            if self.case == Case::NativeFallback {
                return Ok(None);
            }
            if self.case == Case::NativeError {
                return Err(ParseError::Context);
            }
        }
        answer(call, self.profile, self.shape, b).map(Some)
    }
    fn provider(
        &mut self,
        call: &nepl3_reader::model::ProviderCall,
        requirement: &ProviderRequirement,
        budget: &mut nepl3_core::budget::Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<nepl3_reader::runtime::ProviderReply>, ParseError> {
        assert_eq!(requirement.provider, "choose-provider");
        assert_eq!(
            requirement.implementation_digest,
            Digest::of(b"test choose head provider implementation")
        );
        self.reader_calls += 1;
        provide_selector(call, budget, admission).map(Some)
    }

    fn reservation(
        &mut self,
        _: &nepl3_reader::tokenizer::ReservationRequest,
        _: &mut nepl3_core::budget::Budget,
        _: &mut SourceAdmission,
    ) -> Result<Option<nepl3_core::source::SourceReservation>, ParseError> {
        Err(ParseError::Context)
    }
}
#[test]
fn synchronous_head_host_and_owned_fallback_keep_the_same_tree_and_positions() -> TestResult {
    let input = "let n choose alt @let z x tail";
    let owned = run_case(input, Case::Owned, false)?;
    for case in [Case::Native, Case::NativeFallback, Case::NativeError] {
        let native = run_case(input, case, false)?;
        assert_eq!(native.outcome, owned.outcome);
        assert_eq!(native.report.diagnostics, owned.report.diagnostics);
        assert_eq!(native.sources, owned.sources);
        assert_eq!(native.source_maps, owned.source_maps);
        assert_eq!(native.report.usage.depth, owned.report.usage.depth);
    }
    Ok(())
}
#[test]
fn head_cancel_keeps_accepted_diagnostic_event_and_fixed_parent_progress() -> TestResult {
    let result = run_case("choose alt @let z x tail", Case::Cancel, false)?;
    let ParseOutcome::Stopped {
        reason: nepl3_core::budget::StopReason::Cancelled,
        progress: Some(progress),
    } = result.outcome
    else {
        return Err("cancelled progress".into());
    };
    assert_eq!(progress.frames[0].arity, 2);
    assert_eq!(progress.frames[0].children.len(), 1);
    assert_eq!(result.report.diagnostics.len(), 1);
    assert_eq!(result.report.events.len(), 1);
    assert_eq!(result.report.usage.diagnostics, 1);
    assert_eq!(result.report.usage.events, 1);
    assert_eq!(result.sources.len(), 1);
    Ok(())
}
#[test]
fn failed_child_context_preserves_known_parent_arity_and_formal_primary_once() -> TestResult {
    let result = run_case("choose alt @let z x tail", Case::Fail, false)?;
    let ParseOutcome::Recovered { tree, .. } = result.outcome else {
        return Err("recovered parent".into());
    };
    assert_eq!(
        tree.bundle.nodes[tree.bundle.root.0 as usize].fields.len(),
        2
    );
    assert_eq!(
        result
            .report
            .diagnostics
            .iter()
            .filter(|v| v.code == "head-failure")
            .count(),
        1
    );
    assert_eq!(
        result.report.usage.diagnostics,
        result.report.diagnostics.len() as u64
    );
    assert!(
        tree.recovery
            .iter()
            .flat_map(|v| &v.entries)
            .any(|v| matches!(
                v.kind,
                nepl3_engine::recovery::RecoveryKind::Unparsed {
                    reason: nepl3_engine::recovery::UnparsedReason::ProviderFailure,
                    ..
                }
            ))
    );
    Ok(())
}
#[test]
fn long_failed_reply_is_charged_before_primary_equality_and_keeps_prior_report() -> TestResult {
    let result = run_case("choose alt @let z x tail", Case::LongFailure, false)?;
    assert!(matches!(
        result.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::WorkLimit,
            ..
        }
    ));
    assert_eq!(result.report.diagnostics.len(), 1);
    assert_eq!(result.report.events.len(), 1);
    assert_eq!(result.report.diagnostics[0].code, "head-note");
    Ok(())
}

#[test]
fn foreign_completed_selector_uses_local_projection_ids_and_returns_to_host_body() -> TestResult {
    let reply = run_case("choose alt @let z x tail", Case::Foreign, true)?;
    let ParseOutcome::Complete { tree, cursor, .. } = reply.outcome else {
        return Err("complete foreign dynamic".into());
    };
    assert_eq!(cursor, 19);
    let root = &tree.bundle.nodes[tree.bundle.root.0 as usize];
    let FieldValue::Foreign(guest) = &root.fields[0] else {
        return Err("foreign selector".into());
    };
    assert_eq!(guest.category, "Selector");
    let child = &guest.bundle.nodes[guest.root.0 as usize];
    assert_eq!(
        guest.bundle.tokens[child.token.ok_or("guest token")?.0 as usize].payload,
        NdfValue::List(vec![NdfValue::Unit, NdfValue::Unit])
    );
    let host = tree
        .contexts
        .iter()
        .find(|v| v.path.is_empty())
        .ok_or("host contexts")?;
    let selection = host
        .nodes
        .iter()
        .find(|v| v.node == tree.bundle.root)
        .ok_or("root selection")?;
    let ShapeSelection::Dynamic { child_contexts, .. } = &selection.shape else {
        return Err("dynamic".into());
    };
    assert_eq!(child_contexts[0].alias, "Guest");
    assert_eq!(child_contexts[1].alias, "Host");
    assert_eq!(child_contexts[1].mode, "Alt");
    Ok(())
}

fn provide_selector(
    call: &nepl3_reader::model::ProviderCall,
    budget: &mut nepl3_core::budget::Budget,
    admission: &mut SourceAdmission,
) -> Result<nepl3_reader::runtime::ProviderReply, ParseError> {
    use nepl3_core::budget::Resource;
    let nepl3_reader::model::ProviderCall::Read {
        request,
        operation,
        depth_base,
        ..
    } = call
    else {
        return Err(ParseError::Context);
    };
    if operation.name != "read" {
        return Err(ParseError::Context);
    }
    assert!(budget.current_depth() >= *depth_base);
    let source = request
        .sources
        .iter()
        .find(|v| v.reference() == request.snapshot)
        .ok_or(ParseError::Reference)?;
    admission.admit_existing(source, budget)?;
    let end = request.start.checked_add(3).ok_or(ParseError::Reference)?;
    if end > request.limit || source.slice_range(request.start, end)? != "alt" {
        return Err(ParseError::Context);
    }
    budget.charge(Resource::Work, 3)?;
    budget.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<NdfValue>() as u64,
    )?;
    Ok(nepl3_reader::runtime::ProviderReply::Read(Box::new(
        nepl3_reader::model::ReadReply::Matched {
            value: NdfValue::List(vec![NdfValue::Unit, NdfValue::Unit]),
            end,
            new_state: NdfValue::Unit,
            view: nepl3_core::view::ViewBundle {
                elements: vec![],
                roots: vec![],
            },
            facts: vec![],
            sources: vec![],
            source_maps: vec![],
            report: nepl3_core::diagnostic::Report {
                usage: budget.usage(),
                ..nepl3_core::diagnostic::Report::default()
            },
        },
    )))
}
#[test]
fn synchronous_head_reader_head_dispatch_matches_owned_interleaving() -> TestResult {
    let input = "choose alt @let z x tail";
    let owned = run_case(input, Case::MixedOwned, false)?;
    let native = run_case(input, Case::MixedNative, false)?;
    assert_eq!(owned.outcome, native.outcome);
    assert_eq!(owned.sources, native.sources);
    assert_eq!(owned.source_maps, native.source_maps);
    assert_eq!(owned.report.diagnostics, native.report.diagnostics);
    assert_eq!(owned.report.usage.depth, native.report.usage.depth);
    Ok(())
}

#[test]
fn shape_provider_failure_keeps_unknown_arity_unparsed_and_primary_once() -> TestResult {
    let reply = run_case("choose alt @let z x tail", Case::ShapeFail, false)?;
    let ParseOutcome::Recovered { tree, cursor, .. } = reply.outcome else {
        return Err("shape failure recovery".into());
    };
    let root = &tree.bundle.nodes[tree.bundle.root.0 as usize];
    assert_eq!(root.kind, "RecoveryUnparsed");
    assert!(root.fields.is_empty());
    assert_eq!(cursor, 24);
    assert_eq!(
        reply
            .report
            .diagnostics
            .iter()
            .filter(|v| v.code == "shape-failure")
            .count(),
        1
    );
    assert_eq!(
        reply.report.usage.diagnostics,
        reply.report.diagnostics.len() as u64
    );
    Ok(())
}

#[test]
fn deep_failed_payload_respects_saved_call_depth_and_keeps_prior_report() -> TestResult {
    let result = run_case("choose alt @let z x tail", Case::DeepFailure, false)?;
    assert!(matches!(
        result.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::DepthLimit,
            ..
        }
    ));
    assert_eq!(result.report.diagnostics.len(), 1);
    assert_eq!(result.report.events.len(), 1);
    Ok(())
}

#[test]
fn head_resume_source_uri_conflict_and_missing_primary_preserve_the_original_slot() -> TestResult {
    let reply = run_case("choose alt @let z x tail", Case::Sources, false)?;
    assert!(matches!(
        reply.outcome,
        ParseOutcome::Complete { cursor: 19, .. }
    ));
    assert_eq!(reply.report.diagnostics.len(), 1);
    assert_eq!(reply.report.diagnostics[0].code, "accepted-after-retry");
    assert_eq!(reply.report.usage.diagnostics, 1);
    Ok(())
}

#[test]
fn head_trace_overflow_requires_stopped_and_invalid_success_keeps_pending() -> TestResult {
    let reply = run_case("choose alt @let z x tail", Case::Overflow, false)?;
    assert!(matches!(
        reply.outcome,
        ParseOutcome::Complete { cursor: 19, .. }
    ));
    assert!(reply.report.trace_overflow.is_none());
    assert!(reply.report.diagnostics.is_empty());
    let stopped = run_case("choose alt @let z x tail", Case::OverflowStop, false)?;
    assert!(matches!(
        stopped.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::Cancelled,
            ..
        }
    ));
    assert_eq!(
        stopped.report.trace_overflow,
        Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 })
    );
    assert_eq!(stopped.report.diagnostics.len(), 1);
    Ok(())
}

#[test]
fn head_resume_uses_saved_auxiliary_context_and_rejects_caller_conflicts_before_consumption()
-> TestResult {
    let reply = run_case("choose alt @let z x tail", Case::Auxiliary, false)?;
    let ParseOutcome::Complete { tree, cursor, .. } = &reply.outcome else {
        return Err("complete auxiliary parse".into());
    };
    assert_eq!(*cursor, 19);
    assert!(
        tree.bundle
            .sources
            .iter()
            .any(|source| source.identity().source.0 == "head-auxiliary")
    );
    assert_eq!(reply.report.diagnostics.len(), 1);
    assert_eq!(reply.report.usage.diagnostics, 1);
    Ok(())
}

fn unseal(value: ParseCompletion) -> ParseReply {
    match value {
        ParseCompletion::Continue(proof) => proof.into_reply(),
        ParseCompletion::Break(raw) => {
            assert!(!matches!(raw.outcome, ParseOutcome::Complete { .. }));
            raw
        }
    }
}
#[test]
fn completed_head_wrapper_preserves_owned_execution_and_pending_boundaries() -> TestResult {
    let input = "choose alt @let z x tail";
    assert_eq!(
        run_case(input, Case::Owned, true)?,
        run_case(input, Case::Sealed, true)?
    );
    Ok(())
}
