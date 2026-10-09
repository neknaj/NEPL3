//! Checks that the cached expected type never caches a validation result.
use super::*;
use alloc::{format, string::String};
use nepl3_core::{
    budget::{Limits, StopReason},
    schema::foundation,
    source::SourceId,
};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 1024,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn setup() -> Result<(SchemaRegistry, SourceStore, Span), String> {
    let d = foundation::descriptor(&mut budget()).map_err(err)?;
    let s = d.reference(&mut budget()).map_err(err)?;
    let mut r = SchemaRegistry::default();
    r.register(s, d, &mut budget()).map_err(err)?;
    r.finalize(&mut budget()).map_err(err)?;
    let source = SourceSnapshot::new(
        SourceId("文".into()),
        1,
        "memory:test".into(),
        "文x".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let span = source.span(0, 3).map_err(err)?;
    let mut store = SourceStore::default();
    store.insert(source).map_err(err)?;
    Ok((r, store, span))
}
#[test]
fn span_descriptor_is_lazy_shared_and_scopes_start_cold() -> Result<(), String> {
    let (r, s, span) = setup()?;
    let mut a = SourceAdmission::default();
    a.admit_existing(
        s.get_ref(span.snapshot_ref()).ok_or("source")?,
        &mut budget(),
    )
    .map_err(err)?;
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    assert!(c.span_descriptor.is_none());
    let mut cold = budget();
    let value = c.encode_span(&span, &mut cold).map_err(err)?;
    let mut warm = budget();
    assert_eq!(value, c.encode_span(&span, &mut warm).map_err(err)?);
    let cost = ("nepl3.foundation".len() + "Span".len()) as u64;
    assert_eq!(cost, 20);
    let mut expected = warm.usage();
    expected.allocation_units += cost;
    assert_eq!(cold.usage(), expected);
    let mut first_decode = budget();
    assert_eq!(span, c.decode_span(&value, &mut first_decode).map_err(err)?);
    let mut second_decode = budget();
    assert_eq!(
        span,
        c.decode_span(&value, &mut second_decode).map_err(err)?
    );
    assert_eq!(first_decode.usage(), second_decode.usage());
    {
        let mut child = c.scoped(&s);
        assert!(child.span_descriptor.is_none());
        let mut b = budget();
        assert_eq!(span, child.decode_span(&value, &mut b).map_err(err)?);
        let mut expected = first_decode.usage();
        expected.allocation_units += cost;
        assert_eq!(b.usage(), expected);
        let nested = child.scoped(&s);
        assert!(nested.span_descriptor.is_none());
    }
    let mut again = budget();
    assert_eq!(span, c.decode_span(&value, &mut again).map_err(err)?);
    assert_eq!(again.usage(), first_decode.usage());
    Ok(())
}
#[test]
fn descriptor_charge_failure_leaves_cache_cold_and_stops_are_sticky() -> Result<(), String> {
    let (r, s, _) = setup()?;
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    let mut limits = budget().limits();
    limits.allocation_units = 19;
    let mut b = Budget::new(limits);
    assert!(matches!(
        c.validate(&NdfValue::U64(0), "Span", &mut b),
        Err(WireError::Stopped(StopReason::AllocationLimit))
    ));
    assert!(c.span_descriptor.is_none());
    let used = b.usage();
    assert_eq!(used.allocation_units, 0);
    assert!(matches!(
        c.validate(&NdfValue::U64(0), "Span", &mut b),
        Err(WireError::Stopped(StopReason::AllocationLimit))
    ));
    assert_eq!(used, b.usage());
    assert!(
        c.validate(&NdfValue::U64(0), "Span", &mut budget())
            .is_err()
    );
    assert!(c.span_descriptor.is_some());
    assert!(matches!(
        c.validate(&NdfValue::U64(0), "Span", &mut b),
        Err(WireError::Stopped(StopReason::AllocationLimit))
    ));
    assert_eq!(used, b.usage());
    Ok(())
}
#[test]
fn warm_cache_still_validates_every_value() -> Result<(), String> {
    let (r, s, span) = setup()?;
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    let good = c.encode_span(&span, &mut budget()).map_err(err)?;
    for bad in [NdfValue::U64(0), NdfValue::Text("Span".into())] {
        let expected = r
            .validate(&crate::source::expected("Span"), &bad, &mut budget())
            .map(|_| ())
            .map_err(WireError::from);
        assert!(matches!(expected, Err(WireError::Schema(_))));
        assert_eq!(c.decode_span(&bad, &mut budget()).map(|_| ()), expected);
    }
    assert_eq!(span, c.decode_span(&good, &mut budget()).map_err(err)?);
    let empty = SourceStore::default();
    {
        let mut child = c.scoped(&empty);
        assert!(child.decode_span(&good, &mut budget()).is_err());
        assert!(child.span_descriptor.is_some());
        assert!(child.decode_span(&good, &mut budget()).is_err());
    }
    assert_eq!(span, c.decode_span(&good, &mut budget()).map_err(err)?);
    Ok(())
}

#[test]
fn warm_descriptor_rejects_mutated_records_and_public_prestopped_calls() -> Result<(), String> {
    let (r, s, span) = setup()?;
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    let good = c.encode_span(&span, &mut budget()).map_err(err)?;
    for mutation in 0..6 {
        let mut bad = good.clone();
        let NdfValue::Record(record) = &mut bad else {
            return Err("expected record".into());
        };
        match mutation {
            0 => record.schema.digest.0[0] ^= 1,
            1 => record.kind = "SourceRef".into(),
            2 => {
                record.fields.pop();
            }
            3 => record.fields.push(NdfValue::U64(0)),
            4 => record.fields[1] = NdfValue::Text("0".into()),
            _ => record.fields[0] = NdfValue::U64(0),
        }
        let expected = r
            .validate(&crate::source::expected("Span"), &bad, &mut budget())
            .map(|_| ())
            .map_err(WireError::from);
        assert!(matches!(expected, Err(WireError::Schema(_))));
        assert_eq!(c.decode_span(&bad, &mut budget()).map(|_| ()), expected);
    }
    for warm in [false, true] {
        for reason in [StopReason::Cancelled, StopReason::AllocationLimit] {
            let mut local_a = SourceAdmission::default();
            let mut local = FoundationCodec::new(&r, &s, &mut local_a).map_err(err)?;
            if warm {
                local.encode_span(&span, &mut budget()).map_err(err)?;
            }
            let mut b = budget();
            b.stop(reason);
            let before = b.usage();
            assert!(
                matches!(local.encode_span(&span,&mut b),Err(WireError::Stopped(r)) if r==reason)
            );
            assert!(
                matches!(local.decode_span(&NdfValue::U64(0),&mut b),Err(WireError::Stopped(r)) if r==reason)
            );
            assert_eq!(before, b.usage());
            assert_eq!(local.span_descriptor.is_some(), warm);
        }
    }
    Ok(())
}
