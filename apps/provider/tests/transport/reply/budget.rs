use super::*;

fn shared(
    connection: &mut Connection<Cursor<Vec<u8>>, Vec<u8>>,
    request: &Invoke,
    context: Digest,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<OperationReply, ReplyError> {
    let sources = SourceStore::default();
    connection.receive_reply_with_budget(
        request,
        context,
        registry,
        &sources,
        &sources,
        &mut SourceAdmission::default(),
        budget,
    )
}

fn replies(request: &Invoke, context: Digest) -> [OperationReply; 2] {
    [
        OperationReply::Result(OperationResult::Complete {
            value: request.input.clone(),
            report: Report {
                usage: Usage {
                    work: 123_456,
                    allocation_units: 654_321,
                    ..Usage::default()
                },
                ..Report::default()
            },
        }),
        OperationReply::Await {
            continuation: Continuation {
                provider: request.operation.clone(),
                parent_request: request.request_id,
                snapshot_digest: context,
                state: request.input.clone(),
            },
            calls: vec![],
            report: Report::default(),
        },
    ]
}

#[test]
fn one_budget_retains_prior_usage_and_exact_residual_capacity() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let context = Digest::of(b"one cumulative host budget");
    let sources = SourceStore::default();
    for reply in replies(&request, context) {
        let frame = ProviderFrame::Reply {
            request_id: request.request_id,
            reply: reply.clone(),
        };
        // Independent oracle bypasses both high-level reply APIs and their helper.
        let mut transport = budget();
        let mut validation = budget();
        let mut raw = connection(&frame, &registry)?;
        assert_eq!(
            raw.receive(
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut transport
            )
            .map_err(error)?,
            Some(frame.clone())
        );
        nepl3_suite::dispatch::suspending::validate_reply(
            &reply,
            &request,
            context,
            &registry,
            &sources,
            &mut validation,
        )
        .map_err(error)?;
        let mut legacy_transport = budget();
        let mut legacy_validation = budget();
        let mut legacy = connection(&frame, &registry)?;
        assert_eq!(
            legacy
                .receive_reply(
                    &request,
                    context,
                    &registry,
                    &sources,
                    &sources,
                    &mut SourceAdmission::default(),
                    &mut legacy_transport,
                    &mut legacy_validation,
                )
                .map_err(error)?,
            reply
        );
        assert_eq!(legacy_transport.usage(), transport.usage());
        assert_eq!(legacy_validation.usage(), validation.usage());
        assert!(validation.usage().work > 0);
        let prior = 7;
        let t = transport.usage();
        let v = validation.usage();
        let exact = prior + t.work + v.work;
        let expected_usage = Usage {
            source_bytes: t.source_bytes + v.source_bytes,
            work: exact,
            depth: t.depth.max(v.depth),
            nodes: t.nodes + v.nodes,
            allocation_units: t.allocation_units + v.allocation_units,
            output_bytes: t.output_bytes + v.output_bytes,
            diagnostics: t.diagnostics + v.diagnostics,
            events: t.events + v.events,
        };
        for (limit, succeeds) in [(exact, true), (exact - 1, false)] {
            let mut b = Budget::new(Limits {
                work: limit,
                ..budget().limits()
            });
            b.charge(Resource::Work, prior).map_err(error)?;
            let mut c = connection(&frame, &registry)?;
            let result = shared(&mut c, &request, context, &registry, &mut b);
            if succeeds {
                assert_eq!(result.map_err(error)?, reply);
                assert_eq!(b.usage(), expected_usage);
                assert!(!c.is_closed());
            } else {
                assert!(matches!(
                    result,
                    Err(ReplyError::Validation(
                        nepl3_suite::dispatch::suspending::Error::Stopped(StopReason::WorkLimit)
                    ))
                ));
                assert!(c.is_closed());
                assert_eq!(b.poll(), Err(StopReason::WorkLimit));
                let used = b.usage();
                assert!(shared(&mut c, &request, context, &registry, &mut b).is_err());
                assert_eq!(b.usage(), used);
            }
        }
        let allocation_prior = 11;
        let allocation_exact = allocation_prior + t.allocation_units + v.allocation_units;
        for (limit, succeeds) in [(allocation_exact, true), (allocation_exact - 1, false)] {
            let mut b = Budget::new(Limits {
                allocation_units: limit,
                ..budget().limits()
            });
            b.charge(Resource::AllocationUnits, allocation_prior)
                .map_err(error)?;
            let mut c = connection(&frame, &registry)?;
            let result = shared(&mut c, &request, context, &registry, &mut b);
            if succeeds {
                assert_eq!(result.map_err(error)?, reply);
                assert_eq!(b.usage().allocation_units, allocation_exact);
            } else {
                assert!(result.is_err());
                assert!(c.is_closed());
                assert_eq!(b.poll(), Err(StopReason::AllocationLimit));
            }
        }
    }
    Ok(())
}

