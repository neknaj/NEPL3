//! One remote root delegates real language operations to the parent host.
use super::*;
use nepl3_core::budget::{Budget, StopReason};
use nepl3_core::operation::lifetime::{RequestLifetimes, RequestPhase};
use nepl3_suite::{
    dispatch::resume, grants::dependencies::OperationGrant, scheduler, suspension::host,
};

fn remote_budget(stop: bool) -> Budget {
    let mut limits = budget().limits();
    if stop {
        // Allows Await and literal operands; the 70-digit product exceeds Work.
        limits.work = 20_000;
    }
    Budget::new(limits)
}

pub fn child(text: &str, remote_stop: bool) -> Result<(), String> {
    let model = model()?;
    // Both hosts independently install this fixed fixture plan. Dynamic plan
    // transfer is exercised by the separate aggregate-operation cases.
    let (bytes, sources, _) = packet(text, &model)?;
    let received = transfer::envelope::decode(
        &bytes,
        &model.plan,
        &model.foundation,
        &model.registry,
        &sources,
        &mut budget(),
    )
    .map_err(error)?;
    let program = received.program(&sources, &mut budget()).map_err(error)?;
    let session = model
        .runtime
        .prepare(
            &program,
            &sources,
            &model.registry,
            remote_budget(remote_stop).limits(),
            &mut budget(),
        )
        .map_err(error)?;
    let mut connection = Connection::new(io::stdin().lock(), io::stdout().lock());
    let mut lifetimes = RequestLifetimes::default();
    let mut admission = SourceAdmission::default();
    let mut transport = budget();
    let mut execution = remote_budget(remote_stop);
    session
        .with_registrations(
            &mut budget(),
            |registrations, validation| -> Result<(), String> {
                let registration = &registrations[0];
                let Some(ProviderFrame::Invoke(request)) = connection
                    .receive_managed(
                        &mut lifetimes,
                        &model.registry,
                        &sources,
                        &mut admission,
                        &mut transport,
                        validation,
                        |_| {},
                    )
                    .map_err(error)?
                else {
                    return Err("expected Invoke".into());
                };
                let approved = registration
                    .grants
                    .admit(&request, validation)
                    .map_err(error)?;
                let context = (registration.context)(
                    &request,
                    registration.invoke.implementation,
                    validation,
                )
                .map_err(error)?;
                lifetimes
                    .begin_call(&request, context, None, validation)
                    .map_err(error)?;
                let reply = connection
                    .dispatch_invoke(
                        &registration.invoke,
                        registration.invoke.implementation,
                        &approved,
                        context,
                        &model.registry,
                        &sources,
                        &sources,
                        &mut admission,
                        &mut execution,
                        validation,
                        &mut transport,
                    )
                    .map_err(error)?;
                reply.delivery.map_err(error)?;
                let OperationReply::Await {
                    continuation,
                    calls,
                    report,
                } = reply.reply
                else {
                    return Err("expected root Await".into());
                };
                assert_eq!(report.usage, execution.usage());
                assert!(report.usage.work > 0);
                lifetimes
                    .suspend(
                        request.request_id,
                        continuation.clone(),
                        calls.len(),
                        validation,
                    )
                    .map_err(error)?;
                let mut cancellations = Vec::new();
                let next = connection
                    .receive_managed(
                        &mut lifetimes,
                        &model.registry,
                        &sources,
                        &mut admission,
                        &mut transport,
                        validation,
                        |id| cancellations.push(id),
                    )
                    .map_err(error)?;
                match next {
                    Some(ProviderFrame::Resume(resumed)) => {
                        let authorized = vec![&sources; calls.len()];
                        let saved = resume::SavedAwait {
                            parent: &request,
                            context,
                            continuation: &continuation,
                            calls: &calls,
                            sources: &authorized,
                        };
                        let result = connection
                            .dispatch_resume(
                                &registration.resume,
                                registration.resume.implementation,
                                &saved,
                                &resumed,
                                &mut lifetimes,
                                &model.registry,
                                &sources,
                                &sources,
                                &mut admission,
                                &mut execution,
                                validation,
                                &mut transport,
                            )
                            .map_err(error)?;
                        result.delivery.map_err(error)?;
                        let OperationReply::Result(outcome) = result.reply else {
                            return Err("expected terminal root".into());
                        };
                        let report = match &outcome {
                            OperationResult::Complete { report, .. } if !remote_stop => report,
                            OperationResult::Stopped { reason, report, .. } if remote_stop => {
                                assert_eq!(*reason, nepl3_core::budget::StopReason::WorkLimit);
                                assert_eq!(execution.poll(), Err(*reason));
                                report
                            }
                            _ => return Err("unexpected remote result".into()),
                        };
                        assert_eq!(report.usage, execution.usage());
                        lifetimes
                            .finish(request.request_id, validation)
                            .map_err(error)?;
                        assert!(cancellations.is_empty());
                    }
                    Some(ProviderFrame::Cancel { request_id }) => {
                        assert_eq!(request_id, request.request_id);
                        assert_eq!(cancellations, [request_id]);
                        assert_eq!(
                            lifetimes.phase(request_id, validation).map_err(error)?,
                            RequestPhase::Cancelled
                        );
                    }
                    _ => return Err("expected Resume or Cancel".into()),
                }
                let count = cancellations.len();
                match connection
                    .receive_managed(
                        &mut lifetimes,
                        &model.registry,
                        &sources,
                        &mut admission,
                        &mut transport,
                        validation,
                        |id| cancellations.push(id),
                    )
                    .map_err(error)?
                {
                    Some(ProviderFrame::Close) => {
                        assert_eq!(cancellations.len(), count);
                        Ok(())
                    }
                    _ => Err("expected Close".into()),
                }
            },
        )
        .map_err(error)?
}

