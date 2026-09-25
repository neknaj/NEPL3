use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{SchemaRegistry, foundation},
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    value::{KindRef, NdfValue, SchemaRef},
    view::*,
};
use nepl3_wire::{WireError, decode, encode, view::*};
type TestResult = Result<(), Box<dyn std::error::Error>>;
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
fn fixture() -> Result<(SchemaRef, SchemaRegistry, SourceStore, Token), String> {
    let mut budget = budget();
    let descriptor = foundation::descriptor(&mut budget).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor
        .reference(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut budget)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("document".into()),
        1,
        "memory:doc".into(),
        " [文/ぶん]".as_bytes().to_vec(),
        &mut budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    let span = |start, end| source.span(start, end).map_err(|e| format!("{e:?}"));
    let kind = KindRef {
        schema: schema.clone(),
        local_kind: registry
            .kind_id(&schema, "Token")
            .map_err(|e| format!("{e:?}"))?,
    };
    let token = Token {
        kind: kind.clone(),
        head: span(1, source.text().len() as u64)?,
        payload: NdfValue::List(vec![
            NdfValue::Text("文".into()),
            NdfValue::Text("ぶん".into()),
        ]),
        views: ViewBundle {
            roots: vec![ViewRef(0)],
            elements: vec![
                ViewElement {
                    kind: kind.clone(),
                    span: span(1, source.text().len() as u64)?,
                    fields: vec![ViewField {
                        name: "base".into(),
                        children: vec![ViewRef(1)],
                    }],
                    roles: vec![],
                    relations: vec![],
                },
                ViewElement {
                    kind,
                    span: span(2, 5)?,
                    fields: vec![],
                    roles: vec![PresentationClass {
                        schema: schema.clone(),
                        name: "body".into(),
                        fallback: FallbackRole::Content,
                    }],
                    relations: vec![ViewRelation {
                        schema: schema.clone(),
                        kind: "part-of".into(),
                        target: ViewRef(0),
                    }],
                },
            ],
        },
        leading_trivia: vec![Trivia {
            span: span(0, 1)?,
            kind: TriviaKind::Whitespace,
        }],
    };
    let mut store = SourceStore::default();
    store.insert(source).map_err(|e| format!("{e:?}"))?;
    Ok((schema, registry, store, token))
}

