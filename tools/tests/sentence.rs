//! Sentence provider runs without Doc/Math schemas or fixed language dispatch.
use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{SchemaRegistry, TypeDescriptor},
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{Environment, EnvironmentEntry},
    value::NdfValue,
};
use nepl3_reader::{
    model::{ProviderCall, ReadReply, ReadRequest, ReaderContext},
    plan::{ReaderExpr, ReaderId, ReaderPlan, ReaderRule},
    runtime::{ProviderReply, ReaderSession},
};
use nepl3_tools::sentence::reader;
use nepl3_wire::foundation::FoundationCodec;
#[path = "sentence/package.rs"]
mod package;
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 10_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_reader::schema::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
        reader::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}

#[test]
fn sentence_provider_rejects_unfinalized_and_changed_operation_descriptors() -> Result<(), String> {
    assert!(reader::signature(&SchemaRegistry::default(), &mut b()).is_err());
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_reader::schema::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
        reader::descriptor(&mut b()),
    ] {
        let mut d = d.map_err(err)?;
        if d.package == "nepl3.sentence.reader" {
            d.operations[0].pure = false;
        }
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    assert!(matches!(
        reader::signature(&r, &mut b()),
        Err(nepl3_reader::runtime::ReaderError::ProviderContract)
    ));
    Ok(())
}
fn run(input: &str, final_input: bool, stop: Option<StopReason>) -> Result<ReadReply, String> {
    let r = registry()?;
    let mut budget = b();
    let signature = reader::signature(&r, &mut budget).map_err(err)?;
    let snapshot = SourceSnapshot::new(
        SourceId("sentence".into()),
        1,
        "memory:sentence".into(),
        input.as_bytes().to_vec(),
        &mut budget,
    )
    .map_err(err)?;
    let mut sources = SourceStore::default();
    sources.insert(snapshot.clone()).map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let raw = ReaderContext {
        schema: signature.operation.schema.clone(),
        category: "Sentence".into(),
        mode: "Literal".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest: nepl3_wire::environment::environment_digest(
                &environment,
                r.selected("nepl3.foundation", 1).ok_or("foundation")?,
                &r,
                &mut budget,
            )
            .map_err(err)?,
            value: environment,
        },
        origins: vec![],
    };
    let mut admission = SourceAdmission::default();
    let context = raw
        .check(
            &mut FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?,
            &sources,
            &r,
            &mut budget,
        )
        .map_err(err)?;
    let mut expressions = vec![ReaderExpr::Call(signature.operation.clone())];
    if stop == Some(StopReason::DepthLimit) {
        for i in 0..64 {
            expressions.push(ReaderExpr::Commit(ReaderId(i)));
        }
    }
    let root = ReaderId((expressions.len() - 1) as u64);
    let plan = ReaderPlan {
        schema: signature.operation.schema.clone(),
        state_type: TypeDescriptor::Unit,
        expressions,
        rules: vec![ReaderRule {
            name: "literal".into(),
            root,
            output: signature.value_output.clone(),
        }],
        providers: vec![signature.clone()],
    };
    let checked = plan.check(&r, &mut budget).map_err(err)?;
    let mut session =
        ReaderSession::new("sentence-test".into(), &checked, &r, &mut budget).map_err(err)?;
    let request = ReadRequest {
        snapshot: &snapshot,
        start: 0,
        limit: input.len() as u64,
        final_input,
        context: &context,
        state: &NdfValue::Unit,
    };
    let suspended = session
        .read("literal", request, &sources, &mut budget, &mut admission)
        .map_err(err)?;
    let ReadReply::Await {
        continuation, call, ..
    } = suspended
    else {
        return Err("expected provider call".into());
    };
    let mut wrong_operation = signature.operation.clone();
    wrong_operation.name = "sentenceReferenced".into();
    let mut legacy = r
        .descriptor(&signature.operation.schema)
        .ok_or("reader descriptor")?
        .clone();
    let mut old_operation = legacy.operations[0].clone();
    old_operation.name = "sentenceReferenced".into();
    legacy.operations.insert(0, old_operation);
    let mut stale = signature.operation.clone();
    stale.schema = legacy.reference(&mut budget).map_err(err)?;
    assert_ne!(stale.schema, signature.operation.schema);
    // Neither an obsolete operation name nor a descriptor augmented with that
    // operation may authorize the current reader operation.
    for wrong_operation in [wrong_operation, stale] {
        assert!(matches!(
            reader::read(
                &wrong_operation,
                request,
                &r,
                &sources,
                &mut budget,
                &mut admission
            ),
            Err(nepl3_reader::runtime::ReaderError::ProviderContract)
        ));
    }
    assert!(matches!(
        reader::read(
            &signature.operation,
            ReadRequest {
                state: &NdfValue::U64(1),
                ..request
            },
            &r,
            &sources,
            &mut budget,
            &mut admission
        ),
        Err(nepl3_reader::runtime::ReaderError::ProviderContract)
    ));
    let ProviderCall::Read {
        operation,
        request: owned,
        depth_base,
        ..
    } = call.as_ref()
    else {
        return Err("read call".into());
    };
    let mut call_sources = SourceStore::default();
    for source in &owned.sources {
        call_sources
            .insert_ref_with_budget(source, &mut budget)
            .map_err(err)?;
    }
    let call_source = call_sources
        .resolve(&owned.snapshot)
        .ok_or("call snapshot")?;
    let call_context = owned
        .context
        .check(
            &mut FoundationCodec::new(&r, &call_sources, &mut admission).map_err(err)?,
            &call_sources,
            &r,
            &mut budget,
        )
        .map_err(err)?;
    let call_request = ReadRequest {
        snapshot: call_source,
        start: owned.start,
        limit: owned.limit,
        final_input: owned.final_input,
        context: &call_context,
        state: &owned.state,
    };
    let reply = budget
        .with_depth_at_least(*depth_base, |budget| {
            if stop == Some(StopReason::Cancelled) {
                budget.cancel();
            }
            if stop == Some(StopReason::WorkLimit) {
                budget.charge(
                    nepl3_core::budget::Resource::Work,
                    budget.limits().work - budget.usage().work,
                )?;
            }
            if stop == Some(StopReason::DepthLimit) {
                let mut ceiling = budget.limits();
                ceiling.depth = depth_base + 1;
                budget.with_ceiling(ceiling, |budget| {
                    reader::read(
                        operation,
                        call_request,
                        &r,
                        &call_sources,
                        budget,
                        &mut admission,
                    )
                })
            } else {
                reader::read(
                    operation,
                    call_request,
                    &r,
                    &call_sources,
                    budget,
                    &mut admission,
                )
            }
        })
        .map_err(err)?;
    assert_eq!(budget.current_depth(), 0);
    if let Some(reason) = stop {
        assert!(matches!(&reply, ReadReply::Stopped { reason: actual, .. } if *actual == reason));
        assert_eq!(budget.poll(), Err(reason));
    }
    let reply = session
        .resume(
            &continuation,
            ProviderReply::Read(Box::new(reply)),
            &sources,
            &mut budget,
            &mut admission,
        )
        .map_err(err)?;
    if let ReadReply::Matched {
        value,
        end,
        view,
        sources: returned,
        ..
    } = &reply
    {
        assert!(returned.is_empty());
        // Actual CBOR receiver uses its declared owner, then checks token association.
        let bytes = nepl3_wire::encode(value, &mut budget).map_err(err)?;
        let value = nepl3_wire::decode(&bytes, &mut budget).map_err(err)?;
        let mut codec = FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
        let syntax = nepl3_sentence_core::portable::literal::from_value(
            &value,
            &snapshot,
            &r,
            &mut codec,
            &mut budget,
        )
        .map_err(err)?;
        assert_eq!(syntax.views[0].head, snapshot.span(0, *end).map_err(err)?);
        assert_eq!(&syntax.views[0].view, view);
    }
    Ok(reply)
}
#[test]
fn sentence_provider_is_admitted_by_reader_session_and_roundtrips_literal_payload()
-> Result<(), String> {
    assert!(matches!(
        run("\"[漢/かん]{語/note}\" tail", true, None)?,
        ReadReply::Matched { .. }
    ));
    assert!(matches!(
        run("\"\"", true, None)?,
        ReadReply::Matched { .. }
    ));
    assert!(matches!(
        run("word", true, None)?,
        ReadReply::NoMatch { .. }
    ));
    assert!(matches!(
        run("\"[漢/", false, None)?,
        ReadReply::NeedMore { .. }
    ));
    Ok(())
}
#[test]
fn sentence_provider_failures_keep_typed_diagnostic_positions_and_stops() -> Result<(), String> {
    let ReadReply::Failed {
        diagnostic, report, ..
    } = run("\"前[文/ぶん", true, None)?
    else {
        return Err("expected unclosed annotation".into());
    };
    assert_eq!(diagnostic.code, "UnclosedAnnotation");
    assert_eq!(report.diagnostics, vec![diagnostic.clone()]);
    let primary = diagnostic.primary.as_ref().ok_or("primary")?;
    assert_eq!((primary.start(), primary.end()), (15, 15));
    let opening = diagnostic.related[0].span.as_ref().ok_or("opening")?;
    assert_eq!((opening.start(), opening.end()), (4, 5));
    let ReadReply::Failed {
        diagnostic, report, ..
    } = run("\"[漢/]\"", true, None)?
    else {
        return Err("expected malformed annotation failure".into());
    };
    assert_eq!(diagnostic.schema.package, "nepl3.sentence.reader");
    assert_eq!(diagnostic.code, "EmptyAnnotationPart");
    assert!(diagnostic.primary.is_some());
    assert_eq!(report.diagnostics, vec![diagnostic]);
    for reason in [StopReason::Cancelled, StopReason::WorkLimit] {
        assert!(
            matches!(run("\"a\"", true, Some(reason))?, ReadReply::Stopped { reason: actual, .. } if actual == reason)
        );
    }
    Ok(())
}

#[test]
fn sentence_provider_shares_nested_reader_depth_and_resumes_stopped_reply() -> Result<(), String> {
    let input = "\"[[[[[a/b]/c]/d]/e]/f]\"";
    assert!(matches!(run(input, true, None)?, ReadReply::Matched { .. }));
    assert!(matches!(
        run(input, true, Some(StopReason::DepthLimit))?,
        ReadReply::Stopped {
            reason: StopReason::DepthLimit,
            ..
        }
    ));
    Ok(())
}
