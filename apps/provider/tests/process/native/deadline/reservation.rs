//! A real blocked provider has no authenticated terminal Usage to settle.
use super::*;
use nepl3_provider::{delegation::IssuedInvocation, reply::ReplyError};
use std::io::Cursor;
struct Counted<R> {
    inner: R,
    calls: u64,
    bytes: u64,
}
impl<R: Read> Read for Counted<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        let result = self.inner.read(bytes);
        if let Ok(n) = result {
            self.bytes += n as u64;
        }
        result
    }
}
fn local_limits(parent: Limits, grant: Limits) -> Limits {
    Limits {
        source_bytes: parent.source_bytes - grant.source_bytes,
        work: parent.work - grant.work,
        depth: parent.depth,
        nodes: parent.nodes - grant.nodes,
        allocation_units: parent.allocation_units - grant.allocation_units,
        output_bytes: parent.output_bytes - grant.output_bytes,
        diagnostics: parent.diagnostics - grant.diagnostics,
        events: parent.events - grant.events,
    }
}
pub(super) fn receive(
    connection: Connection<std::process::ChildStdout, std::process::ChildStdin>,
    mode: Mode,
    ready: mpsc::Sender<()>,
) -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    let limits = request.limits;
    request.limits = Limits {
        source_bytes: limits.source_bytes / 4,
        work: limits.work / 4,
        depth: limits.depth,
        nodes: limits.nodes / 4,
        allocation_units: limits.allocation_units / 4,
        output_bytes: limits.output_bytes / 4,
        diagnostics: limits.diagnostics / 4,
        events: limits.events / 4,
    };
    let sources = granted_sources()?;
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut parent = budget();
    parent.charge(Resource::Work, 117).map_err(error)?;
    let outer_limits = parent.limits();
    let mut lifetimes = RequestLifetimes::default();
    let mut issued = IssuedInvocation::issue_with_parent_validation(
        grants.admit(&request, &mut budget()).map_err(error)?,
        identity(),
        &registry,
        &mut parent,
    )
    .map_err(error)?;
    let context = issued.context();
    issued
        .run_local(|b| Ok(lifetimes.begin_call(&request, context, None, b)))
        .map_err(error)?
        .map_err(error)?;
    // An already terminal lifetime must not receive a cancellation callback.
    issued
        .run_local(|b| {
            Ok((|| {
                lifetimes.begin(99, request.operation.clone(), context, b)?;
                lifetimes.finish(99, b)
            })())
        })
        .map_err(error)?
        .map_err(error)?;
    let (input, output) = connection.into_parts();
    let mut connection = Connection::new(
        Counted {
            inner: input,
            calls: 0,
            bytes: 0,
        },
        output,
    );
    let mut admission = SourceAdmission::default();
    let outgoing = ProviderFrame::Invoke(request.clone());
    issued
        .run_local(|b| Ok(connection.send(&outgoing, &registry, &sources, &mut admission, b)))
        .map_err(error)?
        .map_err(error)?;
    let before_receive = issued.parent_usage();
    // The main thread starts its deadline only after actual Invoke transmission.
    ready.send(()).map_err(error)?;
    let result = issued
        .run_local(|b| {
            Ok(connection.receive_reply_with_budget(
                &request,
                context,
                &registry,
                &sources,
                &sources,
                &mut admission,
                b,
            ))
        })
        .map_err(error)?;
    let expected = matches!(
        (&mode, &result),
        (Mode::Silent, Err(ReplyError::Closed))
            | (
                Mode::Partial,
                Err(ReplyError::Transport(TransportError::Truncated))
            )
    );
    if !expected || !connection.is_closed() {
        return Err(format!("reserved post-termination reply: {result:?}"));
    }
    let after_receive = issued.parent_usage();
    let (input, _output) = connection.into_parts();
    let bytes = match mode {
        Mode::Silent => vec![],
        Mode::Partial => {
            let mut v = nepl3_wire::operation::encode_frame(
                &ProviderFrame::Close,
                &bootstrap()?,
                &SourceStore::default(),
                &mut SourceAdmission::default(),
                &mut budget(),
            )
            .map_err(error)?;
            v.pop();
            v
        }
    };
    assert_eq!(input.bytes, bytes.len() as u64);
    let mut oracle_connection = Connection::new(
        Counted {
            inner: Cursor::new(bytes),
            calls: 0,
            bytes: 0,
        },
        io::sink(),
    );
    let mut oracle = Budget::new(local_limits(outer_limits, request.limits));
    oracle
        .record_observed_usage(before_receive)
        .map_err(error)?;
    let oracle_result = oracle_connection.receive_reply_with_budget(
        &request,
        context,
        &registry,
        &sources,
        &sources,
        &mut SourceAdmission::default(),
        &mut oracle,
    );
    assert!(matches!(
        (&mode, oracle_result),
        (Mode::Silent, Err(ReplyError::Closed))
            | (
                Mode::Partial,
                Err(ReplyError::Transport(TransportError::Truncated))
            )
    ));
    let (oracle_input, _) = oracle_connection.into_parts();
    let mut expected_usage = oracle.usage();
    // fill charges one Work before each read, including interrupted attempts.
    // Normalize only measured call counts; OS chunking is not fixed by Cursor.
    expected_usage.work = expected_usage
        .work
        .checked_sub(oracle_input.calls)
        .and_then(|v| v.checked_add(input.calls))
        .ok_or("read-call accounting overflow")?;
    assert_eq!(after_receive, expected_usage);
    assert_eq!(issued.run_local(|b| Ok(b.poll())).map_err(error)?, Ok(()));
    let mut cancelled = Vec::new();
    lifetimes.close(|id| cancelled.push(id));
    lifetimes.close(|id| cancelled.push(id));
    assert_eq!(cancelled, vec![request.request_id]);
    assert_eq!(
        lifetimes
            .phase(request.request_id, &mut budget())
            .map_err(error)?,
        RequestPhase::Cancelled
    );
    assert_eq!(
        lifetimes.phase(99, &mut budget()).map_err(error)?,
        RequestPhase::Finished
    );
    assert_eq!(
        lifetimes.begin(100, request.operation.clone(), context, &mut budget()),
        Err(nepl3_core::operation::lifetime::LifetimeError::Closed)
    );
    let spent = issued.parent_usage();
    // Forced exit supplies no trustworthy Usage. Do not fabricate zero settlement.
    drop(issued);
    assert_eq!(parent.usage(), spent);
    assert_eq!(parent.limits(), outer_limits);
    assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    assert_eq!(parent.charge(Resource::Work, 1), Err(StopReason::Cancelled));
    assert_eq!(parent.usage(), spent);
    assert!(matches!(
        IssuedInvocation::issue_with_parent_validation(
            grants.admit(&request, &mut budget()).map_err(error)?,
            identity(),
            &registry,
            &mut parent,
        ),
        Err(nepl3_provider::delegation::Error::Stopped(
            StopReason::Cancelled
        ))
    ));
    assert_eq!(parent.usage(), spent);
    Ok(())
}
