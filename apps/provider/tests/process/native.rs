use nepl3_core::operation::lifetime::RequestPhase;
use nepl3_core::{
    budget::*,
    diagnostic::{OperationResult, Report},
    operation::*,
    operation::{dependencies::PendingDependencies, lifetime::RequestLifetimes},
    schema::*,
    source::*,
    value::*,
};
use nepl3_provider::reply::{ReplyContext, ReplyRoutes};
use nepl3_provider::{Connection, process::Process};
use nepl3_suite::dispatch::{resume, suspending};
use nepl3_suite::grants::Grants;
use std::{
    io,
    process::{Command, ExitStatus},
    sync::mpsc,
    time::{Duration, Instant},
};
mod model;
mod reference;
mod routing;
mod schema_failure;
use model::*;

fn child() -> Result<(), String> {
    let (registry, prototype) = fixture()?;
    let sources = granted_sources()?;
    let authority =
        Grants::new(&prototype.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut connection = Connection::new(io::stdin().lock(), io::stdout().lock());
    connection
        .serve_schemas(&registry, &mut budget())
        .map_err(error)?;
    let mut admission = SourceAdmission::default();
    let mut lifetimes = RequestLifetimes::default();
    let mut execution = budget();
    let mut pending = None;
    loop {
        let frame = connection
            .receive_managed(
                &mut lifetimes,
                &registry,
                &sources,
                &mut admission,
                &mut budget(),
                &mut budget(),
                |_| {},
            )
            .map_err(error)?;
        match frame {
            Some(ProviderFrame::Invoke(request)) => {
                // This provider grants the fixed Unicode fixture and no resources.
                if pending.is_some() {
                    return Err("concurrent fixture request".into());
                }
                let approved = authority.admit(&request, &mut budget()).map_err(error)?;
                let context = context(&request, &registry)?;
                lifetimes
                    .begin_call(&request, context, None, &mut budget())
                    .map_err(error)?;
                let registration = suspending::Registration {
                    operation: &prototype.operation,
                    implementation: identity(),
                    invoke: &await_increment,
                };
                let reply = connection
                    .dispatch_invoke(
                        &registration,
                        identity(),
                        &approved,
                        context,
                        &registry,
                        &sources,
                        &sources,
                        &mut admission,
                        &mut execution,
                        &mut budget(),
                        &mut budget(),
                    )
                    .map_err(error)?;
                reply.delivery.map_err(error)?;
                let OperationReply::Await {
                    continuation,
                    calls,
                    ..
                } = reply.reply
                else {
                    return Err("expected fixture Await".into());
                };
                lifetimes
                    .suspend(
                        request.request_id,
                        continuation.clone(),
                        calls.len(),
                        &mut budget(),
                    )
                    .map_err(error)?;
                pending = Some((request, context, continuation, calls));
            }
            Some(ProviderFrame::Resume(request)) => {
                let Some((parent, context, continuation, calls)) = pending.take() else {
                    return Err("unsolicited Resume".into());
                };
                let grants = [&sources];
                let saved = resume::SavedAwait {
                    parent: &parent,
                    context,
                    continuation: &continuation,
                    calls: &calls,
                    sources: &grants,
                };
                let registration = resume::Registration {
                    operation: &prototype.operation,
                    implementation: identity(),
                    resume: &finish,
                };
                let result = connection
                    .dispatch_resume(
                        &registration,
                        identity(),
                        &saved,
                        &request,
                        &mut lifetimes,
                        &registry,
                        &sources,
                        &sources,
                        &mut admission,
                        &mut execution,
                        &mut budget(),
                        &mut budget(),
                    )
                    .map_err(error)?;
                result.delivery.map_err(error)?;
                if !matches!(result.reply, OperationReply::Result(_)) {
                    return Err("expected terminal result".into());
                }
                lifetimes
                    .finish(parent.request_id, &mut budget())
                    .map_err(error)?;
                assert!(execution.usage().work >= 12);
            }
            Some(ProviderFrame::Cancel { request_id }) => {
                let Some((parent, _, _, _)) = pending.take() else {
                    return Err("cancel without suspended request".into());
                };
                if request_id != parent.request_id
                    || lifetimes.phase(request_id, &mut budget()).map_err(error)?
                        != nepl3_core::operation::lifetime::RequestPhase::Cancelled
                {
                    return Err("cancel did not terminate the suspended lifetime".into());
                }
            }
            Some(ProviderFrame::Close) if pending.is_none() => return Ok(()),
            _ => return Err("unexpected fixture protocol message".into()),
        }
    }
}

fn exchange(
    mut connection: Connection<std::process::ChildStdout, std::process::ChildStdin>,
    input: u64,
    cancel: bool,
) -> Result<(), String> {
    let (_, mut request) = fixture()?;
    let base = bootstrap()?;
    assert!(base.selected("test.process", 1).is_none());
    let registry = connection
        .request_schemas(
            base,
            core::slice::from_ref(&request.operation.schema),
            &mut budget(),
        )
        .map_err(error)?;
    assert_eq!(
        registry.selected("test.process", 1),
        Some(&request.operation.schema)
    );
    if let TypedValue::Record(record) = &mut request.input {
        record.fields[0] = NdfValue::U64(input);
    }
    let sources = granted_sources()?;
    let context = context(&request, &registry)?;
    let mut admission = SourceAdmission::default();
    let mut lifetimes = RequestLifetimes::default();
    lifetimes
        .begin_call(&request, context, None, &mut budget())
        .map_err(error)?;
    let contexts = [ReplyContext {
        request: &request,
        context,
        authorized_sources: &sources,
    }];
    let routes = ReplyRoutes::new(&contexts, &mut budget()).map_err(error)?;
    connection
        .send(
            &ProviderFrame::Invoke(request.clone()),
            &registry,
            &sources,
            &mut admission,
            &mut budget(),
        )
        .map_err(error)?;
    let (route, reply) = connection
        .receive_managed_reply(
            &routes,
            &mut lifetimes,
            &registry,
            &sources,
            &mut admission,
            &mut budget(),
            &mut budget(),
            |_| {},
        )
        .map_err(error)?;
    assert_eq!(route, 0);
    assert_eq!(
        lifetimes
            .phase(request.request_id, &mut budget())
            .map_err(error)?,
        RequestPhase::Running
    );
    let OperationReply::Await { calls, .. } = &reply else {
        return Err("expected remote Await".into());
    };
    if calls.len() != 1 || calls[0].operation.name != "increment" || calls[0].request_id != 18 {
        return Err("unexpected dependency selection".into());
    }
    if cancel {
        lifetimes
            .cancel_tree(request.request_id, &mut budget(), |_| {})
            .map_err(error)?;
        assert_eq!(
            lifetimes
                .phase(request.request_id, &mut budget())
                .map_err(error)?,
            RequestPhase::Cancelled
        );
        for frame in [
            ProviderFrame::Cancel {
                request_id: request.request_id,
            },
            ProviderFrame::Close,
        ] {
            connection
                .send(&frame, &registry, &sources, &mut admission, &mut budget())
                .map_err(error)?;
        }
        return Ok(());
    }
    let mut selected = request.operation.clone();
    selected.name = "increment".into();
    let registration = suspending::Registration {
        operation: &selected,
        implementation: identity(),
        invoke: &increment,
    };
    let authority =
        Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let policy = [nepl3_suite::grants::dependencies::OperationGrant {
        operation: &selected,
        grants: &authority,
    }];
    let dependency_context = model::context(&calls[0], &registry)?;
    let active = nepl3_suite::suspension::host::activate(
        &request,
        context,
        &reply,
        &policy,
        &[dependency_context],
        &registry,
        &sources,
        &mut lifetimes,
        &mut budget(),
    )
    .map_err(error)?;
    let mut pending = active.pending;
    let [approved] = active.authorized.as_slice() else {
        return Err("expected one authorized dependency".into());
    };
    let dependency = approved.invocation().request();
    let OperationReply::Result(result) = suspending::invoke(
        &registration,
        identity(),
        approved.invocation().request(),
        dependency_context,
        &registry,
        &sources,
        &mut budget(),
        &mut budget(),
    )
    .map_err(error)?
    else {
        return Err("expected terminal dependency".into());
    };
    pending
        .accept_active(
            dependency.request_id,
            dependency_context,
            result,
            &registry,
            &sources,
            &mut lifetimes,
            &mut budget(),
        )
        .map_err(error)?;
    let resume = pending.take_resume(&mut budget()).map_err(error)?;
    lifetimes.resume(&resume, &mut budget()).map_err(error)?;
    connection
        .send(
            &ProviderFrame::Resume(resume),
            &registry,
            &sources,
            &mut admission,
            &mut budget(),
        )
        .map_err(error)?;
    let (route, reply) = connection
        .receive_managed_reply(
            &routes,
            &mut lifetimes,
            &registry,
            &sources,
            &mut admission,
            &mut budget(),
            &mut budget(),
            |_| {},
        )
        .map_err(error)?;
    assert_eq!(route, 0);
    assert_eq!(
        lifetimes
            .phase(request.request_id, &mut budget())
            .map_err(error)?,
        RequestPhase::Finished
    );
    reference::compare(&reference::execute(&registry, &request)?, &reply)?;
    // Independent arithmetic expectation includes the checked overflow boundary.
    match (input.checked_add(1), reply) {
        (
            Some(expected),
            OperationReply::Result(OperationResult::Complete {
                value: TypedValue::Record(record),
                ..
            }),
        ) if record.fields == vec![NdfValue::U64(expected)] => {}
        (
            None,
            OperationReply::Result(OperationResult::Invalid {
                partial: None,
                report,
            }),
        ) => {
            let [diagnostic] = report.diagnostics.as_slice() else {
                return Err("expected overflow diagnostic".into());
            };
            let span = diagnostic
                .primary
                .as_ref()
                .ok_or("missing diagnostic span")?;
            if diagnostic.code != "increment-overflow"
                || span.start() != 4
                || span.end() != 10
                || input_source()?.slice(span).map_err(error)? != "世界"
            {
                return Err("incorrect Unicode diagnostic position".into());
            }
        }
        _ => return Err("incorrect increment value or overflow result".into()),
    }
    connection
        .send(
            &ProviderFrame::Close,
            &registry,
            &sources,
            &mut admission,
            &mut budget(),
        )
        .map_err(error)?;
    Ok(())
}

fn reap(process: &mut Process) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match process.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            outcome => {
                return Err(format!("provider failed to exit: {outcome:?}"));
            }
        }
    }
}

