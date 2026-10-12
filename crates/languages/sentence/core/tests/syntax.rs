use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Mapping, MappingKind, Origin, OriginId},
    schema::SchemaRegistry,
    source::{LineIndex, PositionEncoding, SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    value::{KindRef, NdfValue},
    view::{ViewBundle, ViewElement, ViewRef},
};
use nepl3_sentence_core::{model::*, portable, syntax::*};
use nepl3_wire::foundation::FoundationCodec;
fn b() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        depth: 100_000,
        source_bytes: 1_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn fixture(r: &SchemaRegistry) -> Result<SentenceSyntax, String> {
    let source = SourceSnapshot::new(
        SourceId("sentence".into()),
        7,
        "memory:sentence".into(),
        "\"漢𝄞\"".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let full = source.span(0, 9).map_err(err)?;
    let content = source.span(1, 8).map_err(err)?;
    let schema = r.selected("nepl3.sentence", 1).ok_or("schema")?;
    // Presentation fixture, not a claim that the standalone literal parser ran.
    let kind = KindRef {
        schema: schema.clone(),
        local_kind: r.kind_id(schema, "SentenceKind").map_err(err)?,
    };
    Ok(SentenceSyntax {
        value: SentenceValue {
            root: Root::Sentence(SentenceRef(0)),
            nodes: vec![
                Kind::Sentence {
                    inlines: vec![InlineRef(1)],
                },
                Kind::Text {
                    text: "漢𝄞".into()
                },
            ],
            embeds: vec![],
        },
        locations: vec![
            NodeLocation {
                origin: OriginId(0),
                head: Some(full.clone()),
                cover: Some(full.clone()),
            },
            NodeLocation {
                origin: OriginId(1),
                head: None,
                cover: Some(content.clone()),
            },
        ],
        sources: vec![source],
        origins: vec![
            Origin::Direct(full.clone()),
            Origin::Direct(content.clone()),
        ],
        views: vec![SentenceView {
            owner: 0,
            head: full,
            view: ViewBundle {
                roots: vec![ViewRef(0)],
                elements: vec![ViewElement {
                    kind,
                    span: content,
                    fields: vec![],
                    roles: vec![],
                    relations: vec![],
                }],
            },
        }],
        source_maps: vec![],
    })
}
fn checked(v: &SentenceSyntax, r: &SchemaRegistry) -> Result<(), Error> {
    v.validate(r, &mut b(), &mut SourceAdmission::default())
        .map(|_| ())
}

#[test]
fn repeated_views_reuse_mapping_source_checks_within_one_validation() -> Result<(), String> {
    let r = registry()?;
    let mut prior_delta = None;
    for mappings in [16, 64, 256] {
        let mut v = fixture(&r)?;
        let mapped = SourceSnapshot::new(
            SourceId("mapped".into()),
            1,
            "memory:mapped".into(),
            "漢".repeat(mappings).into_bytes(),
            &mut b(),
        )
        .map_err(err)?;
        for index in 0..mappings {
            v.source_maps.push(Mapping {
                kind: MappingKind::Exact,
                source: v.sources[0].span(1, 4).map_err(err)?,
                target: mapped
                    .span(index as u64 * 3, (index as u64 + 1) * 3)
                    .map_err(err)?,
            });
        }
        v.sources.push(mapped);
        let mut baseline = b();
        v.validate(&r, &mut baseline, &mut SourceAdmission::default())
            .map_err(err)?;
        let view = v.views[0].clone();
        for _ in 0..32 {
            v.views.push(view.clone());
        }
        let mut repeated = b();
        v.validate(&r, &mut repeated, &mut SourceAdmission::default())
            .map_err(err)?;
        let delta = repeated.usage().work - baseline.usage().work;
        // These views are locally contained. Adding them must not repeat the
        // complete mapping declaration scan; mapping validation itself remains.
        if let Some(prior) = prior_delta {
            assert!(delta <= prior * 2, "{mappings}: {delta} vs {prior}");
        }
        prior_delta = Some(delta);
        // A new invocation must check its own declarations, despite a previous
        // successful validation of this same model.
        v.sources.pop();
        assert!(checked(&v, &r).is_err());
    }
    Ok(())
}
fn encode(v: &SentenceSyntax, r: &SchemaRegistry) -> Result<NdfValue, String> {
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
    portable::syntax::to_value(v, r, &mut c, &mut b()).map_err(err)
}

#[test]
fn source_origin_view_roundtrip_is_closed_and_keeps_unicode_positions() -> Result<(), String> {
    let r = registry()?;
    let v = fixture(&r)?;
    let bytes = nepl3_wire::encode(&encode(&v, &r)?, &mut b()).map_err(err)?;
    let raw = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let mut receiver = b();
    let actual = portable::syntax::from_value(&raw, &r, &mut c, &mut receiver).map_err(err)?;
    assert_eq!(actual, v);
    assert_eq!(receiver.usage().source_bytes, 9);
    assert_eq!(
        nepl3_wire::encode(&encode(&actual, &r)?, &mut b()).map_err(err)?,
        bytes
    );
    let source = &actual.sources[0];
    let index = LineIndex::new(source, &mut b()).map_err(err)?;
    assert_eq!(
        index
            .position(source, 8, PositionEncoding::Utf8)
            .map_err(err)?
            .character,
        8
    );
    assert_eq!(
        index
            .position(source, 8, PositionEncoding::Utf16)
            .map_err(err)?
            .character,
        4
    );
    assert!(source.span(2, 8).is_err());
    Ok(())
}

#[test]
fn generated_values_require_real_origins_but_no_invented_spans() -> Result<(), String> {
    let r = registry()?;
    let v = SentenceSyntax {
        value: SentenceValue {
            root: Root::Inline(InlineRef(0)),
            nodes: vec![Kind::Break],
            embeds: vec![],
        },
        locations: vec![NodeLocation {
            origin: OriginId(0),
            head: None,
            cover: None,
        }],
        sources: vec![],
        origins: vec![Origin::Synthetic {
            reason: "generated sentence".into(),
            anchor: None,
        }],
        views: vec![],
        source_maps: vec![],
    };
    checked(&v, &r).map_err(err)?;
    encode(&v, &r)?;
    let mut bad = v.clone();
    bad.origins.clear();
    assert_eq!(checked(&bad, &r), Err(Error::OriginReference(0)));
    let mut bad = v;
    bad.locations.clear();
    assert_eq!(checked(&bad, &r), Err(Error::LocationCount));
    Ok(())
}

#[test]
fn missing_sources_stale_snapshots_and_duplicate_revisions_are_rejected() -> Result<(), String> {
    let r = registry()?;
    let v = fixture(&r)?;
    let mut bad = v.clone();
    bad.sources.clear();
    assert!(checked(&bad, &r).is_err());
    let mut bad = v.clone();
    bad.sources.push(v.sources[0].clone());
    assert_eq!(checked(&bad, &r), Err(Error::DuplicateSource));
    let mut bad = v.clone();
    bad.sources[0] = SourceSnapshot::new(
        SourceId("sentence".into()),
        8,
        "memory:sentence".into(),
        "\"漢𝄞\"".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    assert!(checked(&bad, &r).is_err());
    let mut raw = encode(&v, &r)?;
    let NdfValue::Record(record) = &mut raw else {
        return Err("syntax".into());
    };
    record.fields[2] = NdfValue::List(vec![]);
    let mut ambient = SourceStore::default();
    ambient.insert(v.sources[0].clone()).map_err(err)?;
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &ambient, &mut a).map_err(err)?;
    assert!(portable::syntax::from_value(&raw, &r, &mut c, &mut b()).is_err());
    Ok(())
}

#[test]
fn head_cover_and_view_owner_are_independent_rejected_boundaries() -> Result<(), String> {
    let r = registry()?;
    let v = fixture(&r)?;
    let mut bad = v.clone();
    bad.locations[0].cover = None;
    assert_eq!(checked(&bad, &r), Err(Error::HeadCover(0)));
    let mut bad = v.clone();
    bad.locations[0].cover = Some(v.sources[0].span(1, 8).map_err(err)?);
    assert_eq!(checked(&bad, &r), Err(Error::HeadCover(0)));
    let mut bad = v.clone();
    bad.views[0].owner = 99;
    assert_eq!(checked(&bad, &r), Err(Error::ViewOwner(99)));
    let mut bad = v.clone();
    bad.views[0].owner = 1;
    assert_eq!(checked(&bad, &r), Err(Error::ViewOwner(1)));
    let mut bad = v.clone();
    bad.views[0].head = v.sources[0].span(1, 4).map_err(err)?;
    assert_eq!(checked(&bad, &r), Err(Error::ViewOwner(0)));
    let mut bad = encode(&v, &r)?;
    let NdfValue::Record(record) = &mut bad else {
        return Err("syntax".into());
    };
    record.fields[1] = NdfValue::List(vec![]);
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    assert!(matches!(
        portable::syntax::from_value(&bad, &r, &mut c, &mut b()),
        Err(portable::Error::Presentation(Error::LocationCount))
    ));
    Ok(())
}

#[test]
fn explicit_source_mapping_admits_transformed_view_without_losing_provenance() -> Result<(), String>
{
    let r = registry()?;
    let mut v = fixture(&r)?;
    let source = SourceSnapshot::new(
        SourceId("decoded".into()),
        1,
        "memory:decoded".into(),
        "漢𝄞".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let mapped = source.span(0, 7).map_err(err)?;
    let mut child = v.views[0].view.elements[0].clone();
    child.span = mapped.clone();
    v.views[0].view.elements[0]
        .fields
        .push(nepl3_core::view::ViewField {
            name: "decoded".into(),
            children: vec![ViewRef(1)],
        });
    v.views[0].view.elements.push(child);
    v.sources.push(source);
    assert!(matches!(checked(&v, &r), Err(Error::View(_))));
    v.source_maps.push(Mapping {
        source: v.sources[0].span(1, 8).map_err(err)?,
        target: mapped,
        kind: MappingKind::Exact,
    });
    checked(&v, &r).map_err(err)?;
    let bytes = nepl3_wire::encode(&encode(&v, &r)?, &mut b()).map_err(err)?;
    let raw = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let actual = portable::syntax::from_value(&raw, &r, &mut c, &mut b()).map_err(err)?;
    // Source tables follow foundation canonical ordering; local locations,
    // origins, view indices and mappings retain their original association.
    assert_eq!(actual.value, v.value);
    assert_eq!(actual.locations, v.locations);
    assert_eq!(actual.origins, v.origins);
    assert_eq!(actual.views, v.views);
    assert_eq!(actual.source_maps, v.source_maps);
    assert_eq!(
        nepl3_wire::encode(&encode(&actual, &r)?, &mut b()).map_err(err)?,
        bytes
    );
    let mut bad = v;
    bad.source_maps[0].source = bad.sources[0].span(1, 4).map_err(err)?;
    assert!(matches!(checked(&bad, &r), Err(Error::Origin(_))));
    Ok(())
}

#[test]
fn closure_validation_and_codec_stops_are_sticky() -> Result<(), String> {
    let r = registry()?;
    let v = fixture(&r)?;
    let raw = encode(&v, &r)?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    for reason in [
        StopReason::SourceLimit,
        StopReason::WorkLimit,
        StopReason::DepthLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::SourceLimit => limits.source_bytes = 0,
            StopReason::WorkLimit => limits.work = 0,
            StopReason::DepthLimit => limits.depth = 0,
            StopReason::NodeLimit => limits.nodes = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            _ => {}
        }
        for receiving in [true, false] {
            let mut budget = Budget::new(limits);
            if reason == StopReason::Cancelled {
                budget.cancel();
            }
            if receiving {
                assert_eq!(
                    portable::syntax::from_value(&raw, &r, &mut c, &mut budget),
                    Err(portable::Error::Stopped(reason))
                );
            } else {
                assert_eq!(
                    portable::syntax::to_value(&v, &r, &mut c, &mut budget),
                    Err(portable::Error::Stopped(reason))
                );
            }
            assert_eq!(budget.poll(), Err(reason));
        }
    }
    Ok(())
}

fn generated_root(r: &SchemaRegistry) -> Result<SentenceSyntax, String> {
    let mut v = fixture(r)?;
    let generated = SourceSnapshot::new(
        SourceId("generated".into()),
        1,
        "memory:generated".into(),
        "漢𝄞".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let cover = generated.span(0, 7).map_err(err)?;
    // The generated node has its own real spelling; the View retains the
    // original quoted spelling. Neither head nor cover is fabricated/dropped.
    v.locations[0].head = Some(cover.clone());
    v.locations[0].cover = Some(cover.clone());
    v.source_maps.push(Mapping {
        source: v.views[0].head.clone(),
        target: cover,
        kind: MappingKind::Transformed,
    });
    v.sources.push(generated);
    Ok(v)
}

#[test]
fn generated_owner_retains_original_view_in_native_and_first_receiver() -> Result<(), String> {
    let r = registry()?;
    let v = generated_root(&r)?;
    checked(&v, &r).map_err(err)?;
    let bytes = nepl3_wire::encode(&encode(&v, &r)?, &mut b()).map_err(err)?;
    let raw = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut budget = b();
    let actual = portable::syntax::from_value(&raw, &r, &mut codec, &mut budget).map_err(err)?;
    assert_eq!(actual.value, v.value);
    assert_eq!(actual.locations, v.locations);
    assert_eq!(actual.views, v.views);
    assert_eq!(actual.origins, v.origins);
    assert_eq!(actual.source_maps, v.source_maps);
    assert_eq!(budget.usage().source_bytes, 16);
    let mut missing = raw.clone();
    let NdfValue::Record(record) = &mut missing else {
        return Err("syntax record".into());
    };
    record.fields[5] = NdfValue::List(vec![]);
    let mut fresh = SourceAdmission::default();
    let mut receiver = FoundationCodec::new(&r, &empty, &mut fresh).map_err(err)?;
    assert!(portable::syntax::from_value(&missing, &r, &mut receiver, &mut b()).is_err());
    Ok(())
}

#[test]
fn generated_owner_requires_complete_exclusive_provenance() -> Result<(), String> {
    let r = registry()?;
    let v = generated_root(&r)?;
    let mut bad = v.clone();
    bad.source_maps.clear();
    assert_eq!(checked(&bad, &r), Err(Error::ViewOwner(0)));
    let mut bad = v.clone();
    bad.source_maps[0].target = bad.sources[1].span(0, 3).map_err(err)?;
    assert_eq!(checked(&bad, &r), Err(Error::ViewOwner(0)));
    let unrelated = SourceSnapshot::new(
        SourceId("unrelated".into()),
        1,
        "memory:unrelated".into(),
        b"other".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let mut bad = v.clone();
    bad.source_maps.push(Mapping {
        source: unrelated.span(0, 5).map_err(err)?,
        target: bad.sources[1].span(0, 7).map_err(err)?,
        kind: MappingKind::Transformed,
    });
    bad.sources.push(unrelated);
    assert_eq!(checked(&bad, &r), Err(Error::ViewOwner(0)));
    let mut bad = v.clone();
    let outside = SourceSnapshot::new(
        SourceId("outside-view".into()),
        1,
        "memory:outside-view".into(),
        b"outside".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    bad.views[0].view.elements[0].span = outside.span(0, 7).map_err(err)?;
    bad.sources.push(outside);
    // Owner association succeeds; the independent ViewElement boundary fails.
    assert_eq!(checked(&bad, &r), Err(Error::ViewOwner(0)));
    let mut bad = v.clone();
    bad.sources.pop();
    assert!(checked(&bad, &r).is_err());
    let mut bad = v;
    bad.locations[0].head = Some(bad.views[0].head.clone());
    assert_eq!(checked(&bad, &r), Err(Error::HeadCover(0)));
    Ok(())
}

#[test]
fn original_view_does_not_expand_same_snapshot_owner() -> Result<(), String> {
    let r = registry()?;
    let mut v = generated_root(&r)?;
    let smaller = v.sources[0].span(1, 8).map_err(err)?;
    v.locations[0].head = Some(smaller.clone());
    v.locations[0].cover = Some(smaller);
    assert_eq!(checked(&v, &r), Err(Error::ViewOwner(0)));
    Ok(())
}

#[test]
fn generated_owner_transitive_mapping_preserves_stops_and_depth() -> Result<(), String> {
    let r = registry()?;
    let mut v = generated_root(&r)?;
    let target = v.source_maps[0].target.clone();
    let intermediate = SourceSnapshot::new(
        SourceId("intermediate".into()),
        1,
        "memory:intermediate".into(),
        b"middle".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let middle = intermediate.span(0, 6).map_err(err)?;
    v.source_maps[0].target = middle.clone();
    v.source_maps.push(Mapping {
        source: middle,
        target,
        kind: MappingKind::Transformed,
    });
    v.sources.push(intermediate);
    let mut measured = b();
    v.validate(&r, &mut measured, &mut SourceAdmission::default())
        .map_err(err)?;
    let mut nested = b();
    nested
        .with_depth_at_least(7, |budget| {
            let before = budget.current_depth();
            v.validate(&r, budget, &mut SourceAdmission::default())?;
            assert_eq!(budget.current_depth(), before);
            Ok::<_, Error>(())
        })
        .map_err(err)?;
    assert_eq!(nested.current_depth(), 0);
    for reason in [
        StopReason::WorkLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::WorkLimit => limits.work = measured.usage().work - 1,
            StopReason::NodeLimit => limits.nodes = measured.usage().nodes - 1,
            StopReason::DepthLimit => limits.depth = measured.usage().depth - 1,
            _ => return Err("unexpected stop".into()),
        }
        let mut limited = Budget::new(limits);
        assert_eq!(
            v.validate(&r, &mut limited, &mut SourceAdmission::default())
                .err(),
            Some(Error::Stopped(reason))
        );
        assert_eq!(limited.poll(), Err(reason));
        assert_eq!(limited.current_depth(), 0);
    }
    Ok(())
}
