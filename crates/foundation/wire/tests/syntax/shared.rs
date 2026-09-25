use super::*;
use nepl3_core::{
    budget::StopReason,
    origin::{Mapping, MappingKind},
};

fn fixture_with_map() -> Result<(SchemaRef, SchemaRegistry, SyntaxBundle), String> {
    let (s, r, mut bundle) = fixture()?;
    let source = &bundle.sources[0];
    let original = source.span(0, 1).map_err(|e| format!("{e:?}"))?;
    let transformed = source.span(1, 2).map_err(|e| format!("{e:?}"))?;
    bundle.source_maps.push(Mapping {
        source: original,
        target: transformed.clone(),
        kind: MappingKind::Transformed,
    });
    bundle.tokens[0].views.elements[0].span = transformed;
    bundle
        .validate_with_sources(&r, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    Ok((s, r, bundle))
}

fn edit_member(
    value: &mut NdfValue,
    member: usize,
    field: usize,
    replacement: NdfValue,
) -> Result<(), String> {
    let NdfValue::Record(set) = value else {
        return Err("set".into());
    };
    let NdfValue::List(members) = &mut set.fields[2] else {
        return Err("members".into());
    };
    let NdfValue::Record(bundle) = &mut members[member] else {
        return Err("bundle".into());
    };
    bundle.fields[field] = replacement;
    Ok(())
}

fn multiple_entries() -> Result<(SchemaRef, SchemaRegistry, SyntaxBundle), String> {
    let (s, r, mut bundle) = fixture_with_map()?;
    let source = SourceSnapshot::new(
        SourceId("extra".into()),
        1,
        "memory:extra".into(),
        b"xyz".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    bundle.source_maps.push(Mapping {
        source: source.span(0, 1).map_err(|e| format!("{e:?}"))?,
        target: source.span(1, 2).map_err(|e| format!("{e:?}"))?,
        kind: MappingKind::Transformed,
    });
    bundle.sources.push(source);
    bundle
        .validate_with_sources(&r, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    Ok((s, r, bundle))
}

#[test]
fn shared_tables_require_sorted_unique_used_content() -> TestResult {
    let (s, r, bundle) = multiple_entries()?;
    let bytes = shared::encode(
        std::slice::from_ref(&bundle),
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let restored = shared::decode(
        &bytes,
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored, vec![bundle.clone()]);
    let mut reordered = bundle;
    reordered.sources.reverse();
    assert_eq!(
        shared::encode(
            &[reordered],
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(|e| format!("{e:?}"))?,
        bytes
    );
    for field in 0..2 {
        for duplicate in [false, true] {
            let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
            let NdfValue::Record(set) = &mut value else {
                return Err("set".into());
            };
            let NdfValue::List(pool) = &mut set.fields[field] else {
                return Err("pool".into());
            };
            assert_eq!(pool.len(), 2);
            if duplicate {
                pool[1] = pool[0].clone();
            } else {
                pool.reverse();
            }
            let changed = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
            assert!(matches!(
                shared::decode(
                    &changed,
                    &s,
                    &r,
                    &mut SourceAdmission::default(),
                    &mut budget()
                ),
                Err(WireError::NonCanonical)
            ));
        }
    }
    // No member consumes either table: the tables must not become an ambient scope.
    let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(set) = &mut value else {
        return Err("set".into());
    };
    set.fields[2] = NdfValue::List(vec![]);
    let changed = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        shared::decode(
            &changed,
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::NonCanonical)
    ));
    Ok(())
}

#[test]
fn invalid_later_member_never_returns_an_earlier_bundle() -> TestResult {
    let (s, r, bundle) = multiple_entries()?;
    let bytes = shared::encode(
        &[bundle.clone(), bundle],
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    for length in [31, 32, 33] {
        let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        edit_member(
            &mut value,
            1,
            0,
            NdfValue::List(vec![NdfValue::Bytes(vec![0; length])]),
        )?;
        let changed = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let result = shared::decode(
            &changed,
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        );
        match result {
            Err(WireError::InvalidType) if length == 32 => (),
            Err(WireError::Schema(_)) if length != 32 => (),
            other => {
                return Err(format!("unexpected reference rejection {length}: {other:?}").into());
            }
        }
    }
    Ok(())
}

#[test]
fn shared_bundle_set_roundtrip_preserves_nested_scopes_and_deduplicates_tables() -> TestResult {
    let (s, r, bundle) = fixture_with_map()?;
    let bundles = vec![bundle; 16];
    let bytes = shared::encode(
        &bundles,
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(value) = &value else {
        return Err("set record".into());
    };
    for field in &value.fields[..2] {
        let NdfValue::List(values) = field else {
            return Err("pool".into());
        };
        assert_eq!(values.len(), 1);
    }
    let restored = shared::decode(
        &bytes,
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored, bundles);
    let again = shared::encode(
        &restored,
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(bytes, again);
    let ordinary = encode_syntax(
        &bundles[0],
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(bytes.len() < ordinary.len() * bundles.len());
    Ok(())
}

#[test]
fn shared_pool_cannot_supply_undeclared_source_or_mapping_to_a_member() -> TestResult {
    let (s, r, bundle) = fixture_with_map()?;
    let bytes = shared::encode(
        &[bundle.clone(), bundle],
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    for (field, replacement, expected) in [
        (
            0,
            NdfValue::List(vec![]),
            WireError::Source(nepl3_core::source::SourceError::MissingSnapshot),
        ),
        (
            1,
            NdfValue::List(vec![]),
            WireError::Syntax(SyntaxError::View(ViewError::Cover)),
        ),
        (
            0,
            NdfValue::List(vec![NdfValue::Bytes(vec![0; 32])]),
            WireError::InvalidType,
        ),
    ] {
        let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        edit_member(&mut value, 0, field, replacement)?;
        let changed = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            shared::decode(
                &changed,
                &s,
                &r,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .err()
            .ok_or("undeclared member dependency was accepted")?,
            expected
        );
    }
    Ok(())
}

#[test]
fn shared_bundle_set_budget_boundaries_return_no_partial_success() -> TestResult {
    let (s, r, bundle) = fixture_with_map()?;
    let input = [bundle];
    let mut measured = budget();
    let bytes = shared::encode(
        &input,
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
        StopReason::OutputLimit,
        StopReason::SourceLimit,
    ] {
        for exact in [true, false] {
            let mut limits = budget().limits();
            let usage = measured.usage();
            let amount = match reason {
                StopReason::WorkLimit => &mut limits.work,
                StopReason::AllocationLimit => &mut limits.allocation_units,
                StopReason::NodeLimit => &mut limits.nodes,
                StopReason::DepthLimit => &mut limits.depth,
                StopReason::OutputLimit => &mut limits.output_bytes,
                StopReason::SourceLimit => &mut limits.source_bytes,
                _ => return Err("unexpected reason".into()),
            };
            *amount = match reason {
                StopReason::WorkLimit => usage.work,
                StopReason::AllocationLimit => usage.allocation_units,
                StopReason::NodeLimit => usage.nodes,
                StopReason::DepthLimit => usage.depth,
                StopReason::OutputLimit => usage.output_bytes,
                StopReason::SourceLimit => usage.source_bytes,
                _ => 0,
            } - u64::from(!exact);
            let mut b = Budget::new(limits);
            let result = shared::encode(&input, &s, &r, &mut SourceAdmission::default(), &mut b);
            if exact {
                assert_eq!(result.map_err(|e| format!("{e:?}"))?, bytes);
            } else {
                assert!(result.is_err());
                assert_eq!(b.poll(), Err(reason));
            }
        }
    }
    Ok(())
}

#[test]
fn shared_decode_preserves_stop_reason_and_enclosing_depth() -> TestResult {
    use nepl3_core::value_codec::FoundationCodecError;
    let (s, r, bundle) = multiple_entries()?;
    let input = [bundle.clone(), bundle];
    let bytes = shared::encode(
        &input,
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut measured = budget();
    measured
        .with_depth(|b| shared::decode(&bytes, &s, &r, &mut SourceAdmission::default(), b))
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(measured.current_depth(), 0);
    let usage = measured.usage();
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
        StopReason::SourceLimit,
    ] {
        for exact in [true, false] {
            let mut limits = budget().limits();
            let (limit, used) = match reason {
                StopReason::WorkLimit => (&mut limits.work, usage.work),
                StopReason::AllocationLimit => {
                    (&mut limits.allocation_units, usage.allocation_units)
                }
                StopReason::NodeLimit => (&mut limits.nodes, usage.nodes),
                StopReason::DepthLimit => (&mut limits.depth, usage.depth),
                StopReason::SourceLimit => (&mut limits.source_bytes, usage.source_bytes),
                _ => return Err("reason".into()),
            };
            assert!(used > 0);
            *limit = used - u64::from(!exact);
            let mut b = Budget::new(limits);
            let result = b
                .with_depth(|b| shared::decode(&bytes, &s, &r, &mut SourceAdmission::default(), b));
            assert_eq!(b.current_depth(), 0);
            if exact {
                assert_eq!(result.map_err(|e| format!("{e:?}"))?, input);
            } else {
                let error = result
                    .err()
                    .ok_or("resource boundary accepted the whole set")?;
                assert_eq!(error.stop_reason(), Some(reason));
                assert_eq!(b.poll(), Err(reason));
            }
        }
    }
    let mut b = budget();
    b.cancel();
    let result = shared::decode(&bytes, &s, &r, &mut SourceAdmission::default(), &mut b);
    assert_eq!(
        result
            .err()
            .ok_or("cancelled decode succeeded")?
            .stop_reason(),
        Some(StopReason::Cancelled)
    );
    assert_eq!(b.current_depth(), 0);
    Ok(())
}
