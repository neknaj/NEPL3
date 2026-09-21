use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    operation::{Continuation, ContinuationError, Resume},
    source::Digest,
    value::{NdfValue, OperationRef, Record, SchemaRef, TypedValue, Variant},
};

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
