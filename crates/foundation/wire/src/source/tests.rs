use super::*;
use nepl3_core::budget::{Limits, StopReason};
use nepl3_core::schema::foundation;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        source_bytes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        nodes: 1_000_000,
        depth: 1024,
        ..Limits::default()
    })
}

// Previous conversion, retained only as an independent accounting/byte oracle.
fn owned_span_value(
    span: &Span,
    schema: &SchemaRef,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    let id = span.snapshot_ref();
    b.charge(Resource::AllocationUnits, id.source.0.len() as u64)?;
    let reference = SourceRef {
        source_id: id.source.clone(),
        revision: id.revision,
        digest: id.digest,
    };
    record(
        schema,
        "Span",
        [
            source_ref(&reference, schema, b)?,
            NdfValue::U64(span.start()),
            NdfValue::U64(span.end()),
        ],
        b,
    )
}

#[test]
fn borrowed_span_identity_preserves_bytes_and_removes_one_id_copy() -> Result<(), WireError> {
    let schema = foundation::descriptor(&mut budget())?.reference(&mut budget())?;
    for id in [
        String::from("x"),
        String::from("原文😀"),
        "x".repeat(65_536),
    ] {
        let source = SourceSnapshot::new(
            SourceId(id.clone()),
            7,
            "memory:borrowed".into(),
            "a界z".as_bytes().to_vec(),
            &mut budget(),
        )?;
        for (start, end) in [(0, 0), (1, 4), (0, 5), (5, 5)] {
            let span = source.span(start, end)?;
            let mut old = budget();
            let previous = owned_span_value(&span, &schema, &mut old)?;
            let mut current = budget();
            let value = span_value(&span, &schema, &mut current)?;
            assert_eq!(value, previous);
            assert_eq!(
                crate::encode(&value, &mut budget())?,
                crate::encode(&previous, &mut budget())?
            );
            assert_eq!(
                old.usage().allocation_units - current.usage().allocation_units,
                id.len() as u64
            );
            assert_eq!(old.usage().work, current.usage().work);
            assert_eq!(old.usage().source_bytes, current.usage().source_bytes);
        }
    }
    Ok(())
}

#[test]
fn borrowed_span_identity_has_exact_allocation_boundary_and_sticky_stop() -> Result<(), WireError> {
    let schema = foundation::descriptor(&mut budget())?.reference(&mut budget())?;
    let source = SourceSnapshot::new(
        SourceId("source".repeat(100)),
        2,
        "memory:bounded".into(),
        Vec::new(),
        &mut budget(),
    )?;
    let span = source.span(0, 0)?;
    let mut measured = budget();
    let expected = span_value(&span, &schema, &mut measured)?;
    let used = measured.usage().allocation_units;
    let mut exact = Budget::new(Limits {
        allocation_units: used,
        ..budget().limits()
    });
    assert_eq!(span_value(&span, &schema, &mut exact)?, expected);
    let mut short = Budget::new(Limits {
        allocation_units: used - 1,
        ..budget().limits()
    });
    assert!(matches!(
        span_value(&span, &schema, &mut short),
        Err(WireError::Stopped(StopReason::AllocationLimit))
    ));
    assert_eq!(short.poll(), Err(StopReason::AllocationLimit));
    assert!(span_value(&span, &schema, &mut short).is_err());
    let mut cancelled = budget();
    cancelled.stop(StopReason::Cancelled);
    assert!(matches!(
        span_value(&span, &schema, &mut cancelled),
        Err(WireError::Stopped(StopReason::Cancelled))
    ));
    assert_eq!(cancelled.usage().allocation_units, 0);
    Ok(())
}

#[test]
fn borrowed_source_table_preserves_owned_reference_encoding() -> Result<(), WireError> {
    let schema = foundation::descriptor(&mut budget())?.reference(&mut budget())?;
    let mut sources = Vec::new();
    for (id, revision, text_value) in [
        ("長い識別子".repeat(256), 3, "界"),
        ("a".into(), 2, "second"),
        ("a".into(), 1, "first"),
    ] {
        sources.push(SourceSnapshot::new(
            SourceId(id),
            revision,
            "memory:source".into(),
            text_value.as_bytes().to_vec(),
            &mut budget(),
        )?);
    }
    let mut admission = SourceAdmission::default();
    let mut current = budget();
    let actual = sources_value(&sources, &schema, &mut admission, &mut current)?;
    let mut old = budget();
    let mut old_admission = SourceAdmission::default();
    let mut owned = Vec::new();
    for source in &sources {
        old_admission.admit_existing(source, &mut old)?;
        old.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(SourceRef, &SourceSnapshot)>() as u64
                + source.identity().source.0.len() as u64,
        )?;
        owned.push((source.reference(), source));
    }
    // Oracle ordering uses the documented canonical tuple, independently of heapsort.
    owned.sort_by(|a, z| a.0.cmp(&z.0));
    let mut values = Vec::new();
    for (reference, source) in &owned {
        old.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<NdfValue>() as u64,
        )?;
        values.push(record(
            &schema,
            "SourceContent",
            [
                source_ref(reference, &schema, &mut old)?,
                text(source.uri(), &mut old)?,
                text(source.text(), &mut old)?,
            ],
            &mut old,
        )?);
    }
    let expected = NdfValue::List(values);
    assert_eq!(actual, expected);
    assert_eq!(
        crate::encode(&actual, &mut budget())?,
        crate::encode(&expected, &mut budget())?
    );
    let savings: u64 = sources
        .iter()
        .map(|s| {
            s.identity().source.0.len() as u64
                + (core::mem::size_of::<(SourceRef, &SourceSnapshot)>()
                    - core::mem::size_of::<(&SnapshotId, &SourceSnapshot)>())
                    as u64
        })
        .sum();
    assert_eq!(
        old.usage().allocation_units - current.usage().allocation_units,
        savings
    );
    for allocation in [
        current.usage().allocation_units,
        current.usage().allocation_units - 1,
    ] {
        let mut limited = Budget::new(Limits {
            allocation_units: allocation,
            ..budget().limits()
        });
        let result = sources_value(
            &sources,
            &schema,
            &mut SourceAdmission::default(),
            &mut limited,
        );
        if allocation == current.usage().allocation_units {
            assert_eq!(result?, actual);
        } else {
            assert!(matches!(
                result,
                Err(WireError::Stopped(StopReason::AllocationLimit))
            ));
            assert_eq!(limited.poll(), Err(StopReason::AllocationLimit));
        }
    }
    Ok(())
}

// Original serializer, independent of the borrowed production helper.
fn source_ref(
    reference: &SourceRef,
    schema: &SchemaRef,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    b.charge(Resource::AllocationUnits, 32)?;
    record(
        schema,
        "SourceRef",
        [
            text(&reference.source_id.0, b)?,
            NdfValue::U64(reference.revision),
            NdfValue::Bytes(reference.digest.0.to_vec()),
        ],
        b,
    )
}
