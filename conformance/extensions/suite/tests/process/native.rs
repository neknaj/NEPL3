use external_composition_runtime::{execution::Runtime, program::transfer};
use external_hello_language::{budget, composition, error};
use nepl3_core::{
    diagnostic::OperationResult,
    operation::{Invoke, OperationReply, ProviderFrame},
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceStore},
    value::{NdfValue, OperationRef, Record, SchemaRef, TypedValue},
};
use nepl3_provider::{Connection, process::Process};
use nepl3_suite::{dispatch::suspending, grants::Grants};
use std::{
    io::{self, Read},
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
mod model;
mod suspension;
use model::*;

fn child(text: &str, no_grant: bool) -> Result<(), String> {
    let model = model()?;
    // Host-owned fixture authorization, established independently of the packet.
    let sources = if no_grant {
        SourceStore::default()
    } else {
        sources(text)?
    };
    let environment = environment(&model);
    let grants = Grants::new(&environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut connection = Connection::new(io::stdin().lock(), io::stdout().lock());
    let mut admission = SourceAdmission::default();
    let mut transport = budget();
    let mut validation = budget();
    let Some(ProviderFrame::Invoke(request)) = connection
        .receive(&model.registry, &sources, &mut admission, &mut transport)
        .map_err(error)?
    else {
        return Err("expected Invoke".into());
    };
    grants.admit(&request, &mut validation).map_err(error)?;
    request
        .validate_input(&model.operation, &model.registry, &mut validation)
        .map_err(error)?;
    let context = nepl3_wire::operation::context_digest(
        &request,
        identity(),
        &model.registry,
        &mut validation,
    )
    .map_err(error)?;
    let TypedValue::Record(record) = &request.input else {
        return Err("expected Packet".into());
    };
    let [NdfValue::Bytes(bytes)] = record.fields.as_slice() else {
        return Err("expected packet bytes".into());
    };
    let received = transfer::envelope::decode(
        bytes,
        &model.plan,
        &model.foundation,
        &model.registry,
        &sources,
        &mut validation,
    )
    .map_err(error)?;
    let program = received.program(&sources, &mut validation).map_err(error)?;
    // One execution Budget covers every nested native operation. Outer request
    // limits restrict the host limit; validation has a separate finite budget.
    let mut execution = budget();
    let result = execution
        .with_ceiling(request.limits, |execution| {
            model.runtime.run(
                &program,
                &sources,
                &model.registry,
                execution,
                &mut validation,
                |_, _| {},
                |_| {},
            )
        })
        .map_err(error)?;
    let reply = OperationReply::Result(result);
    suspending::validate_reply(
        &reply,
        &request,
        context,
        &model.registry,
        &sources,
        &mut validation,
    )
    .map_err(error)?;
    connection
        .send(
            &ProviderFrame::Reply {
                request_id: request.request_id,
                reply,
            },
            &model.registry,
            &sources,
            &mut admission,
            &mut transport,
        )
        .map_err(error)?;
    match connection
        .receive(&model.registry, &sources, &mut admission, &mut transport)
        .map_err(error)?
    {
        Some(ProviderFrame::Close) => Ok(()),
        _ => Err("expected Close".into()),
    }
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Valid,
    Corrupt,
    NoGrant,
    WrongOperation,
    Suspending,
    Cancel,
    Stopped,
}

fn exchange(
    mut connection: Connection<std::process::ChildStdout, std::process::ChildStdin>,
    text: &str,
    expected: i64,
    case: Case,
) -> Result<(), String> {
    if matches!(case, Case::Suspending | Case::Cancel | Case::Stopped) {
        return suspension::exchange(
            connection,
            text,
            expected,
            matches!(case, Case::Cancel),
            matches!(case, Case::Stopped),
        );
    }
    let model = model()?;
    let (mut bytes, sources, native) = packet(text, &model)?;
    if matches!(case, Case::Corrupt) {
        bytes = vec![0xff];
    }
    let mut request = Invoke {
        request_id: 1,
        operation: model.operation.clone(),
        input: TypedValue::Record(Record {
            schema: model.operation.schema.clone(),
            kind: "Packet".into(),
            fields: vec![NdfValue::Bytes(bytes)],
        }),
        environment: environment(&model),
        sources: sources.snapshots().to_vec(),
        resources: vec![],
        limits: budget().limits(),
    };
    if matches!(case, Case::WrongOperation) {
        request.operation.name = "unknown".into();
    }
    let mut admission = SourceAdmission::default();
    let mut transport = budget();
    connection
        .send(
            &ProviderFrame::Invoke(request.clone()),
            &model.registry,
            &sources,
            &mut admission,
            &mut transport,
        )
        .map_err(error)?;
    if !matches!(case, Case::Valid) {
        // Rejected input terminates the child without publishing a success value.
        return match connection.receive(&model.registry, &sources, &mut admission, &mut transport) {
            Ok(None) => Ok(()),
            other => Err(format!("expected clean EOF after rejection: {other:?}")),
        };
    }
    let context =
        nepl3_wire::operation::context_digest(&request, identity(), &model.registry, &mut budget())
            .map_err(error)?;
    let reply = connection
        .receive_reply(
            &request,
            context,
            &model.registry,
            &sources,
            &sources,
            &mut admission,
            &mut transport,
            &mut budget(),
        )
        .map_err(error)?;
    let OperationReply::Result(result) = reply else {
        return Err("expected terminal result".into());
    };
    let actual = complete_value(&result)?;
    assert_eq!(actual, complete_value(&native)?);
    // Independent arithmetic oracle: (-7)+2=-5; (-3)*(4+2)=-18.
    let [NdfValue::Integer(integer)] = actual.fields.as_slice() else {
        return Err("expected Integer".into());
    };
    assert_eq!(integer.as_bigint().to_string(), expected.to_string());
    connection
        .send(
            &ProviderFrame::Close,
            &model.registry,
            &sources,
            &mut admission,
            &mut transport,
        )
        .map_err(error)
}

fn run_case(text: &'static str, expected: i64, case: Case) -> Result<(), String> {
    let mut command = Command::new(std::env::current_exe().map_err(error)?);
    command.arg("--child").arg(text);
    if matches!(case, Case::Suspending | Case::Cancel | Case::Stopped) {
        command.arg("--suspending");
    }
    command.stderr(Stdio::piped());
    if matches!(case, Case::NoGrant) {
        command.arg("--no-grant");
    }
    let mut process = Process::spawn(&mut command).map_err(error)?;
    let Some(connection) = process.take_transport() else {
        return Err(format!(
            "missing transport; cleanup: {:?}",
            process.terminate()
        ));
    };
    let Some(stderr) = process.take_stderr() else {
        return Err(format!(
            "missing stderr; cleanup: {:?}",
            process.terminate()
        ));
    };
    let diagnostics = std::thread::spawn(move || {
        let mut text = String::new();
        stderr
            .take(16_384)
            .read_to_string(&mut text)
            .map_err(error)?;
        Ok::<_, String>(text)
    });
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let outcome = exchange(connection, text, expected, case);
        let _ = sender.send(()); // Receiver may have timed out; outcome is joined.
        outcome
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let observed: Result<std::process::ExitStatus, String> = (|| {
        receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(error)?;
        loop {
            if let Some(status) = process.try_wait().map_err(error)? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err("child exit deadline exceeded".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    let (status, timeout) = match observed {
        Ok(status) => (status, None),
        Err(reason) => match process.terminate() {
            Ok(status) => (status, Some(reason)),
            Err(cleanup) => {
                return Err(format!(
                    "{reason}; child cleanup failed: {cleanup}; worker detached"
                ));
            }
        },
    };
    // Confirmed child exit releases its protocol handles before joining.
    let result = worker
        .join()
        .map_err(|_| "protocol worker panicked".to_owned())?;
    let diagnostics = diagnostics
        .join()
        .map_err(|_| "stderr worker panicked".to_owned())??;
    if let Some(reason) = timeout {
        return Err(format!("{reason}; worker: {result:?}"));
    }
    result?;
    if status.success()
        != matches!(
            case,
            Case::Valid | Case::Suspending | Case::Cancel | Case::Stopped
        )
    {
        return Err(format!("unexpected child exit {status} for {case:?}"));
    }
    let expected_diagnostic = match case {
        Case::Valid | Case::Suspending | Case::Cancel | Case::Stopped => "",
        Case::Corrupt => "Error: \"Wire(InvalidType)\"",
        Case::NoGrant => "Error: \"Source\"",
        Case::WrongOperation => "Error: \"OperationMismatch\"",
    };
    assert_eq!(diagnostics.trim(), expected_diagnostic);
    Ok(())
}

pub fn run() -> Result<(), String> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "--child") {
        if args.iter().any(|arg| arg == "--suspending") {
            return suspension::child(args.get(2).ok_or("missing fixture input")?);
        }
        return child(
            args.get(2).ok_or("missing fixture input")?,
            args.iter().any(|arg| arg == "--no-grant"),
        );
    }
    run_case("add framed frame neg 7 2", -5, Case::Valid)?;
    run_case("mul neg 3 add 4 2", -18, Case::Valid)?;
    for case in [Case::Corrupt, Case::NoGrant, Case::WrongOperation] {
        run_case("add framed frame neg 7 2", -5, case)?;
    }
    run_case("add framed frame neg 7 2", -5, Case::Suspending)?;
    run_case("mul neg 3 add 4 2", -18, Case::Suspending)?;
    run_case("add framed frame neg 7 2", -5, Case::Cancel)?;
    run_case(
        concat!(
            "add framed frame mul ",
            "1234567890123456789012345678901234567890123456789012345678901234567890 ",
            "1234567890123456789012345678901234567890123456789012345678901234567890 2"
        ),
        0,
        Case::Stopped,
    )?;
    println!(
        "process: 9 passed (4 native/process comparisons; 3 admission failures; explicit and stopped-dependency cancellation)"
    );
    Ok(())
}
