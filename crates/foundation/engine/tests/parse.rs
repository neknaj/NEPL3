#[path = "parse/foreign.rs"]
mod foreign;
#[path = "parse/host.rs"]
mod host;
#[path = "parse/support.rs"]
mod support;
use nepl3_core::{
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{Environment, EnvironmentEntry, FieldValue},
    value::NdfValue,
};
use nepl3_engine::{parse::*, profile::*};
use nepl3_reader::model::ReaderContext;
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
use support::*;
fn run(input: &str, final_input: bool) -> Result<ParseReply, String> {
    run_config(input, final_input, false, None)
}
fn run_config(
    input: &str,
    final_input: bool,
    list: bool,
    cap: Option<u64>,
) -> Result<ParseReply, String> {
    run_options(input, final_input, list, cap, None)
}
fn run_options(
    input: &str,
    final_input: bool,
    list: bool,
    cap: Option<u64>,
    depth: Option<u64>,
) -> Result<ParseReply, String> {
    run_scenario(
        input,
        final_input,
        Scenario {
            list,
            cap,
            depth,
            ..Scenario::default()
        },
    )
}
#[derive(Default)]
struct Scenario {
    sealed: bool,
    work: Option<u64>,
    list: bool,
    cap: Option<u64>,
    depth: Option<u64>,
    text: bool,
    unknown: bool,
    caller_depth: u64,
    provider: bool,
    cancel_await: bool,
    restart: Option<&'static str>,
    native: Option<host::Action>,
}
fn run_scenario(input: &str, final_input: bool, options: Scenario) -> Result<ParseReply, String> {
    let Scenario {
        sealed,
        list,
        cap,
        work,
        depth,
        text,
        unknown,
        caller_depth,
        provider,
        cancel_await,
        restart,
        native,
    } = options;
    let (mut package, registry) = fixture()?;
    if unknown {
        package.leaves.clear();
    }
    if text {
        use nepl3_engine::package::ReadSpec;
        let ReadSpec::Builtin { reader, .. } = &mut package.reads[0] else {
            return Err("builtin fixture".into());
        };
        *reader = nepl3_reader::builtin::BuiltinReader::Text;
    }
    if provider {
        use nepl3_reader::plan::*;
        let signature = ProviderSignature {
            operation: nepl3_core::value::OperationRef {
                schema: package.schema.clone(),
                name: "read".into(),
            },
            kind: ProviderKind::Read,
            value_input: nepl3_core::schema::TypeDescriptor::Unit,
            value_output: nepl3_core::schema::TypeDescriptor::Text,
            pure: true,
            state_type: nepl3_core::schema::TypeDescriptor::Unit,
            continuation_type: nepl3_core::schema::TypeDescriptor::Named(
                nepl3_core::schema::TypeRef {
                    package: "nepl3.reader".into(),
                    revision: 1,
                    name: "ReaderContinuation".into(),
                },
            ),
        };
        package
            .reader
            .expressions
            .push(ReaderExpr::Call(signature.operation.clone()));
        package.reader.providers.push(signature);
        package.reader.rules.push(ReaderRule {
            name: "provided".into(),
            root: ReaderId(0),
            output: nepl3_core::schema::TypeDescriptor::Text,
        });
        package.modes[0].take[0].reader =
            nepl3_reader::tokenizer::TokenReader::Rule("provided".into());
    }
    if depth.is_some() {
        use nepl3_reader::plan::{CharClass, ReaderExpr, ReaderId, ReaderRule};
        package.forms[0].spelling = "f".into();
        if !provider {
            package
                .reader
                .expressions
                .push(ReaderExpr::Scalar(CharClass::Any));
        }
        for i in 0..20 {
            package.reader.expressions.push(ReaderExpr::Capture {
                name: format!("capture{i}"),
                body: ReaderId(i),
            });
        }
        package.reader.rules.push(ReaderRule {
            name: "deep".into(),
            root: ReaderId(20),
            output: nepl3_core::schema::TypeDescriptor::Text,
        });
        package.modes[0].take[0].reader = nepl3_reader::tokenizer::TokenReader::Rule("deep".into());
    }
    if list {
        let kind = |name: &str| -> Result<nepl3_core::value::KindRef, String> {
            Ok(nepl3_core::value::KindRef {
                schema: package.schema.clone(),
                local_kind: registry
                    .kind_id(&package.schema, name)
                    .map_err(|e| format!("{e:?}"))?,
            })
        };
        package.reads.push(nepl3_engine::package::ReadSpec::ListOf {
            element: nepl3_engine::package::ReadSpecId(1),
            cons: kind("List:LocalCons")?,
            nil: kind("List:Nil")?,
        });
        package.forms[0].fields[1].read = nepl3_engine::package::ReadSpecId(2);
    }
    let mut setup = budget();
    let identity = package
        .check(&registry, &mut setup)
        .map_err(|e| format!("{e:?}"))?
        .semantic_identity(&mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let mut profile = ParseProfile {
        id: "parse-fixture".into(),
        languages: vec![LanguageRegistration {
            alias: "Host".into(),
            package: identity,
            default_category: "Expr".into(),
        }],
        schemas: vec![
            package.schema.clone(),
            foundation.clone(),
            registry
                .selected("nepl3.reader", 1)
                .ok_or("reader schema")?
                .clone(),
        ],
        head_providers: vec![],
        category_modes: vec![],
        providers: vec![],
        allowlist: vec![],
        resources: vec![],
        limits: budget().limits(),
    };
    let mut providers = vec![];
    if provider {
        let operation = package.reader.providers[0].operation.clone();
        let digest = nepl3_core::source::Digest::of(b"fixture reader implementation manifest");
        profile.providers.push(ProviderRequirement {
            provider: "test-provider".into(),
            revision: 1,
            implementation_digest: digest,
            operation: operation.clone(),
        });
        profile.allowlist.push(operation.clone());
        providers.push(ProviderImplementation {
            provider: "test-provider".into(),
            revision: 1,
            implementation_digest: digest,
            operations: vec![operation],
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
        .map_err(|e| format!("{e:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("input".into()),
        0,
        "memory:input".into(),
        input.as_bytes().to_vec(),
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut sources = SourceStore::default();
    sources
        .insert(source.clone())
        .map_err(|e| format!("{e:?}"))?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&environment, &foundation, &registry, &mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let raw = ReaderContext {
        schema: package.schema.clone(),
        category: "Expr".into(),
        mode: "Code".into(),
        origins: vec![],
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
    };
    let mut setup_admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut setup_admission)
        .map_err(|e| format!("{e:?}"))?;
    let checked = raw
        .check(&mut codec, &sources, &registry, &mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let environments = ParseEnvironmentSet::prepare(
        &resolved,
        &[EnvironmentInput {
            alias: "Host",
            context: &checked,
        }],
        &sources,
        &mut codec,
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    let entry = resolved
        .entry("Host", None, &mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let mut session = ParseSession::new(
        "parse-operation".into(),
        &resolved,
        &environments,
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut operation = if let Some(cap) = cap {
        let mut limits = budget().limits();
        limits.allocation_units = cap;
        nepl3_core::budget::Budget::new(limits)
    } else {
        budget()
    };
    if let Some(depth) = depth {
        let mut limits = operation.limits();
        limits.depth = depth;
        operation = nepl3_core::budget::Budget::new(limits);
    }
    if let Some(work) = work {
        let mut limits = operation.limits();
        limits.work = work;
        operation = nepl3_core::budget::Budget::new(limits);
    }
    let mut admission = SourceAdmission::default();
    let mut reply = operation
        .with_depth_at_least(caller_depth, |operation| {
            let request = ParseRequest {
                snapshot: &source,
                start: 0,
                limit: input.len() as u64,
                final_input,
                entry: &entry,
                states: &[LanguageReaderState {
                    alias: "Host".into(),
                    state: NdfValue::Unit,
                }],
            };
            if let Some(action) = native {
                let mut host = host::Host {
                    action,
                    calls: 0,
                    minimum_depth: caller_depth + 3,
                };
                let result = session.read_with_host(
                    request,
                    &sources,
                    operation,
                    &mut admission,
                    &mut host,
                )?;
                if matches!(
                    action,
                    host::Action::FailSecond | host::Action::GeneratedFailSecond
                ) {
                    assert_eq!(result.host_error, Some(ParseError::Context));
                    assert!(matches!(result.reply.outcome, ParseOutcome::Await { .. }));
                    assert_eq!(result.reply.report.diagnostics.len(), 1);
                }
                Ok(result.reply)
            } else {
                if sealed {
                    session
                        .read_completed(request, &sources, operation, &mut admission)
                        .map(unseal)
                } else {
                    session.read(request, &sources, operation, &mut admission)
                }
            }
        })
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(operation.current_depth(), 0);
    if native == Some(host::Action::CloseAfterDecline) {
        let ParseOutcome::Await { continuation, .. } = &reply.outcome else {
            return Err("owned fallback".into());
        };
        session.close();
        let terminal = nepl3_reader::runtime::ProviderReply::Read(Box::new(
            nepl3_reader::model::ReadReply::Stopped {
                reason: nepl3_core::budget::StopReason::Cancelled,
                report: nepl3_core::diagnostic::Report::default(),
                sources: vec![],
                source_maps: vec![],
            },
        ));
        assert!(matches!(
            session.resume(
                continuation,
                terminal,
                &sources,
                &mut operation,
                &mut admission
            ),
            Err(ParseError::Closed)
        ));
        return Ok(reply);
    }
    if let Some(next_input) = restart {
        let ParseOutcome::NeedMore { progress, .. } = &reply.outcome else {
            return Err(format!("expected NeedMore: {reply:?}"));
        };
        let before = reply.report.usage;
        let revision = if input == next_input { 0 } else { 1 };
        let next = SourceSnapshot::new(
            SourceId("input".into()),
            revision,
            "memory:input".into(),
            next_input.as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut next_store = SourceStore::default();
        for source in sources.snapshots() {
            next_store
                .insert(source.clone())
                .map_err(|e| format!("{e:?}"))?;
        }
        next_store
            .insert(next.clone())
            .map_err(|e| format!("{e:?}"))?;
        let states = [LanguageReaderState {
            alias: "Host".into(),
            state: NdfValue::Unit,
        }];
        let request = || ParseRequest {
            snapshot: &next,
            start: 0,
            limit: next_input.len() as u64,
            final_input: true,
            entry: &entry,
            states: &states,
        };
        let mut fresh = nepl3_core::budget::Budget::new(operation.limits());
        fresh.cancel();
        assert!(matches!(
            session.continue_input(progress, request(), &next_store, &mut fresh, &mut admission),
            Err(ParseError::Continuation)
        ));
        let mut altered = progress.clone();
        altered.cursor += 1;
        assert!(matches!(
            session.continue_input(
                &altered,
                request(),
                &next_store,
                &mut operation,
                &mut admission
            ),
            Err(ParseError::Continuation)
        ));
        if !input.is_empty() {
            let changed = SourceSnapshot::new(
                SourceId("input".into()),
                revision + 1,
                "memory:input".into(),
                format!("!{next_input}").into_bytes(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let mut changed_request = request();
            changed_request.snapshot = &changed;
            changed_request.limit = changed.text().len() as u64;
            assert!(matches!(
                session.continue_input(
                    progress,
                    changed_request,
                    &next_store,
                    &mut operation,
                    &mut admission
                ),
                Err(ParseError::Continuation)
            ));
        }
        let resumed = session
            .continue_input(
                progress,
                request(),
                &next_store,
                &mut operation,
                &mut admission,
            )
            .map_err(|e| format!("continue: {e:?}"))?;
        assert!(matches!(
            session.continue_input(
                progress,
                request(),
                &next_store,
                &mut operation,
                &mut admission
            ),
            Err(ParseError::NoPending)
        ));
        assert!(resumed.report.usage.work > before.work);
        assert_eq!(
            resumed.report.usage.source_bytes,
            if revision == 0 {
                input.len() as u64
            } else {
                (input.len() + next_input.len()) as u64
            }
        );
        reply = resumed;
    }
    let mut count = usize::from(matches!(
        native,
        Some(
            host::Action::DeclineSecond
                | host::Action::FailSecond
                | host::Action::GeneratedFailSecond
        )
    ));
    loop {
        if provider {
            while let ParseOutcome::Await { call, continuation } = &reply.outcome {
                count += 1;
                let nepl3_reader::model::ProviderCall::Read {
                    request,
                    depth_base,
                    ..
                } = call.as_ref()
                else {
                    return Err("read call".into());
                };
                assert!(*depth_base >= caller_depth + 3);
                let mut fresh = nepl3_core::budget::Budget::new(operation.limits());
                fresh.cancel();
                let empty = nepl3_reader::runtime::ProviderReply::Read(Box::new(
                    nepl3_reader::model::ReadReply::Stopped {
                        reason: nepl3_core::budget::StopReason::Cancelled,
                        report: nepl3_core::diagnostic::Report::default(),
                        sources: vec![],
                        source_maps: vec![],
                    },
                ));
                assert!(matches!(
                    session.resume(continuation, empty, &sources, &mut fresh, &mut admission),
                    Err(ParseError::Continuation)
                ));
                if cancel_await && count == 2 {
                    operation.cancel();
                    let terminal = nepl3_reader::runtime::ProviderReply::Read(Box::new(
                        nepl3_reader::model::ReadReply::Stopped {
                            reason: nepl3_core::budget::StopReason::Cancelled,
                            report: nepl3_core::diagnostic::Report::default(),
                            sources: vec![],
                            source_maps: vec![],
                        },
                    ));
                    let stopped = session
                        .resume(
                            continuation,
                            terminal,
                            &sources,
                            &mut operation,
                            &mut admission,
                        )
                        .map_err(|e| format!("cancel: {e:?}"))?;
                    reply = stopped;
                    break;
                }
                let start = request.start as usize;
                let end = input[start..].find(' ').map_or(input.len(), |i| start + i);
                let value = &input[start..end];
                let terminal = operation
                    .with_depth_at_least(
                        *depth_base,
                        |b| -> Result<_, nepl3_core::budget::StopReason> {
                            b.charge(nepl3_core::budget::Resource::Work, value.len() as u64)?;
                            b.charge(
                                nepl3_core::budget::Resource::AllocationUnits,
                                value.len() as u64,
                            )?;
                            b.charge(nepl3_core::budget::Resource::Diagnostics, 1)?;
                            let diagnostic = nepl3_core::diagnostic::Diagnostic {
                                schema: package.schema.clone(),
                                code: "FixtureRead".into(),
                                stage: "reader".into(),
                                severity: nepl3_core::diagnostic::Severity::Information,
                                arguments: nepl3_core::value::TypedValue::Record(
                                    nepl3_core::value::Record {
                                        schema: package.schema.clone(),
                                        kind: "List:Nil".into(),
                                        fields: vec![],
                                    },
                                ),
                                primary: None,
                                related: vec![],
                                fixes: vec![],
                            };
                            Ok(nepl3_reader::runtime::ProviderReply::Read(Box::new(
                                nepl3_reader::model::ReadReply::Matched {
                                    value: NdfValue::Text(value.into()),
                                    end: end as u64,
                                    new_state: NdfValue::Unit,
                                    view: nepl3_core::view::ViewBundle {
                                        elements: vec![],
                                        roots: vec![],
                                    },
                                    facts: vec![],
                                    sources: vec![],
                                    source_maps: vec![],
                                    report: nepl3_core::diagnostic::Report {
                                        diagnostics: vec![diagnostic],
                                        usage: b.usage(),
                                        ..nepl3_core::diagnostic::Report::default()
                                    },
                                },
                            )))
                        },
                    )
                    .map_err(|e| format!("host: {e:?}"))?;
                let resumed = operation
                    .with_depth_at_least(caller_depth, |b| {
                        if sealed {
                            session
                                .resume_completed(
                                    continuation,
                                    terminal,
                                    &sources,
                                    b,
                                    &mut admission,
                                )
                                .map(unseal)
                        } else {
                            session.resume(continuation, terminal, &sources, b, &mut admission)
                        }
                    })
                    .map_err(|e| format!("resume: {e:?}"))?;
                reply = resumed;
                assert_eq!(operation.current_depth(), 0);
            }
        }
        if text && matches!(reply.outcome, ParseOutcome::Reserve { .. }) {
            let ParseOutcome::Reserve {
                ref continuation, ..
            } = reply.outcome
            else {
                return Err(format!("expected reservation: {reply:?}"));
            };
            let reservation = nepl3_core::source::SourceReservation {
                source_id: SourceId("decoded".into()),
                revision: 0,
                uri: "memory:decoded".into(),
            };
            let mut fresh = nepl3_core::budget::Budget::new(operation.limits());
            fresh.cancel();
            assert!(matches!(
                session.reserve(
                    continuation,
                    &reservation,
                    &sources,
                    &mut fresh,
                    &mut admission
                ),
                Err(ParseError::Continuation)
            ));
            let mut altered = continuation.clone();
            altered.progress.frames[0].arity += 1;
            assert!(matches!(
                session.reserve(
                    &altered,
                    &reservation,
                    &sources,
                    &mut operation,
                    &mut admission
                ),
                Err(ParseError::Continuation)
            ));
            let resumed = operation
                .with_depth_at_least(caller_depth, |operation| {
                    if sealed {
                        session
                            .reserve_completed(
                                continuation,
                                &reservation,
                                &sources,
                                operation,
                                &mut admission,
                            )
                            .map(unseal)
                    } else {
                        session.reserve(
                            continuation,
                            &reservation,
                            &sources,
                            operation,
                            &mut admission,
                        )
                    }
                })
                .map_err(|e| format!("reserve: {e:?}"))?;
            assert_eq!(operation.current_depth(), 0);
            let stale = session.reserve(
                continuation,
                &reservation,
                &sources,
                &mut operation,
                &mut admission,
            );
            if matches!(
                resumed.outcome,
                ParseOutcome::Await { .. } | ParseOutcome::Reserve { .. }
            ) {
                assert!(matches!(stale, Err(ParseError::Continuation)));
            } else {
                assert!(matches!(stale, Err(ParseError::NoPending)));
            }
            reply = resumed;
            continue;
        }
        break;
    }

    if let ParseOutcome::Complete { tree, .. } | ParseOutcome::Recovered { tree, .. } =
        &reply.outcome
    {
        let proof = tree
            .validate(&resolved, &mut budget(), &mut SourceAdmission::default())
            .map_err(|e| format!("tree: {e:?}"))?;
        let printed = print::source_tree(&proof, &mut budget(), &mut SourceAdmission::default());
        if matches!(reply.outcome, ParseOutcome::Recovered { .. }) {
            assert!(matches!(printed, Err(print::PrintError::Recovered)));
        } else {
            let mut denied = nepl3_core::budget::Budget::new(nepl3_core::budget::Limits {
                source_bytes: 0,
                ..budget().limits()
            });
            assert!(matches!(
                print::source_tree(&proof, &mut denied, &mut SourceAdmission::default())
                    .map_err(|e| format!("{e:?}"))?
                    .outcome,
                print::PrintOutcome::Stopped {
                    reason: nepl3_core::budget::StopReason::SourceLimit,
                    ..
                }
            ));
            let mut shared = budget();
            let mut source_admission = SourceAdmission::default();
            for source in &tree.bundle.sources {
                source_admission
                    .admit_existing(source, &mut shared)
                    .map_err(|e| format!("{e:?}"))?;
            }
            let before = shared.usage().source_bytes;
            print::source_tree(&proof, &mut shared, &mut source_admission)
                .map_err(|e| format!("{e:?}"))?;
            assert_eq!(shared.usage().source_bytes, before);
            let print::PrintOutcome::Complete(output) =
                printed.map_err(|e| format!("print: {e:?}"))?.outcome
            else {
                return Err("unexpected print stop".into());
            };
            let root = &tree.bundle.nodes[tree.bundle.root.0 as usize];
            let cover = root.cover.as_ref().ok_or("root cover")?;
            let current = tree
                .bundle
                .sources
                .iter()
                .find(|v| v.identity() == cover.snapshot_ref())
                .ok_or("print source")?;
            assert_eq!(
                output,
                current
                    .slice_range(0, cover.end())
                    .map_err(|e| format!("{e:?}"))?
            );
            let mut limited = nepl3_core::budget::Budget::new(nepl3_core::budget::Limits {
                output_bytes: 0,
                ..budget().limits()
            });
            assert!(matches!(
                print::source_tree(&proof, &mut limited, &mut SourceAdmission::default())
                    .map_err(|e| format!("{e:?}"))?
                    .outcome,
                print::PrintOutcome::Stopped {
                    reason: nepl3_core::budget::StopReason::OutputLimit,
                    ..
                }
            ));
            if !provider && !text {
                let printed_source = SourceSnapshot::new(
                    SourceId("printed".into()),
                    0,
                    "memory:printed".into(),
                    output.into_bytes(),
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?;
                let mut printed_store = SourceStore::default();
                printed_store
                    .insert(printed_source.clone())
                    .map_err(|e| format!("{e:?}"))?;
                let mut parser = ParseSession::new(
                    "printed-session".into(),
                    &resolved,
                    &environments,
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?;
                let reparsed = parser
                    .read(
                        ParseRequest {
                            snapshot: &printed_source,
                            start: 0,
                            limit: printed_source.text().len() as u64,
                            final_input: true,
                            entry: &entry,
                            states: &[LanguageReaderState {
                                alias: "Host".into(),
                                state: NdfValue::Unit,
                            }],
                        },
                        &printed_store,
                        &mut budget(),
                        &mut SourceAdmission::default(),
                    )
                    .map_err(|e| format!("reparse: {e:?}"))?;
                let ParseOutcome::Complete { tree: again, .. } = reparsed.outcome else {
                    return Err("print reparse not complete".into());
                };
                assert_eq!(
                    tree.bundle
                        .nodes
                        .iter()
                        .map(|n| (&n.kind, &n.fields))
                        .collect::<Vec<_>>(),
                    again
                        .bundle
                        .nodes
                        .iter()
                        .map(|n| (&n.kind, &n.fields))
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    tree.bundle
                        .tokens
                        .iter()
                        .map(|t| &t.payload)
                        .collect::<Vec<_>>(),
                    again
                        .bundle
                        .tokens
                        .iter()
                        .map(|t| &t.payload)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    Ok(reply)
}
#[test]
fn static_form_keeps_builtin_children_and_token_payloads() -> TestResult {
    // Let has exactly two declared slots: Name and local Expr. The suffix is host-owned.
    let reply = run("let x y trailing", true)?;
    let ParseOutcome::Complete { tree, cursor, .. } = reply.outcome else {
        return Err(format!("{reply:?}").into());
    };
    assert_eq!(cursor, 7);
    assert_eq!(tree.bundle.nodes.len(), 3);
    assert_eq!(tree.bundle.tokens.len(), 3);
    let root = &tree.bundle.nodes[0];
    assert_eq!(root.kind, "Form:Let");
    assert_eq!(
        root.fields,
        vec![
            FieldValue::Child(nepl3_core::syntax::NodeRef(1)),
            FieldValue::Child(nepl3_core::syntax::NodeRef(2))
        ]
    );
    assert_eq!(tree.bundle.nodes[1].kind, "Builtin:Name");
    assert!(tree.bundle.nodes[1].fields.is_empty());
    assert_eq!(tree.bundle.tokens[1].payload, NdfValue::Text("x".into()));
    assert_eq!(tree.bundle.tokens[2].payload, NdfValue::Text("y".into()));
    assert_eq!(tree.bundle.tokens[2].head.start(), 6);
    assert_eq!(tree.bundle.tokens[2].leading_trivia.len(), 1);
    assert_eq!(reply.report.usage.source_bytes, 16);
    Ok(())
}
#[test]
fn missing_final_child_and_nonfinal_input_are_distinct() -> TestResult {
    assert!(matches!(
        run("let x", false)?.outcome,
        ParseOutcome::NeedMore { .. }
    ));
    let reply = run("let x", true)?;
    let ParseOutcome::Recovered { tree, .. } = reply.outcome else {
        return Err(format!("{reply:?}").into());
    };
    assert_eq!(reply.report.diagnostics.len(), 1);
    assert_eq!(reply.report.diagnostics[0].code, "MissingChild");
    assert_eq!(tree.recovery.len(), 1);
    assert_eq!(tree.recovery[0].entries.len(), 1);
    assert!(matches!(
        tree.recovery[0].entries[0].kind,
        nepl3_engine::recovery::RecoveryKind::Missing { .. }
    ));
    Ok(())
}

#[test]
fn missing_children_share_one_bundle_recovery_table() -> TestResult {
    for (input, count) in [("let", 2), ("let x let", 2)] {
        let reply = run(input, true)?;
        let ParseOutcome::Recovered { tree, .. } = reply.outcome else {
            return Err(format!("{reply:?}").into());
        };
        assert_eq!(tree.recovery.len(), 1);
        assert_eq!(tree.recovery[0].entries.len(), count);
        assert_eq!(reply.report.diagnostics.len(), count);
    }
    Ok(())
}
#[test]
fn explicit_list_spine_preserves_declared_head_and_tail_children() -> TestResult {
    let reply = run_config("let x cons y cons z nil suffix", true, true, None)?;
    let ParseOutcome::Complete { tree, cursor, .. } = reply.outcome else {
        return Err(format!("{reply:?}").into());
    };
    assert_eq!(cursor, 23);
    assert_eq!(tree.bundle.nodes.len(), 7);
    assert_eq!(tree.bundle.nodes[2].kind, "List:LocalCons");
    assert_eq!(tree.bundle.nodes[4].kind, "List:LocalCons");
    assert_eq!(tree.bundle.nodes[6].kind, "List:Nil");
    assert_eq!(
        tree.bundle.nodes[2].fields,
        vec![
            FieldValue::Child(nepl3_core::syntax::NodeRef(3)),
            FieldValue::Child(nepl3_core::syntax::NodeRef(4))
        ]
    );
    Ok(())
}
#[test]
fn allocation_stop_preserves_actual_usage_and_never_invents_a_root() -> TestResult {
    let reply = run_config("let x y", true, false, Some(0))?;
    assert!(matches!(
        reply.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::AllocationLimit,
            progress: None
        }
    ));
    assert_eq!(reply.report.usage.allocation_units, 0);
    let mut retained_diagnostic = false;
    for cap in (500..80_000).step_by(500) {
        let reply = run_config("let", true, false, Some(cap))?;
        if let ParseOutcome::Stopped { progress, .. } = &reply.outcome {
            assert!(reply.report.usage.allocation_units <= cap);
            if !reply.report.diagnostics.is_empty() {
                retained_diagnostic = true;
                assert!(progress.is_some());
                for diagnostic in &reply.report.diagnostics {
                    let span = diagnostic.primary.as_ref().ok_or("missing primary")?;
                    assert!(
                        reply
                            .sources
                            .iter()
                            .any(|source| source.identity() == span.snapshot_ref())
                    );
                }
            }
        }
    }
    assert!(retained_diagnostic);
    Ok(())
}

#[test]
fn prefix_depth_composes_with_nested_reader_frames() -> TestResult {
    let plain = run_options("y", true, false, None, Some(25))?;
    assert!(matches!(plain.outcome, ParseOutcome::Complete { .. }));
    // Fifteen still-open f(Name, Expr) frames coexist with twenty Capture frames.
    let input = format!("{}y", "f x ".repeat(15));
    let stopped = run_options(&input, true, false, None, Some(25))?;
    assert!(matches!(
        stopped.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::DepthLimit,
            progress: Some(_)
        }
    ));
    let completed = run_options(&input, true, false, None, Some(80))?;
    assert!(matches!(completed.outcome, ParseOutcome::Complete { .. }));
    assert!(completed.report.usage.depth >= 36);
    Ok(())
}

#[test]
fn accepted_trivia_without_a_token_survives_eof_and_need_more() -> TestResult {
    for (input, final_input, expected) in [
        ("let ", true, vec![(3, 4)]),
        ("let x # comment", true, vec![(3, 4), (5, 6), (6, 15)]),
        ("let x # comment", false, vec![(3, 4), (5, 6)]),
    ] {
        let reply = run(input, final_input)?;
        let batches = match &reply.outcome {
            ParseOutcome::Recovered { facts, .. } => facts,
            ParseOutcome::NeedMore { progress, .. } => &progress.facts,
            _ => return Err(format!("{reply:?}").into()),
        };
        let ranges: Vec<_> = batches
            .iter()
            .flat_map(|b| b.trivia.iter())
            .map(|v| (v.span.start(), v.span.end()))
            .collect();
        assert_eq!(ranges, expected);
        assert!(
            batches
                .iter()
                .all(|b| b.path.is_empty() && b.entry.alias == "Host")
        );
    }
    Ok(())
}

#[test]
fn text_reservation_preserves_prefix_identity_and_composed_caller_depth() -> TestResult {
    let reply = run_scenario(
        "let \"x\\n\" y",
        true,
        Scenario {
            text: true,
            caller_depth: 3,
            ..Scenario::default()
        },
    )?;
    let ParseOutcome::Complete { tree, .. } = reply.outcome else {
        return Err(format!("{reply:?}").into());
    };
    assert_eq!(tree.bundle.tokens[1].payload, NdfValue::Text("x\n".into()));
    assert!(!tree.bundle.source_maps.is_empty());
    assert_eq!(tree.bundle.sources.len(), 2);
    assert!(reply.report.usage.depth >= 6);
    Ok(())
}
#[test]
fn unknown_heads_preserve_remainder_without_inventing_a_leaf_arity() -> TestResult {
    for input in ["unknown", "unknown tail", "let x unknown tail"] {
        let reply = run_scenario(
            input,
            true,
            Scenario {
                unknown: true,
                ..Scenario::default()
            },
        )?;
        let ParseOutcome::Recovered { tree, cursor, .. } = reply.outcome else {
            return Err(format!("{reply:?}").into());
        };
        assert_eq!(cursor, input.len() as u64);
        assert_eq!(tree.recovery[0].entries.len(), 1);
        assert!(matches!(
            tree.recovery[0].entries[0].kind,
            nepl3_engine::recovery::RecoveryKind::Unparsed { .. }
        ));
        assert_eq!(reply.report.diagnostics[0].code, "UnparsedInput");
    }
    Ok(())
}

#[test]
fn provider_resume_keeps_prefix_frames_report_and_caller_depth() -> TestResult {
    let reply = run_scenario(
        "let x y",
        true,
        Scenario {
            provider: true,
            caller_depth: 3,
            ..Scenario::default()
        },
    )?;
    assert!(matches!(reply.outcome, ParseOutcome::Complete { .. }));
    assert_eq!(reply.report.diagnostics.len(), 2);
    assert_eq!(reply.report.usage.diagnostics, 2);
    let reply = run_scenario(
        "let x y",
        true,
        Scenario {
            provider: true,
            cancel_await: true,
            caller_depth: 3,
            ..Scenario::default()
        },
    )?;
    let ParseOutcome::Stopped {
        reason: nepl3_core::budget::StopReason::Cancelled,
        progress: Some(progress),
    } = &reply.outcome
    else {
        return Err(format!("{reply:?}").into());
    };
    assert_eq!(progress.frames[0].arity, 2);
    assert_eq!(progress.frames[0].children.len(), 1);
    assert_eq!(reply.report.diagnostics.len(), 1);
    assert_eq!(reply.report.usage.diagnostics, 1);
    assert!(
        progress
            .facts
            .iter()
            .flat_map(|v| &v.trivia)
            .any(|v| v.span.start() == 5 && v.span.end() == 6)
    );
    Ok(())
}

#[test]
fn append_retry_reparses_every_scalar_split_and_keeps_cumulative_usage() -> TestResult {
    const COMPLETE: &str = "let \u{65e5}\u{672c} \u{540d}\u{524d}";
    for end in (0..=COMPLETE.len()).filter(|i| COMPLETE.is_char_boundary(*i)) {
        let reply = run_scenario(
            &COMPLETE[..end],
            false,
            Scenario {
                restart: Some(COMPLETE),
                ..Scenario::default()
            },
        )?;
        let ParseOutcome::Complete { tree, cursor, .. } = reply.outcome else {
            return Err(format!("{reply:?}").into());
        };
        assert_eq!(cursor, COMPLETE.len() as u64);
        assert_eq!(
            tree.bundle.tokens[1].payload,
            NdfValue::Text("\u{65e5}\u{672c}".into())
        );
        assert_eq!(
            tree.bundle.tokens[2].payload,
            NdfValue::Text("\u{540d}\u{524d}".into())
        );
        assert!(
            tree.bundle
                .nodes
                .iter()
                .filter_map(|v| v.head.as_ref())
                .all(|v| v.snapshot_ref().revision == if end == COMPLETE.len() { 0 } else { 1 })
        );
    }
    Ok(())
}

#[test]
fn append_retry_retains_original_caller_depth_when_host_resumes_at_zero() -> TestResult {
    const FULL: &str = "f x f x f x f x f x y";
    let direct = run_scenario(
        FULL,
        true,
        Scenario {
            depth: Some(80),
            caller_depth: 7,
            ..Scenario::default()
        },
    )?;
    let resumed = run_scenario(
        "f x ",
        false,
        Scenario {
            restart: Some(FULL),
            depth: Some(80),
            caller_depth: 7,
            ..Scenario::default()
        },
    )?;
    assert!(matches!(resumed.outcome, ParseOutcome::Complete { .. }));
    assert_eq!(resumed.report.usage.depth, direct.report.usage.depth);
    Ok(())
}

#[test]
fn native_host_preserves_tree_reports_maps_and_owned_fallback() -> TestResult {
    let owned = run_scenario(
        "let x y tail",
        true,
        Scenario {
            provider: true,
            caller_depth: 7,
            ..Scenario::default()
        },
    )?;
    for action in [
        host::Action::Serve,
        host::Action::DeclineSecond,
        host::Action::FailSecond,
    ] {
        let native = run_scenario(
            "let x y tail",
            true,
            Scenario {
                provider: true,
                caller_depth: 7,
                native: Some(action),
                ..Scenario::default()
            },
        )?;
        // Two provider heads, unchanged fixed arity, and the next host token remains unread.
        assert_eq!(native.outcome, owned.outcome);
        assert_eq!(native.report.diagnostics, owned.report.diagnostics);
        assert_eq!(native.report.events, owned.report.events);
        assert_eq!(native.sources, owned.sources);
        assert_eq!(native.source_maps, owned.source_maps);
        assert_eq!(native.report.usage.depth, owned.report.usage.depth);
        if action == host::Action::Serve {
            assert!(native.report.usage.allocation_units < owned.report.usage.allocation_units);
        }
    }
    let owned = run_scenario(
        "let \"x\\n\" y tail",
        true,
        Scenario {
            text: true,
            caller_depth: 7,
            ..Scenario::default()
        },
    )?;
    let native = run_scenario(
        "let \"x\\n\" y tail",
        true,
        Scenario {
            text: true,
            caller_depth: 7,
            native: Some(host::Action::Serve),
            ..Scenario::default()
        },
    )?;
    assert_eq!(native.outcome, owned.outcome);
    assert_eq!(native.sources, owned.sources);
    assert_eq!(native.source_maps, owned.source_maps);
    Ok(())
}
#[test]
fn native_host_cancel_and_invalid_reply_keep_boundary_semantics() -> TestResult {
    let stopped = run_scenario(
        "let x y",
        true,
        Scenario {
            provider: true,
            caller_depth: 7,
            native: Some(host::Action::CancelSecond),
            ..Scenario::default()
        },
    )?;
    assert!(matches!(
        stopped.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::Cancelled,
            ..
        }
    ));
    assert_eq!(stopped.report.diagnostics.len(), 1);
    assert_eq!(stopped.report.usage.diagnostics, 1);
    assert!(
        run_scenario(
            "let x y",
            true,
            Scenario {
                provider: true,
                native: Some(host::Action::WrongSecond),
                ..Scenario::default()
            }
        )
        .is_err()
    );
    // Derive boundary probes from the successful operation rather than assume
    // that a previously insufficient fixed cap must remain insufficient.
    let complete = run_scenario(
        "let x y",
        true,
        Scenario {
            provider: true,
            native: Some(host::Action::Serve),
            ..Scenario::default()
        },
    )?;
    assert!(matches!(complete.outcome, ParseOutcome::Complete { .. }));
    let needed = complete.report.usage.allocation_units;
    for cap in [0, needed / 4, needed / 2, needed - 1] {
        let stopped = run_scenario(
            "let x y",
            true,
            Scenario {
                provider: true,
                cap: Some(cap),
                native: Some(host::Action::Serve),
                ..Scenario::default()
            },
        )?;
        assert!(matches!(
            stopped.outcome,
            ParseOutcome::Stopped {
                reason: nepl3_core::budget::StopReason::AllocationLimit,
                ..
            }
        ));
        assert!(stopped.report.usage.allocation_units <= cap);
    }
    let needed_work = complete.report.usage.work;
    for work in [0, needed_work / 4, needed_work / 2, needed_work - 1] {
        let reply = run_scenario(
            "let x y",
            true,
            Scenario {
                provider: true,
                work: Some(work),
                native: Some(host::Action::Serve),
                ..Scenario::default()
            },
        )?;
        assert!(matches!(
            reply.outcome,
            ParseOutcome::Stopped {
                reason: nepl3_core::budget::StopReason::WorkLimit,
                ..
            }
        ));
        assert!(reply.report.usage.work <= work);
    }
    let closed = run_scenario(
        "let x y",
        true,
        Scenario {
            provider: true,
            native: Some(host::Action::CloseAfterDecline),
            ..Scenario::default()
        },
    )?;
    assert_eq!(closed.report.diagnostics.len(), 1);
    let mixed_cancel = run_scenario(
        "let x y",
        true,
        Scenario {
            provider: true,
            cancel_await: true,
            native: Some(host::Action::DeclineSecond),
            ..Scenario::default()
        },
    )?;
    assert!(matches!(
        mixed_cancel.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::Cancelled,
            ..
        }
    ));
    assert_eq!(mixed_cancel.report.diagnostics.len(), 1);
    assert_eq!(mixed_cancel.report.usage.diagnostics, 1);
    let stopped = run_scenario(
        "let x y",
        true,
        Scenario {
            provider: true,
            depth: Some(10),
            caller_depth: 7,
            native: Some(host::Action::Serve),
            ..Scenario::default()
        },
    )?;
    assert!(matches!(
        stopped.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::DepthLimit,
            ..
        }
    ));
    Ok(())
}

#[test]
fn native_host_prefix_growth_reduces_measured_copy_work() -> TestResult {
    for parents in [1, 4, 8] {
        let input = format!("{}y", "let x ".repeat(parents));
        let owned = run_scenario(
            &input,
            true,
            Scenario {
                provider: true,
                ..Scenario::default()
            },
        )?;
        let native = run_scenario(
            &input,
            true,
            Scenario {
                provider: true,
                native: Some(host::Action::Serve),
                ..Scenario::default()
            },
        )?;
        assert_eq!(native.outcome, owned.outcome);
        assert_eq!(native.report.diagnostics, owned.report.diagnostics);
        assert!(native.report.usage.work < owned.report.usage.work);
        assert!(native.report.usage.allocation_units < owned.report.usage.allocation_units);
        eprintln!(
            "parents {parents}: owned {:?}, native {:?}",
            owned.report.usage, native.report.usage
        );
    }
    Ok(())
}

#[test]
fn native_collector_transfer_preserves_generated_sources_at_allocation_boundaries() -> TestResult {
    let input = format!("{}y", "let \"x\\n\" ".repeat(6));
    // The request already declares its primary source. Replies need only own
    // additional sources; check the union, not the reply vector alone.
    let primary = SourceSnapshot::new(
        SourceId("input".into()),
        0,
        "memory:input".into(),
        input.as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let scenario = |cap| Scenario {
        provider: true,
        text: true,
        cap,
        native: Some(host::Action::Serve),
        ..Scenario::default()
    };
    let complete = run_scenario(&input, true, scenario(None))?;
    assert!(matches!(complete.outcome, ParseOutcome::Complete { .. }));
    assert!(complete.sources.len() >= 6);
    assert!(complete.source_maps.len() >= 12);
    let needed = complete.report.usage.allocation_units;
    for cap in (0..16)
        .map(|part| needed * part / 16)
        .chain([needed - 1, needed])
    {
        let result = run_scenario(&input, true, scenario(Some(cap)))?;
        assert!(result.report.usage.allocation_units <= cap);
        if cap < needed {
            assert!(matches!(
                result.outcome,
                ParseOutcome::Stopped {
                    reason: nepl3_core::budget::StopReason::AllocationLimit,
                    ..
                }
            ));
        } else {
            assert_eq!(result.outcome, complete.outcome);
        }
        for span in result
            .source_maps
            .iter()
            .flat_map(|map| [&map.source, &map.target])
            .chain(
                result
                    .report
                    .diagnostics
                    .iter()
                    .filter_map(|d| d.primary.as_ref()),
            )
            .chain(result.report.events.iter().filter_map(|e| e.span.as_ref()))
        {
            assert!(
                result
                    .sources
                    .iter()
                    .chain(core::iter::once(&primary))
                    .any(|source| source.identity() == span.snapshot_ref()),
                "cap={cap}, missing={:?}, sources={:?}",
                span.snapshot_ref(),
                result
                    .sources
                    .iter()
                    .map(|s| s.identity())
                    .collect::<Vec<_>>()
            );
        }
    }
    Ok(())
}

#[test]
fn native_host_failure_keeps_generated_diagnostic_and_event_closure() -> TestResult {
    for action in [
        host::Action::GeneratedFailSecond,
        host::Action::GeneratedCancelSecond,
    ] {
        let reply = run_scenario(
            "let x y",
            true,
            Scenario {
                provider: true,
                native: Some(action),
                ..Scenario::default()
            },
        )?;
        let primary = reply.report.diagnostics[0]
            .primary
            .as_ref()
            .ok_or("generated primary")?;
        assert_eq!(primary.snapshot_ref().source.0, "host-generated");
        assert_eq!(reply.report.events.len(), 1);
        assert_eq!(reply.report.events[0].span.as_ref(), Some(primary));
        assert_eq!(reply.report.usage.events, 1);
        assert!(
            reply
                .sources
                .iter()
                .any(|source| source.identity() == primary.snapshot_ref())
        );
        assert_eq!(
            reply.report.usage.diagnostics,
            if action == host::Action::GeneratedFailSecond {
                2
            } else {
                1
            }
        );
    }
    Ok(())
}

#[test]
fn native_host_mixed_text_provider_fallback_preserves_decoded_sources() -> TestResult {
    let input = "let \"x\\n\" y tail";
    let owned = run_scenario(
        input,
        true,
        Scenario {
            provider: true,
            text: true,
            caller_depth: 7,
            ..Scenario::default()
        },
    )?;
    for action in [
        host::Action::Serve,
        host::Action::DeclineSecond,
        host::Action::FailSecond,
    ] {
        let native = run_scenario(
            input,
            true,
            Scenario {
                provider: true,
                text: true,
                caller_depth: 7,
                native: Some(action),
                ..Scenario::default()
            },
        )?;
        assert_eq!(native.outcome, owned.outcome);
        assert_eq!(native.sources, owned.sources);
        assert_eq!(native.source_maps, owned.source_maps);
        assert_eq!(native.report.diagnostics, owned.report.diagnostics);
        assert_eq!(native.report.usage.depth, owned.report.usage.depth);
    }
    let cancelled = run_scenario(
        input,
        true,
        Scenario {
            provider: true,
            text: true,
            native: Some(host::Action::CancelSecond),
            ..Scenario::default()
        },
    )?;
    assert!(matches!(
        cancelled.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::Cancelled,
            ..
        }
    ));
    assert_eq!(cancelled.report.diagnostics.len(), 1);
    assert_eq!(cancelled.sources.len(), 1);
    assert_eq!(cancelled.source_maps.len(), 2);
    Ok(())
}

fn unseal(value: ParseCompletion) -> ParseReply {
    match value {
        ParseCompletion::Continue(proof) => {
            let cursor = proof.cursor();
            let raw = proof.into_reply();
            assert!(matches!(raw.outcome,ParseOutcome::Complete{cursor:v,..} if v==cursor));
            raw
        }
        ParseCompletion::Break(raw) => {
            assert!(!matches!(raw.outcome, ParseOutcome::Complete { .. }));
            raw
        }
    }
}
#[test]
fn completed_parse_wrappers_preserve_raw_outcomes_reports_and_costs() -> TestResult {
    for (input, final_input, text, provider, work) in [
        ("let x x", true, false, false, None),
        ("let x", true, false, false, None),
        ("let x", false, false, false, None),
        ("x", true, false, false, Some(0)),
        ("let \"x\" \"x\"", true, true, false, None),
        ("let x x", true, false, true, None),
    ] {
        let raw = run_scenario(
            input,
            final_input,
            Scenario {
                text,
                provider,
                work,
                ..Scenario::default()
            },
        )?;
        let sealed = run_scenario(
            input,
            final_input,
            Scenario {
                sealed: true,
                text,
                provider,
                work,
                ..Scenario::default()
            },
        )?;
        assert_eq!(raw, sealed, "{input}");
    }
    Ok(())
}

#[test]
fn native_host_nested_stop_is_sticky_without_host_mutating_budget() -> TestResult {
    let reply = run_scenario(
        "let x y",
        true,
        Scenario {
            provider: true,
            native: Some(host::Action::NestedStopSecond),
            ..Scenario::default()
        },
    )?;
    assert!(matches!(
        reply.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::Cancelled,
            ..
        }
    ));
    assert_eq!(reply.report.diagnostics.len(), 1);
    Ok(())
}
