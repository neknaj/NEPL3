//! Real blocked reads must end after the host terminates and reaps the child.
use super::*;
use nepl3_provider::TransportError;
use std::io::{Read, Write};

#[derive(Clone, Copy, Debug)]
pub(super) enum Mode {
    Silent,
    Partial,
}
impl Mode {
    fn argument(self) -> &'static str {
        match self {
            Self::Silent => "--deadline-silent",
            Self::Partial => "--deadline-partial",
        }
    }
}
pub(super) fn child_mode() -> Option<Mode> {
    [Mode::Silent, Mode::Partial]
        .into_iter()
        .find(|mode| std::env::args().any(|arg| arg == mode.argument()))
}
pub(super) fn child(mode: Mode) -> Result<(), String> {
    let mut output = io::stdout().lock();
    // Private test startup marker, consumed before constructing Connection.
    output.write_all(&[0x7f]).map_err(error)?;
    if matches!(mode, Mode::Partial) {
        let bytes = nepl3_wire::operation::encode_frame(
            &ProviderFrame::Close,
            &bootstrap()?,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        let prefix = bytes
            .get(..bytes.len().saturating_sub(1))
            .ok_or("frame prefix")?;
        output.write_all(prefix).map_err(error)?;
    }
    output.flush().map_err(error)?;
    // The parent keeps stdin open. Only explicit host termination ends this read.
    let mut byte = [0];
    while io::stdin().read(&mut byte).map_err(error)? != 0 {}
    Err("fixture exited through stdin instead of forced termination".into())
}

pub(super) fn run() -> Result<(), String> {
    for mode in [Mode::Silent, Mode::Partial] {
        run_case(mode).map_err(|e| format!("deadline {mode:?}: {e}"))?;
    }
    Ok(())
}
fn run_case(mode: Mode) -> Result<(), String> {
    let mut command = Command::new(std::env::current_exe().map_err(error)?);
    command.arg(mode.argument());
    let mut process = Process::spawn(&mut command).map_err(error)?;
    let Some(connection) = process.take_transport() else {
        let cleanup = process.terminate();
        return Err(format!("missing pipes; cleanup: {cleanup:?}"));
    };
    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || -> Result<(), String> {
        let (mut input, output) = connection.into_parts();
        let mut marker = [0];
        input.read_exact(&mut marker).map_err(error)?;
        if marker != [0x7f] {
            return Err("incorrect startup marker".into());
        }
        ready_tx.send(()).map_err(error)?;
        let mut connection = Connection::new(input, output);
        let result = connection.receive(
            &bootstrap()?,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget(),
        );
        let expected = matches!(
            (&mode, &result),
            (Mode::Silent, Ok(None)) | (Mode::Partial, Err(TransportError::Truncated))
        );
        let observed = if expected && connection.is_closed() {
            Ok(())
        } else {
            Err(format!("unexpected post-termination reply: {result:?}"))
        };
        done_tx.send(observed).map_err(error)
    });
    let ready = ready_rx.recv_timeout(Duration::from_secs(10));
    let before = if ready.is_ok() {
        Some(done_rx.recv_timeout(Duration::from_secs(1)))
    } else {
        None
    };
    // Cleanup precedes every assertion and worker join. Failure to confirm exit
    // is reported explicitly; a potentially blocked worker is then detached.
    let status = process
        .terminate()
        .map_err(|e| format!("termination failed: {e}; worker detached"))?;
    let after = done_rx.recv_timeout(Duration::from_secs(10));
    if after.is_err() {
        return Err(format!(
            "worker failed to finish after reaping: {after:?}; startup: {ready:?}; before: {before:?}"
        ));
    }
    worker.join().map_err(|_| "deadline worker panicked")??;
    ready.map_err(error)?;
    if !matches!(before, Some(Err(mpsc::RecvTimeoutError::Timeout))) {
        return Err(format!("receive completed before deadline: {before:?}"));
    }
    after.map_err(error)??;
    if status.success() {
        return Err("blocked fixture exited successfully".into());
    }
    if process.try_wait().map_err(error)? != Some(status)
        || process.terminate().map_err(error)? != status
    {
        return Err("reaped exit status was not preserved".into());
    }
    Ok(())
}
