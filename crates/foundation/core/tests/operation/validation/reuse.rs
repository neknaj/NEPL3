use super::*;
use nepl3_core::{
    diagnostic::{Event, validation::ReportValidationError},
    operation::{
        Invoke,
        dependencies::PendingDependencies,
        lifetime::{RequestLifetimes, RequestPhase},
        validation::ValidatedResult,
    },
    source::{SourceError, SourceId, SourceSnapshot},
};

fn event_report(operation: &OperationRef, value: &TypedValue, count: usize) -> Report {
    let mut report = Report::default();
    report.usage.events = count as u64;
    for _ in 0..count {
        report.events.push(Event {
            schema: operation.schema.clone(),
            kind: "result".into(),
            operation_path: vec![30],
            span: None,
            payload: value.clone(),
        });
    }
    report
}

#[test]
fn result_scope_preserves_outcomes_and_does_not_rescan_larger_reports() -> Result<(), String> {
    let (registry, operation, value) = fixture()?;
    let sources = SourceStore::default();
    let mut prior_raw_work = 0;
    let mut reuse_work = None;
    for count in [8, 16, 32] {
        let report = event_report(&operation, &value, count);
        for result in [
            OperationResult::Complete {
                value: value.clone(),
                report: report.clone(),
            },
            OperationResult::Invalid {
                partial: Some(value.clone()),
                report: report.clone(),
            },
            OperationResult::Stopped {
                reason: StopReason::Cancelled,
                partial: Some(value.clone()),
                report: report.clone(),
            },
        ] {
            let mut construction = budget();
            let checked = ValidatedResult::new(
                result.clone(),
                &operation,
                &registry,
                &sources,
                &mut construction,
            )
            .map_err(|e| format!("{e:?}"))?;
            assert_eq!(checked.result(), &result);
            let mut reuse = Budget::new(Limits {
                allocation_units: 0,
                ..budget().limits()
            });
            checked
                .validate_for(&operation, &registry, &sources, &mut reuse)
                .map_err(|e| format!("{e:?}"))?;
            if let Some(expected) = reuse_work {
                assert_eq!(reuse.usage().work, expected);
            }
            reuse_work = Some(reuse.usage().work);
            assert_eq!(reuse.usage().allocation_units, 0);
            assert!(reuse.usage().work < construction.usage().work);
            assert_eq!(checked.into_inner(), result);
        }
        let result = OperationResult::Complete {
            value: value.clone(),
            report,
        };
        let mut raw = budget();
        result
            .validate_for(&operation, &registry, &sources, &mut raw)
            .map_err(|e| format!("{e:?}"))?;
        assert!(raw.usage().work > prior_raw_work);
        prior_raw_work = raw.usage().work;
    }
    Ok(())
}

