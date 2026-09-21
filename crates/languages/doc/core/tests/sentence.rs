//! Doc normalization remains a consumer responsibility; literal parsing belongs
//! to Sentence core and its bridge is exercised by the host integration tests.
use nepl3_core::{
    budget::{Budget, Limits},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot},
};
use nepl3_doc_core::model::*;

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 100_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn text_merge_preserves_known_and_source_less_provenance() -> Result<(), String> {
    let mut b = budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b),
        nepl3_doc_core::schema::descriptor(&mut b),
    ] {
        let descriptor = descriptor.map_err(err)?;
        registry
            .register(
                descriptor.reference(&mut b).map_err(err)?,
                descriptor,
                &mut b,
            )
            .map_err(err)?;
    }
    registry.finalize(&mut b).map_err(err)?;
    let source = SourceSnapshot::new(
        SourceId("sentence".into()),
        1,
        "memory:sentence".into(),
        b"a".to_vec(),
        &mut b,
    )
    .map_err(err)?;
    let span = source.span(0, 1).map_err(err)?;
    let input = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Sentence(SentenceRef(2)),
            nodes: vec![
                DocNode {
                    locations: vec![],
                    kind: DocKind::Text { text: "a".into() },
                    origin: Some(OriginId(0)),
                    span: Some(span.clone()),
                },
                DocNode {
                    locations: vec![],
                    kind: DocKind::Text { text: "b".into() },
                    origin: None,
                    span: None,
                },
                DocNode {
                    locations: vec![],
                    kind: DocKind::Sentence {
                        inlines: vec![InlineRef(0), InlineRef(1)],
                    },
                    origin: None,
                    span: None,
                },
            ],
            embeds: vec![],
        },
        sources: vec![source],
        origins: vec![Origin::Direct(span.clone())],
        views: vec![],
        source_maps: vec![],
    };
    let out = nepl3_doc_core::normalize::document(
        &input,
        &registry,
        &mut b,
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(out.value.nodes[0].kind, DocKind::Text { text: "ab".into() });
    assert_eq!(out.value.nodes[0].span, None);
    assert_eq!(out.origins[0], Origin::Direct(span));
    assert!(matches!(&out.origins[1],Origin::Synthetic{anchor:None,reason} if !reason.is_empty()));
    assert_eq!(
        out.origins[2],
        Origin::Composite(vec![OriginId(0), OriginId(1)])
    );
    assert_eq!(out.value.nodes[0].origin, Some(OriginId(2)));
    Ok(())
}
