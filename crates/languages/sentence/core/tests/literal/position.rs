use super::*;
use nepl3_core::value::{NdfValue, Record, SchemaRef};
use nepl3_sentence_core::portable::literal as payload;
use nepl3_wire::foundation::FoundationCodec;

fn record(schema: &SchemaRef, name: &str, fields: Vec<NdfValue>) -> NdfValue {
    NdfValue::Record(Record {
        schema: schema.clone(),
        kind: name.into(),
        fields,
    })
}
fn span(schema: &SchemaRef, start: u64, end: u64) -> NdfValue {
    record(
        schema,
        "SentenceLiteralSpan",
        vec![NdfValue::U64(start), NdfValue::U64(end)],
    )
}
fn location_mut(raw: &mut NdfValue) -> Result<&mut Record, String> {
    let NdfValue::Record(payload) = raw else {
        return Err("payload".into());
    };
    let NdfValue::List(locations) = &mut payload.fields[1] else {
        return Err("locations".into());
    };
    let NdfValue::Record(location) = &mut locations[0] else {
        return Err("location".into());
    };
    Ok(location)
}

#[test]
fn literal_positions_match_independent_records_and_reject_invalid_ranges() -> Result<(), String> {
    let r = registry()?;
    let v = parse("\"漢\" tail", &r)?;
    let s = r.selected("nepl3.sentence", 1).ok_or("schema")?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let raw = payload::to_value(&v.syntax, &r, &mut codec, &mut b()).map_err(err)?;
    let NdfValue::Record(encoded) = &raw else {
        return Err("payload".into());
    };
    let NdfValue::List(locations) = &encoded.fields[1] else {
        return Err("locations".into());
    };
    // UTF-8 bytes: quote at 0, Han character at 1..4, closing quote at 4.
    // The native location provides only the independently checked Origin index.
    assert_eq!(
        v.syntax.locations[0].cover.as_ref().ok_or("cover")?.start(),
        1
    );
    assert_eq!(
        v.syntax.locations[0].cover.as_ref().ok_or("cover")?.end(),
        4
    );
    assert_eq!(
        locations[0],
        record(
            s,
            "SentenceLiteralLocation",
            vec![
                NdfValue::U64(v.syntax.locations[0].origin.0),
                match &v.syntax.locations[0].head {
                    Some(_) => NdfValue::Some(Box::new(span(s, 1, 4))),
                    None => NdfValue::None,
                },
                span(s, 1, 4),
            ]
        )
    );
    let received = payload::from_value(
        &raw,
        &v.syntax.sources[0],
        &v.syntax.views[0].view,
        &r,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    assert_eq!(received, v.syntax);
    // Reversed, non-character boundary, out of source, and within source but
    // outside the literal. Schema validation alone accepts these U64 pairs.
    for (start, end) in [(4, 1), (2, 4), (1, u64::MAX), (6, 10)] {
        let mut forged = raw.clone();
        location_mut(&mut forged)?.fields[2] = span(s, start, end);
        assert!(
            payload::from_value(
                &forged,
                &v.syntax.sources[0],
                &v.syntax.views[0].view,
                &r,
                &mut codec,
                &mut b()
            )
            .is_err(),
            "{start}..{end}"
        );
    }
    let mut forged = raw.clone();
    location_mut(&mut forged)?.fields[0] = NdfValue::U64(u64::MAX);
    assert!(
        payload::from_value(
            &forged,
            &v.syntax.sources[0],
            &v.syntax.views[0].view,
            &r,
            &mut codec,
            &mut b()
        )
        .is_err()
    );
    let mut no_head = raw.clone();
    location_mut(&mut no_head)?.fields[1] = NdfValue::None;
    let actual = payload::from_value(
        &no_head,
        &v.syntax.sources[0],
        &v.syntax.views[0].view,
        &r,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    assert_eq!(actual.locations[0].head, None);
    let mut bad_origin = raw.clone();
    let NdfValue::Record(payload) = &mut bad_origin else {
        return Err("payload".into());
    };
    let NdfValue::List(origins) = &mut payload.fields[2] else {
        return Err("origins".into());
    };
    origins[0] = span(s, 2, 4);
    assert!(
        payload::from_value(
            &bad_origin,
            &v.syntax.sources[0],
            &v.syntax.views[0].view,
            &r,
            &mut codec,
            &mut b()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn literal_position_bytes_and_work_grow_with_content() -> Result<(), String> {
    let r = registry()?;
    let empty = SourceStore::default();
    let mut previous = None;
    for count in [16, 32, 64] {
        let v = parse(&format!("\"{}\"", "[漢/かん]".repeat(count)), &r)?;
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        let mut budget = b();
        let raw = payload::to_value(&v.syntax, &r, &mut codec, &mut budget).map_err(err)?;
        let work = budget.usage().work;
        let bytes = nepl3_wire::encode(&raw, &mut b()).map_err(err)?;
        let full =
            nepl3_sentence_core::portable::syntax::to_value(&v.syntax, &r, &mut codec, &mut b())
                .map_err(err)?;
        let (NdfValue::Record(compact), NdfValue::Record(full)) = (&raw, &full) else {
            return Err("records".into());
        };
        // Compare the same native node locations in the two explicit schemas.
        let compact_locations = nepl3_wire::encode(&compact.fields[1], &mut b()).map_err(err)?;
        let full_locations = nepl3_wire::encode(&full.fields[1], &mut b()).map_err(err)?;
        assert!(compact_locations.len() * 2 < full_locations.len());
        let received = payload::from_value(
            &raw,
            &v.syntax.sources[0],
            &v.syntax.views[0].view,
            &r,
            &mut codec,
            &mut b(),
        )
        .map_err(err)?;
        assert_eq!(received, v.syntax);
        if let Some((previous_work, previous_bytes)) = previous {
            assert!(work < previous_work * 3);
            assert!(bytes.len() < previous_bytes * 3);
        }
        previous = Some((work, bytes.len()));
    }
    Ok(())
}

#[test]
fn compact_literal_positions_preserve_exact_work_and_allocation_limits() -> Result<(), String> {
    let r = registry()?;
    let v = parse("\"[漢/かん]{字/note}\"", &r)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let raw = payload::to_value(&v.syntax, &r, &mut codec, &mut b()).map_err(err)?;
    for receiving in [false, true] {
        let run = |budget: &mut Budget| {
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
            if receiving {
                payload::from_value(
                    &raw,
                    &v.syntax.sources[0],
                    &v.syntax.views[0].view,
                    &r,
                    &mut codec,
                    budget,
                )
                .map(|_| ())
            } else {
                payload::to_value(&v.syntax, &r, &mut codec, budget).map(|_| ())
            }
            .map_err(err)
        };
        let mut measured = b();
        run(&mut measured)?;
        for allocation in [false, true] {
            let mut limits = b().limits();
            if allocation {
                limits.allocation_units = measured.usage().allocation_units;
            } else {
                limits.work = measured.usage().work;
            }
            run(&mut Budget::new(limits))?;
            let expected = if allocation {
                limits.allocation_units -= 1;
                StopReason::AllocationLimit
            } else {
                limits.work -= 1;
                StopReason::WorkLimit
            };
            let mut stopped = Budget::new(limits);
            assert!(run(&mut stopped).is_err());
            assert_eq!(stopped.poll(), Err(expected));
        }
    }
    Ok(())
}
