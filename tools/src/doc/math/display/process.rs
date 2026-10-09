//! Poll-driven native process ownership for trusted Node and bridge installations.
//! Completed bytes are unchecked; no JSON, visual or Doc admission happens here.
use super::{PreparedDisplay, request::PreparedRequest};
use nepl3_core::budget::{Budget, Resource, StopReason};
use std::{
    io::{self, Read, Write},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config<'a> {
    pub node: &'a Path,
    pub bridge: &'a Path,
    pub modules_url: &'a str,
    pub input_cap: usize,
    pub output_cap: usize,
    pub timeout: Duration,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Budget(StopReason),
    Cancelled,
    Deadline,
    Configuration,
    InputLimit,
    Spawn(io::ErrorKind),
    Pipe,
    ThreadSpawn,
    ThreadPanic,
    InputIo(io::ErrorKind),
    OutputIo(io::ErrorKind),
    OutputLimit,
    UnexpectedStderr,
    Exit,
    EmptyReply,
}
impl Failure {
    fn stopped(self) -> bool {
        matches!(self, Self::Budget(_) | Self::Cancelled | Self::Deadline)
    }
}
#[must_use]
pub struct Completed<'a> {
    request: &'a PreparedRequest<'a>,
    config: Config<'a>,
    bytes: Vec<u8>,
    exit: ExitStatus,
}
impl Completed<'_> {
    /// Retains the actual launch input, not replacement decoder-side controls.
    pub fn request(&self) -> &PreparedRequest<'_> {
        self.request
    }
    /// Recorded trusted host selection; path equality is not a code identity.
    pub fn config(&self) -> Config<'_> {
        self.config
    }
    pub fn owner(&self) -> &PreparedDisplay {
        self.request.owner()
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn exit(&self) -> ExitStatus {
        self.exit
    }
}
#[must_use]
pub enum Poll<'a> {
    Pending {
        failure: Option<Failure>,
        termination_error: Option<io::ErrorKind>,
    },
    Complete(Completed<'a>),
    Failed(Failure),
    Consumed,
}
type Input = JoinHandle<Result<(), io::ErrorKind>>;
type Output = JoinHandle<Result<Vec<u8>, Failure>>;
type Diagnostic = JoinHandle<Result<(), Failure>>;
/// Drive poll until terminal. A pending cleanup remains owned, including after
/// deadline/cancel/kill failure. Drop makes a best-effort kill but certifies no
/// exit or pipe cleanup; callers requiring confirmation must not abandon it.
/// Deadline enforcement is cooperative at poll calls. No process-tree or
/// inherited-pipe reclamation guarantee exists: dropping pending I/O detaches
/// its threads and their bounded buffers may remain alive until the pipe closes.
#[must_use]
pub struct Process<'a> {
    request: &'a PreparedRequest<'a>,
    config: Config<'a>,
    state: State,
}
// Only this private lifetime-free state owns the native resources and Drop.
struct State {
    child: Child,
    deadline: Instant,
    input: Option<Input>,
    output: Option<Output>,
    diagnostic: Option<Diagnostic>,
    bytes: Option<Vec<u8>>,
    exit: Option<ExitStatus>,
    failure: Option<Failure>,
    termination_error: Option<io::ErrorKind>,
    consumed: bool,
}
enum StatePoll {
    Pending {
        failure: Option<Failure>,
        termination_error: Option<io::ErrorKind>,
    },
    Complete {
        bytes: Vec<u8>,
        exit: ExitStatus,
    },
    Failed {
        failure: Failure,
        exit: ExitStatus,
    },
    Consumed,
}

