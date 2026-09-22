use super::*;
use nepl3_core::operation::{
    Continuation, OperationReply,
    lifetime::{LifetimeError, RequestLifetimes, RequestPhase},
};
use nepl3_suite::{
    grants::{Grants, dependencies::OperationGrant},
    suspension::host::{ActivationError, activate, activate_owned, prepare_owned},
};

fn running(parent: &Invoke, context: Digest) -> RequestLifetimes {
    let mut table = RequestLifetimes::default();
    assert_eq!(
        table.begin_call(parent, context, None, &mut budget()),
        Ok(())
    );
    table
}

#[test]
fn prepared_await_moves_calls_and_avoids_repeated_validation_cost() -> Result<(), String> {
    let (registry, parent) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&parent.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let policy = [OperationGrant {
        operation: &parent.operation,
        grants: &grants,
    }];
    let context = Digest::of(b"prepared await");
    let mut old_budget = budget();
    let old_reply = reply(&parent, context);
    nepl3_suite::dispatch::suspending::validate_reply(
        &old_reply,
        &parent,
        context,
        &registry,
        &sources,
        &mut old_budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    let old = activate_owned(
        &parent,
        context,
        old_reply,
        &policy,
        &[context],
        &registry,
        &sources,
        &mut running(&parent, context),
        &mut old_budget,
    )
    .map_err(|e| format!("{e:?}"))?;

    let mut measured = budget();
    let mut table = running(&parent, context);
    let prepared = prepare_owned(
        &parent,
        context,
        reply(&parent, context),
        &registry,
        &sources,
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        table.phase(parent.request_id, &mut budget()),
        Ok(RequestPhase::Running)
    );
    let calls = prepared.calls().as_ptr();
    let active = prepared
        .activate(&policy, &[context], &mut table, &mut measured)
        .map_err(|e| format!("{e:?}"))?;
    // Owned calls survive activation at the same address; the indexed pending
    // collection moves intact. This compares the old two-stage path's cost.
    assert_eq!(active.pending.calls().as_ptr(), calls);
    assert_eq!(active.pending.calls(), old.pending.calls());
    assert!(measured.usage().work < old_budget.usage().work);
    assert!(measured.usage().allocation_units < old_budget.usage().allocation_units);
    assert_eq!(
        table.phase(parent.request_id, &mut budget()),
        Ok(RequestPhase::Awaiting)
    );
    Ok(())
}

#[test]
fn prepared_await_rejects_parent_and_duplicate_ids_before_publication() -> Result<(), String> {
    use nepl3_core::operation::dependencies::DependencyError;
    use nepl3_suite::suspension::AwaitError;
    let (registry, parent) = fixture()?;
    let sources = SourceStore::default();
    let context = Digest::of(b"prepared ID rejection");
    for duplicate in [false, true] {
        let mut reply = reply(&parent, context);
        let OperationReply::Await { calls, .. } = &mut reply else {
            return Err("expected fixture Await".into());
        };
        if duplicate {
            calls.push(calls[0].clone());
        } else {
            calls[0].request_id = parent.request_id;
        }
        let result = prepare_owned(&parent, context, reply, &registry, &sources, &mut budget());
        assert!(matches!(result,
            Err(ActivationError::Await(AwaitError::Dependencies(error)))
            if error == if duplicate { DependencyError::DuplicateId } else { DependencyError::ParentId }
        ));
    }
    Ok(())
}

#[test]
fn owned_activation_retains_calls_and_publishes_only_after_all_budgeted_checks()
-> Result<(), String> {
    let (registry, parent) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&parent.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let policy = [OperationGrant {
        operation: &parent.operation,
        grants: &grants,
    }];
    let context = Digest::of(b"owned host context");
    let mut table = running(&parent, context);
    let mut measured = budget();
    // The temporary provider reply is moved into state which survives admission.
    let active = activate_owned(
        &parent,
        context,
        reply(&parent, context),
        &policy,
        &[context],
        &registry,
        &sources,
        &mut table,
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(active.report, Report::default());
    let mut pending = active.pending;
    let admission_usage = measured.usage();
    assert_eq!(
        table.phase(parent.request_id, &mut budget()),
        Ok(RequestPhase::Awaiting)
    );
    let child_id = pending.calls()[0].request_id;
    let value = pending.calls()[0].input.clone();
    assert_eq!(
        table.phase(child_id, &mut budget()),
        Ok(RequestPhase::Running)
    );
    pending
        .accept_active(
            child_id,
            context,
            OperationResult::Complete {
                value,
                report: Report::default(),
            },
            &registry,
            &sources,
            &mut table,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    let resume = pending
        .take_resume(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(table.check_resume(&resume, &mut budget()), Ok(()));
    for allocation in [false, true] {
        let count = if allocation {
            admission_usage.allocation_units
        } else {
            admission_usage.work
        };
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
                matches!(activate_owned(&parent, context, reply(&parent, context),
                &policy, &[context], &registry, &sources, &mut table, &mut limited),
                Err(ActivationError::Stopped(s)) if s == reason)
            );
            unchanged(&table, &parent);
        }
    }
    let mut table = running(&parent, context);
    assert!(matches!(
        activate_owned(
            &parent,
            context,
            reply(&parent, context),
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
    Ok(())
}
#[test]
fn owned_activation_preserves_provider_report_without_absorbing_claimed_usage() -> Result<(), String>
{
    use nepl3_core::diagnostic::{Diagnostic, Event, Severity};
    let (registry, parent) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&parent.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let policy = [OperationGrant {
        operation: &parent.operation,
        grants: &grants,
    }];
    let context = Digest::of(b"report context");
    let mut expected = Report::default();
    expected.usage.work = u64::MAX;
    expected.usage.diagnostics = 1;
    expected.usage.events = 1;
    expected.diagnostics.push(Diagnostic {
        schema: parent.operation.schema.clone(),
        code: "await-note".into(),
        severity: Severity::Information,
        stage: "invoke".into(),
        arguments: parent.input.clone(),
        primary: None,
        related: vec![],
        fixes: vec![],
    });
    expected.events.push(Event {
        schema: parent.operation.schema.clone(),
        kind: "suspend".into(),
        operation_path: vec![parent.request_id],
        span: None,
        payload: parent.input.clone(),
    });
    let mut response = reply(&parent, context);
    if let OperationReply::Await { report, .. } = &mut response {
        *report = expected.clone();
    }
    let mut table = running(&parent, context);
    let mut measured = budget();
    let active = activate_owned(
        &parent,
        context,
        response,
        &policy,
        &[context],
        &registry,
        &sources,
        &mut table,
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(active.report, expected);
    assert_eq!(measured.poll(), Ok(()));
    assert!(measured.usage().work < measured.limits().work);
    assert_eq!(active.pending.remaining(), 1);
    Ok(())
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
