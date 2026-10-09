//! Trusted native child accounting composed with private Tokenizer reply echoes.
//! This is not a common Tokenizer operation or a remote measurement protocol.
use nepl3_core::{
    budget::*,
    diagnostic::OperationResult,
    operation::{Invoke, OperationReply},
    schema::*,
    source::*,
    syntax::*,
    value::*,
    value_codec::FoundationValueCodec,
};
use nepl3_provider::delegation::IssuedInvocation;
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    model::*,
    plan::*,
    portable,
    runtime::{ProviderReply, ReaderError},
    tokenizer::*,
};
use nepl3_suite::grants::Grants;
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
#[path = "tokenizer/dependent.rs"]
mod dependent;
use dependent::{Actual, execute};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 100_000_000,
        depth: 512,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
fn additive(u: Usage) -> [u64; 7] {
    [
        u.source_bytes,
        u.work,
        u.nodes,
        u.allocation_units,
        u.output_bytes,
        u.diagnostics,
        u.events,
    ]
}
fn assert_accounted(before: Usage, cost: Usage, after: Usage) {
    for ((start, used), end) in additive(before)
        .into_iter()
        .zip(additive(cost))
        .zip(additive(after))
    {
        assert_eq!(end, start + used);
    }
    assert_eq!(after.depth, before.depth.max(cost.depth));
}
#[test]
fn actual_native_child_roundtrips_and_resumes_tokenizer() -> Result<(), String> {
    for envelope in [false, true] {
        scenario(envelope, false, Validation::Separate, false)?;
    }
    Ok(())
}
#[test]
fn actual_child_work_stop_preserves_tokenizer_prefix_and_parent_capacity() -> Result<(), String> {
    for envelope in [false, true] {
        scenario(envelope, true, Validation::Separate, false)?;
    }
    Ok(())
}
#[derive(Clone, Copy)]
enum Validation {
    Separate,
    Parent,
}
#[test]
fn parent_validated_native_child_preserves_tokenizer_accounting() -> Result<(), String> {
    for envelope in [false, true] {
        for limited in [false, true] {
            scenario(envelope, limited, Validation::Parent, false)?;
        }
    }
    Ok(())
}
#[test]
fn dependent_native_child_preserves_tokenizer_accounting() -> Result<(), String> {
    for envelope in [false, true] {
        for limited in [false, true] {
            scenario(envelope, limited, Validation::Parent, true)?;
        }
    }
    Ok(())
}
fn scenario(
    envelope: bool,
    limited: bool,
    validation_mode: Validation,
    dependent: bool,
) -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?,
        nepl3_reader::schema::descriptor(&mut budget()).map_err(err)?,
    ] {
        let r = d.reference(&mut budget()).map_err(err)?;
        registry.register(r, d, &mut budget()).map_err(err)?;
    }
    let dependent_signature = dependent::register(&mut registry)?;
    registry.finalize(&mut budget()).map_err(err)?;
    let schema = registry
        .selected("nepl3.reader", 1)
        .ok_or("reader schema")?
        .clone();
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?;
    let sig = if dependent {
        dependent_signature
    } else {
        provider::signature(BuiltinReader::Name, &registry, &mut budget()).map_err(err)?
    };
    let expressions = if dependent {
        vec![
            ReaderExpr::Scalar(CharClass::Any),
            ReaderExpr::Then {
                provider: sig.operation.clone(),
                first: ReaderId(0),
            },
        ]
    } else {
        vec![ReaderExpr::Call(sig.operation.clone())]
    };
    let plan = ReaderPlan {
        schema: schema.clone(),
        state_type: TypeDescriptor::Unit,
        expressions,
        rules: vec![ReaderRule {
            name: "entry".into(),
            root: ReaderId(u64::from(dependent)),
            output: TypeDescriptor::Text,
        }],
        providers: vec![sig.clone()],
    };
    let checked = plan.check(&registry, &mut budget()).map_err(err)?;
    let modes = vec![ReaderMode {
        name: "default".into(),
        skip: vec![SkipRule {
            reader: TokenReader::Builtin(BuiltinReader::Trivia),
        }],
        take: vec![TakeRule {
            reader: TokenReader::Rule("entry".into()),
            kind: KindRef {
                schema: schema.clone(),
                local_kind: 0,
            },
        }],
    }];
    let text = if dependent { " a変数 " } else { " 変数 " };
    let token_end = text.len() as u64 - 1;
    let source = SourceSnapshot::new(
        SourceId("tokenizer-native".into()),
        1,
        "memory:tokenizer-native".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let mut sources = SourceStore::default();
    sources.insert(source.clone()).map_err(err)?;
    let env = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&env, foundation, &registry, &mut budget()).map_err(err)?;
    let raw = ReaderContext {
        schema: schema.clone(),
        category: "fixture".into(),
        mode: "default".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value: env,
        },
        origins: vec![],
    };
    let mut parent = budget();
    parent.charge(Resource::Work, 1_000_000).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let context = {
        let mut c = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        raw.check(&mut c, &sources, &registry, &mut parent)
            .map_err(err)?
    };
    let mut session = TokenizationSession::new(
        "native-tokenizer".into(),
        &modes,
        &checked,
        &registry,
        &mut parent,
    )
    .map_err(err)?;
    let waiting = parent
        .with_depth_at_least(64, |b| {
            session.read(
                TokenizationRequest {
                    snapshot: &source,
                    start: 0,
                    limit: text.len() as u64,
                    final_input: true,
                    context: &context,
                    state: &NdfValue::Unit,
                },
                &sources,
                b,
                &mut admission,
            )
        })
        .map_err(err)?;
    let TokenizationOutcome::Await { call, continuation } = &waiting.outcome else {
        return Err("Await".into());
    };
    let TokenizationWait::Provider {
        continuation: inner,
    } = &continuation.pending
    else {
        return Err("Provider wait".into());
    };
    assert!(continuation.usage.work > inner.usage.work);
    assert_eq!(continuation.request.start, 0);
    let (operation, request, depth_base) = match call.as_ref() {
        ProviderCall::Read {
            operation,
            request,
            depth_base,
            ..
        } if !dependent => (operation, request, depth_base),
        ProviderCall::Dependent {
            operation,
            request,
            depth_base,
            ..
        } if dependent => {
            assert_eq!(request.first, NdfValue::Text("a".into()));
            assert_eq!(request.end, 2);
            assert_eq!(request.request.start, 2);
            (operation, &request.request, depth_base)
        }
        _ => return Err("selected provider call".into()),
    };
    assert_eq!(request.start, if dependent { 2 } else { 1 });
    assert!(*depth_base >= 64);
    assert_eq!(
        session
            .pending_read()
            .map_err(err)?
            .saved_depth()
            .map_err(err)?,
        *depth_base
    );
    assert_eq!(waiting.trivia.len(), 1);
    assert_eq!(waiting.trivia[0].span, source.span(0, 1).map_err(err)?);
    assert_eq!(
        waiting.trivia[0].kind,
        nepl3_core::view::TriviaKind::Whitespace
    );
    assert_eq!(continuation.usage.source_bytes, text.len() as u64);
    let mut c = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let echo = if envelope {
        session.pending_reply_value(&mut c, &mut parent)
    } else {
        session.pending_continuation_value(&mut c, &mut parent)
    }
    .map_err(err)?;
    let echo = nepl3_wire::decode(
        &nepl3_wire::encode(&echo, &mut parent).map_err(err)?,
        &mut parent,
    )
    .map_err(err)?;
    let input = if let ProviderCall::Dependent { request, .. } = call.as_ref() {
        portable::dependent::to_value(request, &schema, &mut c, &sources, &registry, &mut parent)
            .map_err(err)?
    } else {
        portable::request_to_value(request, &schema, &mut c, &sources, &registry, &mut parent)
            .map_err(err)?
    };
    input.charge_clone(&mut parent).map_err(err)?;
    let NdfValue::Record(input) = &input else {
        return Err("input".into());
    };
    let environment = c
        .encode_environment(&request.context.environment, &mut parent)
        .map_err(err)?;
    environment.charge_clone(&mut parent).map_err(err)?;
    let NdfValue::Record(environment) = &environment else {
        return Err("environment".into());
    };
    let full = parent.limits();
    let mut invoke = Invoke {
        request_id: 91,
        operation: operation.clone(),
        input: TypedValue::Record(input.clone()),
        environment: TypedValue::Record(environment.clone()),
        sources: request.sources.clone(),
        resources: vec![],
        limits: Limits {
            source_bytes: full.source_bytes / 4,
            work: full.work / 4,
            allocation_units: full.allocation_units / 4,
            nodes: full.nodes / 4,
            output_bytes: full.output_bytes / 4,
            diagnostics: full.diagnostics / 4,
            events: full.events / 4,
            ..full
        },
    };
    if limited {
        // Calibration is separate fixture work. No probe counter is admitted
        // as a measurement of the live child or its parent.
        let mut probe = budget();
        let mut probe_admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&registry, &sources, &mut probe_admission).map_err(err)?;
        let actual = probe
            .with_depth_at_least(*depth_base, |b| {
                Ok::<_, StopReason>(execute(
                    &sig,
                    &invoke.operation,
                    &invoke.input,
                    &registry,
                    &sources,
                    &mut codec,
                    b,
                ))
            })
            .map_err(err)??;
        assert!(matches!(actual.reply(), ReadReply::Matched { .. }));
        invoke.limits.work = probe.usage().work.checked_sub(1).ok_or("probe Work")?;
    }
    let mut validation = budget();
    let before_validation = parent.usage();
    let on_parent = matches!(validation_mode, Validation::Parent);
    let grants = Grants::new(
        &invoke.environment,
        &sources,
        &[],
        if on_parent {
            &mut parent
        } else {
            &mut validation
        },
    )
    .map_err(err)?;
    let authorized = grants
        .admit(
            &invoke,
            if on_parent {
                &mut parent
            } else {
                &mut validation
            },
        )
        .map_err(err)?;
    let implementation = Digest::of(b"trusted native tokenizer builtinName");
    let mut issued = if on_parent {
        IssuedInvocation::issue_with_parent_validation(
            authorized,
            implementation,
            &registry,
            &mut parent,
        )
    } else {
        IssuedInvocation::issue(
            authorized,
            implementation,
            &registry,
            &mut parent,
            &mut validation,
        )
    }
    .map_err(err)?;
    if on_parent {
        // Independent validation oracle only; these costs are never recorded
        // onto the parent a second time. All live phases already used parent.
        let oracle_grants =
            Grants::new(&invoke.environment, &sources, &[], &mut validation).map_err(err)?;
        let _ = oracle_grants.admit(&invoke, &mut validation).map_err(err)?;
        invoke
            .validate_input(&invoke.operation, &registry, &mut validation)
            .map_err(err)?;
        let context = nepl3_wire::operation::context_digest(
            &invoke,
            implementation,
            &registry,
            &mut validation,
        )
        .map_err(err)?;
        assert_eq!(context, issued.context());
    } else {
        // Compatibility path: post-charge this successful local validation once.
        // It still does not cover validation/issue failure or recording failure.
        issued
            .run_local(|b| b.record_observed_usage(validation.usage()))
            .map_err(err)?;
    }
    let basis = issued.parent_usage();
    assert_accounted(before_validation, validation.usage(), basis);
    assert_eq!(validation.usage().source_bytes, text.len() as u64);
    assert_eq!(before_validation.source_bytes, text.len() as u64);
    assert!(basis.work > continuation.usage.work);
    let identity = issued.context();
    let local = issued
        .execute_local_child(*depth_base, |bound, execution, child_admission| {
            assert!(std::ptr::eq(bound, &invoke));
            let mut codec =
                FoundationCodec::new(&registry, &sources, child_admission).map_err(err)?;
            execute(
                &sig,
                &bound.operation,
                &bound.input,
                &registry,
                &sources,
                &mut codec,
                execution,
            )
            .map_err(err)
        })
        .map_err(|failure| err(failure.error))?;
    assert_eq!(local.request_id(), 91);
    assert_eq!(local.implementation(), implementation);
    assert_eq!(local.context(), identity);
    let observed = local.execution();
    assert_eq!(observed.basis(), basis);
    assert_eq!(parent.usage(), observed.cumulative_usage());
    assert_accounted(basis, observed.settled_usage(), observed.cumulative_usage());
    assert_eq!(observed.settled_usage().source_bytes, text.len() as u64);
    assert_eq!(
        parent.usage().source_bytes,
        basis.source_bytes + text.len() as u64
    );
    assert!(observed.cumulative_usage().depth >= *depth_base);
    assert_eq!(
        observed.stopped(),
        if limited {
            Some(StopReason::WorkLimit)
        } else {
            None
        }
    );
    let actual = observed.output().as_ref().map_err(err)?;
    assert_eq!(actual.execution_usage(), observed.cumulative_usage());
    if limited {
        assert!(
            matches!(actual.reply(),ReadReply::Stopped{reason:StopReason::WorkLimit,report,..} if report.usage==observed.cumulative_usage())
        );
    } else {
        assert!(
            matches!(actual.reply(),ReadReply::Matched{value:NdfValue::Text(v),end,report,..} if v=="変数" && *end==token_end && report.usage==observed.cumulative_usage())
        );
    }
    assert_eq!(parent.poll(), Ok(()));
    let mut c = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let operation_reply = match actual {
        Actual::Name(value) => value.to_operation_reply(&mut c, &mut parent).map_err(err)?,
        Actual::Dependent { reply, .. } => portable::read::operation::to_reply(
            reply,
            &session.pending_read().map_err(err)?,
            &mut c,
            &mut parent,
        )
        .map_err(err)?,
    };
    let bytes = nepl3_wire::operation::encode_reply(
        &operation_reply,
        &registry,
        &sources,
        c.source_admission(),
        &mut parent,
    )
    .map_err(err)?;
    let operation_reply = nepl3_wire::operation::decode_reply(
        &bytes,
        &registry,
        &sources,
        c.source_admission(),
        &mut parent,
    )
    .map_err(err)?;
    let pending = session.pending_read().map_err(err)?;
    let mut wrong = operation_reply.clone();
    let OperationReply::Result(result) = &mut wrong else {
        return Err("terminal operation".into());
    };
    match result {
        OperationResult::Complete { report, .. }
        | OperationResult::Invalid { report, .. }
        | OperationResult::Stopped { report, .. } => report.usage.work += 1,
    };
    assert!(portable::read::operation::from_reply(&wrong, &pending, &mut c, &mut parent).is_err());
    let decoded =
        portable::read::operation::from_reply(&operation_reply, &pending, &mut c, &mut parent)
            .map_err(err)?;
    let value = match decoded {
        OperationResult::Complete { value, .. } if !limited => value,
        OperationResult::Stopped {
            reason: StopReason::WorkLimit,
            partial: Some(value),
            report,
        } if limited => {
            assert_eq!(report.usage, observed.cumulative_usage());
            value
        }
        _ => return Err("outcome".into()),
    };
    assert_eq!(&value, actual.reply());
    assert!(session.pending_read().is_ok());
    let mut wrong = echo.clone();
    if envelope {
        let NdfValue::Record(r) = &mut wrong else {
            return Err("reply".into());
        };
        r.fields[1] = NdfValue::U64(99);
    } else {
        let NdfValue::Record(r) = &mut wrong else {
            return Err("continuation".into());
        };
        r.fields[3] = NdfValue::Unit;
    }
    let invalid = ProviderReply::Read(Box::new(value.clone()));
    let rejected = if envelope {
        session.resume_reply_value(&wrong, invalid, &sources, &mut c, &mut parent)
    } else {
        session.resume_continuation_value(&wrong, invalid, &sources, &mut c, &mut parent)
    };
    assert!(matches!(
        rejected,
        Err(portable::PortableError::Reader(ReaderError::Continuation))
    ));
    assert!(session.pending_read().is_ok());
    parent
        .charge(
            Resource::AllocationUnits,
            core::mem::size_of::<ReadReply>() as u64,
        )
        .map_err(err)?;
    let reply = ProviderReply::Read(Box::new(value));
    let done = if envelope {
        session.resume_reply_value(&echo, reply, &sources, &mut c, &mut parent)
    } else {
        session.resume_continuation_value(&echo, reply, &sources, &mut c, &mut parent)
    }
    .map_err(err)?;
    assert_eq!(done.trivia, waiting.trivia);
    if limited {
        assert!(matches!(
            done.outcome,
            TokenizationOutcome::Stopped {
                reason: StopReason::WorkLimit
            }
        ));
        assert_eq!(done.cursor, 1);
    } else {
        let TokenizationOutcome::Token(token) = done.outcome else {
            return Err("Token".into());
        };
        assert_eq!(token.payload, NdfValue::Text("変数".into()));
        assert_eq!(token.leading_trivia, waiting.trivia);
        assert_eq!(token.head, source.span(1, token_end).map_err(err)?);
        assert_eq!(done.cursor, token_end);
    }
    assert!(matches!(
        session.pending_read(),
        Err(ReaderError::NoPending)
    ));
    assert_eq!(parent.poll(), Ok(()));
    assert!(parent.usage().work > observed.cumulative_usage().work);
    assert_eq!(actual.execution_usage(), observed.cumulative_usage());
    assert_eq!(parent.usage().source_bytes, 3 * text.len() as u64);
    assert_eq!(done.report.usage, parent.usage());
    drop(c);
    // A new operation must reach its own inner Await, proving that both saved
    // slots were consumed even on the child's terminal WorkLimit path.
    let next = session
        .read(
            TokenizationRequest {
                snapshot: &source,
                start: 0,
                limit: text.len() as u64,
                final_input: true,
                context: &context,
                state: &NdfValue::Unit,
            },
            &sources,
            &mut parent,
            &mut admission,
        )
        .map_err(err)?;
    let TokenizationOutcome::Await {
        continuation: next, ..
    } = next.outcome
    else {
        return Err("new Await".into());
    };
    assert_ne!(next.scope.operation_id, continuation.scope.operation_id);
    session.discard_pending();
    assert!(matches!(
        session.pending_read(),
        Err(ReaderError::NoPending)
    ));
    assert_eq!(parent.poll(), Ok(()));
    Ok(())
}