/// Reclamation status only. This never certifies a decoded reply, successful
/// generation, renderer/Worker cleanup, or process-tree termination.
#[must_use]
pub enum CleanupPoll {
    Pending {
        failure: Option<Failure>,
        termination_error: Option<io::ErrorKind>,
    },
    Reclaimed {
        failure: Option<Failure>,
        exit: ExitStatus,
    },
    Consumed,
}
/// Owns only already-admitted native resources, independent of request/config
/// storage. Keep this handle until Reclaimed/Consumed; Drop remains best effort.
/// No new execution budget, threads, decoder or allocation is introduced by
/// transfer or polling. OS calls still have no hard wall-clock guarantee.
#[must_use]
pub struct Cleanup {
    state: State,
    parent_stop: Option<StopReason>,
}
impl Cleanup {
    /// Parent stop observed at transfer, separately from the selected transport
    /// cause. This is not a complete driver outcome or execution Usage report.
    pub fn parent_stop(&self) -> Option<StopReason> {
        self.parent_stop
    }
    pub fn poll(&mut self) -> CleanupPoll {
        match self.state.poll(None) {
            StatePoll::Pending {
                failure,
                termination_error,
            } => CleanupPoll::Pending {
                failure,
                termination_error,
            },
            StatePoll::Failed { failure, exit } => {
                self.state.bytes = None;
                CleanupPoll::Reclaimed {
                    failure: Some(failure),
                    exit,
                }
            }
            StatePoll::Complete { bytes: _, exit } => CleanupPoll::Reclaimed {
                failure: None,
                exit,
            },
            StatePoll::Consumed => CleanupPoll::Consumed,
        }
    }
}
impl<'a> Process<'a> {
    pub fn cancel(&mut self) {
        self.state.cancel();
    }
    /// One execution poll using the original cumulative execution Budget.
    /// Do not substitute a fresh ledger. A stopped budget still permits native cleanup.
    pub fn poll(&mut self, b: &mut Budget) -> Poll<'a> {
        match self.state.poll(Some(b)) {
            StatePoll::Pending {
                failure,
                termination_error,
            } => Poll::Pending {
                failure,
                termination_error,
            },
            StatePoll::Complete { bytes, exit } => Poll::Complete(Completed {
                request: self.request,
                config: self.config,
                bytes,
                exit,
            }),
            StatePoll::Failed { failure, .. } => Poll::Failed(failure),
            StatePoll::Consumed => Poll::Consumed,
        }
    }
    /// Use the original cumulative execution Budget to snapshot the parent stop.
    /// Infallible move into cleanup-only ownership with no new application allocation. Preserve a
    /// selected cause; fill an absent cause from the stopped parent budget or
    /// host cancellation. Do not use ordinary cancel(), which reprioritizes
    /// stopped failures. An already-consumed process stays consumed; this does
    /// not reconstruct earlier EmptyReply/decode errors or delivered replies.
    pub fn into_cleanup(self, b: &Budget) -> Cleanup {
        self.state.into_cleanup(b)
    }
}
fn buffer(n: usize, b: &mut Budget) -> Result<Vec<u8>, Failure> {
    if n > isize::MAX as usize {
        return Err(Failure::Budget(b.stop(StopReason::AllocationLimit)));
    }
    b.charge(Resource::AllocationUnits, n as u64)
        .map_err(Failure::Budget)?;
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| Failure::Budget(b.stop(StopReason::AllocationLimit)))?;
    Ok(v)
}
/// Host configuration is trusted. No document-supplied executable/module URL is
/// accepted by the prepared input. Node preload/module-search options are removed; the remaining host
/// environment and executable installation are trusted. Charges
/// prepay retained native byte buffers and their maximum byte-processing work;
/// process/thread/runtime overhead and external Work/Allocation stay unmeasured.
pub fn start<'a>(
    request: &'a PreparedRequest<'_>,
    c: Config<'a>,
    b: &mut Budget,
) -> Result<Process<'a>, Failure> {
    let state = start_state(request.bytes(), c, b)?;
    Ok(Process {
        request,
        config: c,
        state,
    })
}
// Private transport construction. Only sealed launch owners may pair this state
// with request provenance; raw bytes alone authorize no decoded association.
fn start_state(bytes: &[u8], c: Config<'_>, b: &mut Budget) -> Result<State, Failure> {
    b.poll().map_err(Failure::Budget)?;
    let millis = c.timeout.as_millis();
    if c.output_cap < 256
        || c.input_cap as u128 > ((1u128 << 53) - 1)
        || c.output_cap as u128 > ((1u128 << 53) - 1)
        || millis == 0
        || millis > 2147483647
        || c.timeout != Duration::from_millis(millis as u64)
    {
        return Err(Failure::Configuration);
    }
    if bytes.len() > c.input_cap {
        return Err(Failure::InputLimit);
    }
    let deadline = Instant::now()
        .checked_add(c.timeout)
        .ok_or(Failure::Configuration)?;
    let mut input = buffer(bytes.len(), b)?;
    let output = buffer(c.output_cap, b)?;
    let work = bytes
        .len()
        .checked_add(c.output_cap)
        .and_then(|x| x.checked_add(1))
        .ok_or_else(|| Failure::Budget(b.stop(StopReason::WorkLimit)))?;
    b.charge(Resource::Work, work as u64)
        .map_err(Failure::Budget)?;
    input.extend_from_slice(bytes);
    if Instant::now() >= deadline {
        return Err(Failure::Deadline);
    }
    let child = Command::new(c.node)
        .arg("--")
        .arg(c.bridge)
        .arg(c.modules_url)
        .arg(c.input_cap.to_string())
        .arg(c.output_cap.to_string())
        .arg(millis.to_string())
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Failure::Spawn(e.kind()))?;
    let mut p = State {
        child,
        deadline,
        input: None,
        output: None,
        diagnostic: None,
        bytes: None,
        exit: None,
        failure: None,
        termination_error: None,
        consumed: false,
    };
    let (Some(mut stdin), Some(mut stdout), Some(mut stderr)) = (
        p.child.stdin.take(),
        p.child.stdout.take(),
        p.child.stderr.take(),
    ) else {
        p.fail(Failure::Pipe);
        p.terminate();
        return Ok(p);
    };
    match thread::Builder::new()
        .name("math-stdin".into())
        .spawn(move || stdin.write_all(&input).map_err(|e| e.kind()))
    {
        Ok(h) => p.input = Some(h),
        Err(_) => p.fail(Failure::ThreadSpawn),
    }
    let cap = c.output_cap;
    match thread::Builder::new()
        .name("math-stdout".into())
        .spawn(move || {
            let mut bytes = output;
            let mut chunk = [0u8; 4096];
            loop {
                let want = (cap - bytes.len() + 1).min(chunk.len());
                match stdout.read(&mut chunk[..want]) {
                    Ok(0) => return Ok(bytes),
                    Ok(n) => {
                        if n > cap - bytes.len() {
                            return Err(Failure::OutputLimit);
                        }
                        bytes.extend_from_slice(&chunk[..n]);
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(Failure::OutputIo(e.kind())),
                }
            }
        }) {
        Ok(h) => p.output = Some(h),
        Err(_) => p.fail(Failure::ThreadSpawn),
    }
    match thread::Builder::new()
        .name("math-stderr".into())
        .spawn(move || {
            let mut byte = [0u8; 1];
            loop {
                match stderr.read(&mut byte) {
                    Ok(0) => return Ok(()),
                    Ok(_) => return Err(Failure::UnexpectedStderr),
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(Failure::OutputIo(e.kind())),
                }
            }
        }) {
        Ok(h) => p.diagnostic = Some(h),
        Err(_) => p.fail(Failure::ThreadSpawn),
    }
    if p.failure.is_some() {
        p.terminate();
    }
    Ok(p)
}
impl State {
    fn into_cleanup(mut self, b: &Budget) -> Cleanup {
        let parent_stop = b.poll().err();
        if !self.consumed {
            if self.failure.is_none() {
                self.failure = Some(match parent_stop {
                    Some(s) => Failure::Budget(s),
                    None => Failure::Cancelled,
                });
            }
            self.terminate();
        }
        Cleanup {
            state: self,
            parent_stop,
        }
    }
}