#[test]
fn shared_views_validate_large_field_sets_and_preserve_error_order() -> TestResult {
    let (schema, registry, sources, mut token) = fixture()?;
    for length in [8, 128] {
        for count in [128, 256, 512] {
            token.views.elements[0].fields = (0..count)
                .rev()
                .map(|index| ViewField {
                    name: format!("{}{:04}", "x".repeat(length), index),
                    children: vec![],
                })
                .collect();
            let bytes = shared::encode(
                &token.views,
                &schema,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let decoded = shared::decode(
                &bytes,
                &schema,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            assert_eq!(decoded, token.views);
            for duplicate in [1, count / 2, count - 1] {
                let mut bad = token.views.clone();
                bad.elements[0].fields[duplicate].name = bad.elements[0].fields[0].name.clone();
                assert_eq!(
                    bad.validate(&sources, &registry, &mut budget()),
                    Err(ViewError::DuplicateField)
                );
            }
        }
    }
    token.views.elements[0].fields = vec![
        ViewField {
            name: "z".into(),
            children: vec![ViewRef(999)],
        },
        ViewField {
            name: "z".into(),
            children: vec![],
        },
    ];
    // An invalid child in an earlier declaration precedes a later duplicate.
    assert_eq!(
        token.views.validate(&sources, &registry, &mut budget()),
        Err(ViewError::Reference)
    );
    token.views.elements[0].fields[0].children.clear();
    token.views.elements[0].fields[1]
        .children
        .push(ViewRef(999));
    assert_eq!(
        token.views.validate(&sources, &registry, &mut budget()),
        Err(ViewError::DuplicateField)
    );
    Ok(())
}

#[test]
fn shared_views_receive_independent_multi_identity_fixture() -> TestResult {
    use nepl3_core::{
        schema::SchemaDescriptor,
        value::{Record, Variant},
    };
    let (schema, mut registry, mut sources, token) = fixture()?;
    let descriptor = SchemaDescriptor {
        package: "zz.presentation".into(),
        revision: 3,
        types: vec![],
        operations: vec![],
    };
    let presentation = descriptor
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .register(presentation.clone(), descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let extra = SourceSnapshot::new(
        SourceId("z".into()),
        9,
        "memory:z".into(),
        b"z".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    sources
        .insert(extra.clone())
        .map_err(|e| format!("{e:?}"))?;
    let record = |kind: &str, fields| {
        NdfValue::Record(Record {
            schema: schema.clone(),
            kind: kind.into(),
            fields,
        })
    };
    let schema_value = |s: &SchemaRef| {
        record(
            "SchemaRef",
            vec![
                NdfValue::Text(s.package.clone()),
                NdfValue::U64(s.revision),
                NdfValue::Bytes(s.digest.0.to_vec()),
            ],
        )
    };
    let source_value = |s: &nepl3_core::source::SnapshotId| {
        record(
            "SourceRef",
            vec![
                NdfValue::Text(s.source.0.clone()),
                NdfValue::U64(s.revision),
                NdfValue::Bytes(s.digest.0.to_vec()),
            ],
        )
    };
    let empty = || NdfValue::List(vec![]);
    let role = record(
        "SharedPresentationClass",
        vec![
            NdfValue::U64(1),
            NdfValue::Text("body".into()),
            NdfValue::Variant(Variant {
                schema: schema.clone(),
                type_name: "FallbackRole".into(),
                variant: "Content".into(),
                fields: vec![],
            }),
        ],
    );
    let relation = record(
        "SharedViewRelation",
        vec![
            NdfValue::U64(1),
            NdfValue::Text("peer".into()),
            NdfValue::U64(1),
        ],
    );
    let first = &token.views.elements[0];
    let value = record(
        "SharedViewBundle",
        vec![
            NdfValue::List(vec![schema_value(&schema), schema_value(&presentation)]),
            NdfValue::List(vec![
                source_value(first.span.snapshot_ref()),
                source_value(extra.identity()),
            ]),
            NdfValue::List(vec![
                record(
                    "SharedViewElement",
                    vec![
                        NdfValue::U64(0),
                        NdfValue::U64(first.kind.local_kind),
                        NdfValue::U64(0),
                        NdfValue::U64(first.span.start()),
                        NdfValue::U64(first.span.end()),
                        empty(),
                        NdfValue::List(vec![role]),
                        NdfValue::List(vec![relation]),
                    ],
                ),
                record(
                    "SharedViewElement",
                    vec![
                        NdfValue::U64(0),
                        NdfValue::U64(first.kind.local_kind),
                        NdfValue::U64(1),
                        NdfValue::U64(0),
                        NdfValue::U64(1),
                        empty(),
                        empty(),
                        empty(),
                    ],
                ),
            ]),
            NdfValue::List(vec![NdfValue::U64(0), NdfValue::U64(1)]),
        ],
    );
    let bytes = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let expected = ViewBundle {
        roots: vec![ViewRef(0), ViewRef(1)],
        elements: vec![
            ViewElement {
                kind: first.kind.clone(),
                span: first.span.clone(),
                fields: vec![],
                roles: vec![PresentationClass {
                    schema: presentation.clone(),
                    name: "body".into(),
                    fallback: FallbackRole::Content,
                }],
                relations: vec![ViewRelation {
                    schema: presentation,
                    kind: "peer".into(),
                    target: ViewRef(1),
                }],
            },
            ViewElement {
                kind: first.kind.clone(),
                span: extra.span(0, 1).map_err(|e| format!("{e:?}"))?,
                fields: vec![],
                roles: vec![],
                relations: vec![],
            },
        ],
    };
    assert_eq!(
        shared::decode(
            &bytes,
            &schema,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(|e| format!("{e:?}"))?,
        expected
    );
    assert_eq!(
        shared::encode(
            &expected,
            &schema,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(|e| format!("{e:?}"))?,
        bytes
    );
    Ok(())
}

#[test]
fn shared_views_reduce_repeated_identity_bytes_with_bounded_growth() -> TestResult {
    let (schema, registry, sources, mut token) = fixture()?;
    let mut previous_work = None;
    for count in [128, 256, 512] {
        let mut leaf = token.views.elements[0].clone();
        leaf.fields.clear();
        leaf.roles.clear();
        leaf.relations.clear();
        token.views.elements = vec![leaf; count];
        token.views.roots = (0..count as u64).map(ViewRef).collect();
        let mut b = budget();
        let bytes = shared::encode(
            &token.views,
            &schema,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut b,
        )
        .map_err(|e| format!("{e:?}"))?;
        let ordinary = encode_token(
            &token,
            &schema,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        // Repeated full schema/source identities dominate the ordinary form;
        // the local table must remove that byte cost, not merely rename it.
        assert!(bytes.len() * 2 < ordinary.len());
        if let Some(previous) = previous_work {
            assert!(b.usage().work < previous * 3);
        }
        previous_work = Some(b.usage().work);
        assert_eq!(
            shared::decode(
                &bytes,
                &schema,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .map_err(|e| format!("{e:?}"))?,
            token.views
        );
    }
    Ok(())
}

#[test]
fn shared_views_preserve_structure_and_reject_table_authority() -> TestResult {
    let (schema, registry, sources, token) = fixture()?;
    let mut measured = budget();
    let bytes = shared::encode(
        &token.views,
        &schema,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    let restored = shared::decode(
        &bytes,
        &schema,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored, token.views);
    let mut decoded_budget = budget();
    shared::decode(
        &bytes,
        &schema,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut decoded_budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    let usage = decoded_budget.usage();
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        for shortage in [0, 1] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = usage.work - shortage,
                StopReason::AllocationLimit => {
                    limits.allocation_units = usage.allocation_units - shortage
                }
                StopReason::DepthLimit => limits.depth = usage.depth - shortage,
                StopReason::Cancelled => {}
                _ => return Err("boundary reason".into()),
            }
            let mut b = Budget::new(limits);
            if reason == StopReason::Cancelled {
                b.stop(reason);
            }
            let result = shared::decode(
                &bytes,
                &schema,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut b,
            );
            if shortage == 0 && reason != StopReason::Cancelled {
                assert_eq!(result.map_err(|e| format!("{e:?}"))?, token.views);
            } else {
                assert!(result.is_err());
                assert_eq!(b.poll(), Err(reason));
            }
        }
    }
    for decoding in [false, true] {
        let mut b = Budget::new(Limits {
            source_bytes: 0,
            ..budget().limits()
        });
        let mut admission = SourceAdmission::default();
        let stopped = if decoding {
            shared::decode(&bytes, &schema, &registry, &sources, &mut admission, &mut b).is_err()
        } else {
            shared::encode(
                &token.views,
                &schema,
                &registry,
                &sources,
                &mut admission,
                &mut b,
            )
            .is_err()
        };
        assert!(stopped);
        assert_eq!(b.poll(), Err(StopReason::SourceLimit));
    }
    assert_eq!(
        shared::encode(
            &restored,
            &schema,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(|e| format!("{e:?}"))?,
        bytes
    );
    assert!(
        shared::decode(
            &bytes,
            &schema,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    for shortage in [0, 1] {
        let mut b = Budget::new(Limits {
            work: measured.usage().work - shortage,
            ..budget().limits()
        });
        let result = shared::encode(
            &token.views,
            &schema,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut b,
        );
        if shortage == 0 {
            assert_eq!(result.map_err(|e| format!("{e:?}"))?, bytes);
        } else {
            assert!(result.is_err());
            assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        }
    }
    for mutation in 0..4 {
        let mut b = budget();
        let mut value = decode(&bytes, &mut b).map_err(|e| format!("{e:?}"))?;
        let NdfValue::Record(record) = &mut value else {
            return Err("SharedViewBundle".into());
        };
        match mutation {
            0 | 1 => {
                let NdfValue::List(table) = &mut record.fields[mutation] else {
                    return Err("table".into());
                };
                table.push(table[0].clone());
            }
            2 => {
                let NdfValue::List(elements) = &mut record.fields[2] else {
                    return Err("elements".into());
                };
                let NdfValue::Record(element) = &mut elements[0] else {
                    return Err("element".into());
                };
                element.fields[2] = NdfValue::U64(u64::MAX);
            }
            3 => {
                let NdfValue::List(elements) = &mut record.fields[2] else {
                    return Err("elements".into());
                };
                elements.clear();
                record.fields[3] = NdfValue::List(vec![]);
            }
            _ => return Err("mutation".into()),
        }
        let altered = encode(&value, &mut b).map_err(|e| format!("{e:?}"))?;
        assert!(
            shared::decode(
                &altered,
                &schema,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn structured_payload_views_roles_relations_and_trivia_roundtrip_without_reparse() -> TestResult {
    let (schema, registry, sources, token) = fixture()?;
    let mut admission = SourceAdmission::default();
    let mut budget = budget();
    let bytes = encode_token(
        &token,
        &schema,
        &registry,
        &sources,
        &mut admission,
        &mut budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    let charged = budget.usage().source_bytes;
    let decoded = decode_token(
        &bytes,
        &schema,
        &registry,
        &sources,
        &mut admission,
        &mut budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(decoded, token);
    assert_eq!(budget.usage().source_bytes, charged);
    assert_eq!(
        decoded.views.elements[0].fields[0].children,
        vec![ViewRef(1)]
    );
    assert_eq!(decoded.leading_trivia[0].span.start(), 0);
    Ok(())
}

#[test]
fn token_wire_rejects_invalid_local_view_and_trivia_references_and_source_limit() -> TestResult {
    let (schema, registry, sources, token) = fixture()?;
    let bytes = encode_token(
        &token,
        &schema,
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    // Structurally valid ViewRef, but outside this token's own table.
    if let NdfValue::Record(token) = &mut value
        && let NdfValue::Record(views) = &mut token.fields[3]
        && let NdfValue::List(roots) = &mut views.fields[1]
        && let NdfValue::Record(reference) = &mut roots[0]
    {
        reference.fields[0] = NdfValue::U64(2);
    }
    let bad = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        decode_token(
            &bad,
            &schema,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::View(ViewError::Reference))
    ));
    let mut bad_trivia = token.clone();
    bad_trivia.leading_trivia[0].kind = TriviaKind::Bom;
    assert!(matches!(
        encode_token(
            &bad_trivia,
            &schema,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::View(ViewError::Trivia))
    ));
    let mut limits = budget().limits();
    limits.source_bytes = 0;
    assert!(matches!(
        decode_token(
            &bytes,
            &schema,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut Budget::new(limits)
        ),
        Err(WireError::Stopped(StopReason::SourceLimit))
    ));
    Ok(())
}

#[test]
fn codec_mapping_scope_is_explicit_validated_and_cleared_on_rebind() -> TestResult {
    use nepl3_core::{
        origin::{Mapping, MappingKind},
        value_codec::{FoundationCodecError, FoundationValueCodec},
    };
    use nepl3_wire::foundation::FoundationCodec;
    let (_, registry, mut sources, token) = fixture()?;
    let mut views = token.views;
    let original = views.elements[1].span.clone();
    let source = sources.get_ref(original.snapshot_ref()).ok_or("source")?;
    let decoded = SourceSnapshot::new(
        SourceId("decoded".into()),
        1,
        "memory:decoded".into(),
        source
            .slice(&original)
            .map_err(|e| format!("{e:?}"))?
            .as_bytes()
            .to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mapped = decoded
        .span(0, decoded.text().len() as u64)
        .map_err(|e| format!("{e:?}"))?;
    let view_source_bytes = source.text().len() as u64 + decoded.text().len() as u64;
    let unused = SourceSnapshot::new(
        SourceId("unused-map-target".into()),
        1,
        "memory:unused".into(),
        decoded.text().as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let unused_span = unused
        .span(0, unused.text().len() as u64)
        .map_err(|e| format!("{e:?}"))?;
    sources.insert(decoded).map_err(|e| format!("{e:?}"))?;
    sources.insert(unused).map_err(|e| format!("{e:?}"))?;
    views.elements[1].span = mapped.clone();
    let maps = [
        Mapping {
            source: original.clone(),
            target: mapped,
            kind: MappingKind::Exact,
        },
        Mapping {
            source: original.clone(),
            target: unused_span,
            kind: MappingKind::Exact,
        },
    ];
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&registry, &sources, &mut admission).map_err(|e| format!("{e:?}"))?;
    assert!(codec.encode_views(&views, &mut budget()).is_err());
    let raw = {
        let mut scoped = codec.scoped_with_mappings(&sources, &maps);
        let raw = scoped
            .encode_views(&views, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            scoped
                .decode_views(&raw, &mut budget())
                .map_err(|e| format!("{e:?}"))?,
            views
        );
        // Warm mapping proof cannot authenticate a different View or mask a stop.
        let mut broken = views.clone();
        broken.elements[0].fields[0].children[0] = ViewRef(99);
        assert!(scoped.encode_views(&broken, &mut budget()).is_err());
        for cancelled in [false, true] {
            let mut limits = budget().limits();
            if !cancelled {
                limits.work = 0;
            }
            let mut stopped = Budget::new(limits);
            let expected = if cancelled {
                stopped.cancel();
                StopReason::Cancelled
            } else {
                StopReason::WorkLimit
            };
            let error = scoped
                .decode_views(&raw, &mut stopped)
                .err()
                .ok_or("expected warm scope stop")?;
            assert_eq!(error.stop_reason(), Some(expected));
            assert_eq!(stopped.poll(), Err(expected));
        }
        // Replacing admission must invalidate even unused mapping endpoints.
        *scoped.source_admission() = SourceAdmission::default();
        let mut limits = budget().limits();
        limits.source_bytes = view_source_bytes;
        let mut stopped = Budget::new(limits);
        let error = scoped
            .decode_views(&raw, &mut stopped)
            .err()
            .ok_or("expected mapping admission stop")?;
        assert_eq!(error.stop_reason(), Some(StopReason::SourceLimit));
        assert_eq!(stopped.poll(), Err(StopReason::SourceLimit));
        for mapped_child in [false, true] {
            scoped
                .decode_views(&raw, &mut budget())
                .map_err(|e| format!("{e:?}"))?;
            if mapped_child {
                let mut child = scoped.scoped_with_mappings(&sources, &maps);
                *child.source_admission() = SourceAdmission::default();
            } else {
                let mut child = scoped.scoped(&sources);
                *child.source_admission() = SourceAdmission::default();
            }
            // Mutating admission through a child also invalidates the warm parent.
            let mut stopped = Budget::new(limits);
            let error = scoped
                .decode_views(&raw, &mut stopped)
                .err()
                .ok_or("expected parent mapping admission stop")?;
            assert_eq!(error.stop_reason(), Some(StopReason::SourceLimit));
            assert_eq!(stopped.poll(), Err(StopReason::SourceLimit));
        }
        // A nested payload must not inherit its caller's containment authority.
        let mut rebound = scoped.scoped(&sources);
        assert!(rebound.encode_views(&views, &mut budget()).is_err());
        assert!(rebound.decode_views(&raw, &mut budget()).is_err());
        raw
    };
    assert!(codec.decode_views(&raw, &mut budget()).is_err());
    let mut forged = maps.clone();
    forged[0].source = token.head;
    let mut invalid = codec.scoped_with_mappings(&sources, &forged);
    assert!(invalid.encode_views(&views, &mut budget()).is_err());
    assert!(invalid.decode_views(&raw, &mut budget()).is_err());
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&registry, &sources, &mut admission).map_err(|e| format!("{e:?}"))?;
    let mut scoped = codec.scoped_with_mappings(&sources, &maps);
    let mut limits = budget().limits();
    limits.source_bytes = 0;
    let mut stopped = Budget::new(limits);
    let error = scoped
        .decode_views(&raw, &mut stopped)
        .err()
        .ok_or("expected stop")?;
    assert_eq!(error.stop_reason(), Some(StopReason::SourceLimit));
    assert_eq!(stopped.poll(), Err(StopReason::SourceLimit));
    Ok(())
}