#[test]
fn changed_registry_operation_and_source_permissions_require_validation() -> Result<(), String> {
    let (registry, operation, value) = fixture()?;
    let source = SourceSnapshot::new(
        SourceId("input".into()),
        1,
        "memory:input".into(),
        "世界".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut report = event_report(&operation, &value, 1);
    report.events[0].span = Some(source.span(0, 6).map_err(|e| format!("{e:?}"))?);
    let mut sources = SourceStore::default();
    sources
        .insert_with_budget(source, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let result = OperationResult::Complete { value, report };
    let checked = ValidatedResult::new(
        result.clone(),
        &operation,
        &registry,
        &sources,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let empty = SourceStore::default();
    assert_eq!(
        checked.validate_for(&operation, &registry, &empty, &mut budget()),
        Err(ResultValidationError::Report(
            ReportValidationError::Source(SourceError::MissingSnapshot)
        ))
    );
    assert_eq!(
        checked.validate_for(
            &operation,
            &SchemaRegistry::default(),
            &sources,
            &mut budget()
        ),
        Err(ResultValidationError::Schema(SchemaError::Unfinalized))
    );
    for field in 0..4 {
        let mut changed = operation.clone();
        match field {
            0 => changed.name = "missing".into(),
            1 => changed.schema.package.push('x'),
            2 => changed.schema.revision += 1,
            _ => changed.schema.digest.0[0] ^= 1,
        }
        assert!(
            checked
                .validate_for(&changed, &registry, &sources, &mut budget())
                .is_err()
        );
    }
    let (other_registry, _, _) = fixture()?;
    let mut raw = budget();
    result
        .validate_for(&operation, &other_registry, &sources, &mut raw)
        .map_err(|e| format!("{e:?}"))?;
    let mut rechecked = budget();
    checked
        .validate_for(&operation, &other_registry, &sources, &mut rechecked)
        .map_err(|e| format!("{e:?}"))?;
    assert!(rechecked.usage().work >= raw.usage().work);
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert_eq!(
        checked.validate_for(&operation, &registry, &sources, &mut stopped),
        Err(ResultValidationError::Stopped(StopReason::WorkLimit))
    );
    // Failed alternate validation leaves the original immutable proof intact.
    checked
        .validate_for(&operation, &registry, &sources, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(checked.into_inner(), result);
    Ok(())
}

#[test]
fn checked_acceptance_preserves_value_and_state_at_every_work_boundary() -> Result<(), String> {
    let (registry, operation, value) = fixture()?;
    let sources = SourceStore::default();
    let context = Digest::of(b"child context");
    let continuation = Continuation {
        provider: operation.clone(),
        parent_request: 7,
        snapshot_digest: context,
        state: value.clone(),
    };
    let calls = [Invoke {
        request_id: 30,
        operation: operation.clone(),
        input: value.clone(),
        environment: value.clone(),
        sources: vec![],
        resources: vec![],
        limits: budget().limits(),
    }];
    let setup = || {
        let pending = PendingDependencies::new(&continuation, &calls, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let mut lifetimes = RequestLifetimes::default();
        lifetimes
            .begin(30, operation.clone(), context, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        Ok::<_, String>((pending, lifetimes))
    };
    let report = event_report(&operation, &value, 8);
    for result in [
        OperationResult::Complete {
            value: value.clone(),
            report: report.clone(),
        },
        OperationResult::Invalid {
            partial: Some(value.clone()),
            report: report.clone(),
        },
        OperationResult::Stopped {
            reason: StopReason::Cancelled,
            partial: Some(value.clone()),
            report,
        },
    ] {
        let checked = || {
            ValidatedResult::new(
                result.clone(),
                &operation,
                &registry,
                &sources,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))
        };
        let (mut pending, mut lifetimes) = setup()?;
        let mut measured = budget();
        pending
            .try_accept_validated(
                30,
                context,
                checked()?,
                &registry,
                &sources,
                &mut lifetimes,
                &mut measured,
            )
            .map_err(|e| format!("{:?}", e.cause))?;
        for work in 0..=measured.usage().work {
            let (mut pending, mut lifetimes) = setup()?;
            let mut limited = Budget::new(Limits {
                work,
                allocation_units: 0,
                ..budget().limits()
            });
            let accepted = pending.try_accept_validated(
                30,
                context,
                checked()?,
                &registry,
                &sources,
                &mut lifetimes,
                &mut limited,
            );
            if work == measured.usage().work {
                assert!(accepted.is_ok());
                assert_eq!(
                    pending.accepted_results().next(),
                    Some((&calls[0], &result))
                );
                assert_eq!(
                    lifetimes.phase(30, &mut budget()),
                    Ok(RequestPhase::Finished)
                );
            } else {
                let Err(rejected) = accepted else {
                    return Err("expected Work stop".into());
                };
                assert_eq!(rejected.result, result);
                assert_eq!(pending.remaining(), 1);
                assert_eq!(
                    lifetimes.phase(30, &mut budget()),
                    Ok(RequestPhase::Running)
                );
            }
        }
        let (mut pending, mut lifetimes) = setup()?;
        let Err(rejected) = pending.try_accept_validated(
            30,
            Digest::of(b"wrong"),
            checked()?,
            &registry,
            &sources,
            &mut lifetimes,
            &mut budget(),
        ) else {
            return Err("wrong context accepted".into());
        };
        assert_eq!(rejected.result, result);
        assert_eq!(pending.remaining(), 1);
        assert_eq!(
            lifetimes.phase(30, &mut budget()),
            Ok(RequestPhase::Running)
        );
    }
    Ok(())
}
