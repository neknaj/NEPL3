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
