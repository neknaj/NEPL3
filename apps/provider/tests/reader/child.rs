use super::*;
use nepl3_provider::delegation::IssuedInvocation;
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    portable,
};
use nepl3_suite::grants::Grants;

/// Host composition, not a replacement parser or a generic provider dispatcher.
/// Source authority is narrowed before any nested request context is decoded.
fn execute(
    invoke: &Invoke,
    signature: &ProviderSignature,
    registry: &SchemaRegistry,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<ReadReply, String> {
    let mut authorized = SourceStore::default();
    for source in &invoke.sources {
        authorized
            .insert_with_budget(source.clone_with_budget(budget).map_err(error)?, budget)
            .map_err(error)?;
    }
    let TypedValue::Record(record) = invoke.input.clone_with_budget(budget).map_err(error)? else {
        return Err("expected ReadRequest record".into());
    };
    let value = NdfValue::Record(record);
    let schema = registry
        .selected("nepl3.reader", 1)
        .ok_or("Reader schema")?;
    let mut codec = FoundationCodec::new(registry, &authorized, admission).map_err(error)?;
    let declared =
        portable::request_sources(&value, schema, &mut codec, registry, budget).map_err(error)?;
    let mut local = SourceStore::default();
    for source in declared {
        let identity = source.identity();
        let granted = authorized
            .get_revision_with_budget(&identity.source, identity.revision, budget)
            .map_err(error)?
            .ok_or("unauthorized nested source")?;
        if !source.eq_with_budget(granted, budget).map_err(error)? {
            return Err("conflicting nested source".into());
        }
        local.insert_with_budget(source, budget).map_err(error)?;
    }
    let mut codec = FoundationCodec::new(registry, &local, admission).map_err(error)?;
    let ProviderInput::Read(request) = dispatch::from_value(
        &invoke.input,
        &DispatchContext {
            signature,
            sources: &local,
            mappings: &[],
            registry,
        },
        &mut codec,
        budget,
    )
    .map_err(error)?
    else {
        return Err("expected typed Read request".into());
    };
    let checked = request
        .context
        .check(&mut codec, &local, registry, budget)
        .map_err(error)?;
    let snapshot = local
        .resolve_with_budget(&request.snapshot, budget)
        .map_err(error)?
        .ok_or("missing declared snapshot")?;
    provider::read(
        &invoke.operation,
        ReadRequest {
            snapshot,
            start: request.start,
            limit: request.limit,
            final_input: request.final_input,
            context: &checked,
            state: &request.state,
        },
        registry,
        &local,
        budget,
        admission,
    )
    .map_err(error)
}

#[test]
fn saved_builtin_name_runs_in_measured_child_and_resumes_original_session() -> Result<(), String> {
    for (text, final_input, expected) in [("alpha", true, 0), ("!", true, 1), ("alpha", false, 2)] {
        run_case(text, final_input, expected, false)?;
    }
    Ok(())
}

#[test]
fn nested_source_rejections_and_child_stop_preserve_the_saved_session() -> Result<(), String> {
    run_case("alpha", true, 0, true)
}

fn run_case(text: &str, final_input: bool, expected: u8, adversarial: bool) -> Result<(), String> {
    let (registry, schema) = registry().map_err(error)?;
    let signature =
        provider::signature(BuiltinReader::Name, &registry, &mut budget()).map_err(error)?;
    let mut p = plan(
        &schema,
        vec![ReaderExpr::Call(signature.operation.clone())],
        0,
        TypeDescriptor::Text,
    );
    p.providers.push(signature.clone());
    let checked_plan = p.check(&registry, &mut budget()).map_err(error)?;
    let mut parent = budget();
    parent.observe_depth(80).map_err(error)?;
    let mut session = ReaderSession::new(
        "builtin-child".into(),
        &checked_plan,
        &registry,
        &mut parent,
    )
    .map_err(error)?;
    let source = source(text).map_err(error)?;
    let mut store = SourceStore::default();
    store.insert(source.clone()).map_err(error)?;
    let mut admission = SourceAdmission::default();
    let raw = context(&schema, &registry).map_err(error)?;
    let checked_context =
        check_context(&raw, &store, &registry, &mut parent, &mut admission).map_err(error)?;
    let ReadReply::Await {
        call, continuation, ..
    } = session
        .read(
            "entry",
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: text.len() as u64,
                final_input,
                context: &checked_context,
                state: &NdfValue::Unit,
            },
            &store,
            &mut parent,
            &mut admission,
        )
        .map_err(error)?
    else {
        return Err("expected saved builtin Await".into());
    };
    let ProviderCall::Read {
        session_id,
        call_id,
        operation,
        request,
        depth_base,
        ..
    } = *call
    else {
        return Err("expected saved Read call".into());
    };
    assert_eq!(session_id, "builtin-child");
    assert_eq!(session_id, continuation.session_id);
    let parent_admitted_bytes = parent.usage().source_bytes;
    assert_eq!(parent_admitted_bytes, text.len() as u64);
    admission
        .admit_existing(&source, &mut parent)
        .map_err(error)?;
    assert_eq!(parent.usage().source_bytes, parent_admitted_bytes);
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let input = dispatch::to_value(
        &ProviderInput::Read(Box::new(request.clone())),
        &DispatchContext {
            signature: &signature,
            sources: &store,
            mappings: &[],
            registry: &registry,
        },
        &mut codec,
        &mut parent,
    )
    .map_err(error)?;
    let NdfValue::Record(ref environment) = codec
        .encode_environment(&request.context.environment, &mut parent)
        .map_err(error)?
    else {
        return Err("expected environment".into());
    };
    let grant = Limits {
        work: 1_000_000,
        source_bytes: 10_000,
        nodes: 10_000,
        allocation_units: 1_000_000,
        output_bytes: 10_000,
        diagnostics: 100,
        events: 100,
        depth: 128,
    };
    let invoke = Invoke {
        request_id: call_id,
        operation,
        input,
        environment: TypedValue::Record(environment.clone()),
        sources: request.sources,
        resources: vec![],
        limits: grant,
    };
    if adversarial {
        for case in 0..4 {
            let mut candidate = invoke.clone();
            match case {
                // The broad host store still has the input, but this Invoke does not.
                0 => candidate.sources.clear(),
                // The authorized outer source cannot replace a missing nested declaration.
                1 => {
                    let TypedValue::Record(record) = &mut candidate.input else {
                        return Err("expected nested record".into());
                    };
                    record.fields[1] = NdfValue::List(vec![]);
                }
                // Same source ID/revision, different content and digest.
                2 => {
                    let conflicting = model::source("other").map_err(error)?;
                    let mut conflicting_store = SourceStore::default();
                    conflicting_store
                        .insert(conflicting.clone())
                        .map_err(error)?;
                    let mut encoding_admission = SourceAdmission::default();
                    let mut encoding = FoundationCodec::new(
                        &registry,
                        &conflicting_store,
                        &mut encoding_admission,
                    )
                    .map_err(error)?;
                    let table = encoding
                        .encode_sources(&[conflicting], &mut parent)
                        .map_err(error)?;
                    let TypedValue::Record(record) = &mut candidate.input else {
                        return Err("expected nested record".into());
                    };
                    record.fields[1] = table;
                }
                _ => candidate.limits.work = 1,
            }
            let grants =
                Grants::new(&candidate.environment, &store, &[], &mut parent).map_err(error)?;
            let issued = IssuedInvocation::issue_with_parent_validation(
                grants.admit(&candidate, &mut parent).map_err(error)?,
                Digest::of(b"production builtinName"),
                &registry,
                &mut parent,
            )
            .map_err(error)?;
            let result = issued
                .execute_local_child(depth_base, |bound, child, child_admission| {
                    execute(bound, &signature, &registry, child, child_admission)
                })
                .map_err(error)?
                .into_execution();
            let Err(failure) = result.output() else {
                return Err(format!("candidate {case} must reject"));
            };
            match case {
                0 => assert_eq!(failure, "unauthorized nested source"),
                1 => assert!(failure.contains("UndeclaredSource"), "{failure}"),
                2 => assert_eq!(failure, "conflicting nested source"),
                _ => assert!(failure.contains("WorkLimit"), "{failure}"),
            }
            assert_eq!(
                result.stopped(),
                if case == 3 {
                    Some(StopReason::WorkLimit)
                } else {
                    None
                }
            );
            assert_eq!(parent.usage(), result.cumulative_usage());
            assert_eq!(parent.poll(), Ok(()));
            assert_eq!(parent.current_depth(), 0);
            assert!(session.pending_read().is_ok());
        }
    }
    let grants = Grants::new(&invoke.environment, &store, &[], &mut parent).map_err(error)?;
    let admitted = grants.admit(&invoke, &mut parent).map_err(error)?;
    let issued = IssuedInvocation::issue_with_parent_validation(
        admitted,
        Digest::of(b"production builtinName"),
        &registry,
        &mut parent,
    )
    .map_err(error)?;
    let basis = issued.parent_usage();
    let result = issued
        .execute_local_child(depth_base, |bound, child, child_admission| {
            assert!(std::ptr::eq(bound, &invoke));
            assert_eq!(bound.limits, grant);
            assert_eq!(child.current_depth(), depth_base);
            assert_eq!(child.usage(), basis);
            assert_eq!(child.limits().work, basis.work + grant.work);
            execute(bound, &signature, &registry, child, child_admission)
        })
        .map_err(error)?;
    assert_eq!(result.request_id(), call_id);
    let result = result.into_execution();
    assert_eq!(result.stopped(), None);
    assert_eq!(result.basis(), basis);
    assert_eq!(
        result.cumulative_usage().work - basis.work,
        result.settled_usage().work
    );
    assert_eq!(
        result.cumulative_usage().source_bytes - basis.source_bytes,
        result.settled_usage().source_bytes
    );
    assert!(result.settled_usage().work > 0);
    assert!(result.settled_usage().depth >= depth_base);
    assert!(result.settled_usage().depth < 80);
    assert_eq!(result.settled_usage().source_bytes, text.len() as u64);
    assert_eq!(parent.usage(), result.cumulative_usage());
    assert_eq!(parent.current_depth(), 0);
    assert_eq!(parent.usage().depth, 80);
    let before_resume_source_bytes = parent.usage().source_bytes;
    let reply = result.into_output()?;
    session
        .pending_read()
        .map_err(error)?
        .validate(&reply, &mut parent, &mut admission)
        .map_err(error)?;
    let resumed = session
        .resume(
            &continuation,
            ProviderReply::Read(Box::new(reply)),
            &store,
            &mut parent,
            &mut admission,
        )
        .map_err(error)?;
    assert_eq!(parent.usage().source_bytes, before_resume_source_bytes);
    assert!(session.pending_read().is_err());
    match (expected, resumed) {
        (0, ReadReply::Matched { value, end, .. }) => {
            assert_eq!(value, NdfValue::Text("alpha".into()));
            assert_eq!(end, 5);
        }
        (1, ReadReply::NoMatch { .. }) | (2, ReadReply::NeedMore { .. }) => {}
        (_, other) => return Err(format!("unexpected resumed reply: {other:?}")),
    }
    Ok(())
}
