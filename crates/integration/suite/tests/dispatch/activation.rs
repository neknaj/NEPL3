use super::*;
use nepl3_core::operation::{
    Continuation, OperationReply,
    lifetime::{LifetimeError, RequestLifetimes, RequestPhase},
};
use nepl3_suite::{
    grants::{Grants, dependencies::OperationGrant},
    suspension::host::{ActivationError, activate},
};

fn running(parent: &Invoke, context: Digest) -> RequestLifetimes {
    let mut table = RequestLifetimes::default();
    assert_eq!(
        table.begin_call(parent, context, None, &mut budget()),
        Ok(())
    );
    table
}
fn reply(parent: &Invoke, context: Digest) -> OperationReply {
    let mut call = parent.clone();
    call.request_id = parent.request_id + 1;
    if let TypedValue::Record(record) = &mut call.input {
        record.fields[0] = NdfValue::U64(3);
    }
    OperationReply::Await {
        continuation: Continuation {
            provider: parent.operation.clone(),
            parent_request: parent.request_id,
            snapshot_digest: context,
            state: parent.input.clone(),
        },
        calls: vec![call],
        report: Report::default(),
    }
}
fn unchanged(table: &RequestLifetimes, parent: &Invoke) {
    assert_eq!(
        table.phase(parent.request_id, &mut budget()),
        Ok(RequestPhase::Running)
    );
    assert_eq!(
        table.phase(parent.request_id + 1, &mut budget()),
        Err(LifetimeError::UnknownRequest)
    );
}

#[test]
fn activation_publishes_authorized_calls_and_preallocated_result_collection() -> Result<(), String>
{
    let (registry, parent) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&parent.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let policy = [OperationGrant {
        operation: &parent.operation,
        grants: &grants,
    }];
    let context = Digest::of(b"host context");
    let reply = reply(&parent, context);
    let mut table = running(&parent, context);
    let mut active = activate(
        &parent,
        context,
        &reply,
        &policy,
        &[context],
        &registry,
        &sources,
        &mut table,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        table.phase(parent.request_id, &mut budget()),
        Ok(RequestPhase::Awaiting)
    );
    let [approved] = active.authorized.as_slice() else {
        return Err("one dependency expected".into());
    };
    let call = approved.invocation().request();
    assert_eq!(
        table.phase(call.request_id, &mut budget()),
        Ok(RequestPhase::Running)
    );
    active
        .pending
        .accept_active(
            call.request_id,
            context,
            OperationResult::Complete {
                value: call.input.clone(),
                report: Report::default(),
            },
            &registry,
            &sources,
            &mut table,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        table.phase(call.request_id, &mut budget()),
        Ok(RequestPhase::Finished)
    );
    let resume = active
        .pending
        .take_resume(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(table.check_resume(&resume, &mut budget()), Ok(()));
    Ok(())
}

#[test]
fn dependency_delivery_commits_result_and_lifetime_together() -> Result<(), String> {
    use nepl3_core::operation::dependencies::DependencyError;
    let (registry, parent) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&parent.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let policy = [OperationGrant {
        operation: &parent.operation,
        grants: &grants,
    }];
    let context = Digest::of(b"host context");
    let reply = reply(&parent, context);
    let setup = || {
        let mut table = running(&parent, context);
        let active = activate(
            &parent,
            context,
            &reply,
            &policy,
            &[context],
            &registry,
            &sources,
            &mut table,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        Ok::<_, String>((table, active))
    };
    let result = OperationResult::Complete {
        value: parent.input.clone(),
        report: Report::default(),
    };
    let id = parent.request_id + 1;
    let (mut table, mut active) = setup()?;
    assert!(matches!(
        active.pending.accept_active(
            id,
            Digest::of(b"wrong"),
            result.clone(),
            &registry,
            &sources,
            &mut table,
            &mut budget()
        ),
        Err(DependencyError::Lifetime(_))
    ));
    assert_eq!(active.pending.remaining(), 1);
    assert_eq!(table.phase(id, &mut budget()), Ok(RequestPhase::Running));
    let mut measured = budget();
    active
        .pending
        .accept_active(
            id,
            context,
            result.clone(),
            &registry,
            &sources,
            &mut table,
            &mut measured,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(active.pending.remaining(), 0);
    assert_eq!(table.phase(id, &mut budget()), Ok(RequestPhase::Finished));
    assert!(matches!(
        active.pending.accept_active(
            id,
            context,
            result.clone(),
            &registry,
            &sources,
            &mut table,
            &mut budget()
        ),
        Err(DependencyError::DuplicateReply)
    ));
    for limit in 0..measured.usage().work {
        let (mut table, mut active) = setup()?;
        let mut limited = Budget::new(Limits {
            work: limit,
            ..budget().limits()
        });
        assert!(
            matches!(
                active.pending.accept_active(
                    id,
                    context,
                    result.clone(),
                    &registry,
                    &sources,
                    &mut table,
                    &mut limited
                ),
                Err(DependencyError::Stopped(StopReason::WorkLimit))
            ),
            "limit {limit}"
        );
        assert_eq!(active.pending.remaining(), 1);
        assert_eq!(table.phase(id, &mut budget()), Ok(RequestPhase::Running));
        assert_eq!(
            table.phase(parent.request_id, &mut budget()),
            Ok(RequestPhase::Awaiting)
        );
    }
    Ok(())
}

#[test]
fn activation_failure_and_every_budget_stop_preserve_running_parent() -> Result<(), String> {
    let (registry, parent) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&parent.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let policy = [OperationGrant {
        operation: &parent.operation,
        grants: &grants,
    }];
    let context = Digest::of(b"host context");
    let reply = reply(&parent, context);
    let mut table = running(&parent, context);
    assert!(matches!(
        activate(
            &parent,
            context,
            &reply,
            &policy,
            &[],
            &registry,
            &sources,
            &mut table,
            &mut budget()
        ),
        Err(ActivationError::ContextCount)
    ));
    unchanged(&table, &parent);
    assert!(matches!(
        activate(
            &parent,
            context,
            &reply,
            &[],
            &[context],
            &registry,
            &sources,
            &mut table,
            &mut budget()
        ),
        Err(ActivationError::Grants(_))
    ));
    unchanged(&table, &parent);
    let mut bad = reply.clone();
    if let OperationReply::Await { continuation, .. } = &mut bad {
        continuation.parent_request += 10;
    }
    assert!(matches!(
        activate(
            &parent,
            context,
            &bad,
            &policy,
            &[context],
            &registry,
            &sources,
            &mut table,
            &mut budget()
        ),
        Err(ActivationError::Await(_))
    ));
    unchanged(&table, &parent);
    let mut measured = budget();
    activate(
        &parent,
        context,
        &reply,
        &policy,
        &[context],
        &registry,
        &sources,
        &mut table,
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    for (allocation, count) in [
        (false, measured.usage().work),
        (true, measured.usage().allocation_units),
    ] {
        for limit in 0..count {
            let mut table = running(&parent, context);
            let mut limits = budget().limits();
            let reason = if allocation {
                limits.allocation_units = limit;
                StopReason::AllocationLimit
            } else {
                limits.work = limit;
                StopReason::WorkLimit
            };
            let mut limited = Budget::new(limits);
            assert!(
                matches!(activate(&parent, context, &reply, &policy, &[context], &registry, &sources, &mut table, &mut limited), Err(ActivationError::Stopped(s)) if s == reason),
                "limit {limit}"
            );
            assert_eq!(limited.poll(), Err(reason));
            unchanged(&table, &parent);
        }
    }
    Ok(())
}
