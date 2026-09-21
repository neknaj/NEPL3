//! Native process ownership independent of blocking protocol I/O.
use crate::Connection;
use std::{
    io,
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
};

/// The host retains this handle while the transport may run on an I/O worker.
/// Call `terminate` or obtain an exit status through `try_wait` before dropping
/// it. Like `std::process::Child`, dropping this handle alone does not reap the
/// process. Explicit cleanup returns OS errors so the host can retry/report them.
/// Termination covers this child; sandbox/process-tree containment belongs to
/// the host's launch policy. No shell command is constructed by this adapter.
#[must_use = "the host must terminate/reap the provider process"]
pub struct Process {
    child: Child,
}

impl Process {
    /// Launch the host-authorized executable, arguments, directory and environment.
    /// stdin/stdout become protocol pipes. stderr follows the supplied command;
    /// a piped stderr must be taken and drained independently by the host.
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        command.stdin(Stdio::piped()).stdout(Stdio::piped());
        Ok(Self {
            child: command.spawn()?,
        })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Transfer the protocol pipes once, retaining process control in this owner.
    pub fn take_transport(&mut self) -> Option<Connection<ChildStdout, ChildStdin>> {
        if self.child.stdin.is_none() || self.child.stdout.is_none() {
            return None;
        }
        self.child
            .stdout
            .take()
            .zip(self.child.stdin.take())
            .map(|(stdout, stdin)| Connection::new(stdout, stdin))
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Poll and reap an exited child without waiting for a running operation.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// Force termination and reap the child, independently of protocol pipes.
    /// Already collected exit status is preserved. On OS failure this owner
    /// remains available for another cleanup attempt.
    pub fn terminate(&mut self) -> io::Result<ExitStatus> {
        if let Some(status) = self.child.try_wait()? {
            return Ok(status);
        }
        self.child.kill()?;
        self.child.wait()
    }
}
