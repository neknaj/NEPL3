use super::*;
use nepl3_core::{
    diagnostic::{OperationResult, Report, TraceOverflow},
    operation::validation::ResultValidationError,
    schema::*,
    source::SourceStore,
};

fn fixture() -> Result<(SchemaRegistry, OperationRef, TypedValue), String> {
    let output = TypeDescriptor::Named(TypeRef {
        package: "test.result".into(),
        revision: 1,
        name: "Output".into(),
    });
    let descriptor = SchemaDescriptor {
        package: "test.result".into(),
        revision: 1,
        types: ["Output", "Other"]
            .into_iter()
            .map(|name| NamedType {
                name: name.into(),
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "number".into(),
                        ty: TypeDescriptor::U64,
                    }],
                },
                constraints: vec![],
            })
            .collect(),
        operations: vec![OperationDescriptor {
            name: "run".into(),
            input: TypeDescriptor::Unit,
            output,
            pure: true,
        }],
    };
    let schema = descriptor
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    Ok((
        registry,
        OperationRef {
            schema: schema.clone(),
            name: "run".into(),
        },
        TypedValue::Record(Record {
            schema,
            kind: "Output".into(),
            fields: vec![NdfValue::U64(42)],
        }),
    ))
}

#[test]
fn dependency_results_resume_in_call_order_after_out_of_order_completion() -> Result<(), String> {
    use nepl3_core::operation::{
        Invoke, OperationReply,
        dependencies::{DependencyError, PendingDependencies},
        lifetime::{RequestLifetimes, RequestPhase},
    };
    let (registry, operation, value) = fixture()?;
    let continuation = Continuation {
        provider: operation.clone(),
        parent_request: 7,
        snapshot_digest: Digest::of(b"generation one"),
        state: value.clone(),
    };
    let calls: Vec<_> = [30, 10, 20]
        .into_iter()
        .map(|id| Invoke {
            request_id: id,
            operation: operation.clone(),
            input: value.clone(),
            environment: value.clone(),
            sources: vec![],
            resources: vec![],
            limits: budget().limits(),
        })
        .collect();
    // Input/environment admission belongs to dispatch; this fixture exercises
    // terminal output correlation after calls have been admitted by the host.
    let mut pending = PendingDependencies::new(&continuation, &calls, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        pending.take_resume(&mut budget()),
        Err(DependencyError::Incomplete)
    );
    let sources = SourceStore::default();
    let complete = OperationResult::Complete {
        value,
        report: Report::default(),
    };
    let mut wrong = complete.clone();
    if let OperationResult::Complete {
        value: TypedValue::Record(record),
        ..
    } = &mut wrong
    {
        record.kind = "Other".into();
    }
    assert_eq!(
        pending.accept(30, wrong, &registry, &sources, &mut budget()),
        Err(DependencyError::Output(ResultValidationError::Schema(
            SchemaError::WrongType
        )))
    );
    assert_eq!(pending.remaining(), 3);
    assert_eq!(
        pending.accept(99, complete.clone(), &registry, &sources, &mut budget()),
        Err(DependencyError::UnknownId)
    );
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert_eq!(
        pending.accept(30, complete.clone(), &registry, &sources, &mut stopped),
        Err(DependencyError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(pending.remaining(), 3);
    pending
        .accept(
            20,
            OperationResult::Stopped {
                reason: StopReason::Cancelled,
                partial: None,
                report: Report::default(),
            },
            &registry,
            &sources,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    pending
        .accept(30, complete.clone(), &registry, &sources, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        pending.accept(30, complete, &registry, &sources, &mut budget()),
        Err(DependencyError::DuplicateReply)
    );
    pending
        .accept(
            10,
            OperationResult::Invalid {
                partial: None,
                report: Report::default(),
            },
            &registry,
            &sources,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    let mut allocation = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    assert_eq!(
        pending.take_resume(&mut allocation),
        Err(DependencyError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(pending.remaining(), 0);
    let resume = pending
        .take_resume(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        &resume.dependency_results[..],
        [
            OperationReply::Result(OperationResult::Complete { .. }),
            OperationReply::Result(OperationResult::Invalid { partial: None, .. }),
            OperationReply::Result(OperationResult::Stopped {
                reason: StopReason::Cancelled,
                partial: None,
                ..
            })
        ]
    ));
    assert_eq!(resume.continuation, continuation);
    assert_eq!(
        pending.take_resume(&mut budget()),
        Err(DependencyError::Consumed)
    );
    let mut lifetimes = RequestLifetimes::default();
    lifetimes
        .begin(7, operation, continuation.snapshot_digest, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    lifetimes
        .suspend(7, continuation.clone(), 3, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    lifetimes
        .resume(&resume, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(lifetimes.phase(7, &mut budget()), Ok(RequestPhase::Running));
    let mut duplicate = calls.clone();
    duplicate[1].request_id = duplicate[0].request_id;
    assert!(matches!(
        PendingDependencies::new(&continuation, &duplicate, &mut budget()),
        Err(DependencyError::DuplicateId)
    ));
    duplicate[1].request_id = 7;
    assert!(matches!(
        PendingDependencies::new(&continuation, &duplicate, &mut budget()),
        Err(DependencyError::ParentId)
    ));
    Ok(())
}

#[test]
fn terminal_results_validate_selected_output_and_preserve_outcomes() -> Result<(), String> {
    let (registry, operation, value) = fixture()?;
    let sources = SourceStore::default();
    let cases = [
        OperationResult::Complete {
            value: value.clone(),
            report: Report::default(),
        },
        OperationResult::Invalid {
            partial: Some(value.clone()),
            report: Report::default(),
        },
        OperationResult::Stopped {
            reason: StopReason::Cancelled,
            partial: Some(value.clone()),
            report: Report::default(),
        },
        OperationResult::Invalid {
            partial: None,
            report: Report::default(),
        },
    ];
    for result in cases {
        let before = result.clone();
        result
            .validate_for(&operation, &registry, &sources, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(result, before);
        let mut unknown = operation.clone();
        unknown.name = "missing".into();
        assert_eq!(
            result.validate_for(&unknown, &registry, &sources, &mut budget()),
            Err(ResultValidationError::UnknownOperation)
        );
    }
    let mut wrong = value;
    if let TypedValue::Record(v) = &mut wrong {
        v.kind = "Other".into();
    }
    for result in [
        OperationResult::Complete {
            value: wrong.clone(),
            report: Report::default(),
        },
        OperationResult::Invalid {
            partial: Some(wrong.clone()),
            report: Report::default(),
        },
        OperationResult::Stopped {
            reason: StopReason::WorkLimit,
            partial: Some(wrong),
            report: Report::default(),
        },
    ] {
        assert_eq!(
            result.validate_for(&operation, &registry, &sources, &mut budget()),
            Err(ResultValidationError::Schema(SchemaError::WrongType))
        );
    }
    Ok(())
}

#[test]
fn output_validation_preserves_stops_and_checks_report_status() -> Result<(), String> {
    let (registry, operation, value) = fixture()?;
    let sources = SourceStore::default();
    let result = OperationResult::Complete {
        value,
        report: Report::default(),
    };
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert_eq!(
        result.validate_for(&operation, &registry, &sources, &mut stopped),
        Err(ResultValidationError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    let overflow = OperationResult::Invalid {
        partial: None,
        report: Report {
            trace_overflow: Some(TraceOverflow { dropped: 1 }),
            ..Report::default()
        },
    };
    assert_eq!(
        overflow.validate_for(&operation, &registry, &sources, &mut budget()),
        Err(ResultValidationError::TraceOverflow)
    );
    Ok(())
}

#[test]
fn result_reports_use_only_host_authorized_sources_without_absorbing_usage() -> Result<(), String> {
    use nepl3_core::{
        diagnostic::{Event, validation::ReportValidationError},
        source::{SourceError, SourceId, SourceSnapshot},
    };
    let (registry, operation, value) = fixture()?;
    let source = SourceSnapshot::new(
        SourceId("input".into()),
        1,
        "memory:input".into(),
        "世界".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let span = source.span(0, 6).map_err(|e| format!("{e:?}"))?;
    let mut report = Report::default();
    report.usage.events = 1;
    report.usage.work = u64::MAX;
    report.events.push(Event {
        schema: operation.schema.clone(),
        kind: "result".into(),
        operation_path: vec![7],
        span: Some(span),
        payload: value.clone(),
    });
    let result = OperationResult::Complete { value, report };
    assert_eq!(
        result.validate_for(
            &operation,
            &registry,
            &SourceStore::default(),
            &mut budget()
        ),
        Err(ResultValidationError::Report(
            ReportValidationError::Source(SourceError::MissingSnapshot)
        ))
    );
    let mut sources = SourceStore::default();
    sources
        .insert_with_budget(source, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut local = budget();
    result
        .validate_for(&operation, &registry, &sources, &mut local)
        .map_err(|e| format!("{e:?}"))?;
    assert!(local.usage().work < local.limits().work);
    assert_eq!(local.poll(), Ok(()));
    Ok(())
}
