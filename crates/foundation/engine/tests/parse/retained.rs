use super::*;
use nepl3_engine::package::EntryContext;

pub(super) fn with_context(
    input: &str,
    f: impl FnOnce(
        &ResolvedParseProfile<'_>,
        &ParseEnvironmentSet<'_>,
        &SourceStore,
        &SourceSnapshot,
        &EntryContext,
    ) -> TestResult,
) -> TestResult {
    with_options(input, false, false, f)
}

pub(super) fn with_options(
    input: &str,
    provided: bool,
    text: bool,
    f: impl FnOnce(
        &ResolvedParseProfile<'_>,
        &ParseEnvironmentSet<'_>,
        &SourceStore,
        &SourceSnapshot,
        &EntryContext,
    ) -> TestResult,
) -> TestResult {
    with_options_state(input, provided, text, None, false, f)
}

pub(super) fn with_state_context(
    input: &str,
    state: nepl3_core::schema::TypeDescriptor,
    f: impl FnOnce(
        &ResolvedParseProfile<'_>,
        &ParseEnvironmentSet<'_>,
        &SourceStore,
        &SourceSnapshot,
        &EntryContext,
    ) -> TestResult,
) -> TestResult {
    with_options_state(input, false, false, Some(state), false, f)
}

pub(super) fn with_list_context(
    input: &str,
    f: impl FnOnce(
        &ResolvedParseProfile<'_>,
        &ParseEnvironmentSet<'_>,
        &SourceStore,
        &SourceSnapshot,
        &EntryContext,
    ) -> TestResult,
) -> TestResult {
    with_options_state(input, false, false, None, true, f)
}

