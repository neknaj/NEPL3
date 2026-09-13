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
