use nepl3_core::{
    budget::*,
    diagnostic::{OperationResult, Report},
    operation::*,
    operation::{dependencies::PendingDependencies, lifetime::RequestLifetimes},
    schema::*,
    source::*,
    value::*,
};
use nepl3_provider::{Connection, process::Process};
use nepl3_suite::dispatch::{resume, suspending};
use std::{
    io,
    process::{Command, ExitStatus},
    sync::mpsc,
    time::{Duration, Instant},
};
mod model;
use model::*;

fn child() -> Result<(), String> {
    let (registry, prototype) = fixture()?;
    let sources = SourceStore::default();
    let mut connection = Connection::new(io::stdin().lock(), io::stdout().lock());
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
                // This conformance provider grants no external sources/resources.
                if !request.sources.is_empty()
                    || !request.resources.is_empty()
                    || request.environment != prototype.environment
                    || pending.is_some()
                {
                    return Err("ungranted context or concurrent fixture request".into());
                }
                let context = context(&request, &registry)?;
                lifetimes
                    .begin_call(&request, context, None, &mut budget())
                    .map_err(error)?;
                let registration = suspending::Registration {
                    operation: &prototype.operation,
                    implementation: identity(),
                    invoke: await_increment,
                };
                let reply = connection
                    .dispatch_invoke(
                        &registration,
                        identity(),
                        &request,
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
                let OperationReply::Await {
                    continuation,
                    calls,
                    ..
                } = reply
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
                    resume: finish,
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
                if !matches!(
                    result,
                    OperationReply::Result(OperationResult::Complete { .. })
                ) {
                    return Err("expected completion".into());
                }
                lifetimes
                    .finish(parent.request_id, &mut budget())
                    .map_err(error)?;
                assert!(execution.usage().work >= 12);
            }
            Some(ProviderFrame::Close) => return Ok(()),
            _ => return Err("unexpected fixture protocol message".into()),
        }
    }
}

fn exchange(
    mut connection: Connection<std::process::ChildStdout, std::process::ChildStdin>,
) -> Result<(), String> {
    let (registry, request) = fixture()?;
    let sources = SourceStore::default();
    let context = context(&request, &registry)?;
    let mut admission = SourceAdmission::default();
    connection
        .send(
            &ProviderFrame::Invoke(request.clone()),
            &registry,
            &sources,
            &mut admission,
            &mut budget(),
        )
        .map_err(error)?;
    let reply = connection
        .receive_reply(
            &request,
            context,
            &registry,
            &sources,
            &sources,
            &mut admission,
            &mut budget(),
            &mut budget(),
        )
        .map_err(error)?;
    let OperationReply::Await {
        continuation,
        calls,
        ..
    } = reply
    else {
        return Err("expected remote Await".into());
    };
    if calls.len() != 1 || calls[0].operation.name != "increment" || calls[0].request_id != 18 {
        return Err("unexpected dependency selection".into());
    }
    let mut selected = request.operation.clone();
    selected.name = "increment".into();
    let registration = suspending::Registration {
        operation: &selected,
        implementation: identity(),
        invoke: increment,
    };
    let OperationReply::Result(result) = suspending::invoke(
        &registration,
        identity(),
        &calls[0],
        context,
        &registry,
        &sources,
        &mut budget(),
        &mut budget(),
    )
    .map_err(error)?
    else {
        return Err("expected terminal dependency".into());
    };
    let mut pending =
        PendingDependencies::new(&continuation, &calls, &mut budget()).map_err(error)?;
    pending
        .accept(18, result, &registry, &sources, &mut budget())
        .map_err(error)?;
    let resume = pending.take_resume(&mut budget()).map_err(error)?;
    connection
        .send(
            &ProviderFrame::Resume(resume),
            &registry,
            &sources,
            &mut admission,
            &mut budget(),
        )
        .map_err(error)?;
    let reply = connection
        .receive_reply(
            &request,
            context,
            &registry,
            &sources,
            &sources,
            &mut admission,
            &mut budget(),
            &mut budget(),
        )
        .map_err(error)?;
    let OperationReply::Result(OperationResult::Complete {
        value: TypedValue::Record(record),
        ..
    }) = reply
    else {
        return Err("expected remote completion".into());
    };
    // Independent semantic expectation, not merely equality of two codec paths.
    if record.fields != vec![NdfValue::U64(42)] {
        return Err("expected 41 + 1 = 42".into());
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
                let cleanup = process.terminate();
                return Err(format!(
                    "provider failed to exit: {outcome:?}; cleanup: {cleanup:?}"
                ));
            }
        }
    }
}

pub fn run() -> Result<(), String> {
    if std::env::args().any(|arg| arg == "--provider-child") {
        return child();
    }
    let mut command = Command::new(std::env::current_exe().map_err(error)?);
    command.arg("--provider-child");
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
    let status = if signal.is_ok() {
        reap(&mut process)
    } else {
        process.terminate().map_err(error)
    };
    // Terminating the child releases blocked pipe I/O before joining the worker.
    let result = worker
        .join()
        .map_err(|_| "protocol worker panicked".to_owned())?;
    signal.map_err(error)?;
    result?;
    if !status?.success() {
        return Err("provider process failed".into());
    }
    println!(
        "process_protocol: 1 passed (Invoke -> Await -> native dependency -> Resume -> Complete -> Close)"
    );
    Ok(())
}