fn with_options_state(
    input: &str,
    provided: bool,
    text: bool,
    state: Option<nepl3_core::schema::TypeDescriptor>,
    list: bool,
    f: impl FnOnce(
        &ResolvedParseProfile<'_>,
        &ParseEnvironmentSet<'_>,
        &SourceStore,
        &SourceSnapshot,
        &EntryContext,
    ) -> TestResult,
) -> TestResult {
    let (mut package, registry) = fixture()?;
    if let Some(state) = state {
        package.reader.state_type = state;
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
    if provided {
        use nepl3_core::schema::{TypeDescriptor, TypeRef};
        use nepl3_reader::plan::*;
        package.reader.state_type = TypeDescriptor::Bool;
        let signature = ProviderSignature {
            operation: nepl3_core::value::OperationRef {
                schema: package.schema.clone(),
                name: "read".into(),
            },
            kind: ProviderKind::Read,
            value_input: TypeDescriptor::Unit,
            value_output: TypeDescriptor::Text,
            pure: true,
            state_type: TypeDescriptor::Bool,
            continuation_type: TypeDescriptor::Named(TypeRef {
                package: "nepl3.reader".into(),
                revision: 1,
                name: "ReaderContinuation".into(),
            }),
        };
        package
            .reader
            .expressions
            .push(ReaderExpr::Call(signature.operation.clone()));
        package.reader.providers.push(signature);
        package.reader.rules.push(ReaderRule {
            name: "provided".into(),
            root: ReaderId(0),
            output: TypeDescriptor::Text,
        });
        package.modes[0].take[0].reader =
            nepl3_reader::tokenizer::TokenReader::Rule("provided".into());
    }
    if text {
        let nepl3_engine::package::ReadSpec::Builtin { reader, .. } = &mut package.reads[0] else {
            return Err("builtin fixture".into());
        };
        *reader = nepl3_reader::builtin::BuiltinReader::Text;
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
    if provided {
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
    f(&resolved, &environments, &sources, &source, &entry)
}

#[test]
fn retained_parse_keeps_exact_initial_seed_and_one_start() -> TestResult {
    for (input, recovered) in [("let x x", false), ("let x", true)] {
        with_context(input, |profile, environments, sources, source, entry| {
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Unit,
            }];
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let mut session = RetainedParseSession::new(
                "retained".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start: 0,
                    limit: input.len() as u64,
                    final_input: true,
                    entry,
                    states: &states,
                },
                sources,
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Continue(proof) = session
                .read(&mut b, &mut admission)
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("terminal proof".into());
            };
            assert_eq!(
                proof.execution().kind() == ExecutionKind::Recovered,
                recovered
            );
            assert!(core::ptr::eq(proof.seed().request().snapshot, source));
            assert!(core::ptr::eq(proof.seed().sources(), sources));
            assert!(core::ptr::eq(proof.seed().profile(), profile));
            assert!(core::ptr::eq(proof.seed().environments(), environments));
            assert_eq!(proof.seed().request().states, states);
            assert_eq!(proof.seed().request().start, 0);
            assert_eq!(proof.seed().request().limit, input.len() as u64);
            assert!(proof.seed().request().final_input);
            assert!(core::ptr::eq(proof.seed().request().entry, entry));
            assert!(matches!(
                session.read(&mut b, &mut admission),
                Err(ParseError::Busy)
            ));
            assert!(matches!(
                proof.into_reply().outcome,
                ParseOutcome::Complete { .. } | ParseOutcome::Recovered { .. }
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn retained_append_replaces_seed_only_after_real_execution() -> TestResult {
    with_context("let", |profile, environments, sources, source, entry| {
        let states = [LanguageReaderState {
            alias: "Host".into(),
            state: NdfValue::Unit,
        }];
        let next = SourceSnapshot::new(
            SourceId("input".into()),
            1,
            "memory:input".into(),
            b"let x x".to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut next_sources = SourceStore::default();
        next_sources
            .insert(next.clone())
            .map_err(|e| format!("{e:?}"))?;
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let mut session = RetainedParseSession::new(
            "retained-append".into(),
            profile,
            environments,
            ParseRequest {
                snapshot: source,
                start: 0,
                limit: 3,
                final_input: false,
                entry,
                states: &states,
            },
            sources,
            &mut b,
        )
        .map_err(|e| format!("{e:?}"))?;
        let RetainedParseExecution::Break(ParseReply {
            outcome: ParseOutcome::NeedMore { progress, .. },
            ..
        }) = session
            .read(&mut b, &mut admission)
            .map_err(|e| format!("{e:?}"))?
        else {
            return Err("NeedMore".into());
        };
        let request = || ParseRequest {
            snapshot: &next,
            start: 0,
            limit: 7,
            final_input: true,
            entry,
            states: &states,
        };
        let mut bad = request();
        bad.start = 1;
        assert!(matches!(
            session.continue_input(&progress, bad, &next_sources, &mut b, &mut admission),
            Err(ParseError::Continuation)
        ));
        let RetainedParseExecution::Continue(proof) = session
            .continue_input(&progress, request(), &next_sources, &mut b, &mut admission)
            .map_err(|e| format!("{e:?}"))?
        else {
            return Err("appended proof".into());
        };
        assert_eq!(proof.execution().kind(), ExecutionKind::Complete);
        assert!(core::ptr::eq(proof.seed().request().snapshot, &next));
        assert!(core::ptr::eq(proof.seed().sources(), &next_sources));
        assert_eq!(proof.seed().request().states, states);
        Ok(())
    })
}

#[test]
fn retained_append_stop_and_consumed_failure_cannot_restart() -> TestResult {
    for mode in [0, 1, 2] {
        with_context("let", |profile, environments, sources, source, entry| {
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Unit,
            }];
            let next = SourceSnapshot::new(
                SourceId("input".into()),
                1,
                "memory:input".into(),
                b"let x x".to_vec(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let mut next_sources = SourceStore::default();
            if mode != 2 {
                next_sources
                    .insert(next.clone())
                    .map_err(|e| format!("{e:?}"))?;
            }
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let mut session = RetainedParseSession::new(
                "retained-stop".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start: 0,
                    limit: 3,
                    final_input: false,
                    entry,
                    states: &states,
                },
                sources,
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Break(ParseReply {
                outcome: ParseOutcome::NeedMore { progress, .. },
                ..
            }) = session
                .read(&mut b, &mut admission)
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("NeedMore".into());
            };
            if mode == 0 {
                b.cancel();
            }
            if mode == 1 {
                let remaining = b.limits().work - b.usage().work;
                b.charge(nepl3_core::budget::Resource::Work, remaining - 1)
                    .map_err(|e| format!("{e:?}"))?;
            }
            let request = || ParseRequest {
                snapshot: &next,
                start: 0,
                limit: 7,
                final_input: true,
                entry,
                states: &states,
            };
            let result =
                session.continue_input(&progress, request(), &next_sources, &mut b, &mut admission);
            if mode == 2 {
                assert!(matches!(
                    result,
                    Err(ParseError::Source(
                        nepl3_core::source::SourceError::MissingSnapshot
                    ))
                ));
                assert!(matches!(
                    session.continue_input(
                        &progress,
                        request(),
                        &next_sources,
                        &mut b,
                        &mut admission
                    ),
                    Err(ParseError::NoPending)
                ));
            } else {
                let RetainedParseExecution::Break(reply) = result.map_err(|e| format!("{e:?}"))?
                else {
                    return Err("stop must not produce proof".into());
                };
                assert!(matches!(reply.outcome, ParseOutcome::Stopped { .. }));
                assert_eq!(reply.report.usage, b.usage());
                assert!(matches!(
                    session.continue_input(
                        &progress,
                        request(),
                        &next_sources,
                        &mut budget(),
                        &mut SourceAdmission::default()
                    ),
                    Err(ParseError::Closed)
                ));
            }
            assert!(matches!(
                session.read(&mut budget(), &mut SourceAdmission::default()),
                Err(ParseError::Busy)
            ));
            let mut host = host::Host {
                action: host::Action::Serve,
                calls: 0,
                minimum_depth: 0,
            };
            assert!(matches!(
                session.read_with_host(&mut budget(), &mut SourceAdmission::default(), &mut host),
                Err(ParseError::Busy)
            ));
            assert_eq!(host.calls, 0);
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn retained_host_start_and_failed_start_are_single_use() -> TestResult {
    with_context(
        "let x x",
        |profile, environments, sources, source, entry| {
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Unit,
            }];
            for start in [0, 100] {
                let mut b = budget();
                let mut admission = SourceAdmission::default();
                let mut session = RetainedParseSession::new(
                    "retained-host".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: source,
                        start,
                        limit: 7,
                        final_input: true,
                        entry,
                        states: &states,
                    },
                    sources,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let mut host = host::Host {
                    action: host::Action::Serve,
                    calls: 0,
                    minimum_depth: 0,
                };
                let result = session.read_with_host(&mut b, &mut admission, &mut host);
                if start == 0 {
                    let result = result.map_err(|e| format!("{e:?}"))?;
                    assert!(result.host_error.is_none());
                    assert!(matches!(
                        result.execution,
                        RetainedParseExecution::Continue(_)
                    ));
                } else {
                    assert!(matches!(result, Err(ParseError::Source(_))));
                }
                assert!(matches!(
                    session.read(&mut b, &mut admission),
                    Err(ParseError::Busy)
                ));
            }
            Ok(())
        },
    )
}

pub(super) struct StatefulHost {
    pub(super) inner: host::Host,
}
impl ParseHost for StatefulHost {
    fn provider(
        &mut self,
        call: &nepl3_reader::model::ProviderCall,
        requirement: &ProviderRequirement,
        b: &mut nepl3_core::budget::Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<nepl3_reader::runtime::ProviderReply>, ParseError> {
        let nepl3_reader::model::ProviderCall::Read { request, .. } = call else {
            return Err(ParseError::Context);
        };
        assert_eq!(request.state, NdfValue::Bool(self.inner.calls > 0));
        let mut reply = self.inner.provider(call, requirement, b, admission)?;
        if let Some(nepl3_reader::runtime::ProviderReply::Read(reply)) = &mut reply
            && let nepl3_reader::model::ReadReply::Matched { new_state, .. } = reply.as_mut()
        {
            *new_state = NdfValue::Bool(true);
        }
        Ok(reply)
    }
    fn reservation(
        &mut self,
        request: &nepl3_reader::tokenizer::ReservationRequest,
        b: &mut nepl3_core::budget::Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<nepl3_core::source::SourceReservation>, ParseError> {
        self.inner.reservation(request, b, admission)
    }
}
#[test]
fn retained_seed_keeps_initial_states_after_provider_changes_runtime_state() -> TestResult {
    with_options(
        "let x x",
        true,
        false,
        |profile, environments, sources, source, entry| {
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Bool(false),
            }];
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let mut session = RetainedParseSession::new(
                "retained-state".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start: 0,
                    limit: 7,
                    final_input: true,
                    entry,
                    states: &states,
                },
                sources,
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?;
            let mut host = StatefulHost {
                inner: host::Host {
                    action: host::Action::Serve,
                    calls: 0,
                    minimum_depth: 0,
                },
            };
            let reply = session
                .read_with_host(&mut b, &mut admission, &mut host)
                .map_err(|e| format!("{e:?}"))?;
            assert!(reply.host_error.is_none());
            let RetainedParseExecution::Continue(proof) = reply.execution else {
                return Err("stateful proof".into());
            };
            assert_eq!(proof.seed().request().states, states);
            let ParseOutcome::Complete {
                states: final_states,
                ..
            } = proof.into_reply().outcome
            else {
                return Err("Complete".into());
            };
            assert_eq!(final_states[0].state, NdfValue::Bool(true));
            Ok(())
        },
    )
}

#[test]
fn retained_append_can_suspend_then_resume_using_replacement_store() -> TestResult {
    with_options(
        "let",
        true,
        false,
        |profile, environments, sources, source, entry| {
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Bool(false),
            }];
            let next = SourceSnapshot::new(
                SourceId("input".into()),
                1,
                "memory:input".into(),
                b"let x x".to_vec(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let mut next_sources = SourceStore::default();
            next_sources
                .insert(next.clone())
                .map_err(|e| format!("{e:?}"))?;
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let mut session = RetainedParseSession::new(
                "retained-await".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start: 0,
                    limit: 3,
                    final_input: false,
                    entry,
                    states: &states,
                },
                sources,
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?;
            let host = || StatefulHost {
                inner: host::Host {
                    action: host::Action::Serve,
                    calls: 0,
                    minimum_depth: 0,
                },
            };
            let initial = session
                .read_with_host(&mut b, &mut admission, &mut host())
                .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Break(ParseReply {
                outcome: ParseOutcome::NeedMore { progress, .. },
                ..
            }) = initial.execution
            else {
                return Err("initial NeedMore".into());
            };
            let mut outcome = session
                .continue_input(
                    &progress,
                    ParseRequest {
                        snapshot: &next,
                        start: 0,
                        limit: 7,
                        final_input: true,
                        entry,
                        states: &states,
                    },
                    &next_sources,
                    &mut b,
                    &mut admission,
                )
                .map_err(|e| format!("{e:?}"))?;
            assert!(matches!(
                outcome,
                RetainedParseExecution::Break(ParseReply {
                    outcome: ParseOutcome::Await { .. },
                    ..
                })
            ));
            let mut host = host();
            loop {
                match outcome {
                    RetainedParseExecution::Continue(proof) => {
                        assert_eq!(proof.execution().kind(), ExecutionKind::Complete);
                        assert!(core::ptr::eq(proof.seed().sources(), &next_sources));
                        assert_eq!(
                            proof.seed().request().snapshot.reference(),
                            next.reference()
                        );
                        assert_eq!(proof.seed().request().states, states);
                        break;
                    }
                    RetainedParseExecution::Break(ParseReply {
                        outcome: ParseOutcome::Await { call, continuation },
                        ..
                    }) => {
                        let nepl3_reader::model::ProviderCall::Read { depth_base, .. } =
                            call.as_ref()
                        else {
                            return Err("read call".into());
                        };
                        let reply = b
                            .with_depth_at_least(*depth_base, |b| {
                                host.provider(
                                    &call,
                                    &profile.profile().providers[0],
                                    b,
                                    &mut admission,
                                )?
                                .ok_or(ParseError::Context)
                            })
                            .map_err(|e| format!("{e:?}"))?;
                        outcome = session
                            .resume(&continuation, reply, &mut b, &mut admission)
                            .map_err(|e| format!("{e:?}"))?;
                    }
                    _ => return Err("unexpected retained continuation".into()),
                }
            }
            Ok(())
        },
    )
}

#[test]
fn retained_cancelled_provider_wait_preserves_usage_and_closes() -> TestResult {
    with_options(
        "let x x",
        true,
        false,
        |profile, environments, sources, source, entry| {
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Bool(false),
            }];
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let mut session = RetainedParseSession::new(
                "retained-provider-stop".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start: 0,
                    limit: 7,
                    final_input: true,
                    entry,
                    states: &states,
                },
                sources,
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Break(ParseReply {
                outcome: ParseOutcome::Await { call, continuation },
                ..
            }) = session
                .read(&mut b, &mut admission)
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("Await".into());
            };
            let nepl3_reader::model::ProviderCall::Read { depth_base, .. } = call.as_ref() else {
                return Err("read call".into());
            };
            let reply = b
                .with_depth_at_least(*depth_base, |b| -> Result<_, ParseError> {
                    b.cancel();
                    Ok(nepl3_reader::runtime::ProviderReply::Read(Box::new(
                        nepl3_reader::model::ReadReply::Stopped {
                            reason: nepl3_core::budget::StopReason::Cancelled,
                            report: nepl3_core::diagnostic::Report {
                                usage: b.usage(),
                                ..Default::default()
                            },
                            sources: vec![],
                            source_maps: vec![],
                        },
                    )))
                })
                .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Break(result) = session
                .resume(&continuation, reply, &mut b, &mut admission)
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("Stopped".into());
            };
            assert!(matches!(result.outcome, ParseOutcome::Stopped { .. }));
            assert_eq!(result.report.usage, b.usage());
            let reply = nepl3_reader::runtime::ProviderReply::Read(Box::new(
                nepl3_reader::model::ReadReply::Stopped {
                    reason: nepl3_core::budget::StopReason::Cancelled,
                    report: Default::default(),
                    sources: vec![],
                    source_maps: vec![],
                },
            ));
            assert!(matches!(
                session.resume(&continuation, reply, &mut b, &mut admission),
                Err(ParseError::Closed)
            ));
            Ok(())
        },
    )
}

#[test]
fn retained_reservations_preserve_seed_and_stop_terminally() -> TestResult {
    // Only the declared name field uses Text; the body remains an Expr.
    let input = "let \"x\" x";
    for stop in [false, true] {
        with_options(
            input,
            false,
            true,
            |profile, environments, sources, source, entry| {
                let states = [LanguageReaderState {
                    alias: "Host".into(),
                    state: NdfValue::Unit,
                }];
                let mut b = budget();
                let mut admission = SourceAdmission::default();
                let mut session = RetainedParseSession::new(
                    "retained-reserve".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: source,
                        start: 0,
                        limit: input.len() as u64,
                        final_input: true,
                        entry,
                        states: &states,
                    },
                    sources,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let mut result = session
                    .read(&mut b, &mut admission)
                    .map_err(|e| format!("{e:?}"))?;
                let mut count = 0;
                loop {
                    match result {
                        RetainedParseExecution::Continue(proof) => {
                            assert!(!stop);
                            assert!(count > 0);
                            assert_eq!(proof.execution().kind(), ExecutionKind::Complete);
                            assert!(core::ptr::eq(proof.seed().sources(), sources));
                            assert_eq!(proof.seed().request().states, states);
                            break;
                        }
                        RetainedParseExecution::Break(ParseReply {
                            outcome: ParseOutcome::Reserve { continuation, .. },
                            ..
                        }) => {
                            count += 1;
                            let reservation = nepl3_core::source::SourceReservation {
                                source_id: SourceId(format!("decoded-{count}")),
                                revision: 0,
                                uri: format!("memory:decoded-{count}"),
                            };
                            if stop {
                                b.cancel();
                            }
                            result = session
                                .reserve(&continuation, &reservation, &mut b, &mut admission)
                                .map_err(|e| format!("{e:?}"))?;
                            if stop {
                                assert!(matches!(
                                    result,
                                    RetainedParseExecution::Break(ParseReply {
                                        outcome: ParseOutcome::Stopped { .. },
                                        ..
                                    })
                                ));
                                assert!(matches!(
                                    session.reserve(
                                        &continuation,
                                        &reservation,
                                        &mut b,
                                        &mut admission
                                    ),
                                    Err(ParseError::Closed)
                                ));
                                break;
                            }
                        }
                        _ => return Err("expected reservation".into()),
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn retained_provider_reported_stop_preserves_accepted_diagnostic() -> TestResult {
    with_options(
        "let x x",
        true,
        false,
        |profile, environments, sources, source, entry| {
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Bool(false),
            }];
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let mut session = RetainedParseSession::new(
                "retained-provider-stop".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start: 0,
                    limit: 7,
                    final_input: true,
                    entry,
                    states: &states,
                },
                sources,
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Break(ParseReply {
                outcome: ParseOutcome::Await { call, continuation },
                ..
            }) = session
                .read(&mut b, &mut admission)
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("Await".into());
            };
            let nepl3_reader::model::ProviderCall::Read { depth_base, .. } = call.as_ref() else {
                return Err("read call".into());
            };
            let reply = b
                .with_depth_at_least(*depth_base, |b| -> Result<_, ParseError> {
                    b.charge(nepl3_core::budget::Resource::Diagnostics, 1)?;
                    let nepl3_reader::model::ProviderCall::Read { operation, .. } = call.as_ref()
                    else {
                        return Err(ParseError::Context);
                    };
                    let diagnostic = nepl3_core::diagnostic::Diagnostic {
                        schema: operation.schema.clone(),
                        code: "RetainedProviderStop".into(),
                        stage: "reader".into(),
                        severity: nepl3_core::diagnostic::Severity::Information,
                        arguments: nepl3_core::value::TypedValue::Record(
                            nepl3_core::value::Record {
                                schema: operation.schema.clone(),
                                kind: "List:Nil".into(),
                                fields: vec![],
                            },
                        ),
                        primary: None,
                        related: vec![],
                        fixes: vec![],
                    };
                    Ok(nepl3_reader::runtime::ProviderReply::Read(Box::new(
                        nepl3_reader::model::ReadReply::Stopped {
                            reason: nepl3_core::budget::StopReason::Cancelled,
                            report: nepl3_core::diagnostic::Report {
                                usage: b.usage(),
                                diagnostics: vec![diagnostic],
                                ..Default::default()
                            },
                            sources: vec![],
                            source_maps: vec![],
                        },
                    )))
                })
                .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Break(result) = session
                .resume(&continuation, reply, &mut b, &mut admission)
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("Stopped".into());
            };
            assert!(matches!(result.outcome, ParseOutcome::Stopped { .. }));
            assert_eq!(result.report.usage, b.usage());
            assert!(
                result
                    .report
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "RetainedProviderStop")
            );
            let reply = nepl3_reader::runtime::ProviderReply::Read(Box::new(
                nepl3_reader::model::ReadReply::Stopped {
                    reason: nepl3_core::budget::StopReason::Cancelled,
                    report: Default::default(),
                    sources: vec![],
                    source_maps: vec![],
                },
            ));
            assert!(matches!(
                session.resume(&continuation, reply, &mut b, &mut admission),
                Err(ParseError::Closed)
            ));
            Ok(())
        },
    )
}
