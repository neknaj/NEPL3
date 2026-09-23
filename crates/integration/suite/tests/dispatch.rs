use nepl3_core::{
    budget::*,
    diagnostic::{OperationResult, Report},
    operation::Invoke,
    schema::*,
    source::{Digest, SourceStore},
    value::*,
};
use nepl3_suite::dispatch::*;
#[path = "dispatch/activation.rs"]
mod activation;
#[path = "dispatch/environment.rs"]
mod environment;
#[path = "dispatch/execution.rs"]
mod execution;
#[path = "dispatch/grants.rs"]
mod grants;
#[path = "dispatch/scheduler.rs"]
mod scheduler;
#[path = "dispatch/suspension.rs"]
mod suspension;

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10000,
        work: 1000000,
        depth: 100,
        nodes: 10000,
        allocation_units: 1000000,
        output_bytes: 10000,
        diagnostics: 100,
        events: 100,
    })
}
fn fixture() -> Result<(SchemaRegistry, Invoke), String> {
    let ty = TypeDescriptor::Named(TypeRef {
        package: "test.dispatch".into(),
        revision: 1,
        name: "Number".into(),
    });
    let descriptor = SchemaDescriptor {
        package: "test.dispatch".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Number".into(),
            shape: TypeShape::Record {
                fields: vec![FieldDescriptor {
                    name: "value".into(),
                    ty: TypeDescriptor::U64,
                }],
            },
            constraints: vec![],
        }],
        operations: vec![OperationDescriptor {
            name: "increment".into(),
            input: ty.clone(),
            output: ty,
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
    let value = TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "Number".into(),
        fields: vec![NdfValue::U64(41)],
    });
    Ok((
        registry,
        Invoke {
            request_id: 1,
            operation: OperationRef {
                schema,
                name: "increment".into(),
            },
            input: value.clone(),
            environment: value,
            sources: vec![],
            resources: vec![],
            limits: budget().limits(),
        },
    ))
}
fn increment(
    call: &Invoke,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason> {
    b.charge(Resource::Work, 1)?;
    let mut value = call.input.clone_with_budget(b)?;
    if let TypedValue::Record(record) = &mut value
        && let Some(NdfValue::U64(n)) = record.fields.first_mut()
    {
        *n += 1;
    }
    Ok(OperationResult::Complete {
        value,
        report: Report::default(),
    })
}
fn stop(
    call: &Invoke,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason> {
    let partial = call.input.clone_with_budget(b)?;
    let reason = b.stop(StopReason::WorkLimit);
    Ok(OperationResult::Stopped {
        reason,
        partial: Some(partial),
        report: Report::default(),
    })
}
fn hide_stop(
    call: &Invoke,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason> {
    let result = increment(call, r, b)?;
    b.stop(StopReason::WorkLimit);
    Ok(result)
}

fn malformed_output(
    call: &Invoke,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason> {
    let mut value = call.input.clone_with_budget(b)?;
    if let TypedValue::Record(record) = &mut value {
        record.fields.clear();
    }
    Ok(OperationResult::Complete {
        value,
        report: Report::default(),
    })
}

fn return_stop_error(
    _: &Invoke,
    _: &SchemaRegistry,
    _: &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason> {
    Err(StopReason::WorkLimit)
}

fn return_stopped(
    call: &Invoke,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason> {
    Ok(OperationResult::Stopped {
        reason: StopReason::WorkLimit,
        partial: Some(call.input.clone_with_budget(b)?),
        report: Report::default(),
    })
}

#[test]
fn admission_and_output_validation_surround_execution_and_limits_are_monotonic()
-> Result<(), String> {
    let (registry, mut call) = fixture()?;
    let selected = call.operation.clone();
    let identity = Digest::of(b"provider");
    let registrations = [Registration {
        operation: &selected,
        implementation: identity,
        invoke: malformed_output,
    }];
    let mut execution = budget();
    let result = invoke_terminal(
        &registrations,
        &selected,
        identity,
        &call,
        &registry,
        &SourceStore::default(),
        &mut execution,
        &mut budget(),
    );
    assert!(matches!(result, Err(DispatchError::Output(_))));
    let original = call.input.clone();
    if let TypedValue::Record(record) = &mut call.input {
        record.fields.clear();
    }
    let mut execution = budget();
    let result = invoke_terminal(
        &registrations,
        &selected,
        identity,
        &call,
        &registry,
        &SourceStore::default(),
        &mut execution,
        &mut budget(),
    );
    assert!(matches!(result, Err(DispatchError::Input(_))));
    assert_eq!(execution.usage(), Usage::default());
    call.input = original;
    call.limits.work = 0;
    let registrations = [Registration {
        operation: &selected,
        implementation: identity,
        invoke: increment,
    }];
    let mut execution = budget();
    let outer = execution.limits();
    let result = invoke_terminal(
        &registrations,
        &selected,
        identity,
        &call,
        &registry,
        &SourceStore::default(),
        &mut execution,
        &mut budget(),
    );
    assert!(matches!(
        result,
        Err(DispatchError::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(execution.limits(), outer);
    assert_eq!(execution.poll(), Err(StopReason::WorkLimit));
    Ok(())
}

#[test]
fn dispatch_executes_selected_callback_and_rejects_missing_or_duplicate_registration()
-> Result<(), String> {
    let (registry, call) = fixture()?;
    let identity = Digest::of(b"test executable");
    let registrations = [Registration {
        operation: &call.operation,
        implementation: identity,
        invoke: increment,
    }];
    let run = |entries: &[Registration<'_>], id| {
        invoke_terminal(
            entries,
            &call.operation,
            id,
            &call,
            &registry,
            &SourceStore::default(),
            &mut budget(),
            &mut budget(),
        )
    };
    let result = run(&registrations, identity).map_err(|e| format!("{e:?}"))?;
    let OperationResult::Complete {
        value: TypedValue::Record(record),
        ..
    } = result
    else {
        return Err("expected complete record".into());
    };
    assert_eq!(record.fields, vec![NdfValue::U64(42)]);
    assert!(matches!(
        run(&registrations, Digest::of(b"other")),
        Err(DispatchError::MissingImplementation)
    ));
    let duplicates = [
        Registration {
            operation: &call.operation,
            implementation: identity,
            invoke: increment,
        },
        Registration {
            operation: &call.operation,
            implementation: identity,
            invoke: stop,
        },
    ];
    assert!(matches!(
        run(&duplicates, identity),
        Err(DispatchError::AmbiguousImplementation)
    ));
    Ok(())
}

#[test]
fn execution_stop_preserves_checked_partial_and_cannot_be_reported_as_complete()
-> Result<(), String> {
    let (registry, call) = fixture()?;
    for (callback, has_partial) in [
        (return_stop_error as TerminalOperation, false),
        (return_stopped as TerminalOperation, true),
        (stop as TerminalOperation, true),
        (hide_stop as TerminalOperation, false),
    ] {
        let identity = Digest::of(b"provider");
        let registrations = [Registration {
            operation: &call.operation,
            implementation: identity,
            invoke: callback,
        }];
        let mut execution = budget();
        let result = invoke_terminal(
            &registrations,
            &call.operation,
            identity,
            &call,
            &registry,
            &SourceStore::default(),
            &mut execution,
            &mut budget(),
        );
        assert_eq!(execution.poll(), Err(StopReason::WorkLimit));
        if has_partial {
            let result = result.map_err(|e| format!("{e:?}"))?;
            assert!(
                matches!(result, OperationResult::Stopped { reason: StopReason::WorkLimit, partial: Some(ref p), .. } if p == &call.input)
            );
        } else {
            assert!(matches!(
                result,
                Err(DispatchError::Stopped(StopReason::WorkLimit))
            ));
        }
    }
    Ok(())
}
