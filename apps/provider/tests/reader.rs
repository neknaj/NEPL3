//! Reader reports remain operation-local even after lawful delegated settlement.
use nepl3_core::{
    budget::*, schema::*, source::*, syntax::*, value::*, value_codec::FoundationValueCodec,
};
use nepl3_provider::delegation::IssuedInvocation;
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    model::*,
    plan::*,
    portable,
    runtime::{ProviderReply, ReaderError, ReaderSession},
};
use nepl3_suite::grants::Grants;
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
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
#[test]
fn settled_operation_usage_does_not_forge_saved_reader_accounting() -> Result<(), String> {
    scenario(1_000_000, Mode::ZeroBased)
}
#[test]
fn settled_relative_depth_does_not_satisfy_saved_absolute_depth() -> Result<(), String> {
    scenario(0, Mode::ZeroBased)
}
#[test]
fn separately_admitted_cumulative_child_resumes_real_reader() -> Result<(), String> {
    scenario(1_000_000, Mode::Cumulative)?;
    scenario(0, Mode::Cumulative)
}
#[test]
fn separate_child_source_admission_exhaustion_keeps_pending_reader() -> Result<(), String> {
    scenario(1_000_000, Mode::SourceLimited)
}
#[test]
fn terminal_child_stop_survives_roundtrip_and_saved_reader_resume() -> Result<(), String> {
    scenario(1_000_000, Mode::WorkLimited)
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    ZeroBased,
    Cumulative,
    SourceLimited,
    WorkLimited,
}
fn scenario(initial_work: u64, mode: Mode) -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?,
        nepl3_reader::schema::descriptor(&mut budget()).map_err(err)?,
    ] {
        let reference = descriptor.reference(&mut budget()).map_err(err)?;
        registry
            .register(reference, descriptor, &mut budget())
            .map_err(err)?;
    }
    registry.finalize(&mut budget()).map_err(err)?;
    let schema = registry
        .selected("nepl3.reader", 1)
        .ok_or("Reader schema")?
        .clone();
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?;
    let signature =
        provider::signature(BuiltinReader::Name, &registry, &mut budget()).map_err(err)?;
    let plan = ReaderPlan {
        schema: schema.clone(),
        state_type: TypeDescriptor::Unit,
        expressions: vec![ReaderExpr::Call(signature.operation.clone())],
        rules: vec![ReaderRule {
            name: "entry".into(),
            root: ReaderId(0),
            output: TypeDescriptor::Text,
        }],
        providers: vec![signature],
    };
    let checked = plan.check(&registry, &mut budget()).map_err(err)?;
    // A longer real token makes all additive child counters exceed the saved
    // baseline in the depth-only case, without fabricating any Usage.
    let name = if initial_work == 0 {
        "変数".repeat(512)
    } else {
        "変数".to_owned()
    };
    let text = format!("{name} ");
    let source = SourceSnapshot::new(
        SourceId("reader-accounting".into()),
        1,
        "memory:reader-accounting".into(),
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
    parent.charge(Resource::Work, initial_work).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let context = {
        let mut c = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        raw.check(&mut c, &sources, &registry, &mut parent)
            .map_err(err)?
    };
    let mut session =
        ReaderSession::new("accounting-proof".into(), &checked, &registry, &mut parent)
            .map_err(err)?;
    let reply = parent
        .with_depth_at_least(64, |b| {
            session.read(
                "entry",
                ReadRequest {
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
    let ReadReply::Await {
        call, continuation, ..
    } = reply
    else {
        return Err("Await".into());
    };
    let ProviderCall::Read {
        operation,
        request,
        depth_base,
        ..
    } = call.as_ref()
    else {
        return Err("Read call".into());
    };
    assert!(*depth_base >= 64);
    let saved = continuation.usage;
    assert!(saved.work >= initial_work);
    let mut fixture_admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&registry, &sources, &mut fixture_admission).map_err(err)?;
    let input =
        portable::request_to_value(request, &schema, &mut c, &sources, &registry, &mut budget())
            .map_err(err)?;
    let NdfValue::Record(input) = &input else {
        return Err("input record".into());
    };
    let environment = c
        .encode_environment(&request.context.environment, &mut budget())
        .map_err(err)?;
    let NdfValue::Record(environment) = &environment else {
        return Err("environment record".into());
    };
    let full = budget().limits();
    let limits = Limits {
        source_bytes: if mode == Mode::SourceLimited {
            0
        } else {
            full.source_bytes / 4
        },
        work: full.work / 4,
        allocation_units: full.allocation_units / 4,
        nodes: full.nodes / 4,
        output_bytes: full.output_bytes / 4,
        diagnostics: full.diagnostics / 4,
        events: full.events / 4,
        ..full
    };
    let mut invoke = nepl3_core::operation::Invoke {
        request_id: 91,
        operation: operation.clone(),
        input: TypedValue::Record(input.clone()),
        environment: TypedValue::Record(environment.clone()),
        sources: request.sources.clone(),
        resources: vec![],
        limits,
    };
    if mode == Mode::WorkLimited {
        // Independently measure this actual input, then allow one less work
        // unit. This reaches the builtin and produces its own terminal stop.
        let mut probe = budget();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(&registry, &sources, &mut a).map_err(err)?;
        let depth = session
            .pending_read()
            .map_err(err)?
            .saved_depth()
            .map_err(err)?;
        let completed = probe
            .with_depth_at_least(depth, |b| {
                portable::read::sender::execute_name(
                    &invoke.operation,
                    &invoke.input,
                    &registry,
                    &sources,
                    &mut c,
                    b,
                )
            })
            .map_err(err)?;
        assert!(matches!(completed.reply(), ReadReply::Matched { .. }));
        invoke.limits.work = probe.usage().work.checked_sub(1).ok_or("probe Work")?;
    }
    let grants = Grants::new(&invoke.environment, &sources, &[], &mut budget()).map_err(err)?;
    let authorized = grants.admit(&invoke, &mut budget()).map_err(err)?;
    let implementation = Digest::of(b"local observed builtinName");
    let issued = IssuedInvocation::issue(
        authorized,
        implementation,
        &registry,
        &mut parent,
        &mut budget(),
    )
    .map_err(err)?;
    let identity = issued.context();
    let before = issued.parent_usage();
    if mode != Mode::ZeroBased {
        let depth = session
            .pending_read()
            .map_err(err)?
            .saved_depth()
            .map_err(err)?;
        assert_eq!(depth, *depth_base);
        let local = issued
            .execute_local_child(depth, |bound, execution, admission| {
                assert!(std::ptr::eq(bound, &invoke));
                let mut codec =
                    FoundationCodec::new(&registry, &sources, admission).map_err(err)?;
                portable::read::sender::execute_name(
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
        assert_eq!(local.request_id(), invoke.request_id);
        assert_eq!(local.implementation(), implementation);
        assert_eq!(local.context(), identity);
        let observed = local.execution();
        if mode == Mode::SourceLimited {
            assert_eq!(observed.stopped(), Some(StopReason::SourceLimit));
            assert!(observed.output().is_err());
            assert_eq!(observed.settled_usage().source_bytes, 0);
            assert!(observed.settled_usage().work > 0);
            assert_eq!(parent.usage(), observed.cumulative_usage());
            assert_eq!(parent.poll(), Ok(()));
            assert!(session.pending_read().is_ok());
            return Ok(());
        }
        assert_eq!(
            observed.stopped(),
            if mode == Mode::WorkLimited {
                Some(StopReason::WorkLimit)
            } else {
                None
            }
        );
        assert_eq!(observed.basis(), before);
        assert_eq!(parent.usage(), observed.cumulative_usage());
        assert_eq!(observed.settled_usage().source_bytes, text.len() as u64);
        // This is an explicit second admission context, not shared native
        // admission or a claimed cross-process deduplication exemption.
        assert_eq!(
            parent.usage().source_bytes,
            before.source_bytes + text.len() as u64
        );
        assert!(observed.cumulative_usage().depth >= depth);
        let actual = observed.output().as_ref().map_err(err)?;
        assert_eq!(actual.execution_usage(), observed.cumulative_usage());
        if mode == Mode::WorkLimited {
            assert!(
                matches!(actual.reply(), ReadReply::Stopped { reason: StopReason::WorkLimit, report, .. } if report.usage == observed.cumulative_usage())
            );
        } else {
            assert!(
                matches!(actual.reply(), ReadReply::Matched { report, .. } if report.usage == observed.cumulative_usage())
            );
        }
        let mut encoding_admission = SourceAdmission::default();
        let mut c =
            FoundationCodec::new(&registry, &sources, &mut encoding_admission).map_err(err)?;
        let operation_reply = actual
            .to_operation_reply(&mut c, &mut budget())
            .map_err(err)?;
        let decoded = {
            let pending = session.pending_read().map_err(err)?;
            let mut c = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
            portable::read::operation::from_reply(&operation_reply, &pending, &mut c, &mut parent)
                .map_err(err)?
        };
        let value = match decoded {
            nepl3_core::diagnostic::OperationResult::Complete { value, .. }
                if mode != Mode::WorkLimited =>
            {
                value
            }
            nepl3_core::diagnostic::OperationResult::Stopped {
                reason: StopReason::WorkLimit,
                partial: Some(value),
                report,
            } if mode == Mode::WorkLimited => {
                assert_eq!(report.usage, observed.cumulative_usage());
                value
            }
            _ => return Err("child terminal outcome".into()),
        };
        assert_eq!(&value, actual.reply());
        parent
            .charge(
                Resource::AllocationUnits,
                core::mem::size_of::<ReadReply>() as u64,
            )
            .map_err(err)?;
        let resumed = session
            .resume(
                &continuation,
                ProviderReply::Read(Box::new(value)),
                &sources,
                &mut parent,
                &mut admission,
            )
            .map_err(err)?;
        if mode == Mode::WorkLimited {
            assert!(matches!(
                &resumed,
                ReadReply::Stopped {
                    reason: StopReason::WorkLimit,
                    ..
                }
            ));
            // The narrower child grant stopped. The Reader returns that stop;
            // it does not exhaust the parent's still-available framing budget.
            assert_eq!(parent.poll(), Ok(()));
        } else {
            assert!(
                matches!(&resumed,ReadReply::Matched { value:NdfValue::Text(value),end,.. } if value==&name && *end==name.len() as u64)
            );
        }
        assert!(matches!(
            session.pending_read(),
            Err(ReaderError::NoPending)
        ));
        return Ok(());
    }
    // Independent observation is the local execution Budget, never a copied wire Report.
    let mut execution = Budget::new(invoke.limits);
    let mut child_admission = SourceAdmission::default();
    let mut child_codec =
        FoundationCodec::new(&registry, &sources, &mut child_admission).map_err(err)?;
    let actual = portable::read::sender::execute_name(
        &invoke.operation,
        &invoke.input,
        &registry,
        &sources,
        &mut child_codec,
        &mut execution,
    )
    .map_err(err)?;
    let observed = execution.usage();
    assert!(
        matches!(actual.reply(),ReadReply::Matched{value:NdfValue::Text(value),end,report,..} if value==&name && *end==name.len() as u64 && report.usage==observed)
    );
    let original_reply = actual.reply().clone();
    let operation_reply = actual
        .to_operation_reply(&mut child_codec, &mut budget())
        .map_err(err)?;
    if initial_work > 0 {
        assert!(observed.work < saved.work);
    } else {
        assert!(observed.source_bytes >= saved.source_bytes);
        assert!(observed.work >= saved.work);
        assert!(observed.nodes >= saved.nodes);
        assert!(observed.allocation_units >= saved.allocation_units);
        assert!(observed.output_bytes >= saved.output_bytes);
        assert!(observed.diagnostics >= saved.diagnostics);
        assert!(observed.events >= saved.events);
    }
    assert!(observed.depth < *depth_base);
    issued
        .settle(invoke.request_id, implementation, identity, observed)
        .map_err(err)?;
    assert_eq!(parent.usage().work, before.work + observed.work);
    assert_eq!(parent.usage().depth, before.depth.max(observed.depth));
    let before_validation = parent.usage();
    {
        let pending = session.pending_read().map_err(err)?;
        let mut c = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        assert!(matches!(
            portable::read::operation::from_reply(&operation_reply, &pending, &mut c, &mut parent),
            Err(portable::PortableError::Reader(
                ReaderError::ProviderContract
            ))
        ));
    }
    assert!(session.pending_read().is_ok());
    assert!(parent.usage().work >= before_validation.work);
    assert!(matches!(
        session.resume(
            &continuation,
            ProviderReply::Read(Box::new(original_reply)),
            &sources,
            &mut parent,
            &mut admission
        ),
        Err(ReaderError::ProviderContract)
    ));
    assert!(session.pending_read().is_ok());
    assert_eq!(actual.execution_usage(), observed);
    assert_eq!(execution.usage(), observed);
    // Positive control: the same real builtin executes on the shared host Budget
    // at the saved absolute depth. Its bounded encode/decode then permits resume.
    let operation_reply = {
        let mut c = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        let native = session
            .execute_pending_name(invoke.limits, &mut c, &mut parent)
            .map_err(err)?;
        native
            .to_operation_reply(&mut c, &mut parent)
            .map_err(err)?
    };
    let decoded = {
        let pending = session.pending_read().map_err(err)?;
        let mut c = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        portable::read::operation::from_reply(&operation_reply, &pending, &mut c, &mut parent)
            .map_err(err)?
    };
    let nepl3_core::diagnostic::OperationResult::Complete { value, .. } = decoded else {
        return Err("native complete".into());
    };
    parent
        .charge(
            Resource::AllocationUnits,
            core::mem::size_of::<ReadReply>() as u64,
        )
        .map_err(err)?;
    let resumed = session
        .resume(
            &continuation,
            ProviderReply::Read(Box::new(value)),
            &sources,
            &mut parent,
            &mut admission,
        )
        .map_err(err)?;
    assert!(
        matches!(&resumed,ReadReply::Matched{value:NdfValue::Text(value),end,..} if value==&name && *end==name.len() as u64)
    );
    assert!(matches!(
        session.pending_read(),
        Err(ReaderError::NoPending)
    ));
    Ok(())
}
