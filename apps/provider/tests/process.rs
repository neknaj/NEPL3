#![cfg(not(target_family = "wasm"))]
use nepl3_provider::process::Process;
use std::{
    io::{self, Read},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn child() -> io::Result<Process> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            "--ignored",
            "--exact",
            "child_waits_for_input_eof",
            "--nocapture",
        ])
        .env("NEPL3_PROCESS_TEST_CHILD", "1")
        .stderr(Stdio::null());
    Process::spawn(&mut command)
}

// This fixture is run explicitly by the parent tests in a separate OS process.
#[test]
#[ignore = "subprocess fixture; invoked by process ownership tests"]
fn child_waits_for_input_eof() -> io::Result<()> {
    if std::env::var_os("NEPL3_PROCESS_TEST_CHILD").is_none() {
        return Ok(());
    }
    let mut byte = [0];
    while io::stdin().read(&mut byte)? != 0 {}
    Ok(())
}

#[test]
fn terminate_reaps_child_while_transport_is_owned_separately() -> io::Result<()> {
    let mut process = child()?;
    let id = process.id();
    let transport = process.take_transport();
    let second = process.take_transport();
    let running = process.try_wait()?;
    // Cleanup precedes assertions, including failure of the running-state check.
    let status = process.terminate()?;
    assert_ne!(id, 0);
    assert!(transport.is_some());
    assert!(second.is_none());
    assert!(running.is_none());
    assert_eq!(process.try_wait()?, Some(status));
    assert_eq!(process.terminate()?, status);
    drop(transport);
    Ok(())
}

#[test]
fn pipe_eof_allows_normal_exit_and_exit_status_is_preserved() -> io::Result<()> {
    let mut process = child()?;
    let Some(transport) = process.take_transport() else {
        process.terminate()?;
        return Err(io::Error::other("protocol pipes unavailable"));
    };
    let (mut stdout, stdin) = transport.into_parts();
    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = process.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            process.terminate()?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "fixture did not exit after EOF",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut output = String::new();
    stdout.read_to_string(&mut output)?;
    assert!(status.success(), "{output}");
    assert_eq!(process.terminate()?, status);
    Ok(())
}
