use super::*;
use nepl3_provider::reply::{ReplyContext, ReplyRoutes, RouteError};

#[test]
fn await_and_validation_stops_preserve_running_until_host_commit() -> Result<(), String> {
    use nepl3_core::operation::lifetime::{RequestLifetimes, RequestPhase};
    let (registry, request) = fixture()?;
    let sources = SourceStore::default();
    let context = Digest::of(b"saved");
    let entries = [ReplyContext {
        request: &request,
        context,
        authorized_sources: &sources,
    }];
    let routes = ReplyRoutes::new(&entries, &mut budget()).map_err(error)?;
    let awaiting = ProviderFrame::Reply {
        request_id: request.request_id,
        reply: OperationReply::Await {
            continuation: Continuation {
                provider: request.operation.clone(),
                parent_request: request.request_id,
                snapshot_digest: context,
                state: request.input.clone(),
            },
            calls: vec![],
            report: Report::default(),
        },
    };
    let mut observed_work = 0;
    for (frame, phase) in [
        (awaiting, RequestPhase::Running),
        (reply(&request), RequestPhase::Finished),
    ] {
        let mut lifetimes = RequestLifetimes::default();
        lifetimes
            .begin(
                request.request_id,
                request.operation.clone(),
                context,
                &mut budget(),
            )
            .map_err(error)?;
        let mut transport = connection(&frame, &registry)?;
        let mut validation = budget();
        transport
            .receive_active_reply(
                &routes,
                &mut lifetimes,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget(),
                &mut validation,
            )
            .map_err(error)?;
        assert_eq!(
            lifetimes
                .phase(request.request_id, &mut budget())
                .map_err(error)?,
            phase
        );
        if phase == RequestPhase::Finished {
            observed_work = validation.usage().work;
        }
    }
    // Every insufficient validation allowance, including the final finish lookup,
    // must preserve Running and close transport rather than publish completion.
    for work in 0..observed_work {
        let mut lifetimes = RequestLifetimes::default();
        lifetimes
            .begin(
                request.request_id,
                request.operation.clone(),
                context,
                &mut budget(),
            )
            .map_err(error)?;
        let mut transport = connection(&reply(&request), &registry)?;
        let mut validation = Budget::new(Limits {
            work,
            ..budget().limits()
        });
        assert!(
            transport
                .receive_active_reply(
                    &routes,
                    &mut lifetimes,
                    &registry,
                    &sources,
                    &mut SourceAdmission::default(),
                    &mut budget(),
                    &mut validation
                )
                .is_err()
        );
        assert!(transport.is_closed());
        assert_eq!(validation.poll(), Err(StopReason::WorkLimit));
        assert_eq!(
            lifetimes
                .phase(request.request_id, &mut budget())
                .map_err(error)?,
            RequestPhase::Running
        );
    }
    Ok(())
}

#[test]
fn active_routing_finishes_once_and_rejects_inactive_or_changed_bindings() -> Result<(), String> {
    use nepl3_core::operation::lifetime::{RequestLifetimes, RequestPhase};
    let (registry, request) = fixture()?;
    let sources = SourceStore::default();
    let context = Digest::of(b"saved");
    let entries = [ReplyContext {
        request: &request,
        context,
        authorized_sources: &sources,
    }];
    let routes = ReplyRoutes::new(&entries, &mut budget()).map_err(error)?;
    for mode in 0..6 {
        let mut lifetimes = RequestLifetimes::default();
        let mut provider = request.operation.clone();
        if mode == 4 {
            provider.name = "other".into();
        }
        if mode != 5 {
            lifetimes
                .begin(
                    request.request_id,
                    provider,
                    if mode == 3 {
                        Digest::of(b"changed")
                    } else {
                        context
                    },
                    &mut budget(),
                )
                .map_err(error)?;
        }
        match mode {
            1 => lifetimes
                .finish(request.request_id, &mut budget())
                .map_err(error)?,
            2 => lifetimes
                .cancel(request.request_id, &mut budget())
                .map_err(error)?,
            _ => (),
        }
        let mut transport = connection(&reply(&request), &registry)?;
        let outcome = transport.receive_active_reply(
            &routes,
            &mut lifetimes,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
            &mut budget(),
        );
        if mode == 0 {
            assert_eq!(outcome.map_err(error)?.0, 0);
            assert_eq!(
                lifetimes
                    .phase(request.request_id, &mut budget())
                    .map_err(error)?,
                RequestPhase::Finished
            );
            let mut duplicate = connection(&reply(&request), &registry)?;
            assert!(matches!(
                duplicate.receive_active_reply(
                    &routes,
                    &mut lifetimes,
                    &registry,
                    &sources,
                    &mut SourceAdmission::default(),
                    &mut budget(),
                    &mut budget()
                ),
                Err(RouteError::Lifetime(_))
            ));
            assert!(duplicate.is_closed());
        } else {
            assert!(matches!(outcome, Err(RouteError::Lifetime(_))));
            assert!(transport.is_closed());
        }
    }
    Ok(())
}