impl State {
    fn fail(&mut self, f: Failure) {
        if self.failure.is_none() || (f.stopped() && !self.failure.is_some_and(Failure::stopped)) {
            self.failure = Some(f);
        }
    }
    fn terminate(&mut self) {
        if self.exit.is_some() {
            return;
        }
        if let Err(e) = self.child.kill() {
            self.termination_error = Some(e.kind());
        }
        // kill can race with an already completed child; only wait proves exit.
        match self.child.try_wait() {
            Ok(Some(s)) => {
                self.exit = Some(s);
                self.termination_error = None;
            }
            Ok(None) => {}
            Err(e) => self.termination_error = Some(e.kind()),
        }
    }
    fn cancel(&mut self) {
        self.fail(Failure::Cancelled);
        self.terminate();
    }
}
impl State {
    /// Nonblocking cleanup: never join a live I/O thread. A descendant retaining
    /// a pipe therefore remains Pending instead of hanging or claiming completion.
    /// Budget continuity is the caller's duty; this is not an operation Report.
    fn poll(&mut self, b: Option<&Budget>) -> StatePoll {
        if self.consumed {
            return StatePoll::Consumed;
        }
        if let Some(b) = b {
            if let Err(s) = b.poll() {
                self.fail(Failure::Budget(s));
            }
            if Instant::now() >= self.deadline {
                self.fail(Failure::Deadline);
            }
        }
        if self.input.as_ref().is_some_and(|h| h.is_finished())
            && let Some(h) = self.input.take()
        {
            match h.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => self.fail(Failure::InputIo(e)),
                Err(_) => self.fail(Failure::ThreadPanic),
            }
        }
        if self.output.as_ref().is_some_and(|h| h.is_finished())
            && let Some(h) = self.output.take()
        {
            match h.join() {
                Ok(Ok(bytes)) => self.bytes = Some(bytes),
                Ok(Err(e)) => self.fail(e),
                Err(_) => self.fail(Failure::ThreadPanic),
            }
        }
        if self.diagnostic.as_ref().is_some_and(|h| h.is_finished())
            && let Some(h) = self.diagnostic.take()
        {
            match h.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => self.fail(e),
                Err(_) => self.fail(Failure::ThreadPanic),
            }
        }
        if self.exit.is_none() {
            match self.child.try_wait() {
                Ok(Some(s)) => {
                    self.exit = Some(s);
                    self.termination_error = None;
                }
                Ok(None) => {}
                Err(e) => self.termination_error = Some(e.kind()),
            }
        }
        if self.exit.is_some_and(|s| !s.success()) {
            self.fail(Failure::Exit);
        }
        if self.failure.is_some() {
            self.terminate();
        }
        if self.exit.is_none()
            || self.input.is_some()
            || self.output.is_some()
            || self.diagnostic.is_some()
        {
            return StatePoll::Pending {
                failure: self.failure,
                termination_error: self.termination_error,
            };
        }
        // Final decision is after exit and all pipe tasks, not after message bytes.
        if let Some(b) = b {
            if let Err(s) = b.poll() {
                self.fail(Failure::Budget(s));
            }
            if Instant::now() >= self.deadline {
                self.fail(Failure::Deadline);
            }
        }
        let exit = match self.exit {
            Some(exit) => exit,
            None => {
                return StatePoll::Pending {
                    failure: self.failure,
                    termination_error: self.termination_error,
                };
            }
        };
        self.consumed = true;
        if let Some(failure) = self.failure {
            return StatePoll::Failed { failure, exit };
        }
        match self.bytes.take() {
            Some(bytes) if !bytes.is_empty() => StatePoll::Complete { bytes, exit },
            _ => StatePoll::Failed {
                failure: Failure::EmptyReply,
                exit,
            },
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        if self.exit.is_none() {
            self.terminate();
        }
    }
}

pub mod reply;

pub mod driver;

pub mod supervisor;

pub mod owned;
