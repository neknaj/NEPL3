use nepl3_core::{
    budget::*, diagnostic::validation::ReportValidationError, diagnostic::*, schema::*, source::*,
    value::*, value_codec::FoundationValueCodec,
};
use nepl3_wire::{WireError, report::*};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 10000,
        allocation_units: 20_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn record_fields(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    match value {
        NdfValue::Record(v) => Ok(&mut v.fields),
        _ => Err("expected record fixture".into()),
    }
}
fn list_values(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    match value {
        NdfValue::List(v) => Ok(v),
        _ => Err("expected list fixture".into()),
    }
}

#[test]
fn one_fix_uses_one_snapshot_per_source_but_allows_multiple_sources_and_historical_related()
-> Result<(), String> {
    let (registry, mut sources, report) = fixture()?;
    let next = SourceSnapshot::new(
        SourceId("report-source".into()),
        1,
        "memory:report".into(),
        b"xyz".to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let next_span = next.span(0, 1).map_err(error)?;
    sources.insert(next).map_err(error)?;
    let mut mixed = report.clone();
    mixed.diagnostics[0].fixes[0].edits.push(TextEdit {
        span: next_span.clone(),
        expected_digest: Digest::of(b"x"),
        replacement: "X".into(),
    });
    assert_eq!(
        mixed.validate(&sources, &[], &registry, &mut budget()),
        Err(ReportValidationError::Source(SourceError::SnapshotMismatch))
    );
    assert!(matches!(
        encode_report(
            &mixed,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::Report(ReportValidationError::Source(
            SourceError::SnapshotMismatch
        )))
    ));
    // Each report below is valid independently. Combining their edit records tests
    // a well-typed wire value without relying on the native encoder accepting it.
    let mut only_next = report.clone();
    only_next.diagnostics[0].fixes[0].edits = vec![mixed.diagnostics[0].fixes[0].edits[1].clone()];
    let mut value = nepl3_wire::decode(
        &encode_report(
            &report,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?,
        &mut budget(),
    )
    .map_err(error)?;
    let mut other = nepl3_wire::decode(
        &encode_report(
            &only_next,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?,
        &mut budget(),
    )
    .map_err(error)?;
    fn edits(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
        let diagnostic = &mut list_values(&mut record_fields(value)?[0])?[0];
        let fix = &mut list_values(&mut record_fields(diagnostic)?[7])?[0];
        list_values(&mut record_fields(fix)?[1])
    }
    edits(&mut value)?.push(edits(&mut other)?[0].clone());
    let bytes = nepl3_wire::encode_checked(
        &value,
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.foundation".into(),
            revision: 1,
            name: "Report".into(),
        }),
        &registry,
        &mut budget(),
    )
    .map_err(error)?;
    assert!(matches!(
        decode_report(
            &bytes,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::Report(ReportValidationError::Source(
            SourceError::SnapshotMismatch
        )))
    ));

    let aux = SourceSnapshot::new(
        SourceId("aux".into()),
        0,
        "memory:aux".into(),
        b"a".to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let aux_span = aux.span(0, 1).map_err(error)?;
    sources.insert(aux).map_err(error)?;
    // Different SourceIds can participate in one atomic fix; related locations
    // can still point to a historical snapshot of the edited source.
    only_next.diagnostics[0].fixes[0].edits.push(TextEdit {
        span: aux_span,
        expected_digest: Digest::of(b"a"),
        replacement: "A".into(),
    });
    only_next
        .validate(&sources, &[], &registry, &mut budget())
        .map_err(error)?;
    let bytes = encode_report(
        &only_next,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    assert_eq!(
        decode_report(
            &bytes,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(error)?,
        only_next
    );
    Ok(())
}

#[test]
fn fix_preconditions_reject_digest_and_overlap_in_native_and_structural_wire_values()
-> Result<(), String> {
    let (registry, sources, report) = fixture()?;
    let valid = encode_report(
        &report,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    for expected in [SourceError::ExpectedDigest, SourceError::OverlappingEdits] {
        let mut candidate = report.clone();
        let edits = &mut candidate.diagnostics[0].fixes[0].edits;
        if expected == SourceError::ExpectedDigest {
            edits[0].expected_digest = Digest::of(b"wrong");
        } else {
            let mut second = edits[0].clone();
            second.replacement = "different".into();
            edits.push(second);
        }
        assert_eq!(
            candidate.validate(&sources, &[], &registry, &mut budget()),
            Err(ReportValidationError::Source(expected.clone()))
        );
        assert!(
            matches!(encode_report(&candidate, &registry, &sources, &mut SourceAdmission::default(), &mut budget()), Err(WireError::Report(ReportValidationError::Source(e))) if e == expected)
        );
        // Mutate a schema-valid NDF tree so decode is independent of encode rejection.
        let mut value = nepl3_wire::decode(&valid, &mut budget()).map_err(error)?;
        let diagnostics = list_values(&mut record_fields(&mut value)?[0])?;
        let fixes = list_values(&mut record_fields(&mut diagnostics[0])?[7])?;
        let edits = list_values(&mut record_fields(&mut fixes[0])?[1])?;
        if expected == SourceError::ExpectedDigest {
            record_fields(&mut edits[0])?[1] = NdfValue::Bytes(Digest::of(b"wrong").0.to_vec());
        } else {
            let mut second = edits[0].clone();
            record_fields(&mut second)?[2] = NdfValue::Text("different".into());
            edits.push(second);
        }
        let bytes = nepl3_wire::encode_checked(
            &value,
            &TypeDescriptor::Named(TypeRef {
                package: "nepl3.foundation".into(),
                revision: 1,
                name: "Report".into(),
            }),
            &registry,
            &mut budget(),
        )
        .map_err(error)?;
        assert!(
            matches!(decode_report(&bytes, &registry, &sources, &mut SourceAdmission::default(), &mut budget()), Err(WireError::Report(ReportValidationError::Source(e))) if e == expected)
        );
    }
    let source = &sources.snapshots()[0];
    let mut adjacent = report.clone();
    // Order is preserved, not required to be sorted; adjacent ranges are legal.
    adjacent.diagnostics[0].fixes[0].edits.push(TextEdit {
        span: source.span(1, 2).map_err(error)?,
        expected_digest: Digest::of(b"b"),
        replacement: "B".into(),
    });
    adjacent
        .validate(&sources, &[], &registry, &mut budget())
        .map_err(error)?;
    let bytes = encode_report(
        &adjacent,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    assert_eq!(
        decode_report(
            &bytes,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(error)?,
        adjacent
    );
    let insertion = TextEdit {
        span: source.span(3, 3).map_err(error)?,
        expected_digest: Digest::of(b""),
        replacement: "!".into(),
    };
    adjacent.diagnostics[0].fixes[0]
        .edits
        .push(insertion.clone());
    adjacent
        .validate(&sources, &[], &registry, &mut budget())
        .map_err(error)?;
    adjacent.diagnostics[0].fixes[0].edits.push(insertion);
    assert_eq!(
        adjacent.validate(&sources, &[], &registry, &mut budget()),
        Err(ReportValidationError::Source(SourceError::OverlappingEdits))
    );
    Ok(())
}
fn fixture() -> Result<(SchemaRegistry, SourceStore, Report), String> {
    let mut registry = SchemaRegistry::default();
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(error)?;
    let schema = descriptor.reference(&mut budget()).map_err(error)?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    let source = SourceSnapshot::new(
        SourceId("report-source".into()),
        0,
        "memory:report".into(),
        b"abc".to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let primary = source.span(0, 1).map_err(error)?;
    let related = source.span(1, 2).map_err(error)?;
    let fix = source.span(2, 3).map_err(error)?;
    let mut sources = SourceStore::default();
    sources.insert(source).map_err(error)?;
    // Registered real foundation data, independent of any diagnostic formatter.
    let arguments = TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "TraceOverflow".into(),
        fields: vec![NdfValue::U64(1)],
    });
    let report = Report {
        diagnostics: vec![Diagnostic {
            schema: schema.clone(),
            code: "example".into(),
            severity: Severity::Warning,
            stage: "test".into(),
            arguments: arguments.clone(),
            primary: Some(primary.clone()),
            related: vec![Related {
                span: Some(related),
                code: "related".into(),
                arguments: arguments.clone(),
            }],
            fixes: vec![Fix {
                id: "replace".into(),
                edits: vec![TextEdit {
                    span: fix,
                    expected_digest: Digest::of(b"c"),
                    replacement: "C".into(),
                }],
            }],
        }],
        events: vec![Event {
            schema,
            kind: "observed".into(),
            operation_path: vec![4, 7],
            span: Some(primary),
            payload: arguments,
        }],
        trace_overflow: Some(TraceOverflow { dropped: 2 }),
        usage: Usage {
            source_bytes: 3,
            diagnostics: 1,
            events: 1,
            work: 17,
            ..Usage::default()
        },
    };
    Ok((registry, sources, report))
}
#[test]
fn report_roundtrip_preserves_diagnostics_events_fixes_and_declared_sources() -> Result<(), String>
{
    let (registry, sources, report) = fixture()?;
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let bytes =
        encode_report(&report, &registry, &sources, &mut admission, &mut b).map_err(error)?;
    assert_eq!(
        decode_report(&bytes, &registry, &sources, &mut admission, &mut b).map_err(error)?,
        report
    );
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&registry, &sources, &mut admission)
            .map_err(error)?;
    let value = codec.encode_report(&report, &mut b).map_err(error)?;
    assert_eq!(codec.decode_report(&value, &mut b).map_err(error)?, report);
    assert_eq!(b.usage().source_bytes, 3);
    // Structural Report validation does not pretend to authenticate remote Usage.
    assert_eq!(b.usage().diagnostics, 0);
    assert_eq!(b.usage().events, 0);
    assert!(matches!(
        decode_report(
            &bytes,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::Source(SourceError::MissingSnapshot))
    ));
    Ok(())
}
#[test]
fn report_boundary_rejects_payload_count_and_source_forgery_and_preserves_stops()
-> Result<(), String> {
    let (registry, sources, report) = fixture()?;
    let mut malformed = report.clone();
    malformed.usage.events = 0;
    assert!(matches!(
        encode_report(
            &malformed,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::Report(ReportValidationError::Usage))
    ));
    malformed = report.clone();
    malformed.trace_overflow = Some(TraceOverflow { dropped: 0 });
    assert!(matches!(
        malformed.validate(&sources, &[], &registry, &mut budget()),
        Err(ReportValidationError::Usage)
    ));
    malformed = report.clone();
    if let TypedValue::Record(v) = &mut malformed.diagnostics[0].arguments {
        v.fields[0] = NdfValue::Text("wrong".into());
    }
    assert!(matches!(
        malformed.validate(&sources, &[], &registry, &mut budget()),
        Err(ReportValidationError::Schema(_))
    ));
    let absent = SourceSnapshot::new(
        SourceId("absent".into()),
        0,
        "memory:absent".into(),
        b"x".to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    for location in 0..4 {
        let mut candidate = report.clone();
        let span = absent.span(0, 1).map_err(error)?;
        match location {
            0 => candidate.diagnostics[0].primary = Some(span),
            1 => candidate.diagnostics[0].related[0].span = Some(span),
            2 => candidate.diagnostics[0].fixes[0].edits[0].span = span,
            _ => candidate.events[0].span = Some(span),
        }
        assert!(matches!(
            candidate.validate(&sources, &[], &registry, &mut budget()),
            Err(ReportValidationError::Source(SourceError::MissingSnapshot))
        ));
    }
    let mut stopped = budget();
    stopped.cancel();
    assert_eq!(
        report.validate(&sources, &[], &registry, &mut stopped),
        Err(ReportValidationError::Stopped(StopReason::Cancelled))
    );
    let mut limits = budget().limits();
    limits.work = 0;
    assert_eq!(
        report.validate(&sources, &[], &registry, &mut Budget::new(limits)),
        Err(ReportValidationError::Stopped(StopReason::WorkLimit))
    );
    limits = budget().limits();
    limits.source_bytes = 0;
    assert!(matches!(
        encode_report(
            &report,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut Budget::new(limits)
        ),
        Err(WireError::Stopped(StopReason::SourceLimit))
    ));
    limits = budget().limits();
    limits.allocation_units = 0;
    assert!(matches!(
        encode_report(
            &report,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut Budget::new(limits)
        ),
        Err(WireError::Stopped(StopReason::AllocationLimit))
    ));
    Ok(())
}