#[test]
fn diagnostics_use_selected_route_grants_instead_of_codec_sources() -> Result<(), String> {
    use nepl3_core::diagnostic::{Diagnostic, Severity};
    let (registry, first) = fixture()?;
    let mut second = first.clone();
    second.request_id += 1;
    let source = nepl3_core::source::SourceSnapshot::new(
        nepl3_core::source::SourceId("private".into()),
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
    let entries = [
        ReplyContext {
            request: &first,
            context: Digest::of(b"a"),
            authorized_sources: &sources,
        },
        ReplyContext {
            request: &second,
            context: Digest::of(b"b"),
            authorized_sources: &denied,
        },
    ];
    let routes = ReplyRoutes::new(&entries, &mut budget()).map_err(error)?;
    for (request, allowed) in [(&first, true), (&second, false)] {
        let response = ProviderFrame::Reply {
            request_id: request.request_id,
            reply: OperationReply::Result(OperationResult::Invalid {
                partial: None,
                report: Report {
                    diagnostics: vec![Diagnostic {
                        schema: request.operation.schema.clone(),
                        code: "fixture-error".into(),
                        severity: Severity::Error,
                        stage: "check".into(),
                        arguments: request.input.clone(),
                        primary: Some(span.clone()),
                        related: vec![],
                        fixes: vec![],
                    }],
                    usage: Usage {
                        diagnostics: 1,
                        ..Usage::default()
                    },
                    ..Report::default()
                },
            }),
        };
        let bytes = nepl3_wire::operation::encode_frame(
            &response,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        let mut transport = Connection::new(Cursor::new(bytes), Vec::new());
        let outcome = transport.receive_routed_reply(
            &routes,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
            &mut budget(),
        );
        if allowed {
            assert_eq!(outcome.map_err(error)?.0, 0);
            assert!(!transport.is_closed());
        } else {
            assert!(matches!(
                outcome,
                Err(RouteError::Reply(ReplyError::Validation(_)))
            ));
            assert!(transport.is_closed());
        }
    }
    Ok(())
}

fn reply(request: &Invoke) -> ProviderFrame {
    ProviderFrame::Reply {
        request_id: request.request_id,
        reply: OperationReply::Result(OperationResult::Complete {
            value: request.input.clone(),
            report: Report::default(),
        }),
    }
}

#[test]
fn reverse_arrival_uses_saved_request_and_returns_route_index() -> Result<(), String> {
    let (registry, first) = fixture()?;
    let mut second = first.clone();
    second.request_id += 1;
    let sources = SourceStore::default();
    let entries = [&first, &second].map(|request| ReplyContext {
        request,
        context: Digest::of(b"context"),
        authorized_sources: &sources,
    });
    let routes = ReplyRoutes::new(&entries, &mut budget()).map_err(error)?;
    // One transport carries responses in the reverse of registration order.
    let mut bytes = Vec::new();
    for request in [&second, &first] {
        let (input, _) = connection(&reply(request), &registry)?.into_parts();
        bytes.extend(input.into_inner());
    }
    let mut transport = Connection::new(Cursor::new(bytes), Vec::new());
    for (index, request) in [(1, &second), (0, &first)] {
        let (actual, response) = transport
            .receive_routed_reply(
                &routes,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget(),
                &mut budget(),
            )
            .map_err(error)?;
        assert_eq!(actual, index);
        assert_eq!(
            ProviderFrame::Reply {
                request_id: request.request_id,
                reply: response
            },
            reply(request)
        );
    }
    assert!(!transport.is_closed());
    Ok(())
}

#[test]
fn invalid_routes_unknown_ids_and_lookup_stop_are_rejected() -> Result<(), String> {
    let (registry, first) = fixture()?;
    let mut second = first.clone();
    second.request_id += 1;
    let sources = SourceStore::default();
    for requests in [[&second, &first], [&first, &first]] {
        let entries = requests.map(|request| ReplyContext {
            request,
            context: Digest::of(b"context"),
            authorized_sources: &sources,
        });
        assert!(matches!(
            ReplyRoutes::new(&entries, &mut budget()),
            Err(RouteError::UnorderedOrDuplicate)
        ));
    }
    let entries = [ReplyContext {
        request: &first,
        context: Digest::of(b"context"),
        authorized_sources: &sources,
    }];
    let routes = ReplyRoutes::new(&entries, &mut budget()).map_err(error)?;
    let mut transport = connection(&reply(&second), &registry)?;
    assert!(matches!(transport.receive_routed_reply(
        &routes, &registry, &sources, &mut SourceAdmission::default(), &mut budget(), &mut budget(),
    ), Err(RouteError::UnknownRequest(id)) if id == second.request_id));
    assert!(transport.is_closed());
    let mut transport = connection(&reply(&first), &registry)?;
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        transport.receive_routed_reply(
            &routes,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
            &mut stopped,
        ),
        Err(RouteError::Stopped(StopReason::WorkLimit))
    ));
    assert!(transport.is_closed());
    Ok(())
}

#[test]
fn selected_context_rejects_await_bound_to_another_request() -> Result<(), String> {
    let (registry, first) = fixture()?;
    let mut second = first.clone();
    second.request_id += 1;
    let sources = SourceStore::default();
    let context = Digest::of(b"saved context");
    let entries = [&first, &second].map(|request| ReplyContext {
        request,
        context,
        authorized_sources: &sources,
    });
    let routes = ReplyRoutes::new(&entries, &mut budget()).map_err(error)?;
    let frame = ProviderFrame::Reply {
        request_id: second.request_id,
        reply: OperationReply::Await {
            continuation: Continuation {
                provider: first.operation.clone(),
                parent_request: first.request_id,
                snapshot_digest: context,
                state: first.input.clone(),
            },
            calls: vec![],
            report: Report::default(),
        },
    };
    let mut transport = connection(&frame, &registry)?;
    assert!(matches!(
        transport.receive_routed_reply(
            &routes,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
            &mut budget(),
        ),
        Err(RouteError::Reply(ReplyError::Validation(_)))
    ));
    assert!(transport.is_closed());
    Ok(())
}