pub fn exchange(
    mut connection: Connection<std::process::ChildStdout, std::process::ChildStdin>,
    text: &str,
    expected: i64,
    cancel: bool,
    stop: bool,
    remote_stop: bool,
) -> Result<(), String> {
    let model = model()?;
    let (bytes, sources, native) = packet(text, &model)?;
    let received = transfer::envelope::decode(
        &bytes,
        &model.plan,
        &model.foundation,
        &model.registry,
        &sources,
        &mut budget(),
    )
    .map_err(error)?;
    let program = received.program(&sources, &mut budget()).map_err(error)?;
    let session = model
        .runtime
        .prepare(
            &program,
            &sources,
            &model.registry,
            remote_budget(remote_stop).limits(),
            &mut budget(),
        )
        .map_err(error)?;
    let mut lifetimes = RequestLifetimes::default();
    let mut admission = SourceAdmission::default();
    let mut transport = budget();
    let mut execution = budget();
    session
        .with_registrations(
            &mut budget(),
            |registrations, validation| -> Result<(), String> {
                let root = session.root();
                let context = (registrations[0].context)(
                    root,
                    registrations[0].invoke.implementation,
                    validation,
                )
                .map_err(error)?;
                lifetimes
                    .begin_call(root, context, None, validation)
                    .map_err(error)?;
                connection
                    .send(
                        &ProviderFrame::Invoke(root.clone()),
                        &model.registry,
                        &sources,
                        &mut admission,
                        &mut transport,
                    )
                    .map_err(error)?;
                let reply = connection
                    .receive_reply(
                        root,
                        context,
                        &model.registry,
                        &sources,
                        &sources,
                        &mut admission,
                        &mut transport,
                        validation,
                    )
                    .map_err(error)?;
                let OperationReply::Await { calls, report, .. } = &reply else {
                    return Err("expected remote root Await".into());
                };
                let remote_await_usage = report.usage;
                assert!(remote_await_usage.work > 0);
                let mut contexts = Vec::new();
                for call in calls {
                    let registration = registrations
                        .iter()
                        .find(|r| *r.invoke.operation == call.operation)
                        .ok_or("unregistered dependency")?;
                    registration.grants.admit(call, validation).map_err(error)?;
                    contexts.push(
                        (registration.context)(
                            call,
                            registration.invoke.implementation,
                            validation,
                        )
                        .map_err(error)?,
                    );
                }
                let policy = registrations
                    .iter()
                    .map(|r| OperationGrant {
                        operation: r.invoke.operation,
                        grants: r.grants,
                    })
                    .collect::<Vec<_>>();
                let active = host::activate_owned(
                    root,
                    context,
                    reply,
                    &policy,
                    &contexts,
                    &model.registry,
                    &sources,
                    &mut lifetimes,
                    validation,
                )
                .map_err(error)?;
                let mut pending = active.pending;
                let stopped = if stop {
                    let mut limits = execution.limits();
                    limits.work = 20_000;
                    execution = nepl3_core::budget::Budget::new(limits);
                    let mut cancelled = Vec::new();
                    let Err(failure) = scheduler::run(
                        registrations,
                        &pending.calls()[0],
                        &model.registry,
                        &mut execution,
                        validation,
                        |_, _| {},
                        |id| cancelled.push(id),
                    ) else {
                        return Err("large guest multiplication must stop".into());
                    };
                    assert_eq!(
                        execution.poll(),
                        Err(nepl3_core::budget::StopReason::WorkLimit)
                    );
                    assert!(!cancelled.contains(&3));
                    assert_eq!(
                        cancelled
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len(),
                        cancelled.len()
                    );
                    Some(failure)
                } else {
                    None
                };
                if cancel || stop {
                    let mut cancelled = Vec::new();
                    lifetimes
                        .cancel_tree(root.request_id, validation, |id| cancelled.push(id))
                        .map_err(error)?;
                    assert!(cancelled.contains(&root.request_id));
                    connection
                        .send(
                            &ProviderFrame::Cancel {
                                request_id: root.request_id,
                            },
                            &model.registry,
                            &sources,
                            &mut admission,
                            &mut transport,
                        )
                        .map_err(error)?;
                } else {
                    for (index, child_context) in contexts.iter().enumerate() {
                        let call = &pending.calls()[index];
                        let id = call.request_id;
                        let result = scheduler::run(
                            registrations,
                            call,
                            &model.registry,
                            &mut execution,
                            validation,
                            |_, _| {},
                            |_| {},
                        )
                        .map_err(error)?;
                        pending
                            .accept_active(
                                id,
                                *child_context,
                                result,
                                &model.registry,
                                &sources,
                                &mut lifetimes,
                                validation,
                            )
                            .map_err(error)?;
                    }
                    let resumed = pending.take_resume(validation).map_err(error)?;
                    lifetimes.resume(&resumed, validation).map_err(error)?;
                    connection
                        .send(
                            &ProviderFrame::Resume(resumed),
                            &model.registry,
                            &sources,
                            &mut admission,
                            &mut transport,
                        )
                        .map_err(error)?;
                    let local_usage = execution.usage();
                    let reply = connection
                        .receive_reply(
                            root,
                            context,
                            &model.registry,
                            &sources,
                            &sources,
                            &mut admission,
                            &mut transport,
                            validation,
                        )
                        .map_err(error)?;
                    let OperationReply::Result(result) = reply else {
                        return Err("expected remote terminal result".into());
                    };
                    assert_eq!(execution.usage(), local_usage);
                    let report = match &result {
                        OperationResult::Complete { report, .. } if !remote_stop => {
                            let actual = complete_value(&result)?;
                            assert_eq!(actual, complete_value(&native)?);
                            let [NdfValue::Integer(value)] = actual.fields.as_slice() else {
                                return Err("expected Integer".into());
                            };
                            assert_eq!(value.as_bigint().to_string(), expected.to_string());
                            report
                        }
                        OperationResult::Stopped {
                            reason,
                            partial,
                            report,
                        } if remote_stop => {
                            assert_eq!(*reason, StopReason::WorkLimit);
                            assert!(partial.is_none());
                            let [diagnostic] = report.diagnostics.as_slice() else {
                                return Err("expected remote arithmetic diagnostic".into());
                            };
                            assert_eq!(diagnostic.code, "evaluation-stopped");
                            assert_eq!(diagnostic.schema, root.operation.schema);
                            assert_eq!(diagnostic.arguments, root.input);
                            let span = diagnostic.primary.as_ref().ok_or("missing remote span")?;
                            assert_eq!((span.start(), span.end()), (0, 3));
                            report
                                .validate_with_sources(&sources, &model.registry, validation)
                                .map_err(error)?;
                            assert!(
                                report
                                    .validate_with_sources(
                                        &SourceStore::default(),
                                        &model.registry,
                                        validation
                                    )
                                    .is_err()
                            );
                            report
                        }
                        _ => return Err("unexpected remote terminal result".into()),
                    };
                    assert!(report.usage.work > remote_await_usage.work);
                    assert!(report.usage.allocation_units >= remote_await_usage.allocation_units);
                    lifetimes
                        .finish(root.request_id, validation)
                        .map_err(error)?;
                    assert_eq!(
                        lifetimes
                            .phase(root.request_id, validation)
                            .map_err(error)?,
                        RequestPhase::Finished
                    );
                }
                if let Some(failure) = &stopped {
                    // The host owns this accepted child outcome across remote
                    // cancellation. It is never relabelled as the root output.
                    let mut accepted = failure.accepted_results();
                    let (child, outcome) = accepted.next().ok_or("missing stopped child")?;
                    assert!(accepted.next().is_none());
                    assert_eq!(child.request_id, 3);
                    let OperationResult::Stopped { reason, report, .. } = outcome else {
                        return Err("expected Stopped".into());
                    };
                    assert_eq!(*reason, nepl3_core::budget::StopReason::WorkLimit);
                    let [diagnostic] = report.diagnostics.as_slice() else {
                        return Err("expected guest diagnostic".into());
                    };
                    assert_eq!(diagnostic.code, "evaluation-stopped");
                    assert_eq!(diagnostic.schema, child.operation.schema);
                    assert_eq!(diagnostic.arguments, child.input);
                    let span = diagnostic.primary.as_ref().ok_or("missing guest source")?;
                    assert_eq!((span.start(), span.end()), (17, 20));
                    report
                        .validate_with_sources(&sources, &model.registry, validation)
                        .map_err(error)?;
                    assert!(
                        report
                            .validate_with_sources(
                                &SourceStore::default(),
                                &model.registry,
                                validation
                            )
                            .is_err()
                    );
                }
                connection
                    .send(
                        &ProviderFrame::Close,
                        &model.registry,
                        &sources,
                        &mut admission,
                        &mut transport,
                    )
                    .map_err(error)
            },
        )
        .map_err(error)?
}
