use super::*;
use nepl3_core::{
    budget::StopReason,
    origin::{Mapping, MappingKind},
};

#[test]
fn typed_set_codec_matches_wire_and_keeps_ambient_sources_outside_members() -> TestResult {
    use nepl3_core::{source::SourceStore, value_codec::FoundationValueCodec};
    use nepl3_wire::foundation::FoundationCodec;
    let (s, r, bundle) = multiple_entries()?;
    let bundles = [&bundle, &bundle];
    let mut store = SourceStore::default();
    for source in &bundle.sources {
        store
            .insert_with_budget(source.clone(), &mut budget())
            .map_err(|e| format!("{e:?}"))?;
    }
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&r, &store, &mut admission).map_err(|e| format!("{e:?}"))?;
    let value = codec
        .encode_syntax_set(&bundles, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let bytes = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let wire = shared::encode(
        &[bundle.clone(), bundle.clone()],
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(bytes, wire);
    assert_eq!(
        codec
            .decode_syntax_set(&value, &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        vec![bundle.clone(), bundle]
    );
    let mut invalid = value.clone();
    edit_member(&mut invalid, 0, 0, NdfValue::List(vec![]))?;
    assert_eq!(
        codec
            .decode_syntax_set(&invalid, &mut budget())
            .err()
            .ok_or("ambient source leak")?,
        WireError::Source(nepl3_core::source::SourceError::MissingSnapshot)
    );
    let mut limited = budget();
    limited.cancel();
    assert!(codec.decode_syntax_set(&value, &mut limited).is_err());
    assert_eq!(limited.poll(), Err(StopReason::Cancelled));
    Ok(())
}

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

// Independent format oracle: ordinary bundle encoding supplies complete tables;
// test-owned maps deduplicate canonical content without production pool indexing.
fn reference_set(
    bundles: &[SyntaxBundle],
    s: &SchemaRef,
    r: &SchemaRegistry,
) -> Result<Vec<u8>, String> {
    use nepl3_core::{source::Digest, value::Record};
    use std::collections::BTreeMap;
    let record = |kind: &str, fields| {
        NdfValue::Record(Record {
            schema: s.clone(),
            kind: kind.into(),
            fields,
        })
    };
    let mut pools = [BTreeMap::new(), BTreeMap::new()];
    let mut members = Vec::new();
    for bundle in bundles {
        let bytes = encode_syntax(bundle, s, r, &mut SourceAdmission::default(), &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let NdfValue::Record(value) = &value else {
            return Err("bundle".into());
        };
        let mut refs = Vec::new();
        for (pool, field, domain) in [
            (0, 0, b"NEPL3.SyntaxBundleSet.Source.v1\0".as_slice()),
            (1, 6, b"NEPL3.SyntaxBundleSet.Mapping.v1\0".as_slice()),
        ] {
            let NdfValue::List(values) = &value.fields[field] else {
                return Err("declarations".into());
            };
            let mut selected = Vec::new();
            for value in values {
                let bytes = encode(value, &mut budget()).map_err(|e| format!("{e:?}"))?;
                let digest = Digest::domain(domain, &bytes);
                pools[pool].insert(digest, value.clone());
                selected.push(NdfValue::Bytes(digest.0.to_vec()));
            }
            refs.push(NdfValue::List(selected));
        }
        refs.push(record("SyntaxBody", value.fields[1..6].to_vec()));
        members.push(record("SharedSyntaxBundle", refs));
    }
    let [sources, maps] = pools;
    encode(
        &record(
            "SyntaxBundleSet",
            vec![
                NdfValue::List(sources.into_values().collect()),
                NdfValue::List(maps.into_values().collect()),
                NdfValue::List(members),
            ],
        ),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))
}

#[test]
fn indexed_pool_preserves_independent_format_for_repeated_and_distinct_members() -> TestResult {
    let (s, r, first) = multiple_entries()?;
    let (_, _, second) = fixture_with_map()?;
    let mut independent = first.clone();
    for source in &mut independent.sources {
        *source = SourceSnapshot::new(
            source.identity().source.clone(),
            source.identity().revision,
            source.uri().into(),
            source.text().as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    }
    let mut conflicting = independent.clone();
    let source = &mut conflicting.sources[0];
    *source = SourceSnapshot::new(
        source.identity().source.clone(),
        source.identity().revision,
        "memory:conflicting-uri".into(),
        source.text().as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let conflict = shared::encode(
        &[first.clone(), conflicting],
        &s,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    );
    assert!(
        matches!(
            conflict,
            Err(WireError::Syntax(nepl3_core::syntax::SyntaxError::Source(
                nepl3_core::source::SourceError::IdentityConflict
            )))
        ),
        "{conflict:?}"
    );
    for bundles in [
        vec![],
        vec![first.clone()],
        vec![first.clone(), second.clone(), independent, first, second],
    ] {
        let actual = shared::encode(
            &bundles,
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(actual, reference_set(&bundles, &s, &r)?);
    }
    Ok(())
}

#[test]
fn shared_pool_work_scales_below_quadratic_for_distinct_declarations() -> TestResult {
    let mut prior = None;
    for count in [64, 128, 256] {
        let (s, r, mut bundle) = fixture_with_map()?;
        for index in (0..count).rev() {
            let source = SourceSnapshot::new(
                SourceId(format!("extra-{index:04}")),
                1,
                format!("memory:extra-{index}"),
                b"ab".to_vec(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            bundle.source_maps.push(Mapping {
                source: source.span(0, 1).map_err(|e| format!("{e:?}"))?,
                target: source.span(1, 2).map_err(|e| format!("{e:?}"))?,
                kind: MappingKind::Transformed,
            });
            bundle.sources.push(source);
        }
        let mut b = budget();
        let bytes = shared::encode(
            &[bundle.clone(), bundle],
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut b,
        )
        .map_err(|e| format!("{e:?}"))?;
        assert!(!bytes.is_empty());
        if let Some(prior) = prior {
            // Doubling declarations permits n log n and fixed graph validation
            // costs, while rejecting a dominant all-pairs comparison path.
            assert!(b.usage().work < prior * 3);
        }
        prior = Some(b.usage().work);
    }
    Ok(())
}

#[test]
fn mapping_keys_preserve_revision_span_kind_and_pool_position_independence() -> TestResult {
    let (s, r, original) = fixture_with_map()?;
    let mut bundles = Vec::new();
    for (revision, start, kind) in [
        (1, 0, MappingKind::Exact),
        (1, 0, MappingKind::Transformed),
        (1, 1, MappingKind::Transformed),
        (2, 0, MappingKind::Transformed),
    ] {
        let mut bundle = original.clone();
        let source = SourceSnapshot::new(
            SourceId("extra".into()),
            revision,
            "memory:key".into(),
            b"xxxx".to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        bundle.source_maps.push(Mapping {
            source: source
                .span(start, start + 1)
                .map_err(|e| format!("{e:?}"))?,
            target: source.span(3, 4).map_err(|e| format!("{e:?}"))?,
            kind,
        });
        bundle.sources.push(source);
        bundles.push(bundle);
    }
    for shift in [false, true] {
        if shift {
            // This declaration shifts all later pool positions but leaves the
            // meaning and canonical digest of every mapping unchanged.
            bundles[0].sources.push(
                SourceSnapshot::new(
                    SourceId("a-first".into()),
                    1,
                    "memory:first".into(),
                    b"x".to_vec(),
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?,
            );
        }
        let bytes = shared::encode(
            &bundles,
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(bytes, reference_set(&bundles, &s, &r)?);
        let value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let NdfValue::Record(set) = &value else {
            return Err("set".into());
        };
        let NdfValue::List(maps) = &set.fields[1] else {
            return Err("maps".into());
        };
        assert_eq!(maps.len(), 5);
        let restored = shared::decode(
            &bytes,
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        for (actual, expected) in restored.iter().zip(&bundles) {
            assert_eq!(actual.source_maps, expected.source_maps);
        }
    }
    Ok(())
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
