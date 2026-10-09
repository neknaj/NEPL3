use super::*;
use nepl3_provider::reply::ReplyError;
fn implementation() -> Digest {
    Digest::of(b"reserved transport fixture")
}
fn prepared() -> Result<(SchemaRegistry, Invoke, OperationReply, Usage), String> {
    let (registry, mut request) = fixture()?;
    quota(&mut request);
    let mut execution = Budget::new(request.limits);
    let mut reply =
        increment(&request, implementation(), &registry, &mut execution).map_err(error)?;
    let observed = execution.usage();
    let OperationReply::Result(OperationResult::Complete { report, .. }) = &mut reply else {
        return Err("Complete".into());
    };
    report.usage.work = u64::MAX;
    Ok((registry, request, reply, observed))
}
fn reply_frame(request: &Invoke, reply: OperationReply) -> ProviderFrame {
    ProviderFrame::Reply {
        request_id: request.request_id,
        reply,
    }
}
fn direct(
    registry: &SchemaRegistry,
    request: &Invoke,
    frame: &ProviderFrame,
    context: Digest,
    b: &mut Budget,
) -> Result<(Usage, Usage, OperationReply), String> {
    let mut c = connection(frame, registry)?;
    direct_connection(registry, request, &mut c, context, b)
}
fn local_limits(parent: Limits, grant: Limits) -> Limits {
    Limits {
        source_bytes: parent.source_bytes - grant.source_bytes,
        work: parent.work - grant.work,
        depth: parent.depth,
        nodes: parent.nodes - grant.nodes,
        allocation_units: parent.allocation_units - grant.allocation_units,
        output_bytes: parent.output_bytes - grant.output_bytes,
        diagnostics: parent.diagnostics - grant.diagnostics,
        events: parent.events - grant.events,
    }
}
fn direct_connection(
    registry: &SchemaRegistry,
    request: &Invoke,
    c: &mut Connection<Cursor<Vec<u8>>, Vec<u8>>,
    context: Digest,
    b: &mut Budget,
) -> Result<(Usage, Usage, OperationReply), String> {
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    // Frame construction is fixture setup, outside the measured transport.
    c.send(
        &ProviderFrame::Invoke(request.clone()),
        registry,
        &sources,
        &mut admission,
        b,
    )
    .map_err(error)?;
    let sent = b.usage();
    let Some(ProviderFrame::Reply { request_id, reply }) = c
        .receive(registry, &sources, &mut admission, b)
        .map_err(error)?
    else {
        return Err("reply".into());
    };
    if request_id != request.request_id {
        return Err("request id".into());
    }
    let received = b.usage();
    nepl3_suite::dispatch::suspending::validate_reply(
        &reply, request, context, registry, &sources, b,
    )
    .map_err(error)?;
    Ok((sent, received, reply))
}
#[test]
fn sent_invoke_and_received_reply_use_one_reserved_parent_and_exact_settlement()
-> Result<(), String> {
    let (registry, request, reply, observed) = prepared()?;
    let frame = reply_frame(&request, reply);
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut parent = budget();
    parent.charge(Resource::Work, 43).map_err(error)?;
    let mut issued = IssuedInvocation::issue(
        grants.admit(&request, &mut budget()).map_err(error)?,
        implementation(),
        &registry,
        &mut parent,
        &mut budget(),
    )
    .map_err(error)?;
    let context = issued.context();
    let mut oracle = budget();
    oracle
        .record_observed_usage(issued.parent_usage())
        .map_err(error)?;
    let (_, _, expected) = direct(&registry, &request, &frame, context, &mut oracle)?;
    request.check_saved(&request, &mut oracle).map_err(error)?;
    oracle.record_observed_usage(observed).map_err(error)?;
    let mut c = connection(&frame, &registry)?;
    let mut admission = SourceAdmission::default();
    let outgoing = ProviderFrame::Invoke(request.clone());
    issued
        .run_local(|b| Ok(c.send(&outgoing, &registry, &sources, &mut admission, b)))
        .map_err(error)?
        .map_err(error)?;
    let received = issued
        .run_local(|b| {
            Ok(c.receive_reply_with_budget(
                &request,
                context,
                &registry,
                &sources,
                &sources,
                &mut admission,
                b,
            ))
        })
        .map_err(error)?
        .map_err(error)?;
    assert_eq!(received, expected);
    assert!(!c.is_closed());
    let OperationReply::Result(OperationResult::Complete { report, .. }) = &received else {
        return Err("Complete".into());
    };
    assert_eq!(report.usage.work, u64::MAX);
    issued
        .settle_saved_request(&request, implementation(), context, observed)
        .map_err(error)?;
    assert_eq!(parent.usage(), oracle.usage());
    assert_eq!(parent.poll(), Ok(()));
    assert!(parent.usage().work < u64::MAX);
    let (_, sent) = c.into_parts();
    let (decoded, rest) = nepl3_wire::operation::decode_frame(
        &sent,
        true,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?
    .ok_or("sent frame")?;
    assert_eq!(decoded, outgoing);
    assert!(rest.is_empty());

    Ok(())
}
#[test]
fn local_ceiling_distinguishes_send_stop_from_reply_validation_stop() -> Result<(), String> {
    let (registry, request, reply, _) = prepared()?;
    let frame = reply_frame(&request, reply);
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let context =
        nepl3_wire::operation::context_digest(&request, implementation(), &registry, &mut budget())
            .map_err(error)?;
    let mut full = budget();
    let (sent, received, _) = direct(&registry, &request, &frame, context, &mut full)?;
    assert!(full.usage().work > received.work);
    for phase in 0..3 {
        let local_work = match phase {
            0 => sent.work - 1,
            1 => received.work - 1,
            _ => full.usage().work - 1,
        };
        let limits = Limits {
            work: request.limits.work + local_work,
            ..budget().limits()
        };
        let mut parent = Budget::new(limits);
        let mut issued = IssuedInvocation::issue(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent,
            &mut budget(),
        )
        .map_err(error)?;
        let mut c = connection(&frame, &registry)?;
        let mut admission = SourceAdmission::default();
        let outgoing = ProviderFrame::Invoke(request.clone());
        let sent_result =
            issued.run_local(|b| Ok(c.send(&outgoing, &registry, &sources, &mut admission, b)));
        if phase != 0 {
            sent_result.map_err(error)?.map_err(error)?;
            let failure = issued
                .run_local(|b| {
                    Ok(c.receive_reply_with_budget(
                        &request,
                        context,
                        &registry,
                        &sources,
                        &sources,
                        &mut admission,
                        b,
                    ))
                })
                .err()
                .ok_or("expected validation stop")?;
            assert_eq!(failure.reason, StopReason::WorkLimit);
            if phase == 2 {
                assert!(matches!(
                    failure.output,
                    Some(Err(ReplyError::Validation(
                        nepl3_suite::dispatch::suspending::Error::Stopped(StopReason::WorkLimit)
                    )))
                ));
                assert!(issued.parent_usage().work >= received.work);
            } else {
                assert!(matches!(
                    failure.output,
                    Some(Err(ReplyError::Transport(TransportError::Stopped(
                        StopReason::WorkLimit
                    ))))
                ));
            }
        } else {
            let failure = sent_result.err().ok_or("expected send stop")?;
            assert_eq!(failure.reason, StopReason::WorkLimit);
            assert!(matches!(
                failure.output,
                Some(Err(TransportError::Stopped(StopReason::WorkLimit)))
            ));
        }
        assert!(c.is_closed());
        let spent = issued.parent_usage();
        let mut oracle = Budget::new(local_limits(limits, request.limits));
        assert!(direct(&registry, &request, &frame, context, &mut oracle).is_err());
        assert_eq!(spent, oracle.usage());
        let mut called = false;
        assert!(
            issued
                .run_local(|_| {
                    called = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!called);
        drop(issued);
        assert_eq!(parent.usage(), spent);
        assert_eq!(parent.poll(), Err(StopReason::WorkLimit));
        assert_eq!(parent.limits(), limits);
    }
    Ok(())
}
#[test]
fn nonbudget_reply_rejection_retains_local_cost_and_drops_unsettled_grant() -> Result<(), String> {
    let (registry, request, reply, _) = prepared()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    for variant in 0..4 {
        let mut frame = reply_frame(&request, reply.clone());
        match variant {
            0 => {
                let ProviderFrame::Reply { request_id, .. } = &mut frame else {
                    return Err("reply".into());
                };
                *request_id += 1;
            }
            1 => {
                frame = ProviderFrame::Cancel {
                    request_id: request.request_id,
                }
            }
            2 => {
                let ProviderFrame::Reply {
                    reply: OperationReply::Result(OperationResult::Complete { value, .. }),
                    ..
                } = &mut frame
                else {
                    return Err("complete".into());
                };
                *value = TypedValue::Record(Record {
                    schema: registry
                        .selected("nepl3.foundation", 1)
                        .ok_or("foundation")?
                        .clone(),
                    kind: "Limits".into(),
                    fields: vec![NdfValue::U64(1); 8],
                });
            }
            _ => {}
        }
        let mut parent = budget();
        let mut issued = IssuedInvocation::issue(
            grants.admit(&request, &mut budget()).map_err(error)?,
            implementation(),
            &registry,
            &mut parent,
            &mut budget(),
        )
        .map_err(error)?;
        let context = issued.context();
        let mut c = connection(&frame, &registry)?;
        if variant == 3 {
            let (reader, writer) = c.into_parts();
            let mut bytes = reader.into_inner();
            bytes.pop();
            c = Connection::new(Cursor::new(bytes), writer);
        }
        let mut oracle_connection = connection(&frame, &registry)?;
        if variant == 3 {
            let (reader, writer) = oracle_connection.into_parts();
            let mut bytes = reader.into_inner();
            bytes.pop();
            oracle_connection = Connection::new(Cursor::new(bytes), writer);
        }
        let mut oracle = Budget::new(local_limits(budget().limits(), request.limits));
        assert!(
            direct_connection(
                &registry,
                &request,
                &mut oracle_connection,
                context,
                &mut oracle
            )
            .is_err()
        );
        let outgoing = ProviderFrame::Invoke(request.clone());
        let mut admission = SourceAdmission::default();
        issued
            .run_local(|b| Ok(c.send(&outgoing, &registry, &sources, &mut admission, b)))
            .map_err(error)?
            .map_err(error)?;
        let sent = issued.parent_usage();
        let failure = issued
            .run_local(|b| {
                Ok(c.receive_reply_with_budget(
                    &request,
                    context,
                    &registry,
                    &sources,
                    &sources,
                    &mut admission,
                    b,
                ))
            })
            .map_err(error)?
            .err()
            .ok_or("expected rejection")?;
        match variant {
            0 => assert!(matches!(failure, ReplyError::RequestId { .. })),
            1 => assert!(matches!(failure, ReplyError::UnexpectedFrame)),
            2 => assert!(matches!(failure, ReplyError::Validation(_))),
            _ => assert!(matches!(
                failure,
                ReplyError::Transport(TransportError::Truncated)
            )),
        };
        assert!(c.is_closed());
        assert!(issued.parent_usage().work > sent.work);
        let spent = issued.parent_usage();
        assert_eq!(spent, oracle.usage());
        assert_eq!(issued.run_local(|b| Ok(b.poll())).map_err(error)?, Ok(()));
        drop(issued);
        assert_eq!(parent.usage(), spent);
        assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    }
    Ok(())
}
#[test]
fn send_output_ceiling_and_broken_pipe_keep_actual_cost_before_guard_drop() -> Result<(), String> {
    let (registry, request, _, _) = prepared()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let outgoing = ProviderFrame::Invoke(request.clone());
    let limits = Limits {
        output_bytes: request.limits.output_bytes,
        ..budget().limits()
    };
    let mut parent = Budget::new(limits);
    let mut issued = IssuedInvocation::issue(
        grants.admit(&request, &mut budget()).map_err(error)?,
        implementation(),
        &registry,
        &mut parent,
        &mut budget(),
    )
    .map_err(error)?;
    let mut c = Connection::new(io::empty(), Vec::new());
    let failure = issued
        .run_local(|b| {
            Ok(c.send(
                &outgoing,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                b,
            ))
        })
        .err()
        .ok_or("output stop")?;
    assert_eq!(failure.reason, StopReason::OutputLimit);
    assert!(matches!(
        failure.output,
        Some(Err(TransportError::Stopped(StopReason::OutputLimit)))
    ));
    assert!(c.is_closed());
    let (_, bytes) = c.into_parts();
    assert!(bytes.is_empty());
    let prefix = issued.parent_usage();
    assert!(prefix.work > 0);
    assert!(prefix.allocation_units > 0);
    drop(issued);
    assert_eq!(parent.usage(), prefix);
    assert_eq!(parent.limits(), limits);
    assert_eq!(parent.poll(), Err(StopReason::OutputLimit));
    let mut parent = budget();
    let mut issued = IssuedInvocation::issue(
        grants.admit(&request, &mut budget()).map_err(error)?,
        implementation(),
        &registry,
        &mut parent,
        &mut budget(),
    )
    .map_err(error)?;
    let mut c = Connection::new(io::empty(), crate::Broken);
    let result = issued
        .run_local(|b| {
            Ok(c.send(
                &outgoing,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                b,
            ))
        })
        .map_err(error)?;
    assert!(matches!(result,Err(TransportError::Io(e)) if e.kind()==io::ErrorKind::BrokenPipe));
    assert!(c.is_closed());
    let prefix = issued.parent_usage();
    assert!(prefix.work > 0);
    assert!(prefix.output_bytes > 0);
    assert_eq!(issued.run_local(|b| Ok(b.poll())).map_err(error)?, Ok(()));
    drop(issued);
    assert_eq!(parent.usage(), prefix);
    assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    Ok(())
}
#[test]
fn one_budget_entry_matches_legacy_reply_and_error_stages() -> Result<(), String> {
    let (registry, request, terminal, _) = prepared()?;
    let sources = SourceStore::default();
    let context = Digest::of(b"context");
    let mut child = request.clone();
    child.request_id += 1;
    let awaiting = OperationReply::Await {
        continuation: Continuation {
            provider: request.operation.clone(),
            parent_request: request.request_id,
            snapshot_digest: context,
            state: request.input.clone(),
        },
        calls: vec![child],
        report: Report::default(),
    };
    for variant in 0..6 {
        let mut frame = reply_frame(
            &request,
            if variant == 0 {
                terminal.clone()
            } else {
                awaiting.clone()
            },
        );
        match variant {
            2 => {
                let ProviderFrame::Reply {
                    reply: OperationReply::Await { continuation, .. },
                    ..
                } = &mut frame
                else {
                    return Err("Await".into());
                };
                continuation.snapshot_digest = Digest::of(b"wrong");
            }
            3 => {
                let ProviderFrame::Reply { request_id, .. } = &mut frame else {
                    return Err("reply".into());
                };
                *request_id += 1;
            }
            4 => frame = ProviderFrame::Close,
            _ => {}
        }
        let mut old = connection(&frame, &registry)?;
        let mut new = connection(&frame, &registry)?;
        if variant == 5 {
            old = Connection::new(Cursor::new(vec![]), vec![]);
            new = Connection::new(Cursor::new(vec![]), vec![]);
        }
        let prior = old.receive_reply(
            &request,
            context,
            &registry,
            &sources,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
            &mut budget(),
        );
        let current = new.receive_reply_with_budget(
            &request,
            context,
            &registry,
            &sources,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        );
        match (prior, current) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a, b);
                assert!(variant < 2);
            }
            (Err(a), Err(b)) => assert_eq!(format!("{a:?}"), format!("{b:?}")),
            _ => return Err("different outcome".into()),
        };
        assert_eq!(old.is_closed(), new.is_closed());
        assert_eq!(new.is_closed(), variant >= 2);
    }
    Ok(())
}
