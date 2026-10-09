use super::*;
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    portable::PortableError,
};
use nepl3_wire::foundation::FoundationCodec;
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn grant() -> Limits {
    let l = budget().limits();
    Limits {
        source_bytes: 0,
        work: l.work / 4,
        allocation_units: l.allocation_units / 4,
        nodes: l.nodes / 4,
        output_bytes: l.output_bytes / 4,
        diagnostics: l.diagnostics / 4,
        events: l.events / 4,
        ..l
    }
}
#[test]
fn native_dispatch_uses_pending_prefix_and_preserves_scope() -> Result<(), String> {
    run(0)
}
#[test]
fn native_dispatch_rejects_stops_bad_grants_and_linked_type() -> Result<(), String> {
    for mode in 1..=14 {
        run(mode).map_err(|e| format!("mode {mode}: {e}"))?;
    }
    Ok(())
}
#[test]
fn native_dispatch_resumes_no_match_and_nonfinal_need_more() -> Result<(), String> {
    for mode in [15, 16] {
        run(mode).map_err(|e| format!("mode {mode}: {e}"))?;
    }
    Ok(())
}
#[test]
fn native_need_more_allows_a_new_read_on_an_extended_revision() -> Result<(), String> {
    run(17)
}
fn run(mode: u8) -> Result<(), String> {
    let (registry, schema) = registry().map_err(err)?;
    let mut signature =
        provider::signature(BuiltinReader::Name, &registry, &mut budget()).map_err(err)?;
    if mode == 11 {
        signature.state_type = TypeDescriptor::NdfValue;
    }
    if mode == 12 {
        signature.continuation_type = TypeDescriptor::Unit;
    }
    if mode == 13 {
        signature = super::signature(&schema, ProviderKind::Read);
    }
    if mode == 7 {
        signature.value_output = TypeDescriptor::Rational;
    }
    let mut p = plan(
        &schema,
        vec![
            ReaderExpr::Literal("前 ".into()),
            ReaderExpr::Call(signature.operation.clone()),
            ReaderExpr::Seq(vec![ReaderId(0), ReaderId(1)]),
        ],
        2,
        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    );
    if mode == 11 {
        p.state_type = TypeDescriptor::NdfValue;
    }
    p.providers.push(signature);
    if mode == 12 {
        assert!(matches!(
            p.check(&registry, &mut budget()),
            Err(PlanError::ProviderSignature)
        ));
        return Ok(());
    }
    // Trusted input fixtures are prepared before the measured Reader operation.
    let extended = SourceSnapshot::new(
        SourceId("test".into()),
        1,
        "memory:test".into(),
        "前 後 ".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let checked = p.check(&registry, &mut budget()).map_err(err)?;
    let mut b = budget();
    b.charge(Resource::Work, 123).map_err(err)?;
    let mut session =
        ReaderSession::new("native-dispatch".into(), &checked, &registry, &mut b).map_err(err)?;
    let input = match mode {
        15 => "前 !",
        16 | 17 => "前 後",
        _ => "前 後 ",
    };
    let source = source(input).map_err(err)?;
    let mut sources = SourceStore::default();
    sources.insert(source.clone()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let raw = context(&schema, &registry).map_err(err)?;
    let context = check_context(&raw, &sources, &registry, &mut b, &mut admission).map_err(err)?;
    let suspended = b
        .with_depth_at_least(32, |b| {
            session.read(
                "entry",
                ReadRequest {
                    snapshot: &source,
                    start: 0,
                    limit: input.len() as u64,
                    final_input: !matches!(mode, 16 | 17),
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
    } = suspended
    else {
        return Err("await".into());
    };
    let ProviderCall::Read {
        request,
        depth_base,
        ..
    } = call.as_ref()
    else {
        return Err("read".into());
    };
    assert_eq!(request.start, 4);
    assert_eq!(continuation.request.start, 0);
    let outer = b.limits();
    let depth = b.current_depth();
    let source_bytes = b.usage().source_bytes;
    let mut allowed = grant();
    if mode == 1 {
        b.cancel();
    }
    if mode == 14 {
        allowed.work = 64;
    }
    if mode == 2 {
        allowed.work = 0;
    }
    if mode == 3 {
        allowed.depth = b.usage().depth - 1;
    }
    if mode == 8 {
        allowed.work = outer.work;
    }
    if mode == 4 {
        allowed.work = u64::MAX;
    }
    if mode == 6 {
        session.close();
    }
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    if mode == 10 {
        let empty =
            ReaderSession::new("empty-session".into(), &checked, &registry, &mut b).map_err(err)?;
        let before = b.usage();
        assert!(matches!(
            empty.execute_pending_name(allowed, &mut codec, &mut b),
            Err(PortableError::Reader(ReaderError::NoPending))
        ));
        assert_eq!(b.usage(), before);
        assert!(session.pending_read().is_ok());
        return Ok(());
    }
    if mode == 5 || mode == 9 {
        let mut wrong = Budget::new(Limits {
            work: if mode == 5 {
                outer.work - 1
            } else {
                outer.work
            },
            ..outer
        });
        assert!(matches!(
            session.execute_pending_name(allowed, &mut codec, &mut wrong),
            Err(PortableError::Reader(ReaderError::Continuation))
        ));
        assert!(session.pending_read().is_ok());
        return Ok(());
    }
    if mode == 2 || mode == 14 {
        let issued = session
            .execute_pending_name(allowed, &mut codec, &mut b)
            .map_err(err)?;
        assert!(matches!(
            issued.reply(),
            ReadReply::Stopped {
                reason: StopReason::WorkLimit,
                ..
            }
        ));
        let used = b.usage();
        assert_eq!(issued.execution_usage(), used);
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        assert_eq!(b.limits(), outer);
        assert_eq!(b.current_depth(), depth);
        assert!(session.pending_read().is_ok());
        let encoded = issued
            .to_operation_reply(&mut codec, &mut budget())
            .map_err(err)?;
        assert!(
            matches!(encoded,nepl3_core::operation::OperationReply::Result(nepl3_core::diagnostic::OperationResult::Stopped{reason:StopReason::WorkLimit,report,..}) if report.usage==used)
        );
        assert_eq!(b.usage(), used);
        return Ok(());
    }
    if mode != 0 && mode != 15 && mode != 16 && mode != 17 {
        let result = session.execute_pending_name(allowed, &mut codec, &mut b);
        match mode {
            1 => assert!(matches!(
                result,
                Err(PortableError::Stopped(StopReason::Cancelled))
            )),
            3 => assert!(matches!(
                result,
                Err(PortableError::Stopped(StopReason::DepthLimit))
            )),
            4 | 8 => assert!(matches!(
                result,
                Err(PortableError::Stopped(StopReason::WorkLimit))
            )),
            6 => assert!(matches!(
                result,
                Err(PortableError::Reader(ReaderError::Closed))
            )),
            7 | 11 | 13 => assert!(matches!(
                result,
                Err(PortableError::Reader(ReaderError::ProviderContract))
            )),
            _ => return Err("mode".into()),
        }
        assert_eq!(b.limits(), outer);
        assert_eq!(b.current_depth(), depth);
        if mode <= 4 || mode == 8 {
            let reason = if mode == 1 {
                StopReason::Cancelled
            } else if mode == 3 {
                StopReason::DepthLimit
            } else {
                StopReason::WorkLimit
            };
            assert_eq!(b.poll(), Err(reason));
        }
        if mode != 6 {
            assert!(session.pending_read().is_ok());
        }
        return Ok(());
    }
    let native_reply=b.with_depth_at_least(64,|b|{
        let issued=session.execute_pending_name(allowed,&mut codec,b)?;
        match mode {
            15 => assert!(matches!(issued.reply(), ReadReply::NoMatch { furthest: 4, .. })),
            16 | 17 => assert!(matches!(issued.reply(), ReadReply::NeedMore { .. })),
            _ => assert!(matches!(issued.reply(),ReadReply::Matched{value:NdfValue::Text(value),end,..} if value=="後" && *end==7)),
        }
        assert!(issued.execution_usage().depth>=64 && issued.execution_usage().depth>=*depth_base);
        assert_eq!(b.current_depth(),64);assert_eq!(b.limits(),outer);assert_eq!(b.usage().source_bytes,source_bytes);
        Ok::<_, PortableError<nepl3_wire::WireError>>(issued.into_reply())
    }).map_err(err)?;
    assert_eq!(b.current_depth(), depth);
    assert_eq!(b.limits(), outer);
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<ReadReply>() as u64,
    )
    .map_err(err)?;
    let result = session
        .resume(
            &continuation,
            ProviderReply::Read(Box::new(native_reply)),
            &sources,
            &mut b,
            &mut admission,
        )
        .map_err(err)?;
    match mode {
        15 => assert!(matches!(result, ReadReply::NoMatch { furthest: 4, .. })),
        16 | 17 => assert!(matches!(result, ReadReply::NeedMore { .. })),
        _ => assert!(matches!(result, ReadReply::Matched { end: 7, .. })),
    }
    assert!(matches!(
        session.pending_read(),
        Err(ReaderError::NoPending)
    ));
    assert_eq!(b.current_depth(), depth);
    assert_eq!(b.limits(), outer);
    assert_eq!(b.usage().source_bytes, source_bytes);
    if mode == 17 {
        assert_eq!(source_bytes, 7);
        // NeedMore is terminal for the old immutable snapshot. This is a fresh
        // read of a larger revision, not a forged resume of the consumed slot.
        sources.insert(extended.clone()).map_err(err)?;
        let context =
            check_context(&raw, &sources, &registry, &mut b, &mut admission).map_err(err)?;
        let next = session
            .read(
                "entry",
                ReadRequest {
                    snapshot: &extended,
                    start: 0,
                    limit: 8,
                    final_input: true,
                    context: &context,
                    state: &NdfValue::Unit,
                },
                &sources,
                &mut b,
                &mut admission,
            )
            .map_err(err)?;
        let ReadReply::Await {
            continuation: next_continuation,
            call: next_call,
            ..
        } = next
        else {
            return Err("extended revision did not await".into());
        };
        let ProviderCall::Read {
            request: next_request,
            call_id: next_id,
            ..
        } = next_call.as_ref()
        else {
            return Err("extended revision did not request Read".into());
        };
        let ProviderCall::Read {
            call_id: old_id, ..
        } = call.as_ref()
        else {
            return Err("old Read".into());
        };
        assert_ne!(next_id, old_id);
        assert_eq!(next_request.snapshot.revision, 1);
        assert_eq!(next_request.start, 4);
        assert_eq!(b.usage().source_bytes, source_bytes + 8);
        let reply = {
            let mut codec =
                FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
            session
                .execute_pending_name(grant(), &mut codec, &mut b)
                .map_err(err)?
                .into_reply()
        };
        assert!(
            matches!(&reply, ReadReply::Matched { value: NdfValue::Text(v), end: 7, .. } if v == "後")
        );
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<ReadReply>() as u64,
        )
        .map_err(err)?;
        let before_stale = b.usage();
        assert!(matches!(
            session.resume(
                &continuation,
                ProviderReply::Read(Box::new(reply)),
                &sources,
                &mut b,
                &mut admission,
            ),
            Err(ReaderError::Continuation)
        ));
        assert!(session.pending_read().is_ok());
        // Successful resume with next_continuation below also verifies the
        // private saved continuation remained the exact revision-1 state.
        assert!(b.usage().work >= before_stale.work);
        b.poll().map_err(err)?;
        let after_stale = b.usage();
        // The rejected owned reply is gone. Re-execute the pure pending builtin
        // with the same charged budget; this is not an unmetered encoding retry.
        let reply = {
            let mut codec =
                FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
            session
                .execute_pending_name(grant(), &mut codec, &mut b)
                .map_err(err)?
                .into_reply()
        };
        assert!(b.usage().work > after_stale.work);
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<ReadReply>() as u64,
        )
        .map_err(err)?;
        let finished = session
            .resume(
                &next_continuation,
                ProviderReply::Read(Box::new(reply)),
                &sources,
                &mut b,
                &mut admission,
            )
            .map_err(err)?;
        assert!(matches!(finished, ReadReply::Matched { end: 7, .. }));
        assert!(matches!(
            session.pending_read(),
            Err(ReaderError::NoPending)
        ));
        assert_eq!(b.usage().source_bytes, source_bytes + 8);
        assert_eq!(b.limits(), outer);
        assert_eq!(b.current_depth(), depth);
    }
    Ok(())
}
