use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    operation::{Continuation, ContinuationError, Resume},
    source::Digest,
    value::{NdfValue, OperationRef, Record, SchemaRef, TypedValue, Variant},
};

#[test]
fn request_lifetimes_preserve_rejected_await_and_close_every_pending_request() {
    use nepl3_core::operation::lifetime::{LifetimeError, RequestLifetimes, RequestPhase};
    let saved = saved();
    let mut table = RequestLifetimes::default();
    let mut b = budget();
    for id in [9, 7, 8] {
        assert_eq!(
            table.begin(id, saved.provider.clone(), saved.snapshot_digest, &mut b),
            Ok(())
        );
    }
    assert_eq!(
        table.begin(7, saved.provider.clone(), saved.snapshot_digest, &mut b),
        Err(LifetimeError::DuplicateRequest)
    );
    assert_eq!(table.suspend(7, saved.clone(), 0, &mut b), Ok(()));
    let mut resume = Resume {
        request_id: 7,
        continuation: saved.clone(),
        dependency_results: vec![],
    };
    resume.continuation.snapshot_digest.0[0] ^= 1;
    assert_eq!(
        table.resume(&resume, &mut b),
        Err(LifetimeError::Binding(ContinuationError::Snapshot))
    );
    assert_eq!(table.phase(7, &mut b), Ok(RequestPhase::Awaiting));
    resume.continuation = saved.clone();
    let mut exhausted = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert_eq!(
        table.resume(&resume, &mut exhausted),
        Err(LifetimeError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(table.phase(7, &mut b), Ok(RequestPhase::Awaiting));
    assert_eq!(table.resume(&resume, &mut b), Ok(()));
    assert_eq!(table.resume(&resume, &mut b), Err(LifetimeError::Phase));
    assert_eq!(table.finish(7, &mut b), Ok(()));
    assert_eq!(table.finish(7, &mut b), Err(LifetimeError::Phase));
    assert_eq!(
        table.begin(7, saved.provider.clone(), saved.snapshot_digest, &mut b),
        Err(LifetimeError::DuplicateRequest)
    );
    assert_eq!(table.cancel(8, &mut b), Ok(()));
    assert_eq!(table.finish(8, &mut b), Err(LifetimeError::Phase));
    assert_eq!(table.finish(99, &mut b), Err(LifetimeError::UnknownRequest));
    let mut pending = vec![];
    table.close(|id| pending.push(id));
    assert_eq!(pending, vec![9]);
    table.close(|id| pending.push(id));
    assert_eq!(pending, vec![9]);
    assert_eq!(table.phase(9, &mut b), Ok(RequestPhase::Cancelled));
    assert_eq!(
        table.begin(10, saved.provider.clone(), saved.snapshot_digest, &mut b),
        Err(LifetimeError::Closed)
    );
    assert_eq!(table.resume(&resume, &mut b), Err(LifetimeError::Closed));
}

#[test]
fn failed_registration_and_suspend_leave_the_connection_unchanged() {
    use nepl3_core::operation::lifetime::{LifetimeError, RequestLifetimes, RequestPhase};
    let saved = saved();
    let mut table = RequestLifetimes::default();
    let mut exhausted = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    assert_eq!(
        table.begin(
            7,
            saved.provider.clone(),
            saved.snapshot_digest,
            &mut exhausted
        ),
        Err(LifetimeError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(
        table.phase(7, &mut budget()),
        Err(LifetimeError::UnknownRequest)
    );
    assert_eq!(
        table.begin(
            7,
            saved.provider.clone(),
            saved.snapshot_digest,
            &mut budget()
        ),
        Ok(())
    );
    let mut wrong = saved.clone();
    wrong.parent_request = 8;
    assert_eq!(
        table.suspend(7, wrong, 0, &mut budget()),
        Err(LifetimeError::Binding(ContinuationError::ParentRequest))
    );
    assert_eq!(table.phase(7, &mut budget()), Ok(RequestPhase::Running));
    // Binding is valid; allocation of the saved continuation must still be
    // admitted before the Running state is replaced.
    let mut allocation_stop = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    assert_eq!(
        table.suspend(7, saved.clone(), 0, &mut allocation_stop),
        Err(LifetimeError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(table.phase(7, &mut budget()), Ok(RequestPhase::Running));
    assert_eq!(table.suspend(7, saved, 0, &mut budget()), Ok(()));
    let mut cancelled = vec![];
    table.close(|id| cancelled.push(id));
    assert_eq!(cancelled, vec![7]);
    assert_eq!(table.phase(7, &mut budget()), Ok(RequestPhase::Cancelled));
}

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10000,
        work: 100000,
        depth: 128,
        nodes: 10000,
        allocation_units: 100000,
        output_bytes: 10000,
        diagnostics: 100,
        events: 100,
    })
}
fn saved() -> Continuation {
    let schema = SchemaRef {
        package: "test.operation".into(),
        revision: 1,
        digest: Digest([1; 32]),
    };
    Continuation {
        provider: OperationRef {
            schema: schema.clone(),
            name: "run".into(),
        },
        parent_request: 7,
        snapshot_digest: Digest([2; 32]),
        state: TypedValue::Record(Record {
            schema,
            kind: "State".into(),
            fields: vec![NdfValue::List(vec![NdfValue::Text("保存状態".into())])],
        }),
    }
}

#[test]
fn saved_continuation_rejects_each_changed_identity_and_state() {
    let saved = saved();
    assert_eq!(saved.check_saved(&saved, &mut budget()), Ok(()));
    for (field, expected) in [
        (0, ContinuationError::Provider),
        (1, ContinuationError::ParentRequest),
        (2, ContinuationError::Snapshot),
        (3, ContinuationError::State),
    ] {
        let mut received = saved.clone();
        match field {
            0 => received.provider.schema.revision += 1,
            1 => received.parent_request += 1,
            2 => received.snapshot_digest.0[0] ^= 1,
            _ => {
                if let TypedValue::Record(v) = &mut received.state {
                    v.fields[0] = NdfValue::Text("変更".into());
                }
            }
        }
        assert_eq!(received.check_saved(&saved, &mut budget()), Err(expected));
        assert_eq!(saved.check_saved(&saved, &mut budget()), Ok(()));
    }
}

#[test]
fn resume_checks_outer_id_count_and_exact_saved_state() {
    let saved = saved();
    let mut resume = Resume {
        request_id: 7,
        continuation: saved.clone(),
        dependency_results: vec![],
    };
    assert_eq!(resume.check_binding(&saved, 0, &mut budget()), Ok(()));
    assert_eq!(
        resume.check_binding(&saved, 1, &mut budget()),
        Err(ContinuationError::DependencyCount)
    );
    resume.request_id = 8;
    assert_eq!(
        resume.check_binding(&saved, 0, &mut budget()),
        Err(ContinuationError::ParentRequest)
    );
    resume.request_id = 7;
    if let TypedValue::Record(v) = &mut resume.continuation.state {
        v.kind = "Other".into();
    }
    assert_eq!(
        resume.check_binding(&saved, 0, &mut budget()),
        Err(ContinuationError::State)
    );
}

#[test]
fn typed_state_comparison_distinguishes_record_variant_and_stops() -> Result<(), String> {
    let saved = saved();
    let TypedValue::Record(record) = &saved.state else {
        return Err("expected record fixture".into());
    };
    let variant = TypedValue::Variant(Variant {
        schema: record.schema.clone(),
        type_name: record.kind.clone(),
        variant: "".into(),
        fields: record.fields.clone(),
    });
    assert_eq!(
        saved.state.equal_with_budget(&variant, &mut budget()),
        Ok(false)
    );
    assert_eq!(variant.equal_with_budget(&variant, &mut budget()), Ok(true));
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::DepthLimit => limits.depth = 1,
            _ => (),
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        assert_eq!(
            saved.check_saved(&saved, &mut b),
            Err(ContinuationError::Stopped(reason))
        );
        assert_eq!(b.poll(), Err(reason));
    }
    Ok(())
}