#[test]
fn one_budget_preserves_protocol_and_context_rejection() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let context = Digest::of(b"saved context");
    for (frame, expected) in [
        (
            ProviderFrame::Reply {
                request_id: 99,
                reply: replies(&request, context)[0].clone(),
            },
            "id",
        ),
        (ProviderFrame::Cancel { request_id: 17 }, "direction"),
        (ProviderFrame::Close, "closed"),
        (
            ProviderFrame::Reply {
                request_id: 17,
                reply: replies(&request, Digest::of(b"forged"))[1].clone(),
            },
            "context",
        ),
    ] {
        let mut c = connection(&frame, &registry)?;
        let result = shared(&mut c, &request, context, &registry, &mut budget());
        match expected {
            "id" => assert!(matches!(
                result,
                Err(ReplyError::RequestId {
                    expected: 17,
                    received: 99
                })
            )),
            "direction" => assert!(matches!(result, Err(ReplyError::UnexpectedFrame))),
            "closed" => assert!(matches!(result, Err(ReplyError::Closed))),
            _ => assert!(matches!(
                result,
                Err(ReplyError::Validation(
                    nepl3_suite::dispatch::suspending::Error::Await(_)
                ))
            )),
        }
        assert!(c.is_closed());
    }
    Ok(())
}

#[test]
fn one_budget_honors_sticky_stops_before_consuming_input() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let context = Digest::of(b"context");
    let frame = ProviderFrame::Reply {
        request_id: 17,
        reply: replies(&request, context)[0].clone(),
    };
    for reason in [StopReason::Cancelled, StopReason::WorkLimit] {
        let mut c = connection(&frame, &registry)?;
        let mut b = budget();
        b.charge(Resource::Work, 7).map_err(error)?;
        b.stop(reason);
        b.cancel();
        let before = b.usage();
        assert!(matches!(
            shared(&mut c, &request, context, &registry, &mut b),
            Err(ReplyError::Transport(_))
        ));
        assert!(c.is_closed());
        assert_eq!(b.poll(), Err(reason));
        assert_eq!(b.usage(), before);
        assert_eq!(c.into_parts().0.position(), 0);
    }
    Ok(())
}

#[test]
fn one_budget_keeps_diagnostic_grants_separate_from_codec_sources() -> Result<(), String> {
    use nepl3_core::diagnostic::{Diagnostic, Severity};
    let (registry, request) = fixture()?;
    let source = SourceSnapshot::new(
        SourceId("private".into()),
        1,
        "memory:private".into(),
        "世界".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let span = source.span(0, 6).map_err(error)?;
    let mut sources = SourceStore::default();
    sources
        .insert_with_budget(source, &mut budget())
        .map_err(error)?;
    let denied = SourceStore::default();
    let reply = OperationReply::Result(OperationResult::Invalid {
        partial: None,
        report: Report {
            diagnostics: vec![Diagnostic {
                schema: request.operation.schema.clone(),
                code: "fixture-error".into(),
                severity: Severity::Error,
                stage: "check".into(),
                arguments: request.input.clone(),
                primary: Some(span),
                related: vec![],
                fixes: vec![],
            }],
            usage: Usage {
                diagnostics: 1,
                ..Usage::default()
            },
            ..Report::default()
        },
    });
    let bytes = nepl3_wire::operation::encode_frame(
        &ProviderFrame::Reply {
            request_id: request.request_id,
            reply: reply.clone(),
        },
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    for (grants, allowed) in [(&sources, true), (&denied, false)] {
        let mut c = Connection::new(Cursor::new(bytes.clone()), Vec::new());
        let result = c.receive_reply_with_budget(
            &request,
            Digest::of(b"context"),
            &registry,
            &sources,
            grants,
            &mut SourceAdmission::default(),
            &mut budget(),
        );
        if allowed {
            assert_eq!(result.map_err(error)?, reply);
            assert!(!c.is_closed());
        } else {
            assert!(matches!(result, Err(ReplyError::Validation(_))));
            assert!(c.is_closed());
        }
    }
    Ok(())
}
