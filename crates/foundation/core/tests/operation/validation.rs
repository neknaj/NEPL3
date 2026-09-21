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