fn run_case(input: u64) -> Result<(), String> {
    run_process("--provider-child", move |connection| {
        exchange(connection, input, false)
    })
}

fn run_process(
    child_argument: &str,
    exchange: impl FnOnce(
        Connection<std::process::ChildStdout, std::process::ChildStdin>,
    ) -> Result<(), String>
    + Send
    + 'static,
) -> Result<(), String> {
    let mut command = Command::new(std::env::current_exe().map_err(error)?);
    command.arg(child_argument);
    let mut process = Process::spawn(&mut command).map_err(error)?;
    let Some(connection) = process.take_transport() else {
        let cleanup = process.terminate();
        return Err(format!("missing pipes; cleanup: {cleanup:?}"));
    };
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = exchange(connection);
        sender.send(()).map_err(error)?;
        result
    });
    let signal = receiver.recv_timeout(Duration::from_secs(10));
    let observed = if signal.is_ok() {
        reap(&mut process)
    } else {
        Err(format!("protocol deadline/channel failure: {signal:?}"))
    };
    let (status, failure) = match observed {
        Ok(status) => (status, None),
        Err(reason) => match process.terminate() {
            Ok(status) => (status, Some(reason)),
            Err(cleanup) => {
                // The child may still own its pipe ends. Do not block in join.
                return Err(format!(
                    "{reason}; cleanup failed: {cleanup}; worker detached because child exit is unconfirmed"
                ));
            }
        },
    };
    // Exit/reaping is confirmed, releasing this fixture's blocked pipe I/O.
    let result = worker
        .join()
        .map_err(|_| "protocol worker panicked".to_owned());
    if let Some(failure) = failure {
        return Err(format!(
            "{failure}; cleanup exit: {status}; worker: {result:?}"
        ));
    }
    result??;
    if !status.success() {
        return Err("provider process failed".into());
    }
    Ok(())
}

pub fn run() -> Result<(), String> {
    if std::env::args().any(|arg| arg == "--routing-child") {
        return routing::child();
    }
    if let Some(mode) = schema_failure::child_mode() {
        return schema_failure::child(mode);
    }
    if std::env::args().any(|arg| arg == "--provider-child") {
        return child();
    }
    for input in [0, 41, u64::MAX] {
        run_case(input).map_err(|e| format!("input {input}: {e}"))?;
    }
    schema_failure::run()?;
    routing::run()?;
    run_process("--provider-child", |connection| {
        exchange(connection, 41, true)
    })?;
    println!(
        "process_protocol: 8 passed (3 schema failures; 3 native/process comparisons; 1 suspended cancellation; 1 reverse-order routing)"
    );
    Ok(())
}
