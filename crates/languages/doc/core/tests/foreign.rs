use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{
        Environment, EnvironmentEntry, EnvironmentRef, ForeignClosure, ForeignSyntax, NodeRef,
        SyntaxBundle, SyntaxNode,
    },
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{check::StructureError, model::*, portable};
use nepl3_wire::foundation::FoundationCodec;
fn err(v: impl core::fmt::Debug) -> String {
    format!("{v:?}")
}
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 100_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        let s = d.reference(&mut b()).map_err(err)?;
        r.register(s, d, &mut b()).map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn closure(r: &SchemaRegistry) -> Result<ForeignClosure, String> {
    let source = SourceSnapshot::new(
        SourceId("guest".into()),
        1,
        "memory:guest".into(),
        b"opaque guest spelling".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let span = source.span(0, source.text().len() as u64).map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = codec
        .environment_digest(&environment, &mut b())
        .map_err(err)?;
    let schema = r.selected("nepl3.doc", 1).ok_or("schema")?.clone();
    // This is deliberately only a common syntax graph, not a checked Article.
    // Code display cannot acquire a requirement to semantically lower this guest.
    let node = SyntaxNode {
        schema: schema.clone(),
        kind: "View:TextRun".into(),
        fields: vec![],
        head: Some(span.clone()),
        cover: Some(span.clone()),
        origin: OriginId(0),
        token: None,
    };
    Ok(ForeignClosure {
        syntax: ForeignSyntax {
            schema,
            category: "Article".into(),
            root: NodeRef(0),
            bundle: SyntaxBundle {
                sources: vec![source],
                nodes: vec![node],
                origins: vec![Origin::Direct(span)],
                root: NodeRef(0),
                environments: vec![],
                tokens: vec![],
                source_maps: vec![],
            },
            environment: EnvironmentRef { id: 17, digest },
        },
        owner_environment: EnvironmentEntry {
            id: 17,
            digest,
            value: environment,
        },
        owner_origins: vec![],
        owner_sources: vec![],
        owner_source_maps: vec![],
    })
}
#[test]
fn code_guest_stays_syntax_through_doc_structure_and_cbor() -> Result<(), String> {
    let r = registry()?;
    let doc = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Block(BlockRef(0)),
            nodes: vec![DocNode {
                kind: DocKind::Code {
                    syntax: EmbedRef(0),
                },
                origin: None,
                span: None,
            }],
            embeds: vec![DocEmbed {
                kind: EmbedKind::Code,
                closure: closure(&r)?,
            }],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    doc.validate_structure(&r, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let value = portable::to_value(&doc, &r, &mut codec, &mut b()).map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut b()).map_err(err)?;
    let value = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let actual = portable::from_value(&value, &r, &mut codec, &mut b()).map_err(err)?;
    assert_eq!(actual, doc);
    assert_eq!(
        actual.value.embeds[0].closure.syntax.bundle.nodes[0].kind,
        "View:TextRun"
    );
    Ok(())
}
#[test]
fn shared_embed_validation_composes_its_deepest_doc_owner() -> Result<(), String> {
    let r = registry()?;
    let mut nodes = vec![DocNode {
        kind: DocKind::InlineMath {
            syntax: EmbedRef(0),
        },
        origin: None,
        span: None,
    }];
    for i in 1..20 {
        nodes.push(DocNode {
            kind: DocKind::Strong {
                inline: InlineRef(i - 1),
            },
            origin: None,
            span: None,
        });
    }
    nodes.push(DocNode {
        kind: DocKind::Sentence {
            inlines: vec![InlineRef(0), InlineRef(19)],
        },
        origin: None,
        span: None,
    });
    let doc = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Sentence(SentenceRef(20)),
            nodes,
            embeds: vec![DocEmbed {
                kind: EmbedKind::InlineMath,
                closure: closure(&r)?,
            }],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let mut full = b();
    doc.validate_structure(&r, &mut full, &mut SourceAdmission::default())
        .map_err(err)?;
    assert!(full.usage().depth > 21);
    let mut limits = b().limits();
    limits.depth = 21;
    assert!(matches!(
        doc.validate_structure(
            &r,
            &mut Budget::new(limits),
            &mut SourceAdmission::default()
        ),
        Err(StructureError::Stopped(StopReason::DepthLimit))
    ));
    Ok(())
}
