use nepl3_core::{
    budget::*,
    diagnostic::{validation::*, *},
    schema::*,
    source::*,
    value::*,
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        nodes: 1000,
        depth: 100,
        source_bytes: 1000,
        output_bytes: 1000,
        diagnostics: 10,
        events: 10,
    })
}
fn fixture(padding: usize) -> Result<(SchemaRegistry, SchemaRef, TypedValue), SchemaError> {
    let mut registry = SchemaRegistry::default();
    let args = SchemaDescriptor {
        package: "args".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Args".into(),
            shape: TypeShape::Record { fields: vec![] },
            constraints: vec![],
        }],
        operations: vec![],
    };
    let args_schema = args.reference(&mut budget())?;
    registry.register(args_schema.clone(), args, &mut budget())?;
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: format!("padding.{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        registry.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    let owner = SchemaDescriptor {
        package: "owner".into(),
        revision: 1,
        types: vec![],
        operations: vec![],
    };
    let schema = owner.reference(&mut budget())?;
    registry.register(schema.clone(), owner, &mut budget())?;
    registry.finalize(&mut budget())?;
    Ok((
        registry,
        schema,
        TypedValue::Record(Record {
            schema: args_schema,
            kind: "Args".into(),
            fields: vec![],
        }),
    ))
}
fn validate(
    event: bool,
    schema: &SchemaRef,
    args: &TypedValue,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), ReportValidationError> {
    if event {
        validate_event_metadata(schema, "Visit", args, registry, b)
    } else {
        validate_diagnostic_metadata(schema, "Note", "test", args, registry, b)
    }
}
#[test]
fn metadata_lookup_charges_padding_without_charging_unrelated_argument_search()
-> Result<(), ReportValidationError> {
    for event in [false, true] {
        let mut measured = vec![];
        for padding in [0, 32] {
            let (registry, schema, args) = fixture(padding)?;
            let mut b = budget();
            validate(event, &schema, &args, &registry, &mut b)?;
            measured.push(b.usage());
        }
        let mut expected = measured[0];
        // Argument schema is first in both registries. Only metadata owner
        // resolution crosses these 32 additional 10-byte package names.
        expected.work += 32 * (10 + 5 + 9);
        assert_eq!(measured[1], expected);
    }
    Ok(())
}
#[test]
fn missing_metadata_schema_is_metered_before_returning_metadata_error()
-> Result<(), ReportValidationError> {
    let (registry, mut schema, args) = fixture(32)?;
    schema.package = "missing".into();
    for event in [false, true] {
        let mut limits = budget().limits();
        limits.work = 41;
        let mut stopped = Budget::new(limits);
        assert_eq!(
            validate(event, &schema, &args, &registry, &mut stopped),
            Err(ReportValidationError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        let mut b = budget();
        assert_eq!(
            validate(event, &schema, &args, &registry, &mut b),
            Err(ReportValidationError::Metadata)
        );
        let work = 41 + 1 + (4 + 7 + 9) + 32 * (10 + 7 + 9) + (5 + 7 + 9);
        let mut expected = budget();
        expected.charge(Resource::Work, work)?;
        assert_eq!(b.usage(), expected.usage());
    }
    Ok(())
}

#[test]
fn metadata_lookup_keeps_identity_errors_and_sticky_stops() -> Result<(), ReportValidationError> {
    let (registry, schema, args) = fixture(0)?;
    for event in [false, true] {
        for mutation in 0..3 {
            let mut bad = schema.clone();
            match mutation {
                0 => bad.digest.0[0] ^= 1,
                1 => bad.revision += 1,
                _ => bad.package = "missing".into(),
            }
            assert_eq!(
                validate(event, &bad, &args, &registry, &mut budget()),
                Err(ReportValidationError::Metadata)
            );
        }
        // Metadata header39, schema-entry1, args candidate18, owner candidate19,
        // then exact identity46. No typed argument work before that boundary.
        for work in [0, 39, 40, 58, 77, 122] {
            let mut limits = budget().limits();
            limits.work = work;
            let mut b = Budget::new(limits);
            assert_eq!(
                validate(event, &schema, &args, &registry, &mut b),
                Err(ReportValidationError::Stopped(StopReason::WorkLimit))
            );
            assert_eq!(b.usage().nodes, 0);
            assert_eq!(b.usage().allocation_units, 0);
            let before = b.usage();
            assert_eq!(
                validate(event, &schema, &args, &registry, &mut b),
                Err(ReportValidationError::Stopped(StopReason::WorkLimit))
            );
            assert_eq!(b.usage(), before);
        }
        let mut b = budget();
        b.charge(Resource::Work, 7)?;
        b.cancel();
        let before = b.usage();
        assert_eq!(
            validate(event, &schema, &args, &registry, &mut b),
            Err(ReportValidationError::Stopped(StopReason::Cancelled))
        );
        assert_eq!(b.usage(), before);
    }
    let mut limits = budget().limits();
    limits.work = 39;
    let mut b = Budget::new(limits);
    assert_eq!(
        validate_diagnostic_metadata(&schema, "", "test", &args, &registry, &mut b),
        Err(ReportValidationError::Metadata)
    );
    assert_eq!(b.usage().work, 39);
    assert_eq!(b.poll(), Ok(()));
    let mut b = Budget::new(limits);
    assert_eq!(
        validate_event_metadata(&schema, "", &args, &registry, &mut b),
        Err(ReportValidationError::Metadata)
    );
    assert_eq!(b.usage().work, 39);
    Ok(())
}

#[test]
fn report_admission_meters_metadata_and_does_not_absorb_remote_usage()
-> Result<(), ReportValidationError> {
    let (registry, mut schema, args) = fixture(32)?;
    schema.package = "missing".into();
    for event in [false, true] {
        let mut report = Report::default();
        if event {
            report.events.push(Event {
                schema: schema.clone(),
                kind: "Visit".into(),
                operation_path: vec![],
                span: None,
                payload: args.clone(),
            });
            report.usage.events = 1;
        } else {
            report.diagnostics.push(Diagnostic {
                schema: schema.clone(),
                code: "Note".into(),
                severity: Severity::Information,
                stage: "test".into(),
                arguments: args.clone(),
                primary: None,
                related: vec![],
                fixes: vec![],
            });
            report.usage.diagnostics = 1;
        }
        report.usage.work = 999_999;
        let mut limits = budget().limits();
        limits.work = 43;
        let mut b = Budget::new(limits);
        assert_eq!(
            report.validate(&SourceStore::default(), &[], &registry, &mut b),
            Err(ReportValidationError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(b.usage().work, 43);
        assert_eq!(b.usage().diagnostics, 0);
        assert_eq!(b.usage().events, 0);
        let mut b = budget();
        assert_eq!(
            report.validate(&SourceStore::default(), &[], &registry, &mut b),
            Err(ReportValidationError::Metadata)
        );
        assert_eq!(b.usage().work, 43 + 1 + 20 + 32 * 26 + 21);
    }
    Ok(())
}
