use super::*;
use nepl3_core::origin::{Mapping, MappingKind};

#[test]
fn request_work_stops_before_next_call_and_await_keep_accepted_artifacts() -> Result<(), ReaderError>
{
    let (registry, schema) = registry()?;
    let mut p = provider_plan(&schema);
    p.state_type = TypeDescriptor::U64;
    p.providers[0].state_type = TypeDescriptor::U64;
    p.expressions
        .push(ReaderExpr::Seq(vec![ReaderId(0), ReaderId(0), ReaderId(0)]));
    p.rules[0].root = ReaderId(1);
    p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
    let checked = p.check(&registry, &mut budget())?;
    let input = source("abc")?;
    let mut store = SourceStore::default();
    store.insert(input.clone())?;
    let raw = context(&schema, &registry)?;
    let ctx = check_context(
        &raw,
        &store,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let execute = |remaining: Option<u64>, cancel: bool| -> Result<(u64, bool), ReaderError> {
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let mut session =
            ReaderSession::new("request-source-stop".into(), &checked, &registry, &mut b)?;
        let reply = session.read(
            "entry",
            ReadRequest {
                snapshot: &input,
                start: 0,
                limit: 3,
                final_input: true,
                context: &ctx,
                state: &NdfValue::U64(0),
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let ReadReply::Await { continuation, .. } = reply else {
            return Err(ReaderError::Context);
        };
        let generated = admission.create(
            SourceId("生成🙂".into()),
            0,
            "memory:generated".into(),
            b"x".to_vec(),
            &mut b,
        )?;
        let mapping = Mapping {
            source: input.span(0, 1)?,
            target: generated.span(0, 1)?,
            kind: MappingKind::Transformed,
        };
        let mut first = annotated_terminal(&schema, input.span(0, 1)?, 1, &mut b)?;
        let ProviderReply::Read(ref mut reply) = first else {
            return Err(ReaderError::Context);
        };
        let ReadReply::Matched {
            sources,
            source_maps,
            new_state,
            ..
        } = reply.as_mut()
        else {
            return Err(ReaderError::Context);
        };
        *new_state = NdfValue::U64(7);
        sources.push(generated.clone());
        source_maps.push(mapping.clone());
        let reply = session.resume(&continuation, first, &store, &mut b, &mut admission)?;
        let ReadReply::Await { continuation, .. } = reply else {
            return Err(ReaderError::Context);
        };
        assert_eq!(continuation.request.start, 0);
        assert_eq!(continuation.request.state, NdfValue::U64(0));
        let ProviderCall::Read { request, .. } = &continuation.pending else {
            return Err(ReaderError::Context);
        };
        assert_eq!(request.start, 1);
        assert_eq!(request.state, NdfValue::U64(7));
        assert_eq!(request.sources, continuation.request.sources);
        assert_eq!(continuation.current.sources, vec![generated.clone()]);
        assert_eq!(continuation.current.source_maps, vec![mapping.clone()]);
        let mut second = terminal("b", 2, &mut b)?;
        let ProviderReply::Read(reply) = &mut second else {
            return Err(ReaderError::Context);
        };
        let ReadReply::Matched { new_state, .. } = reply.as_mut() else {
            return Err(ReaderError::Context);
        };
        *new_state = NdfValue::U64(8);
        if let Some(remaining) = remaining {
            b.charge(Resource::Work, b.limits().work - b.usage().work - remaining)?;
        }
        if cancel {
            b.cancel();
        }
        let before = b.usage().work;
        let reply = session.resume(&continuation, second, &store, &mut b, &mut admission)?;
        let awaited = matches!(&reply, ReadReply::Await { .. });
        match reply {
            ReadReply::Stopped {
                reason,
                sources,
                source_maps,
                report,
                ..
            } => {
                let expected = if cancel {
                    StopReason::Cancelled
                } else {
                    StopReason::WorkLimit
                };
                assert_eq!(reason, expected);
                assert_eq!(b.poll(), Err(expected));
                assert_eq!(sources, vec![generated]);
                assert_eq!(source_maps, vec![mapping]);
                assert_eq!(report.diagnostics.len(), 1);
                assert_eq!(report.events.len(), 1);
                assert!(
                    session.pending_read().is_err(),
                    "no partial Await may remain"
                );
            }
            ReadReply::Await {
                continuation,
                report,
                ..
            } => {
                assert!(!cancel);
                assert_eq!(continuation.current.sources, vec![generated]);
                assert_eq!(continuation.current.source_maps, vec![mapping]);
                assert_eq!(report.diagnostics.len(), 1);
                assert_eq!(report.events.len(), 1);
                assert!(session.pending_read().is_ok());
            }
            _ => return Err(ReaderError::Context),
        }
        Ok((b.usage().work - before, awaited))
    };
    let (full, awaited) = execute(None, false)?;
    assert!(awaited);
    // Exhaust every Work boundary through echo validation, next-call creation
    // and publication. The union is assembled for the next call and cloned
    // with its costs retained for the continuation, never accepted from echo.
    assert!(full > 0 && full < 100_000, "bounded fixture work={full}");
    for remaining in 0..=full {
        let (used, awaited) = execute(Some(remaining), false)?;
        assert!(used <= remaining);
        assert_eq!(awaited, remaining == full);
    }
    assert_eq!(execute(None, true)?, (0, false));
    Ok(())
}

#[test]
fn dependent_and_transform_continuations_keep_original_metadata_and_source_closure()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    for dependent in [false, true] {
        let mut p = provider_plan(&schema);
        p.state_type = TypeDescriptor::U64;
        p.providers[0].state_type = TypeDescriptor::U64;
        let mut next = signature(
            &schema,
            if dependent {
                ProviderKind::Dependent
            } else {
                ProviderKind::Transform
            },
        );
        next.state_type = TypeDescriptor::U64;
        p.expressions.push(ReaderExpr::Scalar(CharClass::Any));
        p.expressions.push(if dependent {
            ReaderExpr::Then {
                first: ReaderId(1),
                provider: next.operation.clone(),
            }
        } else {
            ReaderExpr::Map {
                body: ReaderId(1),
                provider: next.operation.clone(),
            }
        });
        p.expressions
            .push(ReaderExpr::Seq(vec![ReaderId(0), ReaderId(2)]));
        p.providers.push(next);
        p.rules[0].root = ReaderId(3);
        p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
        let checked = p.check(&registry, &mut budget())?;
        let input = source("abc")?;
        let mut store = SourceStore::default();
        store.insert(input.clone())?;
        let raw = context(&schema, &registry)?;
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let ctx = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
        let mut session =
            ReaderSession::new("prepared-closure".into(), &checked, &registry, &mut b)?;
        let reply = session.read(
            "entry",
            ReadRequest {
                snapshot: &input,
                start: 0,
                limit: 3,
                final_input: true,
                context: &ctx,
                state: &NdfValue::U64(0),
            },
            &store,
            &mut b,
            &mut admission,
        )?;
        let ReadReply::Await { continuation, .. } = reply else {
            return Err(ReaderError::Context);
        };
        let generated = admission.create(
            SourceId("generated".into()),
            0,
            "memory:generated".into(),
            b"x".to_vec(),
            &mut b,
        )?;
        let mut first = terminal("a", 1, &mut b)?;
        let ProviderReply::Read(reply) = &mut first else {
            return Err(ReaderError::Context);
        };
        let ReadReply::Matched {
            new_state, sources, ..
        } = reply.as_mut()
        else {
            return Err(ReaderError::Context);
        };
        *new_state = NdfValue::U64(7);
        sources.push(generated.clone());
        let reply = session.resume(&continuation, first, &store, &mut b, &mut admission)?;
        let ReadReply::Await { continuation, .. } = reply else {
            return Err(ReaderError::Context);
        };
        assert_eq!(continuation.request.start, 0);
        assert_eq!(continuation.request.state, NdfValue::U64(0));
        assert_eq!(
            continuation.request.sources,
            vec![input.clone(), generated.clone()]
        );
        assert_eq!(continuation.current.sources, vec![generated]);
        assert_eq!(continuation.current.state, NdfValue::U64(7));
        match &continuation.pending {
            ProviderCall::Dependent { request, .. } => {
                assert!(dependent);
                assert_eq!(request.request.start, 2);
                assert_eq!(request.request.state, NdfValue::U64(7));
                assert_eq!(request.request.sources, continuation.request.sources);
            }
            ProviderCall::Transform { request, .. } => {
                assert!(!dependent);
                assert_eq!(request.value, NdfValue::Text("b".into()));
            }
            _ => return Err(ReaderError::Context),
        }
    }
    Ok(())
}
